//! Bridge between imageflow v2 request types and the zen pipeline.
//!
//! Extracts input bytes from `IoObject` list, runs them through
//! the zen pipeline, and returns results as `JobResult`.
//!
//! Does NOT interact with `Context` IO system — operates on raw bytes.
//! The caller (endpoint handler) is responsible for storing output bytes
//! in Context's output buffers if needed.

use std::collections::HashMap;

use imageflow_types as s;

use crate::errors::*;

use super::execute::{self, ZenError};

/// Result of a zen pipeline build — encoded bytes + metadata.
pub struct ZenBuildOutput {
    pub job_result: s::JobResult,
    /// Encoded output bytes keyed by io_id.
    pub output_buffers: HashMap<i32, Vec<u8>>,
}

/// Execute a Build001 request through the zen pipeline.
///
/// Extracts input bytes from the `io` objects in the request,
/// runs the framewise pipeline through zenpipe+zencodecs,
/// and returns the encoded output bytes alongside a `JobResult`.
pub fn zen_build(parsed: &s::Build001) -> std::result::Result<ZenBuildOutput, FlowError> {
    // 1. Extract input bytes from IO objects.
    let io_bytes = extract_io_bytes(&parsed.io)?;

    // 2. Execute through zen pipeline.
    let results = execute::execute_framewise(&parsed.framewise, &io_bytes)
        .map_err(zen_error_to_flow_error)?;

    // 3. Build JobResult and output buffer map.
    let mut encodes = Vec::new();
    let mut output_buffers = HashMap::new();

    for result in results {
        encodes.push(s::EncodeResult {
            io_id: result.io_id,
            w: result.width as i32,
            h: result.height as i32,
            preferred_mime_type: result.mime_type.to_string(),
            preferred_extension: result.extension.to_string(),
            bytes: s::ResultBytes::Elsewhere,
        });
        output_buffers.insert(result.io_id, result.bytes);
    }

    Ok(ZenBuildOutput {
        job_result: s::JobResult {
            encodes,
            decodes: Vec::new(),
            performance: None,
        },
        output_buffers,
    })
}

/// Probe an image and return v2-compatible ImageInfo.
pub fn zen_get_image_info(data: &[u8]) -> std::result::Result<s::ImageInfo, FlowError> {
    let info = execute::zen_get_image_info(data)
        .map_err(zen_error_to_flow_error)?;

    Ok(s::ImageInfo {
        image_width: info.width as i32,
        image_height: info.height as i32,
        preferred_mime_type: info.format.mime_type().to_string(),
        preferred_extension: info.format.extension().to_string(),
        frame_decodes_into: s::PixelFormat::Bgra32,
        multiple_frames: info.is_animation(),
        lossless: false, // TODO: check source encoding details
    })
}

// ─── Helpers ───

/// Extract input bytes from Build001 IoObject list.
fn extract_io_bytes(io_objects: &[s::IoObject]) -> std::result::Result<HashMap<i32, Vec<u8>>, FlowError> {
    let mut map = HashMap::new();
    for obj in io_objects {
        if obj.direction == s::IoDirection::In {
            let bytes = io_enum_to_bytes(&obj.io)?;
            map.insert(obj.io_id, bytes);
        }
    }
    Ok(map)
}

/// Convert IoEnum to raw bytes.
fn io_enum_to_bytes(io: &s::IoEnum) -> std::result::Result<Vec<u8>, FlowError> {
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
                .map_err(|e| nerror!(ErrorKind::DecodingIoError, "cannot read {}: {}", path, e))
        }
        s::IoEnum::OutputBuffer | s::IoEnum::OutputBase64 => {
            Err(nerror!(ErrorKind::InvalidArgument, "output buffer has no input bytes"))
        }
        s::IoEnum::Placeholder => {
            Err(nerror!(ErrorKind::InvalidArgument, "placeholder IO not supported"))
        }
    }
}

fn zen_error_to_flow_error(e: ZenError) -> FlowError {
    match e {
        ZenError::Translate(t) => {
            nerror!(ErrorKind::InvalidNodeParams, "zen translate: {}", t)
        }
        ZenError::Codec(msg) => {
            nerror!(ErrorKind::ImageEncodingError, "zen codec: {}", msg)
        }
        ZenError::Pipeline(p) => {
            nerror!(ErrorKind::InternalError, "zen pipeline: {}", p)
        }
        ZenError::Io(msg) => {
            nerror!(ErrorKind::InvalidArgument, "zen io: {}", msg)
        }
    }
}
