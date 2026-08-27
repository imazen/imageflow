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

/// Walk the RIFF chunks of a WebP file, returning (fourcc, payload) pairs.
fn webp_chunks(bytes: &[u8]) -> Vec<([u8; 4], &[u8])> {
    assert!(bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP", "not a WebP container");
    let riff_len = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    assert_eq!(
        riff_len + 8,
        bytes.len(),
        "RIFF length must cover the whole file (no trailing garbage)"
    );
    let mut chunks = Vec::new();
    let mut pos = 12;
    while pos + 8 <= bytes.len() {
        let fourcc: [u8; 4] = bytes[pos..pos + 4].try_into().unwrap();
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let payload = &bytes[pos + 8..pos + 8 + size];
        chunks.push((fourcc, payload));
        pos += 8 + size + (size & 1);
    }
    chunks
}

/// Durations (ms) of every ANMF frame in an animated WebP.
fn webp_frame_durations(bytes: &[u8]) -> Vec<u32> {
    webp_chunks(bytes)
        .iter()
        .filter(|(fourcc, _)| fourcc == b"ANMF")
        .map(|(_, p)| u32::from_le_bytes([p[12], p[13], p[14], 0]))
        .collect()
}

fn webp_has_chunk(bytes: &[u8], fourcc: &[u8; 4]) -> bool {
    webp_chunks(bytes).iter().any(|(f, _)| f == fourcc)
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
    assert!(webp_has_chunk(&output, b"ANIM"), "animated WebP needs an ANIM chunk");
    // 3 GIF frames at 10cs = 100ms each must become 3 ANMF frames of 100ms.
    assert_eq!(webp_frame_durations(&output), vec![100, 100, 100]);
}

#[test]
fn test_animated_gif_to_webp_lossless_preserves_animation() {
    test_init();
    let input = build_animated_gif(8, 8, &["FF0000", "00FF00", "0000FF", "FFFF00"], 5);
    let output = roundtrip_animated_gif(input, EncoderPreset::WebPLossless);
    assert!(output.starts_with(b"RIFF"), "Output should be WebP");
    assert!(webp_has_chunk(&output, b"ANIM"), "animated lossless WebP needs an ANIM chunk");
    assert_eq!(webp_frame_durations(&output), vec![50, 50, 50, 50]);
}

#[test]
fn test_single_frame_gif_to_webp_is_a_still_image() {
    test_init();
    let input = build_animated_gif(8, 8, &["FF0000"], 10);
    let output = roundtrip_animated_gif(input, EncoderPreset::WebPLossy { quality: 80.0 });
    assert!(output.starts_with(b"RIFF"), "Output should be WebP");
    assert!(!webp_has_chunk(&output, b"ANIM"), "single-frame input must stay a still WebP");
    assert!(webp_frame_durations(&output).is_empty());
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

/// Build an animated GIF whose frames each paint an opaque `color` on half of
/// the canvas (left half on even frames, right half on odd frames) and leave
/// the other half transparent, with `Background` disposal so the transparent
/// half really is transparent when a browser composites the animation.
fn build_half_transparent_gif(size: u16, colors: &[(u8, u8, u8)], delay: u16) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut buf, size, size, &[]).unwrap();
        encoder.set_repeat(gif::Repeat::Infinite).unwrap();
        for (i, &(r, g, b)) in colors.iter().enumerate() {
            let mut pixels = Vec::with_capacity(size as usize * size as usize * 4);
            for _y in 0..size {
                for x in 0..size {
                    let left = x < size / 2;
                    let painted = if i % 2 == 0 { left } else { !left };
                    if painted {
                        pixels.extend_from_slice(&[r, g, b, 255]);
                    } else {
                        pixels.extend_from_slice(&[0, 0, 0, 0]);
                    }
                }
            }
            let mut frame = gif::Frame::from_rgba(size, size, &mut pixels);
            frame.delay = delay;
            frame.dispose = gif::DisposalMethod::Background;
            encoder.write_frame(&frame).unwrap();
        }
    }
    buf
}

/// Composite an animated GIF the way a browser does (transparent index = do not
/// paint; `Background` disposal clears the frame rect before the next frame) and
/// return the full canvas after each frame as RGBA.
fn composite_gif_frames(bytes: &[u8]) -> Vec<(u16, u16, Vec<[u8; 4]>)> {
    let mut opts = gif::DecodeOptions::new();
    opts.set_color_output(gif::ColorOutput::RGBA);
    let mut reader = opts.read_info(bytes).unwrap();
    let (w, h) = (reader.width(), reader.height());
    let mut canvas = vec![[0u8; 4]; w as usize * h as usize];
    let mut out = Vec::new();
    let mut prev: Option<(gif::DisposalMethod, u16, u16, u16, u16)> = None;
    while let Some(frame) = reader.read_next_frame().unwrap() {
        if let Some((gif::DisposalMethod::Background, l, t, fw, fh)) = prev {
            for y in t..t + fh {
                for x in l..l + fw {
                    canvas[y as usize * w as usize + x as usize] = [0, 0, 0, 0];
                }
            }
        }
        for fy in 0..frame.height {
            for fx in 0..frame.width {
                let i = (fy as usize * frame.width as usize + fx as usize) * 4;
                let px = [
                    frame.buffer[i],
                    frame.buffer[i + 1],
                    frame.buffer[i + 2],
                    frame.buffer[i + 3],
                ];
                if px[3] != 0 {
                    let (x, y) = (frame.left + fx, frame.top + fy);
                    canvas[y as usize * w as usize + x as usize] = px;
                }
            }
        }
        out.push((w, h, canvas.clone()));
        prev = Some((frame.dispose, frame.left, frame.top, frame.width, frame.height));
    }
    out
}

/// Issue #653: a transparent animated GIF must stay transparent after a
/// GIF → GIF roundtrip. Frame 1 paints the left half red, frame 2 the right
/// half blue; after frame 2 the left half must be transparent again, not a
/// red ghost left over from frame 1.
#[test]
fn test_animated_gif_transparency_survives_roundtrip_pixel_exact() {
    test_init();
    let input = build_half_transparent_gif(8, &[(255, 0, 0), (0, 0, 255)], 10);
    // Sanity-check the fixture itself composites as intended.
    let frames = composite_gif_frames(&input);
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[1].2[0][3], 0, "fixture: left half must be transparent after frame 2");

    let output = roundtrip_animated_gif(input, EncoderPreset::Gif);
    let frames = composite_gif_frames(&output);
    assert_eq!(frames.len(), 2, "expected 2 frames");
    let (w, _h, ref f1) = frames[0];
    let (_w, _h, ref f2) = frames[1];
    let w = w as usize;
    // Frame 1: left opaque red, right transparent.
    assert_eq!(f1[0][3], 255, "frame 1 left must be opaque");
    assert!(
        f1[0][0] > 200 && f1[0][1] < 60 && f1[0][2] < 60,
        "frame 1 left should be red: {:?}",
        f1[0]
    );
    assert_eq!(f1[w - 1][3], 0, "frame 1 right must be transparent: {:?}", f1[w - 1]);
    // Frame 2: left transparent (no ghost of frame 1), right opaque blue.
    assert_eq!(f2[0][3], 0, "frame 2 left must be transparent (no ghost of frame 1): {:?}", f2[0]);
    assert_eq!(f2[w - 1][3], 255, "frame 2 right must be opaque");
    assert!(
        f2[w - 1][2] > 200 && f2[w - 1][0] < 60,
        "frame 2 right should be blue: {:?}",
        f2[w - 1]
    );
    // Every row agrees, not just row 0.
    for y in 0..8 {
        assert_eq!(f2[y * w][3], 0, "row {y}: ghost pixel in frame 2");
        assert_eq!(f1[y * w + w - 1][3], 0, "row {y}: frame 1 right not transparent");
    }
}

/// Same as above through a resize, which is what imageflow-server does.
#[test]
fn test_animated_gif_transparency_survives_resize_pixel_exact() {
    test_init();
    let input = build_half_transparent_gif(16, &[(255, 0, 0), (0, 0, 255)], 10);
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
    let frames = composite_gif_frames(&output);
    assert_eq!(frames.len(), 2);
    let w = frames[0].0 as usize;
    assert_eq!(w, 8);
    // Stay away from the middle column where resampling blends the halves.
    for y in 0..8 {
        assert_eq!(frames[0].2[y * w + w - 1][3], 0, "row {y}: frame 1 right edge not transparent");
        assert_eq!(frames[1].2[y * w][3], 0, "row {y}: frame 2 left edge ghosted/opaque");
        assert_eq!(frames[0].2[y * w][3], 255, "row {y}: frame 1 left edge not opaque");
        assert_eq!(frames[1].2[y * w + w - 1][3], 255, "row {y}: frame 2 right edge not opaque");
    }
}

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
