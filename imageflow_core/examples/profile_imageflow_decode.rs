//! Standalone profiling target: decode a 1024x1024 JPEG via imageflow's
//! zen adapter path, no resize, to measure adapter overhead.
use imageflow_core::Context;
use imageflow_types as s;

fn main() {
    // Build a 1024x1024 JPEG fixture once
    let jpeg = {
        let mut ctx = Context::create().unwrap();
        ctx.add_output_buffer(1).unwrap();
        let steps = vec![
            s::Node::CreateCanvas {
                w: 1024,
                h: 1024,
                format: s::PixelFormat::Bgra32,
                color: s::Color::Srgb(s::ColorSrgb::Hex("FF8040FF".to_string())),
            },
            s::Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::LibjpegTurbo {
                    quality: Some(85),
                    progressive: Some(false),
                    optimize_huffman_coding: None,
                    matte: None,
                },
            },
        ];
        ctx.execute_1(s::Execute001 {
            graph_recording: Some(s::Build001GraphRecording::off()),
            security: None,
            job_options: None,
            framewise: s::Framewise::Steps(steps),
        })
        .unwrap();
        ctx.take_output_buffer(1).unwrap()
    };
    eprintln!("fixture: {} bytes", jpeg.len());

    // Decode 50 times through imageflow zen adapter path
    for _ in 0..50 {
        let mut ctx = Context::create().unwrap();
        // Force zen decoder for JPEG (matches bench_codecs zen path)
        ctx.enabled_codecs.prefer_decoder(imageflow_core::NamedDecoders::ZenJpegDecoder);
        ctx.enabled_codecs.disable_decoder(imageflow_core::NamedDecoders::MozJpegRsDecoder);
        ctx.enabled_codecs.disable_decoder(imageflow_core::NamedDecoders::ImageRsJpegDecoder);
        ctx.add_input_vector(0, jpeg.clone()).unwrap();
        ctx.execute_1(s::Execute001 {
            graph_recording: Some(s::Build001GraphRecording::off()),
            security: None,
            job_options: None,
            framewise: s::Framewise::Steps(vec![
                s::Node::Decode { io_id: 0, commands: None },
                // No resize — pure decode
            ]),
        })
        .unwrap();
    }
}
