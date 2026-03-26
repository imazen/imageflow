//! Top-level execution: v2 Framewise → zenpipe streaming pipeline → encoded output.
//!
//! Handles three modes:
//! - **Steps**: Linear `Vec<Node>` — sequential pipeline.
//! - **Graph**: DAG with explicit edges — compositing, fan-out, watermarks.
//!
//! Streaming behavior:
//! - Decode: full-frame (JPEG/PNG borrow input data, can't produce `'static` decoders).
//! - Pipeline: streaming strips via zenpipe (zero materialization between operations).
//! - Encode: streaming strips via `push_rows()` / `finish()` on `DynEncoder`.

use std::borrow::Cow;
use std::collections::HashMap;

use imageflow_types::{self as s, Framewise, Node};
use zencodecs::{AllowedFormats, CodecPolicy, ImageFacts, select_format_from_intent};
use zennode::NodeDef as _;
use zenpipe::Source as _;

use super::translate::{self, TranslateError, TranslatedPipeline};

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

/// Output from a single encode operation.
pub struct ZenEncodeResult {
    pub io_id: i32,
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub mime_type: &'static str,
    pub extension: &'static str,
}

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
    // 1. Translate v2 nodes → zennode instances + codec config.
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

    // 2. Probe, resolve format, decode.
    let (decision, source) = probe_and_decode(input_data, &pipeline)?;

    // 3. Build streaming pipeline.
    let converters: &[&dyn zenpipe::bridge::NodeConverter] = &[];
    let pipe_result = zenpipe::bridge::build_pipeline(
        source, &pipeline.nodes, converters,
    )?;

    // 4. Stream-encode.
    stream_encode(pipe_result.source, &decision, encode_io_id)
}

// ─── Graph mode (DAG with compositing, fan-out) ───

fn execute_graph(
    graph: &s::Graph,
    io_buffers: &HashMap<i32, Vec<u8>>,
) -> Result<Vec<ZenEncodeResult>, ZenError> {
    // 1. Topological sort: v2 Graph has string keys + Edge{from, to, kind}.
    //    Build ordered node list and map string IDs to DAG indices.
    let mut id_order: Vec<String> = graph.nodes.keys().cloned().collect();
    // Sort by numeric key for deterministic order (v2 convention: keys are "0", "1", ...)
    id_order.sort_by(|a, b| {
        a.parse::<i32>().unwrap_or(i32::MAX).cmp(&b.parse::<i32>().unwrap_or(i32::MAX))
    });

    let id_to_idx: HashMap<String, usize> = id_order.iter().enumerate()
        .map(|(i, id)| (id.clone(), i))
        .collect();
    // Also map i32 keys for edge lookup (v2 edges use i32 from/to).
    let i32_to_idx: HashMap<i32, usize> = id_order.iter().enumerate()
        .filter_map(|(i, id)| id.parse::<i32>().ok().map(|n| (n, i)))
        .collect();

    // 2. Translate each node, tracking decode/encode io_ids and building DagNodes.
    let mut dag_nodes: Vec<zenpipe::DagNode> = Vec::new();
    let mut decode_io_ids: Vec<(usize, i32)> = Vec::new(); // (dag_idx, io_id)
    let mut encode_io_ids: Vec<(usize, i32)> = Vec::new();
    let mut encode_pipeline: Option<TranslatedPipeline> = None;

    for (dag_idx, id) in id_order.iter().enumerate() {
        let node = &graph.nodes[id];

        // Translate node → zennode instance.
        let mut partial = translate::translate_nodes(&[node.clone()])?;

        if let Some(io_id) = partial.decode_io_id {
            decode_io_ids.push((dag_idx, io_id));
        }
        if let Some(io_id) = partial.encode_io_id {
            encode_io_ids.push((dag_idx, io_id));
            encode_pipeline = Some(partial.clone_config());
        }

        // Build input list from v2 edges.
        let inputs: Vec<usize> = graph.edges.iter()
            .filter(|e| i32_to_idx.get(&e.to).copied() == Some(dag_idx))
            .filter_map(|e| i32_to_idx.get(&e.from).copied())
            .collect();

        // Use the first pixel-processing node, or a placeholder for decode/encode.
        let instance = if !partial.nodes.is_empty() {
            partial.nodes.remove(0)
        } else {
            // Decode/encode nodes: create a placeholder that the bridge will separate.
            create_placeholder_node(node)
        };

        dag_nodes.push(zenpipe::DagNode { instance, inputs });
    }

    // 3. Decode all input sources.
    let registry = AllowedFormats::all();
    let mut sources: Vec<(usize, Box<dyn zenpipe::Source>)> = Vec::new();

    for (dag_idx, io_id) in &decode_io_ids {
        let input_data = io_buffers.get(io_id).ok_or_else(|| {
            ZenError::Io(format!("no input buffer for io_id {io_id}"))
        })?;
        let source = decode_to_source(input_data, &registry)?;
        sources.push((*dag_idx, source));
    }

    // 4. Build DAG pipeline via zenpipe.
    let converters: &[&dyn zenpipe::bridge::NodeConverter] = &[];
    let pipe_result = zenpipe::bridge::build_pipeline_dag(
        sources, &dag_nodes, converters,
    )?;

    // 5. Resolve format + quality from the encode node.
    let first_decode_io = decode_io_ids.first().map(|(_, id)| *id).unwrap_or(0);
    let first_input = io_buffers.get(&first_decode_io).ok_or_else(|| {
        ZenError::Io("no input for format probe".into())
    })?;
    let info = zencodecs::from_bytes(first_input)
        .map_err(|e| ZenError::Codec(format!("probe: {e}")))?;
    let facts = ImageFacts::from_image_info(&info);

    let codec_intent = encode_pipeline
        .as_ref()
        .and_then(|p| p.preset.as_ref())
        .map(|p| &p.intent)
        .cloned()
        .unwrap_or_default();

    let decision = select_format_from_intent(
        &codec_intent, &facts, &registry, &CodecPolicy::default(),
    )
    .map_err(|e| ZenError::Codec(format!("format selection: {e}")))?;

    // 6. Stream-encode.
    let encode_io_id = encode_io_ids.first().map(|(_, id)| *id).unwrap_or(1);
    stream_encode(pipe_result.source, &decision, encode_io_id)
}

// ─── Shared helpers ───

/// Probe source, resolve format, decode to MaterializedSource.
fn probe_and_decode(
    input_data: &[u8],
    pipeline: &TranslatedPipeline,
) -> Result<(zencodecs::FormatDecision, Box<dyn zenpipe::Source>), ZenError> {
    let registry = AllowedFormats::all();
    let info = zencodecs::from_bytes(input_data)
        .map_err(|e| ZenError::Codec(format!("probe: {e}")))?;
    let facts = ImageFacts::from_image_info(&info);

    let codec_intent = pipeline.preset.as_ref()
        .map(|p| &p.intent)
        .cloned()
        .unwrap_or_default();

    let decision = select_format_from_intent(
        &codec_intent, &facts, &registry, &CodecPolicy::default(),
    )
    .map_err(|e| ZenError::Codec(format!("format selection: {e}")))?;

    let source = decode_to_source(input_data, &registry)?;
    Ok((decision, source))
}

/// Build a decode source — streaming if possible, full-frame fallback.
///
/// Tries `build_streaming_decoder()` first (works for JPEG, PNG, GIF, AVIF, HEIC
/// via `job_static()` + `Cow::Owned`). Falls back to full-frame decode +
/// `MaterializedSource` for formats that don't support streaming.
fn decode_to_source(
    data: &[u8],
    registry: &AllowedFormats,
) -> Result<Box<dyn zenpipe::Source>, ZenError> {
    // TODO: Use streaming decode once format negotiation is wired.
    // build_streaming_decoder() works (JPEG/PNG support Cow::Owned + job_static),
    // but we need to know the decoder's output pixel format to construct DecoderSource.
    // For now, use full-frame decode which reports its format via descriptor().
    let decoded = zencodecs::DecodeRequest::new(data)
        .with_registry(registry)
        .decode_full_frame()
        .map_err(|e| ZenError::Codec(format!("decode: {e}")))?;

    let w = decoded.width();
    let h = decoded.height();
    let format = decoded.descriptor();
    let buf = decoded.into_buffer();
    let bytes = buf.copy_to_contiguous_bytes();
    let source = zenpipe::sources::MaterializedSource::from_data(bytes, w, h, format);
    Ok(Box::new(source))
}

/// Pull strips from pipeline source, push directly to encoder.
fn stream_encode(
    mut source: Box<dyn zenpipe::Source>,
    decision: &zencodecs::FormatDecision,
    encode_io_id: i32,
) -> Result<Vec<ZenEncodeResult>, ZenError> {
    let out_w = source.width();
    let out_h = source.height();
    let out_format = source.format();

    let registry = AllowedFormats::all();
    let streaming_enc = zencodecs::EncodeRequest::new(decision.format)
        .with_quality(decision.quality.quality)
        .with_lossless(decision.lossless)
        .with_registry(&registry)
        .build_streaming_encoder(out_w, out_h)
        .map_err(|e| ZenError::Codec(format!("encoder: {e}")))?;

    let mut encoder = streaming_enc.encoder;

    while let Some(strip) = source.next()? {
        let pixels = zenpixels::PixelSlice::new(
            strip.as_strided_bytes(),
            strip.width(),
            strip.rows(),
            strip.stride(),
            out_format,
        )
        .map_err(|e| ZenError::Codec(format!("pixel slice: {e}")))?;

        encoder.push_rows(pixels)
            .map_err(|e| ZenError::Codec(format!("push_rows: {e}")))?;
    }

    let output = encoder.finish()
        .map_err(|e| ZenError::Codec(format!("finish: {e}")))?;

    Ok(vec![ZenEncodeResult {
        io_id: encode_io_id,
        bytes: output.into_vec(),
        width: out_w,
        height: out_h,
        mime_type: decision.format.mime_type(),
        extension: decision.format.extension(),
    }])
}

/// Create a placeholder zennode instance for decode/encode nodes in the DAG.
/// These are separated out by the bridge — the placeholder just needs a valid schema.
fn create_placeholder_node(node: &Node) -> Box<dyn zennode::NodeInstance> {
    // Use zencodecs QualityIntentNode as a benign Encode-role placeholder,
    // or a minimal decode placeholder. The bridge separates these by role.
    match node {
        Node::Decode { .. } => {
            zencodecs::zennode_defs::QUALITY_INTENT_NODE_NODE
                .create_default()
                .expect("placeholder creation")
        }
        Node::Encode { .. } => {
            zencodecs::zennode_defs::QUALITY_INTENT_NODE_NODE
                .create_default()
                .expect("placeholder creation")
        }
        _ => {
            // Shouldn't reach here — translate_nodes already handles pixel ops.
            zencodecs::zennode_defs::QUALITY_INTENT_NODE_NODE
                .create_default()
                .expect("placeholder creation")
        }
    }
}

// Helper: TranslatedPipeline doesn't implement Clone, but we need the config.
impl TranslatedPipeline {
    pub(crate) fn clone_config(&self) -> TranslatedPipeline {
        TranslatedPipeline {
            nodes: Vec::new(), // don't clone the heavy part
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
