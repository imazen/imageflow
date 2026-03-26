//! Minimal zen pipeline exercise for heaptrack profiling.
//!
//! Usage: heaptrack cargo run --example zen_heaptrack --features zen-pipeline --no-default-features

use std::collections::HashMap;
use imageflow_types::*;

fn main() {
    // Generate a synthetic JPEG in memory using zencodecs.
    let jpeg_bytes = generate_test_jpeg(2000, 1500);
    eprintln!("Input JPEG: {} bytes ({}x{})", jpeg_bytes.len(), 2000, 1500);

    let mut io_buffers = HashMap::new();
    io_buffers.insert(0, jpeg_bytes);

    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Constrain(Constraint {
            mode: ConstraintMode::Within,
            w: Some(800),
            h: Some(600),
            hints: None,
            gravity: None,
            canvas_color: None,
        }),
        Node::Encode {
            io_id: 1,
            preset: EncoderPreset::Mozjpeg {
                quality: Some(85),
                progressive: Some(true),
                matte: None,
            },
        },
    ];

    let framewise = Framewise::Steps(steps);

    match imageflow_core::zen::execute_framewise(&framewise, &io_buffers) {
        Ok(results) => {
            for r in &results {
                eprintln!(
                    "Output: io_id={}, {}x{}, {} bytes, {}",
                    r.io_id, r.width, r.height, r.bytes.len(), r.mime_type
                );
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

/// Generate a synthetic JPEG test image using zencodecs.
fn generate_test_jpeg(w: u32, h: u32) -> Vec<u8> {
    // Create a gradient RGBA8 image.
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let offset = ((y * w + x) * 4) as usize;
            pixels[offset] = (x * 255 / w) as u8;     // R
            pixels[offset + 1] = (y * 255 / h) as u8;  // G
            pixels[offset + 2] = 128;                    // B
            pixels[offset + 3] = 255;                    // A
        }
    }

    // Encode to JPEG via zencodecs.
    let descriptor = zenpixels::PixelDescriptor::RGBA8_SRGB;
    let stride = (w * 4) as usize;
    let pixel_slice = zenpixels::PixelSlice::new(&pixels, w, h, stride, descriptor)
        .expect("pixel slice");

    let output = zencodecs::EncodeRequest::new(zencodecs::ImageFormat::Jpeg)
        .with_quality(90.0)
        .encode(pixel_slice, false)
        .expect("encode test jpeg");

    output.into_vec()
}
