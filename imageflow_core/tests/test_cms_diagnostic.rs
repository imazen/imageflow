//! Quick diagnostic: transform a single non-sRGB image with Both backend
//! to see per-channel divergence details.

use imageflow_core::CmsBackend;
use imageflow_core::Context;
use imageflow_types as s;

#[test]
fn cms_diagnostic_single_file() {
    // Test multiple profile types to find divergence
    let test_files = [
        // SC-P600 printer profiles — highest divergence (max 179/224)
        "/mnt/v/output/corpus-builder/wide-gamut/sc-p600-series-premium-luster/flickr_6c33721c499abbae.jpg",
        "/mnt/v/output/corpus-builder/wide-gamut/sc-p600-series-premium-luster/flickr_7b77b06a9c5a7b7e.jpg",
        "/mnt/v/output/corpus-builder/wide-gamut/sc-p600-series-premium-luster/flickr_8c6027f8b9ec751a.jpg",
        "/mnt/v/output/corpus-builder/wide-gamut/sc-p600-series-premium-luster/flickr_ca69175834c103c9.jpg",
        // Other wide-gamut profiles
        "/mnt/v/output/corpus-builder/wide-gamut/apple-wide-color-sharing-profile/wmc_d4e6bfcba7ee8f83.jpg",
        "/mnt/v/output/corpus-builder/wide-gamut/albook20070305colorvision/irsample_e9e92ac0384cdd65.jpg",
        "/mnt/v/output/corpus-builder/wide-gamut/prophoto-rgb/flickr_0d2d634cf46df137.jpg",
    ];
    for test_file in test_files {
        eprintln!("\n=== Testing: {} ===", test_file);
        test_single_file(test_file);
    }
}

fn test_single_file(test_file: &str) {
    let test_file = test_file;
    if !std::path::Path::new(test_file).exists() {
        eprintln!("Skipping: test file not found");
        return;
    }

    let bytes = std::fs::read(test_file).unwrap();
    let file_size = bytes.len();

    let mut ctx = Context::create().unwrap();
    ctx.cms_backend = CmsBackend::Both;

    ctx.add_input_vector(0, bytes).unwrap();
    ctx.add_output_buffer(1).unwrap();

    let execute = s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: Some(s::ExecutionSecurity {
            max_decode_size: Some(s::FrameSizeLimit { w: 100_000, h: 100_000, megapixels: 500.0 }),
            max_frame_size: None,
            max_encode_size: None,
        }),
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
    };

    match ctx.execute_1(execute) {
        Ok(_) => eprintln!("OK ({file_size} bytes)"),
        Err(e) => eprintln!("ERROR ({file_size} bytes): {e}"),
    }
}
