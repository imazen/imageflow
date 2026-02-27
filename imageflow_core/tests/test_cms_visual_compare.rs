//! Visual comparison: convert an image with each CMS backend separately
//! and save the sRGB JPEG outputs for side-by-side viewing.

use imageflow_core::CmsBackend;
use imageflow_core::Context;
use imageflow_core::IoDirection;
use imageflow_types as s;

#[test]
fn cms_visual_compare_rec2020_pq() {
    let test_file =
        "/mnt/v/output/corpus-builder/wide-gamut/rec-2020-pq/flickr_c2d8824d6ffb6e60.jpg";
    if !std::path::Path::new(test_file).exists() {
        eprintln!("Skipping: test file not found");
        return;
    }

    let bytes = std::fs::read(test_file).unwrap();
    let out_dir = "/mnt/v/output/cms-compare";

    for (backend, name) in [(CmsBackend::Moxcms, "moxcms"), (CmsBackend::Lcms2, "lcms2")] {
        let out_path = format!("{out_dir}/rec2020pq_{name}.jpg");

        let mut ctx = Context::create().unwrap();
        ctx.cms_backend = backend;

        ctx.add_input_vector(0, bytes.clone()).unwrap();
        ctx.add_file(1, IoDirection::Out, &out_path).unwrap();

        let execute = s::Execute001 {
            graph_recording: Some(s::Build001GraphRecording::off()),
            security: Some(s::ExecutionSecurity {
                max_decode_size: Some(s::FrameSizeLimit {
                    w: 100_000,
                    h: 100_000,
                    megapixels: 500.0,
                }),
                max_frame_size: None,
                max_encode_size: None,
            }),
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
            Ok(_) => eprintln!("Wrote {out_path}"),
            Err(e) => eprintln!("ERROR ({name}): {e}"),
        }
    }
}
