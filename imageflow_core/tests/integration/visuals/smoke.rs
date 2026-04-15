use crate::common::*;
use imageflow_core::graphics::bitmaps::{BitmapCompositing, ColorSpace};
use imageflow_core::{Context, ErrorKind};
use imageflow_types::{
    Color, ColorSrgb, CommandStringKind, EncoderPreset, Execute001, Filter, Framewise, Node,
    PixelFormat, PixelLayout, PngBitDepth, ResampleHints,
};

const DEBUG_GRAPH: bool = false;
#[test]
fn webp_lossless_alpha_decode_and_encode() {
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "format=webp".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];

    smoke_test(
        Some(IoTestEnum::Url(
            "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/1_webp_ll.webp"
                .to_owned(),
        )),
        Some(IoTestEnum::OutputBuffer),
        None,
        DEBUG_GRAPH,
        steps,
    )
    .unwrap();
}

#[test]
fn webp_lossy_alpha_decode_and_encode() {
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "format=webp&quality=90".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];

    smoke_test(
        Some(IoTestEnum::Url(
            "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/1_webp_a.webp"
                .to_owned(),
        )),
        Some(IoTestEnum::OutputBuffer),
        None,
        DEBUG_GRAPH,
        steps,
    )
    .unwrap();
}

#[test]
fn smoke_test_gif_ir4() {
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "width=200&height=200&format=gif".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];

    smoke_test(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg"
                .to_owned(),
        )),
        Some(IoTestEnum::OutputBuffer),
        None,
        DEBUG_GRAPH,
        steps,
    )
    .unwrap();
}

#[test]
fn smoke_test_ignore_invalid_color_profile() {
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "width=200&height=200&ignore_icc_errors=true".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];

    smoke_test(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/color_profile_error.jpg".to_owned())),
               Some(IoTestEnum::OutputBuffer),
               None,
               DEBUG_GRAPH,
               steps,
    ).unwrap();
}

#[test]
fn smoke_test_invalid_params() {
    let tinypng = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "quality=957".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];

    smoke_test(
        Some(IoTestEnum::ByteArray(tinypng)),
        Some(IoTestEnum::OutputBuffer),
        None,
        DEBUG_GRAPH,
        steps,
    )
    .unwrap();
}

#[test]
fn smoke_test_png_ir4() {
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "width=200&height=200&format=png".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];

    smoke_test(Some(IoTestEnum::Url("https://user-images.githubusercontent.com/2650124/31182064-e1c54784-a8f0-11e7-8bb3-833bba872975.png".to_owned())),
               Some(IoTestEnum::OutputBuffer),
               None,
               DEBUG_GRAPH,
               steps,
    ).unwrap();
}

#[test]
#[cfg_attr(
    not(feature = "c-codecs"),
    ignore = "zenjpeg is more tolerant of truncated input than mozjpeg — this \
              particular corrupt.jpg decodes successfully on zen-only, so \
              the test's decoder-must-reject assertion only holds with c-codecs. \
              The no-crash guarantee still holds on zen-only via other smoke tests."
)]
fn smoke_test_corrupt_jpeg() {
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "format=jpg".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];

    smoke_test(
        Some(IoTestEnum::Url(
            "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/corrupt.jpg"
                .to_owned(),
        )),
        Some(IoTestEnum::OutputBuffer),
        None,
        DEBUG_GRAPH,
        steps,
    )
    .expect_err("Should fail without crashing process");
}

#[test]
fn test_encode_jpeg_smoke() {
    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Resample2D {
            w: 400,
            h: 300,
            hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
        },
        Node::Encode {
            io_id: 1,
            preset: EncoderPreset::LibjpegTurbo {
                quality: Some(100),
                progressive: None,
                optimize_huffman_coding: None,
                matte: None,
            },
        },
    ];

    smoke_test(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_v4_sYCC_8bit.jpg".to_owned())),
               Some(IoTestEnum::OutputBuffer),
               None,
               DEBUG_GRAPH,
               steps,
    ).unwrap();
}

#[test]
fn test_encode_gif_smoke() {
    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Resample2D {
            w: 400,
            h: 300,
            hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
        },
        Node::Encode { io_id: 1, preset: EncoderPreset::Gif },
    ];

    smoke_test(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_v4_sYCC_8bit.jpg".to_owned())),
               Some(IoTestEnum::OutputBuffer),
               None,
               DEBUG_GRAPH,
               steps,
    ).unwrap();
}

#[test]
fn test_encode_png32_smoke() {
    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Resample2D {
            w: 400,
            h: 300,
            hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
        },
        Node::FlipV,
        Node::Crop { x1: 20, y1: 20, x2: 380, y2: 280 },
        Node::Encode {
            io_id: 1,
            preset: EncoderPreset::Libpng {
                depth: Some(PngBitDepth::Png32),
                matte: None,
                zlib_compression: None,
            },
        },
    ];

    smoke_test(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_v4_sYCC_8bit.jpg".to_owned())),
               Some(IoTestEnum::OutputBuffer),
               None,
               DEBUG_GRAPH,
               steps,
    ).unwrap();
}

#[test]
fn test_max_encode_dimensions() {
    let tinypng = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "width=2&height=2&mode=pad&scale=both".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];

    let e = smoke_test(
        Some(IoTestEnum::ByteArray(tinypng)),
        Some(IoTestEnum::OutputBuffer),
        {
            let mut sec = imageflow_types::ExecutionSecurity::unspecified();
            sec.max_encode_size =
                Some(imageflow_types::FrameSizeLimit { w: 3, h: 1, megapixels: 100.0 });
            Some(sec)
        },
        DEBUG_GRAPH,
        steps,
    )
    .expect_err("Should fail");

    assert_eq!(e.kind, ErrorKind::SizeLimitExceeded);

    assert_eq!(e.message, "SizeLimitExceeded: Frame height 2 exceeds max_encode_size.h 1");
}

#[test]
fn test_max_decode_dimensions() {
    let steps = vec![Node::Decode { io_id: 0, commands: None }];

    let e = smoke_test(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg"
                .to_owned(),
        )),
        None,
        {
            let mut sec = imageflow_types::ExecutionSecurity::unspecified();
            sec.max_decode_size =
                Some(imageflow_types::FrameSizeLimit { w: 10, h: 100000, megapixels: 100.0 });
            Some(sec)
        },
        DEBUG_GRAPH,
        steps,
    )
    .expect_err("Should fail");
    assert_eq!(e.kind, ErrorKind::SizeLimitExceeded);
}

#[test]
fn test_max_frame_dimensions() {
    let steps = vec![Node::CreateCanvas {
        format: PixelFormat::Bgra32,
        w: 1000,
        h: 1000,
        color: Color::Transparent,
    }];

    let e = smoke_test(
        None,
        None,
        {
            let mut sec = imageflow_types::ExecutionSecurity::unspecified();
            sec.max_frame_size =
                Some(imageflow_types::FrameSizeLimit { w: 10000, h: 10000, megapixels: 0.5 });
            Some(sec)
        },
        DEBUG_GRAPH,
        steps,
    )
    .expect_err("Should fail");

    assert_eq!(e.kind, ErrorKind::SizeLimitExceeded);
}

#[test]
fn test_dimensions() {
    let steps = vec![
        Node::CreateCanvas { w: 638, h: 423, format: PixelFormat::Bgra32, color: Color::Black },
        Node::Resample2D { w: 200, h: 133, hints: None },
        Node::ExpandCanvas { left: 1, top: 0, right: 0, bottom: 0, color: Color::Transparent },
    ];
    let (w, h) = get_result_dimensions(&steps, vec![], DEBUG_GRAPH);
    assert_eq!(w, 201);
    assert_eq!(h, 133);
}

#[test]
fn test_aspect_crop_dimensions() {
    let steps = vec![
        Node::CreateCanvas { w: 638, h: 423, format: PixelFormat::Bgra32, color: Color::Black },
        Node::Constrain(imageflow_types::Constraint {
            mode: imageflow_types::ConstraintMode::AspectCrop,
            w: Some(200),
            h: Some(133),
            hints: None,
            gravity: None,
            canvas_color: None,
        }),
    ];
    let (w, h) = get_result_dimensions(&steps, vec![], DEBUG_GRAPH);
    assert_eq!(w, 636);
    assert_eq!(h, 423);
}

#[test]
fn test_decode_png_and_scale_dimensions() {
    let tinypng = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Resample2D { w: 300, h: 200, hints: None },
    ];
    let (w, h) = get_result_dimensions(&steps, vec![IoTestEnum::ByteArray(tinypng)], false);
    assert_eq!(w, 300);
    assert_eq!(h, 200);
}

#[test]
fn test_zoom_with_preshrink() {
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "zoom=0.25".to_owned(),
        decode: Some(0),
        encode: None,
        watermarks: None,
    }];
    let (w, _h) = get_result_dimensions(
        &steps,
        vec![IoTestEnum::Url(
            "https://imageflow-resources.s3.us-west-2.amazonaws.com/test_inputs/5760_x_4320.jpg"
                .to_owned(),
        )],
        false,
    );
    assert_eq!(w, 1440);
}

#[test]
fn test_get_info_png() {
    let tinypng = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    let _ = imageflow_core::clients::stateless::LibClient {}
        .get_image_info(&tinypng)
        .expect("Image response should be valid");
}

#[test]
fn test_detect_whitespace_basic() {
    let ctx = Context::create_can_panic().unwrap();

    let red = Color::Srgb(ColorSrgb::Hex("FF0000FF".to_owned()));

    let mut bitmaps = ctx.borrow_bitmaps_mut().unwrap();

    let bitmap_key_a = bitmaps
        .create_bitmap_u8(
            10,
            10,
            PixelLayout::BGRA,
            false,
            true,
            ColorSpace::StandardRGB,
            BitmapCompositing::BlendWithMatte(Color::Black),
        )
        .unwrap();

    {
        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key_a).unwrap();

        bitmap.set_compositing(BitmapCompositing::BlendWithSelf);
        let mut window = bitmap.get_window_u8().unwrap();

        window.fill_rect(1, 1, 9, 9, &red).unwrap();

        let r = ::imageflow_core::graphics::whitespace::detect_content(&window, 1).unwrap();
        assert_eq!((r.x1, r.y1, r.x2, r.y2), (1, 1, 9, 9));
    }

    let bitmap_key_b = bitmaps
        .create_bitmap_u8(
            100,
            100,
            PixelLayout::BGRA,
            false,
            true,
            ColorSpace::StandardRGB,
            BitmapCompositing::BlendWithMatte(Color::Black),
        )
        .unwrap();

    {
        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key_b).unwrap();
        bitmap.set_compositing(BitmapCompositing::BlendWithSelf);
        let mut window = bitmap.get_window_u8().unwrap();
        window.fill_rect(2, 3, 70, 70, &red).unwrap();
        let r = ::imageflow_core::graphics::whitespace::detect_content(&window, 1).unwrap();
        assert_eq!(r.x1, 2);
        assert_eq!(r.y1, 3);
        assert_eq!(r.x2, 70);
        assert_eq!(r.y2, 70);
    }
}

#[test]
fn test_detect_whitespace_all_small_images() {
    let red = Color::Srgb(ColorSrgb::Hex("FF0000FF".to_owned()));
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    let mut failed_count = 0;
    let mut count = 0;

    let mut combinations = vec![];

    // Add smalls
    for w in 3..12u32 {
        for h in 3..12u32 {
            let mut on_canvas = vec![];
            for x in 0..w {
                for y in 0..h {
                    for size_w in 1..3u32 {
                        for size_h in 1..3u32 {
                            if x == 1 && y == 1 && w == 3 && h == 3 {
                                continue; // no checkerboard
                            }
                            if x + size_w <= w && y + size_h <= h && size_w > 0 && size_h > 0 {
                                on_canvas.push((x, y, size_w, size_h));
                            }
                        }
                    }
                }
            }
            combinations.push((w, h, on_canvas));
        }
    }
    // add large sizes
    for (w, h) in [(3000, 2000), (1370, 1370), (1896, 1896), (3000, 3000)] {
        let mut on_canvas = vec![];
        for x in [67, 0, 1, 881] {
            for y in [67, 0, 1, 881] {
                for (r_w, r_h) in [(1, 1), (1896, 1370)] {
                    if x + r_w <= w && y + r_h <= h && r_w > 0 && r_h > 0 {
                        on_canvas.push((x, y, r_w, r_h));
                    }
                }
            }
        }
        combinations.push((w, h, on_canvas));
    }

    let mut failures = vec![];

    for (w, h, on_canvas) in combinations {
        if w < 3 || h < 3 {
            continue;
        }
        let ctx = Context::create_can_panic().unwrap();

        {
            let mut bitmaps = ctx.borrow_bitmaps_mut().unwrap();

            let bitmap_key = bitmaps
                .create_bitmap_u8(
                    w,
                    h,
                    PixelLayout::BGRA,
                    false,
                    true,
                    ColorSpace::StandardRGB,
                    BitmapCompositing::BlendWithMatte(Color::Black),
                )
                .unwrap();

            let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).unwrap();

            bitmap.set_compositing(BitmapCompositing::BlendWithSelf);

            let mut b = bitmap.get_window_u8().unwrap();

            for (x, y, size_w, size_h) in on_canvas {
                b.fill_rect(0, 0, w, h, &Color::Transparent).unwrap();
                b.fill_rect(x, y, x + size_w, y + size_h, &red).unwrap();
                // 1 pixel inset a 2nd rect
                if size_w > 2 {
                    b.fill_rect(x + 1, y + 1, x + size_w - 1, y + size_h - 1, &blue).unwrap();
                }
                let r = ::imageflow_core::graphics::whitespace::detect_content(&b, 1).unwrap();
                let correct =
                    (r.x1 == x) && (r.y1 == y) && (r.x2 == x + size_w) && (r.y2 == y + size_h);
                if !correct {
                    eprint!(
                        "Failed to correctly detect {}x{} dot at {},{} within {}x{}. Detected ",
                        size_w, size_h, x, y, w, h
                    );
                    if r.x1 != x {
                        eprint!("x1={}({})", r.x1, x);
                    }
                    if r.y1 != y {
                        eprint!("y1={}({})", r.y1, y);
                    }
                    if r.x2 != x + size_w {
                        eprint!("Detected x2={}({})", r.x2, x + size_w);
                    }
                    if r.y2 != y + size_h {
                        eprint!("Detected y2={}({})", r.y2, y + size_h);
                    }
                    eprintln!(".");
                    failed_count += 1;
                    failures.push((w, h, x, y, size_w, size_h));
                }
                count += 1;
            }
        }
        ctx.destroy().unwrap();
    }

    if failed_count > 0 {
        if failures.len() > 3 {
            panic!("Failed {} of {} whitespace detection tests", failed_count, count);
        }
    }
}

/// Build a minimal animated GIF with the given frame colors (RGBA hex strings).
/// Each frame is `w`x`h` pixels, solid color, with the given delay in centiseconds.
pub(super) fn build_animated_gif(w: u16, h: u16, colors: &[&str], delay: u16) -> Vec<u8> {
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

#[test]
fn test_animated_gif_roundtrip() {
    let input_gif = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);

    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Encode { io_id: 1, preset: EncoderPreset::Gif },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input_gif).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let execute = Execute001 {
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    };
    ctx.execute_1(execute).unwrap();
    let output_bytes = ctx.take_output_buffer(1).unwrap();

    assert_eq!(
        output_bytes.last(),
        Some(&0x3B),
        "Animated GIF (3 frames) is missing the trailing 0x3B marker. Last bytes: {:02X?}",
        &output_bytes[output_bytes.len().saturating_sub(4)..]
    );

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
    let input_gif = build_animated_gif(8, 8, &["FF0000", "0000FF"], 5);

    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Encode { io_id: 1, preset: EncoderPreset::Gif },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input_gif).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let execute = Execute001 {
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    };
    ctx.execute_1(execute).unwrap();
    let output_bytes = ctx.take_output_buffer(1).unwrap();

    assert_eq!(
        output_bytes.last(),
        Some(&0x3B),
        "Animated GIF (2 frames) is missing the trailing 0x3B marker. Last bytes: {:02X?}",
        &output_bytes[output_bytes.len().saturating_sub(4)..]
    );

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
    let input_gif = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);
    use imageflow_types as s;

    let steps = vec![
        Node::Decode { io_id: 0, commands: Some(vec![s::DecoderCommand::SelectFrame(1)]) },
        Node::Encode { io_id: 1, preset: EncoderPreset::Lodepng { maximum_deflate: None } },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input_gif).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let execute = Execute001 {
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    };
    ctx.execute_1(execute).unwrap();

    let output_bytes = ctx.take_output_buffer(1).unwrap();

    assert_eq!(&output_bytes[1..4], b"PNG", "Output should be a PNG");

    let decoder = lodepng::decode32(&output_bytes).unwrap();
    assert_eq!(decoder.width, 4);
    assert_eq!(decoder.height, 4);
    let pixel = &decoder.buffer[0];
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
    let input_gif = build_animated_gif(4, 4, &["FF0000", "00FF00", "0000FF"], 10);
    use imageflow_types as s;

    let steps = vec![
        Node::Decode { io_id: 0, commands: Some(vec![s::DecoderCommand::SelectFrame(0)]) },
        Node::Encode { io_id: 1, preset: EncoderPreset::Lodepng { maximum_deflate: None } },
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input_gif).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let execute = Execute001 {
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    };
    ctx.execute_1(execute).unwrap();

    let output_bytes = ctx.take_output_buffer(1).unwrap();
    assert_eq!(&output_bytes[1..4], b"PNG", "Output should be a PNG");

    let decoder = lodepng::decode32(&output_bytes).unwrap();
    let pixel = &decoder.buffer[0];
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
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    };
    ctx.execute_1(execute).unwrap();

    let output_bytes = ctx.take_output_buffer(1).unwrap();
    assert_eq!(&output_bytes[1..4], b"PNG", "Output should be a PNG");

    let decoder = lodepng::decode32(&output_bytes).unwrap();
    let pixel = &decoder.buffer[0];
    assert!(
        pixel.g > 200 && pixel.r < 50 && pixel.b < 50,
        "Expected green pixel from frame 1 via querystring, got r={} g={} b={} a={}",
        pixel.r,
        pixel.g,
        pixel.b,
        pixel.a
    );
}

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
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    };
    ctx1.execute_1(execute1).unwrap();
    let bytes = ctx1.take_output_buffer(0).unwrap();

    assert_eq!(
        bytes.last(),
        Some(&0x3B),
        "Still GIF is missing the trailing 0x3B marker. Last bytes: {:02X?}",
        &bytes[bytes.len().saturating_sub(4)..]
    );

    let mut ctx2 = Context::create().unwrap();
    ctx2.add_input_vector(0, bytes.to_vec()).unwrap();
    let execute2 = Execute001 {
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(vec![Node::Decode { io_id: 0, commands: None }]),
    };
    ctx2.execute_1(execute2).unwrap();
}
