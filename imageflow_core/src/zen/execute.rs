//! Top-level execution: v2 Framewise → zenpipe streaming pipeline → encoded output.
//!
//! Handles two Framewise modes:
//! - **Steps**: Linear `Vec<Node>` — sequential pipeline.
//! - **Graph**: DAG with explicit edges — compositing, fan-out, watermarks.
//!
//! Fully streaming: decode (row batches) → process (strips) → encode (push_rows).
//! No full-frame materialization for JPEG/PNG. Formats that don't support
//! streaming decode (WebP, TIFF) fall back to full-frame + MaterializedSource.

use std::collections::HashMap;

use imageflow_types::{self as s, Framewise, Node};
use zencodecs::{AllowedFormats, CodecPolicy, ImageFacts, select_format_from_intent};
use zennode::NodeDef as _;
use zenpipe::Source as _;

use super::translate::{self, TranslateError, TranslatedPipeline};

// ─── Error type ───

/// Error from the zen pipeline execution.
#[derive(Debug)]
pub enum ZenError {
    Translate(TranslateError),
    Codec(String),
    Pipeline(zenpipe::PipeError),
    Io(String),
}

impl std::fmt::Display for ZenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Translate(e) => write!(f, "translate: {e}"),
            Self::Codec(e) => write!(f, "codec: {e}"),
            Self::Pipeline(e) => write!(f, "pipeline: {e}"),
            Self::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for ZenError {}

impl From<TranslateError> for ZenError {
    fn from(e: TranslateError) -> Self { Self::Translate(e) }
}

impl From<zenpipe::PipeError> for ZenError {
    fn from(e: zenpipe::PipeError) -> Self { Self::Pipeline(e) }
}

// ─── Result type ───

/// Output from a single encode operation.
pub struct ZenEncodeResult {
    pub io_id: i32,
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub mime_type: &'static str,
    pub extension: &'static str,
}

// ─── Public API ───

/// Probe an image and return metadata without decoding pixels.
pub fn zen_get_image_info(
    data: &[u8],
) -> Result<zencodecs::ImageInfo, ZenError> {
    zencodecs::from_bytes(data).map_err(|e| ZenError::Codec(format!("{e}")))
}

/// Execute a v2 Framewise pipeline through the zen streaming engine.
///
/// `io_buffers` maps io_id → input bytes. Returns encode results for each output.
pub fn execute_framewise(
    framewise: &Framewise,
    io_buffers: &HashMap<i32, Vec<u8>>,
) -> Result<Vec<ZenEncodeResult>, ZenError> {
    match framewise {
        Framewise::Steps(steps) => execute_steps(steps, io_buffers),
        Framewise::Graph(graph) => execute_graph(graph, io_buffers),
    }
}

// ─── Steps mode (linear pipeline) ───

fn execute_steps(
    steps: &[Node],
    io_buffers: &HashMap<i32, Vec<u8>>,
) -> Result<Vec<ZenEncodeResult>, ZenError> {
    let pipeline = translate::translate_nodes(steps)?;

    let decode_io_id = pipeline.decode_io_id.ok_or_else(|| {
        ZenError::Io("no decode node in pipeline".into())
    })?;
    let encode_io_id = pipeline.encode_io_id.ok_or_else(|| {
        ZenError::Io("no encode node in pipeline".into())
    })?;
    let input_data = io_buffers.get(&decode_io_id).ok_or_else(|| {
        ZenError::Io(format!("no input buffer for io_id {decode_io_id}"))
    })?;

    let (decision, source) = probe_resolve_decode(input_data, &pipeline)?;

    let converters: &[&dyn zenpipe::bridge::NodeConverter] = &[];
    let pipe_result = zenpipe::bridge::build_pipeline(source, &pipeline.nodes, converters)?;

    stream_encode(pipe_result.source, &decision, encode_io_id)
}

// ─── Graph mode (DAG with compositing, fan-out) ───

fn execute_graph(
    graph: &s::Graph,
    io_buffers: &HashMap<i32, Vec<u8>>,
) -> Result<Vec<ZenEncodeResult>, ZenError> {
    // Topological sort: v2 Graph has string keys + Edge{from, to, kind}.
    let mut id_order: Vec<String> = graph.nodes.keys().cloned().collect();
    id_order.sort_by(|a, b| {
        a.parse::<i32>().unwrap_or(i32::MAX).cmp(&b.parse::<i32>().unwrap_or(i32::MAX))
    });

    let i32_to_idx: HashMap<i32, usize> = id_order.iter().enumerate()
        .filter_map(|(i, id)| id.parse::<i32>().ok().map(|n| (n, i)))
        .collect();

    let mut dag_nodes: Vec<zenpipe::DagNode> = Vec::new();
    let mut decode_io_ids: Vec<(usize, i32)> = Vec::new();
    let mut encode_io_ids: Vec<(usize, i32)> = Vec::new();
    let mut encode_pipeline: Option<TranslatedPipeline> = None;

    for (dag_idx, id) in id_order.iter().enumerate() {
        let node = &graph.nodes[id];
        let mut partial = translate::translate_nodes(&[node.clone()])?;

        if let Some(io_id) = partial.decode_io_id {
            decode_io_ids.push((dag_idx, io_id));
        }
        if let Some(io_id) = partial.encode_io_id {
            encode_io_ids.push((dag_idx, io_id));
            encode_pipeline = Some(partial.clone_config());
        }

        let inputs: Vec<usize> = graph.edges.iter()
            .filter(|e| i32_to_idx.get(&e.to).copied() == Some(dag_idx))
            .filter_map(|e| i32_to_idx.get(&e.from).copied())
            .collect();

        let instance = if !partial.nodes.is_empty() {
            partial.nodes.remove(0)
        } else {
            create_encode_role_placeholder()
        };

        dag_nodes.push(zenpipe::DagNode { instance, inputs });
    }

    // Decode all input sources.
    let registry = AllowedFormats::all();
    let mut sources: Vec<(usize, Box<dyn zenpipe::Source>)> = Vec::new();
    for (dag_idx, io_id) in &decode_io_ids {
        let input_data = io_buffers.get(io_id).ok_or_else(|| {
            ZenError::Io(format!("no input buffer for io_id {io_id}"))
        })?;
        sources.push((*dag_idx, decode_to_source(input_data, &registry)?));
    }

    // Build DAG pipeline.
    let converters: &[&dyn zenpipe::bridge::NodeConverter] = &[];
    let pipe_result = zenpipe::bridge::build_pipeline_dag(sources, &dag_nodes, converters)?;

    // Resolve format + quality from the first input.
    let first_input = io_buffers.get(&decode_io_ids[0].1).ok_or_else(|| {
        ZenError::Io("no input for format probe".into())
    })?;
    let info = zencodecs::from_bytes(first_input)
        .map_err(|e| ZenError::Codec(format!("probe: {e}")))?;

    let codec_intent = encode_pipeline
        .as_ref()
        .and_then(|p| p.preset.as_ref())
        .map(|p| &p.intent)
        .cloned()
        .unwrap_or_default();

    let decision = select_format_from_intent(
        &codec_intent, &ImageFacts::from_image_info(&info),
        &registry, &CodecPolicy::default(),
    ).map_err(|e| ZenError::Codec(format!("format selection: {e}")))?;

    let encode_io_id = encode_io_ids.first().map(|(_, id)| *id).unwrap_or(1);
    stream_encode(pipe_result.source, &decision, encode_io_id)
}

// ─── Decode ───

/// Probe, resolve format/quality, and build a streaming decode source.
fn probe_resolve_decode(
    input_data: &[u8],
    pipeline: &TranslatedPipeline,
) -> Result<(zencodecs::FormatDecision, Box<dyn zenpipe::Source>), ZenError> {
    let registry = AllowedFormats::all();
    let info = zencodecs::from_bytes(input_data)
        .map_err(|e| ZenError::Codec(format!("probe: {e}")))?;

    let codec_intent = pipeline.preset.as_ref()
        .map(|p| &p.intent)
        .cloned()
        .unwrap_or_default();

    let decision = select_format_from_intent(
        &codec_intent, &ImageFacts::from_image_info(&info),
        &registry, &CodecPolicy::default(),
    ).map_err(|e| ZenError::Codec(format!("format selection: {e}")))?;

    Ok((decision, decode_to_source(input_data, &registry)?))
}

/// Build a streaming decode source. Tries row-level streaming first
/// (JPEG, PNG, GIF, AVIF, HEIC), falls back to full-frame + MaterializedSource.
fn decode_to_source(
    data: &[u8],
    registry: &AllowedFormats,
) -> Result<Box<dyn zenpipe::Source>, ZenError> {
    match zencodecs::DecodeRequest::new(data)
        .with_registry(registry)
        .build_streaming_decoder()
    {
        Ok(streaming) => {
            let source = zenpipe::codec::DecoderSource::new(streaming)?;
            Ok(Box::new(source))
        }
        Err(_) => {
            let decoded = zencodecs::DecodeRequest::new(data)
                .with_registry(registry)
                .decode_full_frame()
                .map_err(|e| ZenError::Codec(format!("decode: {e}")))?;

            let w = decoded.width();
            let h = decoded.height();
            let format = decoded.descriptor();
            let bytes = decoded.into_buffer().copy_to_contiguous_bytes();
            Ok(Box::new(zenpipe::sources::MaterializedSource::from_data(bytes, w, h, format)))
        }
    }
}

// ─── Encode ───

/// Stream pipeline output into an encoder via EncoderSink.
fn stream_encode(
    mut source: Box<dyn zenpipe::Source>,
    decision: &zencodecs::FormatDecision,
    encode_io_id: i32,
) -> Result<Vec<ZenEncodeResult>, ZenError> {
    let out_w = source.width();
    let out_h = source.height();
    let out_format = source.format();

    let streaming_enc = zencodecs::EncodeRequest::new(decision.format)
        .with_quality(decision.quality.quality)
        .with_lossless(decision.lossless)
        .with_registry(&AllowedFormats::all())
        .build_streaming_encoder(out_w, out_h)
        .map_err(|e| ZenError::Codec(format!("encoder: {e}")))?;

    let mut sink = zenpipe::codec::EncoderSink::new(streaming_enc.encoder, out_format);
    zenpipe::execute(source.as_mut(), &mut sink)?;

    let output = sink.take_output().ok_or_else(|| {
        ZenError::Codec("encoder produced no output".into())
    })?;

    Ok(vec![ZenEncodeResult {
        io_id: encode_io_id,
        bytes: output.into_vec(),
        width: out_w,
        height: out_h,
        mime_type: decision.format.mime_type(),
        extension: decision.format.extension(),
    }])
}

// ─── Helpers ───

/// Create a placeholder Encode-role node for DAG slots that are decode/encode.
/// The bridge separates these by role — the placeholder just needs a valid schema.
fn create_encode_role_placeholder() -> Box<dyn zennode::NodeInstance> {
    zencodecs::zennode_defs::QUALITY_INTENT_NODE_NODE
        .create_default()
        .expect("placeholder creation")
}

impl TranslatedPipeline {
    pub(crate) fn clone_config(&self) -> TranslatedPipeline {
        TranslatedPipeline {
            nodes: Vec::new(),
            preset: self.preset.as_ref().map(|p| super::preset_map::PresetMapping {
                intent: p.intent.clone(),
                explicit_format: p.explicit_format,
            }),
            decode_io_id: self.decode_io_id,
            encode_io_id: self.encode_io_id,
            decoder_commands: self.decoder_commands.clone(),
        }
    }
}
