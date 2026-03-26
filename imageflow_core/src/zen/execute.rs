//! Top-level execution: v2 Framewise → zenpipe streaming pipeline → encoded output.
//!
//! Pipeline stages:
//! 1. Translate v2 `Node` variants into zenode instances
//! 2. Probe source image via zencodecs
//! 3. Resolve format + quality via zencodecs selection engine
//! 4. Decode full frame → `MaterializedSource` (streaming decode blocked on codec
//!    lifetime constraints — JPEG/PNG `ScanlineReader` borrows input `&[u8]`)
//! 5. Stream through zenpipe (strip-based, zero-materialization between operations)
//! 6. Stream-encode: pull strips from pipeline, push directly to encoder

use std::borrow::Cow;
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
/// `io_buffers` maps io_id → input bytes. Returns encode results for each output.
///
/// # Streaming behavior
///
/// - **Decode**: full-frame via `decode_full_frame()`, then wrapped in `MaterializedSource`.
///   JPEG/PNG streaming decoders borrow input data with a non-`'static` lifetime,
///   which is incompatible with `build_pipeline`'s `Box<dyn Source>` (`'static`).
///   The decoded frame is consumed row-by-row by the pipeline — it's the *pipeline*
///   that streams, not the decoder.
///
/// - **Pipeline**: fully streaming via zenpipe. Adjacent operations pull strips from
///   upstream. A 5-step pipeline uses strip-height × width memory, not 5 full frames.
///
/// - **Encode**: streaming via `push_rows()` / `finish()` on `DynEncoder`. Strips from
///   the pipeline are pushed directly into the encoder — no intermediate full-frame buffer.
pub fn execute_framewise(
    framewise: &Framewise,
    io_buffers: &HashMap<i32, Vec<u8>>,
) -> Result<Vec<ZenEncodeResult>, ZenError> {
    let nodes = match framewise {
        Framewise::Steps(steps) => steps.clone(),
        Framewise::Graph(_graph) => {
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

    let decision = select_format_from_intent(
        &codec_intent, &facts, &registry, &CodecPolicy::default(),
    )
    .map_err(|e| ZenError::Codec(format!("format selection failed: {e}")))?;

    // 4. Decode full frame → MaterializedSource.
    let decoded = zencodecs::DecodeRequest::new(input_data)
        .with_registry(&registry)
        .decode_full_frame()
        .map_err(|e| ZenError::Codec(format!("decode failed: {e}")))?;

    let decode_w = decoded.width();
    let decode_h = decoded.height();
    let decode_format: zenpipe::PixelFormat = decoded.descriptor();

    let pixel_buf = decoded.into_buffer();
    let pixel_data = pixel_buf.copy_to_contiguous_bytes();
    let source = zenpipe::sources::MaterializedSource::from_data(
        pixel_data, decode_w, decode_h, decode_format,
    );

    // 5. Build streaming pipeline from zenode instances.
    let converters: &[&dyn zenpipe::bridge::NodeConverter] = &[];
    let pipe_result = zenpipe::bridge::build_pipeline(
        Box::new(source), &pipeline.nodes, converters,
    )?;

    let out_w = pipe_result.source.width();
    let out_h = pipe_result.source.height();
    let out_format = pipe_result.source.format();

    // 6. Stream-encode: build encoder, pull strips from pipeline, push to encoder.
    let streaming_enc = zencodecs::EncodeRequest::new(decision.format)
        .with_quality(decision.quality.quality)
        .with_lossless(decision.lossless)
        .with_registry(&registry)
        .build_streaming_encoder(out_w, out_h)
        .map_err(|e| ZenError::Codec(format!("streaming encoder creation failed: {e}")))?;

    // Stream strips directly into the encoder — no Send bound needed, no Sink trait,
    // no intermediate full-frame buffer. Just a loop.
    let mut pipe_source = pipe_result.source;
    let mut encoder = streaming_enc.encoder;

    while let Some(strip) = pipe_source.next()? {
        let pixels = zenpixels::PixelSlice::new(
            strip.as_strided_bytes(),
            strip.width(),
            strip.rows(),
            strip.stride(),
            out_format,
        )
        .map_err(|e| ZenError::Codec(format!("pixel slice: {e}")))?;

        encoder
            .push_rows(pixels)
            .map_err(|e| ZenError::Codec(format!("encode push_rows: {e}")))?;
    }

    let output = encoder
        .finish()
        .map_err(|e| ZenError::Codec(format!("encode finish: {e}")))?;

    Ok(vec![ZenEncodeResult {
        io_id: encode_io_id,
        bytes: output.into_vec(),
        width: out_w,
        height: out_h,
        mime_type: decision.format.mime_type(),
        extension: decision.format.extension(),
    }])
}
