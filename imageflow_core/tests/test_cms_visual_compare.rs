//! Visual comparison: decode an image with each CMS backend separately
//! and save sRGB outputs for side-by-side viewing.
//!
//! **Local development tool** — writes output files for manual inspection.
//! Not run on CI. Use `cargo test --test test_cms_visual_compare -- --ignored --nocapture`.
//!
//! Configure paths via env vars:
//!   IMAGEFLOW_DEV_DIR  — output base directory (default: /mnt/v on Linux, V:\ on Windows)

mod common;

use common::get_url_bytes_with_retry;
use imageflow_core::CmsBackend;
use imageflow_core::Context;
use imageflow_types as s;
use std::path::PathBuf;

fn dev_dir() -> PathBuf {
    std::env::var("IMAGEFLOW_DEV_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(if cfg!(windows) { "V:\\" } else { "/mnt/v" }))
}

const S3_BASE: &str =
    "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/color-management";

#[test]
#[ignore]
fn cms_visual_compare_rec2020_pq() {
    let url = format!("{S3_BASE}/flickr_c2d8824d6ffb6e60.jpg");
    let bytes = get_url_bytes_with_retry(&url).expect("Failed to download test file from S3");

    let out_dir = dev_dir().join("output").join("cms-compare");
    std::fs::create_dir_all(&out_dir).unwrap();

    for (backend, name) in [(CmsBackend::Moxcms, "moxcms"), (CmsBackend::Lcms2, "lcms2")] {
        let mut ctx = Context::create().unwrap();
        ctx.cms_backend = backend;

        ctx.add_input_vector(0, bytes.clone()).unwrap();
        ctx.add_output_buffer(1).unwrap();

        let execute = s::Execute001 {
            graph_recording: Some(s::Build001GraphRecording::off()),
            security: None,
            framewise: s::Framewise::Steps(vec![
                s::Node::Decode { io_id: 0, commands: None },
                s::Node::Encode {
                    io_id: 1,
                    preset: s::EncoderPreset::Mozjpeg {
                        quality: Some(95),
                        progressive: None,
                        matte: None,
                    },
                },
            ]),
        };

        match ctx.execute_1(execute) {
            Ok(_) => {
                let out_bytes = ctx.take_output_buffer(1).unwrap();
                let out_path = out_dir.join(format!("rec2020pq_{name}.jpg"));
                std::fs::write(&out_path, &out_bytes).unwrap();
                eprintln!("Wrote {}", out_path.display());
            }
            Err(e) => eprintln!("ERROR ({name}): {e}"),
        }
    }
}
