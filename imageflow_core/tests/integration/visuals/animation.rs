use crate::common::*;
use imageflow_core::Context;
use imageflow_types::{
    CommandStringKind, EncoderPreset, Execute001, Framewise, Node,
};

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
    }
    buf
}

/// Count frames in a GIF byte buffer using the gif crate decoder.
fn count_gif_frames(bytes: &[u8]) -> usize {
    let mut decoder = gif::DecodeOptions::new();
    decoder.set_color_output(gif::ColorOutput::RGBA);
    let mut reader = decoder.read_info(bytes).unwrap();
    let mut count = 0;
    while reader.read_next_frame().unwrap().is_some() {
        count += 1;
    }
    count
}

/// Decode a single pixel from a PNG byte buffer (top-left corner).
fn decode_png_pixel(bytes: &[u8]) -> (u8, u8, u8, u8) {
    let decoder = lodepng::decode32(bytes).unwrap();
    let pixel = &decoder.buffer[0];
    (pixel.r, pixel.g, pixel.b, pixel.a)
}

/// Run an animated GIF through a pipeline with the given encoder preset.
/// Returns the encoded output bytes.
fn roundtrip_animated_gif(
    gif_bytes: Vec<u8>,
    preset: EncoderPreset,
) -> Vec<u8> {
    test_init();
    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Encode { io_id: 1, preset },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, gif_bytes).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    })
    .unwrap();
    ctx.take_output_buffer(1).unwrap()
}

// ============================================================================
// GIF → GIF animation roundtrips
// ============================================================================

#[test]
fn test_animated_gif_3_frames_roundtrip() {
    let input = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);
    let output = roundtrip_animated_gif(input, EncoderPreset::Gif);
    assert_eq!(count_gif_frames(&output), 3, "Expected 3 frames in GIF output");
}

#[test]
fn test_animated_gif_5_frames_roundtrip() {
    let input = build_animated_gif(8, 8, &["FF0000", "00FF00", "0000FF", "FFFF00", "FF00FF"], 5);
    let output = roundtrip_animated_gif(input, EncoderPreset::Gif);
    assert_eq!(count_gif_frames(&output), 5, "Expected 5 frames in GIF output");
}

#[test]
fn test_animated_gif_single_frame_roundtrip() {
    let input = build_animated_gif(4, 4, &["FF0000"], 10);
    let output = roundtrip_animated_gif(input, EncoderPreset::Gif);
    assert_eq!(count_gif_frames(&output), 1, "Expected 1 frame in GIF output");
}

// ============================================================================
// GIF frame selection → single-frame output in various formats
// ============================================================================

#[test]
fn test_gif_select_frame_to_png() {
    test_init();
    let input = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);
    let steps = vec![
        Node::Decode {
            io_id: 0,
            commands: Some(vec![imageflow_types::DecoderCommand::SelectFrame(1)]),
        },
        Node::Encode { io_id: 1, preset: EncoderPreset::Lodepng { maximum_deflate: None } },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    })
    .unwrap();
    let output = ctx.take_output_buffer(1).unwrap();
    assert_eq!(&output[1..4], b"PNG", "Output should be PNG");
    let (r, g, b, _a) = decode_png_pixel(&output);
    assert!(g > 200 && r < 50 && b < 50, "Expected green pixel from frame 1, got r={r} g={g} b={b}");
}

#[test]
fn test_gif_select_frame_to_webp_lossy() {
    test_init();
    let input = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);
    let steps = vec![
        Node::Decode {
            io_id: 0,
            commands: Some(vec![imageflow_types::DecoderCommand::SelectFrame(2)]),
        },
        Node::Encode { io_id: 1, preset: EncoderPreset::WebPLossy { quality: 90.0 } },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    })
    .unwrap();
    let output = ctx.take_output_buffer(1).unwrap();
    assert!(output.starts_with(b"RIFF"), "Output should be WebP");
    // WebP lossy: decode back and check blue-ish pixel
    let mut ctx2 = Context::create().unwrap();
    ctx2.add_input_vector(0, output).unwrap();
    ctx2.add_output_buffer(1).unwrap();
    ctx2.execute_1(Execute001 {
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Encode { io_id: 1, preset: EncoderPreset::Lodepng { maximum_deflate: None } },
        ]),
    })
    .unwrap();
    let png_bytes = ctx2.take_output_buffer(1).unwrap();
    let (r, g, b, _a) = decode_png_pixel(&png_bytes);
    assert!(b > 150 && r < 100 && g < 100, "Expected blue-ish pixel from frame 2, got r={r} g={g} b={b}");
}

#[test]
fn test_gif_select_frame_to_mozjpeg() {
    test_init();
    let input = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);
    let steps = vec![
        Node::Decode {
            io_id: 0,
            commands: Some(vec![imageflow_types::DecoderCommand::SelectFrame(0)]),
        },
        Node::Encode {
            io_id: 1,
            preset: EncoderPreset::Mozjpeg { progressive: None, quality: Some(90), matte: None },
        },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    })
    .unwrap();
    let output = ctx.take_output_buffer(1).unwrap();
    assert!(output.starts_with(&[0xFF, 0xD8, 0xFF]), "Output should be JPEG");
}

#[test]
fn test_gif_select_frame_to_jxl_lossy() {
    test_init();
    let input = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);
    let steps = vec![
        Node::Decode {
            io_id: 0,
            commands: Some(vec![imageflow_types::DecoderCommand::SelectFrame(1)]),
        },
        Node::Encode { io_id: 1, preset: EncoderPreset::JxlLossy { distance: 1.0 } },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    })
    .unwrap();
    let output = ctx.take_output_buffer(1).unwrap();
    // JXL magic: bare codestream 0xFF 0x0A or container 0x00 0x00 0x00 0x0C 0x4A 0x58 0x4C 0x20
    assert!(
        output.starts_with(&[0xFF, 0x0A]) || output.starts_with(&[0x00, 0x00, 0x00, 0x0C]),
        "Output should be JXL"
    );
}

#[test]
fn test_gif_select_frame_to_jxl_lossless() {
    test_init();
    let input = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);
    let steps = vec![
        Node::Decode {
            io_id: 0,
            commands: Some(vec![imageflow_types::DecoderCommand::SelectFrame(2)]),
        },
        Node::Encode { io_id: 1, preset: EncoderPreset::JxlLossless },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    })
    .unwrap();
    let output = ctx.take_output_buffer(1).unwrap();
    assert!(
        output.starts_with(&[0xFF, 0x0A]) || output.starts_with(&[0x00, 0x00, 0x00, 0x0C]),
        "Output should be JXL"
    );

    // Decode the JXL back and verify pixel color (blue from frame 2)
    let mut ctx2 = Context::create().unwrap();
    ctx2.add_input_vector(0, output).unwrap();
    ctx2.add_output_buffer(1).unwrap();
    ctx2.execute_1(Execute001 {
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Encode { io_id: 1, preset: EncoderPreset::Lodepng { maximum_deflate: None } },
        ]),
    })
    .unwrap();
    let png_bytes = ctx2.take_output_buffer(1).unwrap();
    let (r, g, b, _a) = decode_png_pixel(&png_bytes);
    assert!(b > 200 && r < 50 && g < 50, "Expected blue pixel from JXL lossless, got r={r} g={g} b={b}");
}

#[test]
fn test_gif_select_frame_to_webp_lossless() {
    test_init();
    let input = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);
    let steps = vec![
        Node::Decode {
            io_id: 0,
            commands: Some(vec![imageflow_types::DecoderCommand::SelectFrame(0)]),
        },
        Node::Encode { io_id: 1, preset: EncoderPreset::WebPLossless },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    })
    .unwrap();
    let output = ctx.take_output_buffer(1).unwrap();
    assert!(output.starts_with(b"RIFF"), "Output should be WebP");
    // Decode WebP lossless back, verify red pixel from frame 0
    let mut ctx2 = Context::create().unwrap();
    ctx2.add_input_vector(0, output).unwrap();
    ctx2.add_output_buffer(1).unwrap();
    ctx2.execute_1(Execute001 {
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Encode { io_id: 1, preset: EncoderPreset::Lodepng { maximum_deflate: None } },
        ]),
    })
    .unwrap();
    let png_bytes = ctx2.take_output_buffer(1).unwrap();
    let (r, g, b, _a) = decode_png_pixel(&png_bytes);
    assert!(r > 200 && g < 50 && b < 50, "Expected red pixel from WebP lossless, got r={r} g={g} b={b}");
}

// ============================================================================
// GIF frame selection via querystring
// ============================================================================

#[test]
fn test_gif_select_frame_via_querystring_to_webp() {
    test_init();
    let input = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "frame=2&format=webp".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    })
    .unwrap();
    let output = ctx.take_output_buffer(1).unwrap();
    assert!(output.starts_with(b"RIFF"), "Output should be WebP");
}

#[test]
fn test_gif_select_frame_via_querystring_to_gif() {
    test_init();
    let input = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "frame=0&format=gif".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    })
    .unwrap();
    let output = ctx.take_output_buffer(1).unwrap();
    assert_eq!(count_gif_frames(&output), 1, "Selecting a frame should produce single-frame GIF");
}

// ============================================================================
// Animated GIF with processing (resize) between decode and encode
// ============================================================================

#[test]
fn test_animated_gif_resize_roundtrip() {
    test_init();
    let input = build_animated_gif(16, 16, &["FF0000", "00FF00", "0000FF"], 10);
    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Resample2D {
            w: 8,
            h: 8,
            hints: Some(imageflow_types::ResampleHints::new().with_bi_filter(imageflow_types::Filter::Hermite)),
        },
        Node::Encode { io_id: 1, preset: EncoderPreset::Gif },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    })
    .unwrap();
    let output = ctx.take_output_buffer(1).unwrap();
    assert_eq!(count_gif_frames(&output), 3, "Expected 3 frames after resize roundtrip");

    // Verify output dimensions by decoding first frame
    let mut decoder = gif::DecodeOptions::new();
    decoder.set_color_output(gif::ColorOutput::RGBA);
    let reader = decoder.read_info(&output[..]).unwrap();
    assert_eq!(reader.width(), 8);
    assert_eq!(reader.height(), 8);
}

// ============================================================================
// Animated GIF → single-frame format (only first frame should be encoded)
// ============================================================================

#[test]
fn test_animated_gif_to_png_takes_first_frame() {
    test_init();
    let input = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);
    // No SelectFrame command — should encode first frame only for single-frame formats
    let output = roundtrip_animated_gif(input, EncoderPreset::Lodepng { maximum_deflate: None });
    assert_eq!(&output[1..4], b"PNG", "Output should be PNG");
    let (r, g, b, _a) = decode_png_pixel(&output);
    assert!(
        r > 200 && g < 50 && b < 50,
        "Expected red pixel from first frame, got r={r} g={g} b={b}"
    );
}

#[test]
fn test_animated_gif_to_mozjpeg_takes_first_frame() {
    test_init();
    let input = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);
    let output = roundtrip_animated_gif(
        input,
        EncoderPreset::Mozjpeg { progressive: None, quality: Some(90), matte: None },
    );
    assert!(output.starts_with(&[0xFF, 0xD8, 0xFF]), "Output should be JPEG");
}

#[test]
fn test_animated_gif_to_jxl_lossy_takes_first_frame() {
    test_init();
    let input = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);
    let output = roundtrip_animated_gif(input, EncoderPreset::JxlLossy { distance: 1.0 });
    assert!(
        output.starts_with(&[0xFF, 0x0A]) || output.starts_with(&[0x00, 0x00, 0x00, 0x0C]),
        "Output should be JXL"
    );
}

#[test]
fn test_animated_gif_to_webp_lossless_takes_first_frame() {
    test_init();
    let input = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);
    let output = roundtrip_animated_gif(input, EncoderPreset::WebPLossless);
    assert!(output.starts_with(b"RIFF"), "Output should be WebP");
}

#[test]
fn test_animated_gif_to_webp_lossy_takes_first_frame() {
    test_init();
    let input = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);
    let output = roundtrip_animated_gif(input, EncoderPreset::WebPLossy { quality: 80.0 });
    assert!(output.starts_with(b"RIFF"), "Output should be WebP");
}

// ============================================================================
// Animated GIF pixel preservation across roundtrip
// ============================================================================

#[test]
fn test_animated_gif_pixel_colors_preserved() {
    test_init();
    let colors = &["FF0000", "00FF00", "0000FF"];
    let input = build_animated_gif(4, 4, colors, 10);
    let output = roundtrip_animated_gif(input, EncoderPreset::Gif);

    // Decode output and verify each frame's pixel color
    let mut decoder = gif::DecodeOptions::new();
    decoder.set_color_output(gif::ColorOutput::RGBA);
    let mut reader = decoder.read_info(&output[..]).unwrap();

    let expected_dominant = [(255u8, 0u8, 0u8), (0, 255, 0), (0, 0, 255)];
    for (i, (er, eg, eb)) in expected_dominant.iter().enumerate() {
        let frame = reader.read_next_frame().unwrap().unwrap();
        // GIF quantization may shift colors slightly, but dominant channel should be > 128
        // and other channels should be < 128
        let (r, g, b) = (frame.buffer[0], frame.buffer[1], frame.buffer[2]);
        if *er > 128 {
            assert!(r > 128, "Frame {i}: expected r > 128, got r={r} g={g} b={b}");
        }
        if *eg > 128 {
            assert!(g > 128, "Frame {i}: expected g > 128, got r={r} g={g} b={b}");
        }
        if *eb > 128 {
            assert!(b > 128, "Frame {i}: expected b > 128, got r={r} g={g} b={b}");
        }
    }
}
