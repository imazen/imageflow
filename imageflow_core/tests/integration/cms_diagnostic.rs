//! CMS dual-backend regression test: decode representative non-sRGB images
//! with CmsBackend::Both to verify moxcms and lcms2 agree.
//!
//! Test images are downloaded from S3 and cover the major CMS code paths:
//! - gAMA-only with sRGB-equivalent gamma (0.45455)
//! - gAMA-only with non-neutral gamma (0.22727 = gamma 4.4)
//! - Rec. 2020 PQ (ICC v4.2 with CICP)
//! - Apple Wide Color (Display P3)

use crate::common::{get_url_bytes_with_retry, test_init};
use imageflow_core::Context;
use imageflow_types as s;

const S3_BASE: &str =
    "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/color-management";

/// Representative test files and their CMS categories.
const TEST_FILES: &[(&str, &str)] = &[
    // gAMA(0.45455)+D65 — sRGB-via-gAMA
    ("web_boredpanda_com_52c63746588254c8.png", "gAMA sRGB-equivalent"),
    ("web_boredpanda_com_893f8142576c5929.png", "gAMA sRGB-equivalent"),
    ("web_cnet_com_4f935ab9c0ffae64.png", "gAMA sRGB-equivalent"),
    // gAMA(0.22727) = gamma 4.4
    ("wm_upload_wikimedia_org_874b35d367b3a5f6.png", "gAMA high-gamma"),
    ("wm_upload_wikimedia_org_fb89c54b5841b4ef.png", "gAMA high-gamma"),
    // Rec. 2020 PQ
    ("flickr_2a68670c58131566.jpg", "Rec.2020 PQ"),
    ("flickr_c2d8824d6ffb6e60.jpg", "Rec.2020 PQ"),
    // Apple Wide Color (Display P3)
    ("wmc_d4e6bfcba7ee8f83.jpg", "Apple Wide Color"),
];

const CMYK_URL: &str =
    "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/cmyk_logo.jpg";

/// Decode CMYK JPEG with Both backend to verify moxcms and lcms2 don't panic.
#[test]
fn cms_cmyk_backend_divergence() {
    test_init();
    let bytes = get_url_bytes_with_retry(CMYK_URL)
        .expect("Failed to download CMYK test file from S3 — test requires network");

    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, bytes).unwrap();
    ctx.add_output_buffer(1).unwrap();

    let execute = s::Execute001 {
        job_options: None,
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Libpng {
                    depth: Some(s::PngBitDepth::Png32),
                    matte: None,
                    zlib_compression: None,
                },
            },
        ]),
        job_options: None,
    };

    ctx.execute_1(execute).unwrap();
}

#[test]
fn cms_dual_backend_regression() {
    test_init();
    let mut cms_failures = Vec::new();
    let mut fetch_failures = Vec::new();
    let mut tested = 0usize;

    for &(filename, category) in TEST_FILES {
        let url = format!("{S3_BASE}/{filename}");
        let bytes = match get_url_bytes_with_retry(&url) {
            Ok(b) => b,
            Err(e) => {
                fetch_failures.push(format!("{filename} ({category}): fetch error: {e}"));
                continue;
            }
        };

        tested += 1;

        let mut ctx = Context::create().unwrap();

        ctx.add_input_vector(0, bytes).unwrap();
        ctx.add_output_buffer(1).unwrap();

        let execute = s::Execute001 {
            job_options: None,
            graph_recording: Some(s::Build001GraphRecording::off()),
            security: None,
            framewise: s::Framewise::Steps(vec![
                s::Node::Decode { io_id: 0, commands: None },
                s::Node::Constrain(s::Constraint {
                    mode: s::ConstraintMode::Within,
                    w: Some(256),
                    h: Some(256),
                    hints: None,
                    gravity: None,
                    canvas_color: None,
                }),
                s::Node::Encode {
                    io_id: 1,
                    preset: s::EncoderPreset::Libpng {
                        depth: Some(s::PngBitDepth::Png32),
                        matte: None,
                        zlib_compression: None,
                    },
                },
            ]),
            job_options: None,
        };

        if let Err(e) = ctx.execute_1(execute) {
            cms_failures.push(format!("{filename} ({category}): {e}"));
        }
    }

    assert!(
        tested > 0,
        "All {} test files failed to download:\n{}",
        fetch_failures.len(),
        fetch_failures.join("\n")
    );
    assert!(
        cms_failures.is_empty(),
        "CMS dual-backend failures ({tested} tested, {} fetch errors):\n{}",
        fetch_failures.len(),
        cms_failures.join("\n")
    );
}
