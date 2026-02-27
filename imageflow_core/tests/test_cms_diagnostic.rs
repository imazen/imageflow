//! Quick diagnostic: transform a single non-sRGB image with Both backend
//! to see per-channel divergence details.

use imageflow_core::CmsBackend;
use imageflow_core::Context;
use imageflow_types as s;

#[test]
fn cms_diagnostic_single_file() {
    // Representative files from each divergence category
    let test_files = [
        // gAMA(0.45455)+D65 — sRGB-via-gAMA, previously max=9
        "/mnt/v/output/corpus-builder/png-24-32/web_boredpanda_com_52c63746588254c8.png",
        "/mnt/v/output/corpus-builder/png-24-32/web_boredpanda_com_893f8142576c5929.png",
        "/mnt/v/output/corpus-builder/png-24-32/web_cnet_com_4f935ab9c0ffae64.png",
        // gAMA(0.22727) = gamma 4.4, previously max=67
        "/mnt/v/output/corpus-builder/png-24-32/wm_upload_wikimedia_org_874b35d367b3a5f6.png",
        "/mnt/v/output/corpus-builder/png-24-32/wm_upload_wikimedia_org_fb89c54b5841b4ef.png",
        // Rec. 2020 PQ, previously max=224
        "/mnt/v/output/corpus-builder/wide-gamut/rec-2020-pq/flickr_2a68670c58131566.jpg",
        "/mnt/v/output/corpus-builder/wide-gamut/rec-2020-pq/flickr_c2d8824d6ffb6e60.jpg",
        // Apple Wide Color, previously max=12
        "/mnt/v/output/corpus-builder/wide-gamut/apple-wide-color-sharing-profile/wmc_d4e6bfcba7ee8f83.jpg",
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
