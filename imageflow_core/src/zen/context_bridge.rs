//! Adapter between imageflow v2 JSON API and the zen streaming pipeline.
//!
//! The v2 Context manages IO buffer lifecycle and C ABI. The zen pipeline
//! takes bytes in and produces bytes out. This module bridges the two:
//!
//! - Extracts input bytes from `Build001.io` objects
//! - Runs the framewise pipeline through `execute.rs`
//! - Returns `JobResult` with encoded output bytes
//!
//! Output bytes are returned in `ZenBuildOutput.output_buffers` — the caller
//! (typically `v1/build` endpoint) is responsible for making them available
//! to `take_output_buffer()` / `get_output_buffer_ptr()` on the Context.

use std::collections::HashMap;

use imageflow_types as s;

use crate::errors::*;

use super::execute::{self, ZenError, ZenEncodeResult};

/// Result of a zen pipeline build.
pub struct ZenBuildOutput {
    /// v2-compatible job result for JSON serialization.
    pub job_result: s::JobResult,
    /// Encoded output bytes keyed by io_id.
    /// Caller stores these in Context output buffers.
    pub output_buffers: HashMap<i32, Vec<u8>>,
    /// Pixel data captured by CaptureBitmapKey nodes.
    pub captured_bitmaps: HashMap<i32, super::CapturedBitmap>,
}

/// Execute a `v1/build` request through the zen pipeline.
pub fn zen_build(parsed: &s::Build001) -> std::result::Result<ZenBuildOutput, FlowError> {
    let io_bytes = extract_input_bytes(&parsed.io)?;

    let result = execute::execute_framewise(&parsed.framewise, &io_bytes)
        .map_err(zen_to_flow)?;

    Ok(build_output(result))
}

/// Execute a `v1/execute` request through the zen pipeline.
///
/// For `execute`, IO is pre-configured on Context. The caller must pass
/// the input bytes explicitly since we don't access Context's IO system.
pub fn zen_execute(
    framewise: &s::Framewise,
    io_bytes: &HashMap<i32, Vec<u8>>,
) -> std::result::Result<ZenBuildOutput, FlowError> {
    let result = execute::execute_framewise(framewise, io_bytes)
        .map_err(zen_to_flow)?;

    Ok(build_output(result))
}

/// Probe an image via zencodecs and return v2-compatible ImageInfo.
pub fn zen_get_image_info(data: &[u8]) -> std::result::Result<s::ImageInfo, FlowError> {
    let info = execute::zen_get_image_info(data).map_err(zen_to_flow)?;

    // Query source encoding details for lossless detection.
    let lossless = info.source_encoding
        .as_ref()
        .map_or(false, |se| se.is_lossless());

    Ok(s::ImageInfo {
        image_width: info.width as i32,
        image_height: info.height as i32,
        preferred_mime_type: info.format.mime_type().to_string(),
        preferred_extension: info.format.extension().to_string(),
        frame_decodes_into: s::PixelFormat::Bgra32,
        multiple_frames: info.is_animation(),
        lossless,
    })
}

// ─── Internal ───

fn build_output(result: execute::ExecuteResult) -> ZenBuildOutput {
    let mut encodes = Vec::with_capacity(result.encode_results.len());
    let mut output_buffers = HashMap::with_capacity(result.encode_results.len());

    for r in result.encode_results {
        encodes.push(s::EncodeResult {
            io_id: r.io_id,
            w: r.width as i32,
            h: r.height as i32,
            preferred_mime_type: r.mime_type.to_string(),
            preferred_extension: r.extension.to_string(),
            bytes: s::ResultBytes::Elsewhere,
        });
        output_buffers.insert(r.io_id, r.bytes);
    }

    ZenBuildOutput {
        job_result: s::JobResult {
            encodes,
            decodes: Vec::new(),
            performance: None,
        },
        output_buffers,
        captured_bitmaps: result.captured_dimensions.captures,
    }
}

/// Extract input bytes from Build001 IoObject list.
///
/// Only processes `In`-direction objects. Output placeholders are skipped.
fn extract_input_bytes(io_objects: &[s::IoObject]) -> std::result::Result<HashMap<i32, Vec<u8>>, FlowError> {
    let mut map = HashMap::new();
    for obj in io_objects {
        if obj.direction == s::IoDirection::In {
            map.insert(obj.io_id, io_to_bytes(&obj.io)?);
        }
    }
    Ok(map)
}

/// Resolve an IoEnum to raw bytes.
fn io_to_bytes(io: &s::IoEnum) -> std::result::Result<Vec<u8>, FlowError> {
    match io {
        s::IoEnum::ByteArray(bytes) => Ok(bytes.clone()),
        s::IoEnum::Base64(b64) => {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| nerror!(ErrorKind::InvalidArgument, "invalid base64: {}", e))
        }
        s::IoEnum::BytesHex(hex) => {
            hex::decode(hex)
                .map_err(|e| nerror!(ErrorKind::InvalidArgument, "invalid hex: {}", e))
        }
        s::IoEnum::Filename(path) => {
            std::fs::read(path)
                .map_err(|e| nerror!(ErrorKind::DecodingIoError, "read {}: {}", path, e))
        }
        s::IoEnum::OutputBuffer | s::IoEnum::OutputBase64 => {
            Err(nerror!(ErrorKind::InvalidArgument, "output IO has no input bytes"))
        }
        s::IoEnum::Placeholder => {
            Err(nerror!(ErrorKind::InvalidArgument, "placeholder IO not resolved"))
        }
    }
}

fn zen_to_flow(e: ZenError) -> FlowError {
    match e {
        ZenError::Translate(t) => nerror!(ErrorKind::InvalidNodeParams, "{}", t),
        ZenError::Codec(msg) => nerror!(ErrorKind::ImageDecodingError, "{}", msg),
        ZenError::Pipeline(p) => nerror!(ErrorKind::InternalError, "{}", p),
        ZenError::Io(msg) => nerror!(ErrorKind::InvalidArgument, "{}", msg),
    }
}
