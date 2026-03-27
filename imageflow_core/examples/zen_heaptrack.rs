//! Minimal zen pipeline exercise for heaptrack profiling.
//!
//! Usage: heaptrack cargo run --example zen_heaptrack --features zen-pipeline --no-default-features --release -- [path.jpg]

use std::collections::HashMap;
use imageflow_types::*;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        // Default test image.
        "/home/lilith/work/filter-research/repos/rawpedia/static/images/Preview_6_focus_2.jpg".to_string()
    });

    let jpeg_bytes = std::fs::read(&path).expect("read input file");
    let info = zencodecs::from_bytes(&jpeg_bytes).expect("probe");
    eprintln!("Input: {} ({}x{}, {} bytes)", path, info.width, info.height, jpeg_bytes.len());

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

    let security = imageflow_types::ExecutionSecurity::sane_defaults();
    match imageflow_core::zen::execute_framewise(&framewise, &io_buffers, &security) {
        Ok(results) => {
            for r in &results.encode_results {
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
