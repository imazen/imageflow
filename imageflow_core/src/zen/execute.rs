//! Top-level execution: v2 Framewise → zenpipe streaming pipeline → encoded output.
//!
//! This is the main entry point for the zen pipeline. It:
//! 1. Translates v2 `Node` variants into zenode instances
//! 2. Probes the source image via zencodecs
//! 3. Resolves format + quality via zencodecs selection engine
//! 4. Builds a streaming pipeline via zenpipe
//! 5. Executes: decode → process → encode

use std::collections::HashMap;

use imageflow_types::{self as s, Framewise, Node};
use zencodecs::{AllowedFormats, CodecPolicy, ImageFacts, select_format_from_intent};
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
    fn from(e: TranslateError) -> Self {
        Self::Translate(e)
    }
}

impl From<zenpipe::PipeError> for ZenError {
    fn from(e: zenpipe::PipeError) -> Self {
        Self::Pipeline(e)
    }
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
/// `io_buffers` maps io_id → input bytes. Returns encode results
/// for each output.
pub fn execute_framewise(
    framewise: &Framewise,
    io_buffers: &HashMap<i32, Vec<u8>>,
) -> Result<Vec<ZenEncodeResult>, ZenError> {
    let nodes = match framewise {
        Framewise::Steps(steps) => steps.clone(),
        Framewise::Graph(_graph) => {
            // TODO: topological sort the graph into a linear sequence.
            // For now, only linear steps are supported.
            return Err(ZenError::Translate(TranslateError::Unsupported(
                "graph mode not yet supported in zen pipeline — use steps".into(),
            )));
        }
    };

    // 1. Translate v2 nodes → zenode instances + codec intent.
    let pipeline = translate::translate_nodes(&nodes)?;

    let decode_io_id = pipeline.decode_io_id.ok_or_else(|| {
        ZenError::Io("no decode node in pipeline".into())
    })?;
    let encode_io_id = pipeline.encode_io_id.ok_or_else(|| {
        ZenError::Io("no encode node in pipeline".into())
    })?;

    let input_data = io_buffers.get(&decode_io_id).ok_or_else(|| {
        ZenError::Io(format!("no input buffer for io_id {decode_io_id}"))
    })?;

    // 2. Probe source image.
    let registry = AllowedFormats::all();
    let info = zencodecs::from_bytes(input_data)
        .map_err(|e| ZenError::Codec(format!("probe failed: {e}")))?;
    let facts = ImageFacts::from_image_info(&info);

    // 3. Resolve format + quality.
    let codec_intent = pipeline
        .preset
        .as_ref()
        .map(|p| &p.intent)
        .cloned()
        .unwrap_or_default();

    let decision = select_format_from_intent(&codec_intent, &facts, &registry, &CodecPolicy::default())
        .map_err(|e| ZenError::Codec(format!("format selection failed: {e}")))?;

    // 4. Decode full frame (no streaming decoder available).
    let decoded = zencodecs::DecodeRequest::new(input_data)
        .with_registry(&registry)
        .decode_full_frame()
        .map_err(|e| ZenError::Codec(format!("decode failed: {e}")))?;

    let decode_w = decoded.width();
    let decode_h = decoded.height();
    let decode_format: zenpipe::PixelFormat = decoded.descriptor();

    // Create a MaterializedSource from the decoded pixel buffer.
    let pixel_buf = decoded.into_buffer();
    let pixel_data = pixel_buf.copy_to_contiguous_bytes();
    let source = zenpipe::sources::MaterializedSource::from_data(
        pixel_data,
        decode_w,
        decode_h,
        decode_format,
    );

    // 5. Build zenpipe pipeline from zenode instances.
    // No custom converters needed — the bridge handles geometry/layout nodes directly.
    let converters: &[&dyn zenpipe::bridge::NodeConverter] = &[];

    let pipe_result = zenpipe::bridge::build_pipeline(
        Box::new(source),
        &pipeline.nodes,
        converters,
    )?;

    // 6. Materialize the pipeline output, then one-shot encode.
    // (Streaming encode requires Box<dyn DynEncoder + Send> but StreamingEncoder
    // only provides Box<dyn DynEncoder>. One-shot encode avoids the mismatch.)
    let materialized = pipe_result.materialize()?;
    let out_w = materialized.pixels.width();
    let out_h = materialized.pixels.height();
    let out_format = materialized.pixels.format();
    let has_alpha = out_format.has_alpha();

    let data = materialized.pixels.data();
    let stride = materialized.pixels.stride();
    let pixel_slice = zenpixels::PixelSlice::new(
        data, out_w, out_h, stride, out_format,
    ).map_err(|e| ZenError::Codec(format!("pixel slice construction failed: {e}")))?;

    let output = zencodecs::EncodeRequest::new(decision.format)
        .with_quality(decision.quality.quality)
        .with_lossless(decision.lossless)
        .with_registry(&registry)
        .encode(pixel_slice, has_alpha)
        .map_err(|e| ZenError::Codec(format!("encode failed: {e}")))?;

    Ok(vec![ZenEncodeResult {
        io_id: encode_io_id,
        bytes: output.into_vec(),
        width: out_w,
        height: out_h,
        mime_type: decision.format.mime_type(),
        extension: decision.format.extension(),
    }])
}
