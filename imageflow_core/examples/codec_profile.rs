//! Codec profiling harness for heaptrack.
//! Exercises every decode→encode combination.
//!
//! Usage: heaptrack cargo run --release -p imageflow_core --example codec_profile -- <image_dir>
//!
//! Expects <image_dir> to contain: test.png, test.jpg, test.gif, test.webp

use imageflow_core::Context;
use imageflow_types as s;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let image_dir = PathBuf::from(args.get(1).expect("Usage: codec_profile <image_dir>"));

    // Source images in different formats
    let sources: Vec<(&str, Vec<u8>)> = ["test.png", "test.jpg", "test.gif", "test.webp"]
        .iter()
        .filter_map(|name| {
            let path = image_dir.join(name);
            std::fs::read(&path).ok().map(|data| (*name, data))
        })
        .collect();

    if sources.is_empty() {
        eprintln!("No test images found in {:?}", image_dir);
        eprintln!("Expected: test.png, test.jpg, test.gif, test.webp");
        std::process::exit(1);
    }

    let encode_presets: Vec<(&str, s::EncoderPreset)> = vec![
        ("jpeg", s::EncoderPreset::Mozjpeg { quality: Some(80), progressive: None, matte: None }),
        ("png", s::EncoderPreset::Lodepng { maximum_deflate: None }),
        ("webp_lossy", s::EncoderPreset::WebPLossy { quality: 80.0 }),
        ("webp_lossless", s::EncoderPreset::WebPLossless),
        ("gif", s::EncoderPreset::Gif),
        ("jxl_lossy", s::EncoderPreset::JxlLossy { distance: 2.0 }),
        ("jxl_lossless", s::EncoderPreset::JxlLossless),
    ];

    let iterations = 3;
    eprintln!(
        "Sources: {:?}",
        sources.iter().map(|(n, d)| format!("{} ({}KB)", n, d.len() / 1024)).collect::<Vec<_>>()
    );
    eprintln!("Encode presets: {:?}", encode_presets.iter().map(|(n, _)| *n).collect::<Vec<_>>());
    eprintln!("Iterations per combo: {}", iterations);

    for (src_name, src_data) in &sources {
        for (enc_name, preset) in &encode_presets {
            for i in 0..iterations {
                let mut ctx = Context::create().unwrap();
                ctx.add_copied_input_buffer(0, src_data).unwrap();
                ctx.add_output_buffer(1).unwrap();

                let framewise = s::Framewise::Steps(vec![
                    s::Node::Decode { io_id: 0, commands: None },
                    s::Node::Constrain(s::Constraint {
                        mode: s::ConstraintMode::Within,
                        w: Some(400),
                        h: Some(400),
                        hints: None,
                        gravity: None,
                        canvas_color: None,
                    }),
                    s::Node::Encode { io_id: 1, preset: preset.clone() },
                ]);

                let build = s::Execute001 { framewise, graph_recording: None, security: None };

                match ctx.execute_1(build) {
                    Ok(_) => {
                        if i == 0 {
                            // Report output size on first iteration
                            let bytes = ctx.take_output_buffer(1).unwrap();
                            eprintln!("  {} → {}: {}KB", src_name, enc_name, bytes.len() / 1024);
                        }
                    }
                    Err(e) => {
                        if i == 0 {
                            eprintln!("  {} → {}: SKIP ({})", src_name, enc_name, e);
                        }
                    }
                }
            }
        }
    }
    eprintln!("Done.");
}
