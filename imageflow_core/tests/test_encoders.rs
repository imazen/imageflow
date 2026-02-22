#[macro_use]
extern crate imageflow_core;
extern crate imageflow_helpers as hlp;
extern crate imageflow_types as s;
extern crate serde_json;
extern crate smallvec;

pub mod common;
use crate::common::*;

use imageflow_core::Context;
use s::{
    Color, ColorSrgb, CommandStringKind, EncoderPreset, Execute001, Framewise, Node, PixelFormat,
    ResponsePayload,
};

const DEBUG_GRAPH: bool = false;
const FRYMIRE_URL: &'static str =
    "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/frymire.png";

#[test]
fn test_encode_png() {
    let steps = reencode_with(s::EncoderPreset::Lodepng { maximum_deflate: None });

    compare_encoded_to_source(
        IoTestEnum::Url(FRYMIRE_URL.to_owned()),
        DEBUG_GRAPH,
        Constraints {
            max_file_size: Some(390_000),
            similarity: Similarity::AllowDssimMatch(0.0, 0.0),
        },
        steps,
    );
}

#[test]
fn test_encode_pngquant() {
    let steps = reencode_with(s::EncoderPreset::Pngquant {
        speed: None,
        quality: Some(100),
        maximum_deflate: None,
        minimum_quality: None,
    });

    compare_encoded_to_source(
        IoTestEnum::Url(FRYMIRE_URL.to_owned()),
        DEBUG_GRAPH,
        Constraints {
            max_file_size: Some(280_000),
            similarity: Similarity::AllowDssimMatch(0.0017, 0.008),
        },
        steps,
    );
}
#[test]
fn test_encode_pngquant_command() {
    let steps = reencode_with_command("png.min_quality=0&png.quality=100");

    compare_encoded_to_source(
        IoTestEnum::Url(FRYMIRE_URL.to_owned()),
        DEBUG_GRAPH,
        Constraints {
            max_file_size: Some(280_000),
            similarity: Similarity::AllowDssimMatch(0.0017, 0.008),
        },
        steps,
    );
}
#[test]
fn test_encode_pngquant_fallback() {
    let steps = reencode_with(s::EncoderPreset::Pngquant {
        speed: None,
        quality: Some(100),
        maximum_deflate: None,
        minimum_quality: Some(99),
    });

    compare_encoded_to_source(
        IoTestEnum::Url(FRYMIRE_URL.to_owned()),
        DEBUG_GRAPH,
        Constraints { max_file_size: None, similarity: Similarity::AllowDssimMatch(0.000, 0.001) },
        steps,
    );
}
#[test]
fn test_encode_pngquant_fallback_command() {
    let steps = reencode_with_command("png.min_quality=99&png.quality=100");

    compare_encoded_to_source(
        IoTestEnum::Url(FRYMIRE_URL.to_owned()),
        DEBUG_GRAPH,
        Constraints { max_file_size: None, similarity: Similarity::AllowDssimMatch(0.000, 0.001) },
        steps,
    );
}

#[test]
fn test_encode_lodepng() {
    let steps = reencode_with(s::EncoderPreset::Lodepng { maximum_deflate: None });

    compare_encoded_to_source(
        IoTestEnum::Url(FRYMIRE_URL.to_owned()),
        DEBUG_GRAPH,
        Constraints {
            max_file_size: Some(390_000),
            similarity: Similarity::AllowDssimMatch(0., 0.),
        },
        steps,
    );
}

#[test]
fn test_encode_mozjpeg_resized() {
    let use_hermite = s::ResampleHints::new().with_bi_filter(s::Filter::Hermite);
    let steps = vec![
        s::Node::Decode { io_id: 0, commands: None },
        s::Node::Resample2D { w: 550, h: 550, hints: Some(use_hermite.clone()) },
        s::Node::Resample2D { w: 1118, h: 1105, hints: Some(use_hermite.clone()) },
        s::Node::Encode {
            io_id: 1,
            preset: s::EncoderPreset::Mozjpeg { progressive: None, quality: Some(50), matte: None },
        },
    ];

    compare_encoded_to_source(
        IoTestEnum::Url(FRYMIRE_URL.to_owned()),
        DEBUG_GRAPH,
        Constraints {
            max_file_size: Some(160_000),
            similarity: Similarity::AllowDssimMatch(0.04, 0.2),
        },
        steps,
    );
}

#[test]
fn test_encode_mozjpeg() {
    let steps = reencode_with(s::EncoderPreset::Mozjpeg {
        progressive: None,
        quality: Some(50),
        matte: None,
    });

    compare_encoded_to_source(
        IoTestEnum::Url(FRYMIRE_URL.to_owned()),
        DEBUG_GRAPH,
        Constraints {
            max_file_size: Some(301_000),
            similarity: Similarity::AllowDssimMatch(0.007, 0.06),
        },
        steps,
    );
}

#[test]
fn test_encode_webp_lossless() {
    let steps = reencode_with(s::EncoderPreset::WebPLossless);

    compare_encoded_to_source(
        IoTestEnum::Url(FRYMIRE_URL.to_owned()),
        DEBUG_GRAPH,
        Constraints {
            max_file_size: Some(301_000),
            similarity: Similarity::AllowDssimMatch(0., 0.),
        },
        steps,
    );
}

#[test]
fn test_roundtrip_webp_lossless() {
    let steps = reencode_with(s::EncoderPreset::WebPLossless);

    compare_encoded_to_source(
        IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/5_webp_ll.webp"
                .to_owned(),
        ),
        DEBUG_GRAPH,
        Constraints {
            max_file_size: Some(301_000),
            similarity: Similarity::AllowDssimMatch(0., 0.),
        },
        steps,
    );
}

#[test]
fn test_encode_webp_lossy() {
    let steps = reencode_with(s::EncoderPreset::WebPLossy { quality: 90f32 });

    compare_encoded_to_source(
        IoTestEnum::Url(FRYMIRE_URL.to_owned()),
        DEBUG_GRAPH,
        Constraints {
            max_file_size: Some(425_000),
            similarity: Similarity::AllowDssimMatch(0., 0.01),
        },
        steps,
    );
}

pub fn reencode_with(preset: s::EncoderPreset) -> Vec<s::Node> {
    vec![s::Node::Decode { io_id: 0, commands: None }, s::Node::Encode { io_id: 1, preset }]
}
pub fn reencode_with_command(command: &str) -> Vec<s::Node> {
    vec![s::Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: command.to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }]
}

/// Compares the encoded result of a given job to the source. If there is a checksum mismatch, a percentage of off-by-one bytes can be allowed.
/// The output io_id is 1
pub fn compare_encoded_to_source(
    input: IoTestEnum,
    debug: bool,
    require: Constraints,
    steps: Vec<s::Node>,
) -> bool {
    let input_copy = input.clone();

    let execute = s::Execute001 {
        graph_recording: default_graph_recording(debug),
        security: None,
        framewise: s::Framewise::Steps(steps),
    };

    if debug {
        println!("{}", serde_json::to_string_pretty(&execute).unwrap());
    }

    let mut context = Context::create().unwrap();
    IoTestTranslator {}.add(&mut context, 0, input).unwrap();
    IoTestTranslator {}.add(&mut context, 1, IoTestEnum::OutputBuffer).unwrap();

    let response = context.execute_1(execute).unwrap();

    match response {
        ResponsePayload::JobResult(r) => {
            assert_eq!(r.decodes.len(), 1);
            assert!(r.decodes[0].preferred_mime_type.len() > 0);
            assert!(r.decodes[0].preferred_extension.len() > 0);
            assert!(r.decodes[0].w > 0);
            assert!(r.decodes[0].h > 0);
            assert_eq!(r.encodes.len(), 1);
            assert!(r.encodes[0].preferred_mime_type.len() > 0);
            assert!(r.encodes[0].preferred_extension.len() > 0);
            assert!(r.encodes[0].w > 0);
            assert!(r.encodes[0].h > 0);
        }
        _ => {}
    }

    let bytes = context.get_output_buffer_slice(1).unwrap();

    let ctx = ChecksumCtx::visuals();

    let mut context2 = Context::create().unwrap();

    let bitmap_key = decode_input(&mut context2, input_copy);
    let original_checksum;
    {
        let bitmaps = context2.borrow_bitmaps().map_err(|e| e.at(here!())).unwrap();

        let mut original = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!())).unwrap();

        let mut original_window = original.get_window_u8().unwrap();

        original_checksum = ChecksumCtx::checksum_bitmap_window(&mut original_window);
        ctx.save_frame(&mut original_window, &original_checksum);
    }

    compare_with(
        &ctx,
        &original_checksum,
        context2,
        bitmap_key,
        ResultKind::Bytes(bytes),
        require,
        true,
    )
}

/// Build a minimal animated GIF with the given frame colors (RGBA hex strings).
/// Each frame is `w`x`h` pixels, solid color, with the given delay in centiseconds.
fn build_animated_gif(w: u16, h: u16, colors: &[&str], delay: u16) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut buf, w, h, &[]).unwrap();
        encoder.set_repeat(gif::Repeat::Infinite).unwrap();
        for color_hex in colors {
            let r = u8::from_str_radix(&color_hex[0..2], 16).unwrap();
            let g = u8::from_str_radix(&color_hex[2..4], 16).unwrap();
            let b = u8::from_str_radix(&color_hex[4..6], 16).unwrap();
            let a = if color_hex.len() == 8 {
                u8::from_str_radix(&color_hex[6..8], 16).unwrap()
            } else {
                255
            };
            let mut pixels = vec![[r, g, b, a]; (w as usize) * (h as usize)]
                .into_iter()
                .flatten()
                .collect::<Vec<u8>>();
            let mut frame = gif::Frame::from_rgba(w, h, &mut pixels);
            frame.delay = delay;
            encoder.write_frame(&frame).unwrap();
        }
    } // encoder dropped here, writes trailer
    buf
}

#[test]
fn test_animated_gif_roundtrip() {
    // Encode a 3-frame animated GIF (red, green, blue), decode+re-encode through imageflow,
    // then decode the output and verify we get 3 frames back.
    let input_gif = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);

    // Decode + re-encode as GIF through imageflow
    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Encode { io_id: 1, preset: EncoderPreset::Gif },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input_gif).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let execute = Execute001 {
        graph_recording: default_graph_recording(DEBUG_GRAPH),
        security: None,
        framewise: Framewise::Steps(steps),
    };
    ctx.execute_1(execute).unwrap();
    let output_bytes = ctx.get_output_buffer_slice(1).unwrap().to_vec();

    // Verify the GIF trailer byte (0x3B) is present at the end
    assert_eq!(
        output_bytes.last(),
        Some(&0x3B),
        "Animated GIF (3 frames) is missing the trailing 0x3B marker. Last bytes: {:02X?}",
        &output_bytes[output_bytes.len().saturating_sub(4)..]
    );

    // Verify the output is a valid GIF with 3 frames
    let mut decoder = gif::DecodeOptions::new();
    decoder.set_color_output(gif::ColorOutput::RGBA);
    let mut reader = decoder.read_info(&output_bytes[..]).unwrap();
    let mut frame_count = 0;
    while reader.read_next_frame().unwrap().is_some() {
        frame_count += 1;
    }
    assert_eq!(frame_count, 3, "Expected 3 frames in the re-encoded animated GIF");
}

#[test]
fn test_animated_gif_two_frames() {
    // Minimal test: 2-frame animated GIF to verify multi-frame encoding works
    let input_gif = build_animated_gif(8, 8, &["FF0000", "0000FF"], 5);

    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Encode { io_id: 1, preset: EncoderPreset::Gif },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input_gif).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let execute = Execute001 {
        graph_recording: default_graph_recording(DEBUG_GRAPH),
        security: None,
        framewise: Framewise::Steps(steps),
    };
    ctx.execute_1(execute).unwrap();
    let output_bytes = ctx.get_output_buffer_slice(1).unwrap().to_vec();

    // Verify the GIF trailer byte (0x3B) is present at the end
    assert_eq!(
        output_bytes.last(),
        Some(&0x3B),
        "Animated GIF (2 frames) is missing the trailing 0x3B marker. Last bytes: {:02X?}",
        &output_bytes[output_bytes.len().saturating_sub(4)..]
    );

    // Verify 2 frames
    let mut decoder = gif::DecodeOptions::new();
    decoder.set_color_output(gif::ColorOutput::RGBA);
    let mut reader = decoder.read_info(&output_bytes[..]).unwrap();
    let mut frame_count = 0;
    while reader.read_next_frame().unwrap().is_some() {
        frame_count += 1;
    }
    assert_eq!(frame_count, 2, "Expected 2 frames in the re-encoded animated GIF");
}

#[test]
fn test_gif_select_frame() {
    // Create a 3-frame animated GIF (red, green, blue), then decode with SelectFrame(1)
    // to extract only the second frame (green). Encode as PNG to get a single static image.
    let input_gif = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);

    let steps = vec![
        Node::Decode { io_id: 0, commands: Some(vec![s::DecoderCommand::SelectFrame(1)]) },
        Node::Encode { io_id: 1, preset: EncoderPreset::Lodepng { maximum_deflate: None } },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input_gif).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let execute = Execute001 {
        graph_recording: default_graph_recording(DEBUG_GRAPH),
        security: None,
        framewise: Framewise::Steps(steps),
    };
    ctx.execute_1(execute).unwrap();

    let output_bytes = ctx.get_output_buffer_slice(1).unwrap().to_vec();

    // Verify it's a valid PNG (starts with PNG signature)
    assert_eq!(&output_bytes[1..4], b"PNG", "Output should be a PNG");

    // Decode the PNG and verify the pixel color is green (the second frame)
    let decoder = lodepng::decode32(&output_bytes).unwrap();
    assert_eq!(decoder.width, 4);
    assert_eq!(decoder.height, 4);
    // Check first pixel — should be green (from frame index 1)
    let pixel = &decoder.buffer[0];
    // GIF quantization may slightly alter values, but green channel should dominate
    assert!(
        pixel.g > 200 && pixel.r < 50 && pixel.b < 50,
        "Expected green pixel from frame 1, got r={} g={} b={} a={}",
        pixel.r,
        pixel.g,
        pixel.b,
        pixel.a
    );
}

#[test]
fn test_gif_select_frame_0() {
    // Verify frame=0 extracts just the first frame from an animated GIF
    let input_gif = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);

    let steps = vec![
        Node::Decode { io_id: 0, commands: Some(vec![s::DecoderCommand::SelectFrame(0)]) },
        Node::Encode { io_id: 1, preset: EncoderPreset::Lodepng { maximum_deflate: None } },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input_gif).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let execute = Execute001 {
        graph_recording: default_graph_recording(DEBUG_GRAPH),
        security: None,
        framewise: Framewise::Steps(steps),
    };
    ctx.execute_1(execute).unwrap();

    let output_bytes = ctx.get_output_buffer_slice(1).unwrap().to_vec();
    assert_eq!(&output_bytes[1..4], b"PNG", "Output should be a PNG");

    let decoder = lodepng::decode32(&output_bytes).unwrap();
    let pixel = &decoder.buffer[0];
    // First frame is red
    assert!(
        pixel.r > 200 && pixel.g < 50 && pixel.b < 50,
        "Expected red pixel from frame 0, got r={} g={} b={} a={}",
        pixel.r,
        pixel.g,
        pixel.b,
        pixel.a
    );
}

#[test]
fn test_gif_select_frame_via_querystring() {
    // Test that &frame=1 works through the CommandString (querystring) API
    let input_gif = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);

    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "frame=1&format=png".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input_gif).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let execute = Execute001 {
        graph_recording: default_graph_recording(DEBUG_GRAPH),
        security: None,
        framewise: Framewise::Steps(steps),
    };
    ctx.execute_1(execute).unwrap();

    let output_bytes = ctx.get_output_buffer_slice(1).unwrap().to_vec();
    assert_eq!(&output_bytes[1..4], b"PNG", "Output should be a PNG");

    let decoder = lodepng::decode32(&output_bytes).unwrap();
    let pixel = &decoder.buffer[0];
    // Second frame is green
    assert!(
        pixel.g > 200 && pixel.r < 50 && pixel.b < 50,
        "Expected green pixel from frame 1 via querystring, got r={} g={} b={} a={}",
        pixel.r,
        pixel.g,
        pixel.b,
        pixel.a
    );
}

// test a job that generates a canvas, encodes to a gif,  then another job decodes it.
#[test]
fn test_gif_roundtrip() {
    let steps = vec![
        Node::CreateCanvas {
            w: 8,
            h: 8,
            format: PixelFormat::Bgra32,
            color: Color::Srgb(ColorSrgb::Hex("FF0000FF".to_owned())),
        },
        Node::Encode { io_id: 0, preset: EncoderPreset::Gif },
    ];
    let mut ctx1 = Context::create().unwrap();
    ctx1.add_output_buffer(0).unwrap();
    let execute1 = Execute001 {
        graph_recording: default_graph_recording(DEBUG_GRAPH),
        security: None,
        framewise: Framewise::Steps(steps),
    };
    ctx1.execute_1(execute1).unwrap();
    let bytes = ctx1.get_output_buffer_slice(0).unwrap().to_vec();

    // Verify the GIF trailer byte (0x3B) is present at the end
    assert_eq!(
        bytes.last(),
        Some(&0x3B),
        "Still GIF is missing the trailing 0x3B marker. Last bytes: {:02X?}",
        &bytes[bytes.len().saturating_sub(4)..]
    );

    let mut ctx2 = Context::create().unwrap();
    ctx2.add_input_vector(0, bytes.to_vec()).unwrap();
    let execute2 = Execute001 {
        graph_recording: default_graph_recording(DEBUG_GRAPH),
        security: None,
        framewise: Framewise::Steps(vec![Node::Decode { io_id: 0, commands: None }]),
    };
    ctx2.execute_1(execute2).unwrap();
    // just a smoke test
}
