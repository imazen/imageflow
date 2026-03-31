//! Fuzz target: decode + re-encode through the zen pipeline.
//!
//! Structured fuzzing: arbitrary image bytes + random output format.
//! Tests the full decode -> encode path including pixel format
//! conversion and encoder robustness with arbitrary decoded pixels.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use std::collections::HashMap;

use imageflow_types::{
    EncoderPreset, ExecutionSecurity, Framewise, FrameSizeLimit, JobOptions, Node,
};

/// Which output format to encode to.
#[derive(Debug, Arbitrary)]
enum FuzzOutputFormat {
    Jpeg,
    Png,
    WebP,
    Gif,
}

/// Structured fuzz input: image bytes + output format choice.
#[derive(Debug, Arbitrary)]
struct FuzzInput {
    /// Raw image bytes (will be interpreted by format detection).
    image_data: Vec<u8>,
    /// Which format to encode to.
    output_format: FuzzOutputFormat,
    /// Quality 0-100 for lossy formats.
    quality_byte: u8,
}

fn fuzz_security() -> ExecutionSecurity {
    ExecutionSecurity {
        max_decode_size: Some(FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 }),
        max_frame_size: Some(FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 }),
        max_encode_size: Some(FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 }),
    }
}

fuzz_target!(|input: FuzzInput| {
    if input.image_data.len() < 8 {
        return;
    }

    let preset = match input.output_format {
        FuzzOutputFormat::Jpeg => EncoderPreset::Mozjpeg {
            quality: Some(input.quality_byte.min(100)),
            progressive: Some(false),
            matte: None,
        },
        FuzzOutputFormat::Png => EncoderPreset::Libpng {
            depth: None,
            matte: None,
            zlib_compression: None,
        },
        FuzzOutputFormat::WebP => EncoderPreset::WebPLossy {
            quality: (input.quality_byte as f32).min(100.0),
        },
        FuzzOutputFormat::Gif => EncoderPreset::Gif,
    };

    let steps = Framewise::Steps(vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Encode { io_id: 1, preset },
    ]);

    let mut io_buffers = HashMap::new();
    io_buffers.insert(0, input.image_data);

    let security = fuzz_security();
    let job_options = JobOptions::default();

    let _ = zenpipe::imageflow_compat::execute::execute_framewise(
        &steps,
        &io_buffers,
        &security,
        &job_options,
    );
});
