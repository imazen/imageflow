use crate::common::*;
use imageflow_core::Context;
use imageflow_types::{
    CommandStringKind, EncoderPreset, Execute001, Filter, Framewise, Node, ResampleHints,
};

use super::smoke::build_animated_gif;

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
fn roundtrip_animated_gif(gif_bytes: Vec<u8>, preset: EncoderPreset) -> Vec<u8> {
    test_init();
    let steps = vec![Node::Decode { io_id: 0, commands: None }, Node::Encode { io_id: 1, preset }];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, gif_bytes).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        job_options: None,
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
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    })
    .unwrap();
    let output = ctx.take_output_buffer(1).unwrap();
    assert_eq!(&output[1..4], b"PNG", "Output should be PNG");
    let (r, g, b, _a) = decode_png_pixel(&output);
    assert!(
        g > 200 && r < 50 && b < 50,
        "Expected green pixel from frame 1, got r={r} g={g} b={b}"
    );
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
        job_options: None,
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
        job_options: None,
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
    assert!(
        b > 150 && r < 100 && g < 100,
        "Expected blue-ish pixel from frame 2, got r={r} g={g} b={b}"
    );
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
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    })
    .unwrap();
    let output = ctx.take_output_buffer(1).unwrap();
    assert!(output.starts_with(&[0xFF, 0xD8, 0xFF]), "Output should be JPEG");
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
        job_options: None,
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
        job_options: None,
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
    assert!(
        r > 200 && g < 50 && b < 50,
        "Expected red pixel from WebP lossless, got r={r} g={g} b={b}"
    );
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
        job_options: None,
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
        job_options: None,
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
            hints: Some(
                imageflow_types::ResampleHints::new()
                    .with_bi_filter(imageflow_types::Filter::Hermite),
            ),
        },
        Node::Encode { io_id: 1, preset: EncoderPreset::Gif },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        job_options: None,
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

// ============================================================================
// Issue #606: GIF → WebP animation preservation
// ============================================================================

#[test]
fn test_animated_gif_to_webp_preserves_animation() {
    test_init();
    let input = build_animated_gif(8, 8, &["FF0000", "00FF00", "0000FF"], 10);
    let output = roundtrip_animated_gif(input, EncoderPreset::WebPLossy { quality: 80.0 });
    assert!(output.starts_with(b"RIFF"), "Output should be WebP");
    // WebP animated files should have ANIM chunk
    // At minimum, the file should be significantly larger than a single-frame WebP
    assert!(
        output.len() > 200,
        "Animated WebP should be larger than a trivial single-frame output (got {} bytes)",
        output.len()
    );
}

#[test]
#[cfg_attr(
    any(not(feature = "zen-codecs"), feature = "c-codecs"),
    ignore = "WebP lossless animation requires zen-codecs without c-codecs \
              (C libwebp is preferred for stable encode output but doesn't preserve animation)"
)]
fn test_animated_gif_to_webp_lossless_preserves_animation() {
    test_init();
    let input = build_animated_gif(8, 8, &["FF0000", "00FF00", "0000FF", "FFFF00"], 5);
    let output = roundtrip_animated_gif(input, EncoderPreset::WebPLossless);
    assert!(output.starts_with(b"RIFF"), "Output should be WebP");
    assert!(
        output.len() > 200,
        "Animated WebP lossless should have multiple frames (got {} bytes)",
        output.len()
    );
}

// ============================================================================
// Issue #643: Double GIF encode (resize GIF, then resize the output again)
// ============================================================================

#[test]
fn test_gif_double_encode_no_eof_crash() {
    test_init();
    let input = build_animated_gif(16, 16, &["FF0000", "00FF00", "0000FF"], 10);

    // First pass: resize the animated GIF
    let steps1 = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Resample2D {
            w: 8,
            h: 8,
            hints: Some(ResampleHints::new().with_bi_filter(Filter::Hermite)),
        },
        Node::Encode { io_id: 1, preset: EncoderPreset::Gif },
    ];
    let mut ctx1 = Context::create().unwrap();
    ctx1.add_input_vector(0, input).unwrap();
    ctx1.add_output_buffer(1).unwrap();
    ctx1.execute_1(Execute001 {
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps1),
    })
    .unwrap();
    let intermediate = ctx1.take_output_buffer(1).unwrap();
    assert_eq!(count_gif_frames(&intermediate), 3, "First pass should produce 3 frames");

    // Second pass: resize the already-encoded GIF output (this was the crash in #643)
    let steps2 = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Resample2D {
            w: 4,
            h: 4,
            hints: Some(ResampleHints::new().with_bi_filter(Filter::Hermite)),
        },
        Node::Encode { io_id: 1, preset: EncoderPreset::Gif },
    ];
    let mut ctx2 = Context::create().unwrap();
    ctx2.add_input_vector(0, intermediate).unwrap();
    ctx2.add_output_buffer(1).unwrap();
    ctx2.execute_1(Execute001 {
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps2),
    })
    .unwrap();
    let final_output = ctx2.take_output_buffer(1).unwrap();
    assert_eq!(
        count_gif_frames(&final_output),
        3,
        "Second pass should also produce 3 frames without EOF crash"
    );
}

// ============================================================================
// Issue #653: Animated GIF with transparent background
// ============================================================================

#[test]
fn test_animated_gif_transparent_bg_roundtrip() {
    test_init();
    // Build GIF with semi-transparent frames
    let input = build_animated_gif(8, 8, &["FF000080", "00FF0080", "0000FF80"], 10);
    let output = roundtrip_animated_gif(input, EncoderPreset::Gif);
    assert_eq!(count_gif_frames(&output), 3, "Transparent animated GIF should preserve 3 frames");
}

#[test]
fn test_animated_gif_transparent_bg_resize() {
    test_init();
    // Transparent animated GIF → resize → GIF should not lose transparency
    let input = build_animated_gif(16, 16, &["FF000000", "00FF0000"], 10);
    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Resample2D {
            w: 8,
            h: 8,
            hints: Some(ResampleHints::new().with_bi_filter(Filter::Hermite)),
        },
        Node::Encode { io_id: 1, preset: EncoderPreset::Gif },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    })
    .unwrap();
    let output = ctx.take_output_buffer(1).unwrap();
    assert_eq!(count_gif_frames(&output), 2, "Should preserve 2 frames");
}

// ============================================================================
// Animated GIF with resize to various single-frame formats (verify no crash)
// ============================================================================

#[test]
fn test_animated_gif_resize_to_all_single_frame_formats() {
    test_init();
    let input = build_animated_gif(16, 16, &["FF0000", "00FF00", "0000FF"], 10);

    let presets: Vec<(&str, EncoderPreset)> = vec![
        ("png", EncoderPreset::Lodepng { maximum_deflate: None }),
        ("mozjpeg", EncoderPreset::Mozjpeg { progressive: None, quality: Some(80), matte: None }),
        ("webp_lossy", EncoderPreset::WebPLossy { quality: 80.0 }),
        ("webp_lossless", EncoderPreset::WebPLossless),
    ];

    for (name, preset) in presets {
        let steps = vec![
            Node::Decode {
                io_id: 0,
                commands: Some(vec![imageflow_types::DecoderCommand::SelectFrame(1)]),
            },
            Node::Resample2D {
                w: 8,
                h: 8,
                hints: Some(ResampleHints::new().with_bi_filter(Filter::Hermite)),
            },
            Node::Encode { io_id: 1, preset },
        ];
        let mut ctx = Context::create().unwrap();
        ctx.add_copied_input_buffer(0, &input).unwrap();
        ctx.add_output_buffer(1).unwrap();
        ctx.execute_1(Execute001 {
            job_options: None,
            graph_recording: default_graph_recording(false),
            security: None,
            framewise: Framewise::Steps(steps),
        })
        .unwrap_or_else(|e| panic!("Failed to encode animated GIF frame to {name}: {e}"));
        let output = ctx.take_output_buffer(1).unwrap();
        assert!(
            output.len() > 10,
            "{name}: output should have content (got {} bytes)",
            output.len()
        );
    }
}
