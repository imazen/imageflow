#[allow(unused_imports)]
use crate::common::*;
use imageflow_core::Context;
use imageflow_types::{
    Color, ColorSrgb, CommandStringKind, Constraint, ConstraintMode, EncoderPreset, Node,
};

// ─── Encoded output tests (compare_encoded → visual_check / visual_check_steps) ──

#[test]
fn test_encode_gradients() {
    visual_check_steps! {
        source: "test_inputs/gradients.png",
        detail: "png32_passthrough",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Encode { io_id: 1, preset: EncoderPreset::libpng32() },
        ],
        similarity: Similarity::AllowOffByOneBytesRatio(0.01),
        max_file_size: 100000,
    }
}

#[test]
fn test_transparent_png_to_png() {
    visual_check! {
        source: "test_inputs/shirt_transparent.png",
        detail: "shirt",
        command: "format=png",
        similarity: Similarity::AllowOffByOneBytesCount(100),
    }
}

#[test]
fn test_problematic_png_lossy() {
    visual_check! {
        source: "test_inputs/png_turns_empty_2.png",
        detail: "crop_1230x760",
        command: "w=1230&h=760&png.quality=75&mode=crop&scale=both",
        // Centos pngquant selects different palette entries for ~946/934800 pixels
        // in the bottom-right corner (max delta B=22, zensim 96.4 vs baseline).
        similarity: Similarity::MaxZdsim(0.05), // measured centos zdsim: 0.036
    }
}

#[test]
fn test_transparent_png_to_png_rounded_corners() {
    visual_check! {
        source: "test_inputs/shirt_transparent.png",
        detail: "shirt_cropped",
        command: "format=png&crop=10,10,70,70&cropxunits=100&cropyunits=100&s.roundcorners=100",
        similarity: Similarity::AllowOffByOneBytesCount(100),
    }
}

#[test]
fn test_transparent_png_to_jpeg() {
    visual_check! {
        source: "test_inputs/shirt_transparent.png",
        detail: "shirt",
        command: "format=jpg",
    }
}

#[test]
fn test_transparent_png_to_jpeg_constrain() {
    visual_check_steps! {
        source: "test_inputs/shirt_transparent.png",
        detail: "300x300_mozjpeg",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Constrain(Constraint {
                mode: ConstraintMode::Within,
                w: Some(300),
                h: Some(300),
                hints: None,
                gravity: None,
                canvas_color: None,
            }),
            Node::Encode {
                io_id: 1,
                preset: EncoderPreset::Mozjpeg { quality: Some(100), progressive: None, matte: None },
            },
        ],
        similarity: Similarity::MaxZdsim(0.03),
    }
}

#[test]
fn test_matte_transparent_png() {
    visual_check_steps! {
        source: "test_inputs/shirt_transparent.png",
        detail: "shirt_300x300_white_matte",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Constrain(Constraint {
                mode: ConstraintMode::Within,
                w: Some(300),
                h: Some(300),
                hints: None,
                gravity: None,
                canvas_color: None,
            }),
            Node::Encode {
                io_id: 1,
                preset: EncoderPreset::Libpng {
                    depth: None,
                    matte: Some(Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_string()))),
                    zlib_compression: None,
                },
            },
        ],
    }
}

// This test uses a branching pipeline producing 2 outputs — not macro-convertible
#[test]
fn test_branching_crop_whitespace() {
    let identity = test_identity!();
    let preset = EncoderPreset::Lodepng { maximum_deflate: None };
    let source_url = "https://imageflow-resources.s3.us-west-2.amazonaws.com/test_inputs/little_gradient_whitespace.jpg";

    let s = imageflow_core::clients::fluent::fluently().decode(0);
    let v1 = s.branch();
    let v2 = v1.branch().crop_whitespace(200, 0f32);
    let framewise =
        v1.encode(1, preset.clone()).builder().with(v2.encode(2, preset.clone())).to_framewise();

    let io_vec = vec![
        IoTestEnum::Url(source_url.to_owned()),
        IoTestEnum::OutputBuffer,
        IoTestEnum::OutputBuffer,
    ];

    let mut context = imageflow_core::Context::create().unwrap();
    let _ = build_framewise(&mut context, framewise, io_vec, None, false).unwrap();

    let tol_spec = Similarity::MaxZdsim(0.02).to_tolerance_spec();

    for output_io_id in [1, 2] {
        let detail = format!("gradient_output_{output_io_id}");
        let bytes = context.take_output_buffer(output_io_id).unwrap();
        check_visual_bytes(&identity, &detail, &bytes, &tol_spec);
    }
}

#[test]
fn test_transparent_webp_to_webp() {
    visual_check! {
        source: "test_inputs/1_webp_ll.webp",
        detail: "lossless_100x100",
        command: "format=webp&width=100&height=100&webp.lossless=true",
        similarity: Similarity::AllowOffByOneBytesCount(500),
    }
}

#[test]
fn test_webp_to_webp_quality() {
    visual_check! {
        source: "test_inputs/1_webp_ll.webp",
        detail: "q5_100x100",
        command: "format=webp&width=100&height=100&quality=5",
        similarity: Similarity::MaxZdsim(0.05),
        max_file_size: 2500,
    }
}

// ─── Bitmap comparison tests (compare → visual_check_bitmap) ──────────────

#[test]
fn test_jpeg_simple() {
    visual_check_bitmap! {
        source: "test_inputs/orientation/Landscape_1.jpg",
        detail: "landscape_within_70x70",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Constrain(Constraint {
                mode: ConstraintMode::Within,
                w: Some(70),
                h: Some(70),
                hints: None,
                gravity: None,
                canvas_color: None,
            }),
        ],
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn test_jpeg_simple_rot_90() {
    visual_check_bitmap! {
        source: "test_inputs/orientation/Landscape_1.jpg",
        detail: "landscape_70x70",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Constrain(Constraint {
                mode: ConstraintMode::Within,
                w: Some(70),
                h: Some(70),
                hints: None,
                gravity: None,
                canvas_color: None,
            }),
            Node::Rotate90,
        ],
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn test_rot_90_and_red_dot() {
    visual_check_bitmap! {
        source: "test_inputs/orientation/Landscape_1.jpg",
        detail: "landscape_70x70",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Constrain(Constraint {
                mode: ConstraintMode::Within,
                w: Some(70),
                h: Some(70),
                hints: None,
                gravity: None,
                canvas_color: None,
            }),
            Node::Rotate90,
            Node::WatermarkRedDot,
        ],
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn test_rot_90_and_red_dot_command_string() {
    visual_check_bitmap! {
        source: "test_inputs/orientation/Landscape_1.jpg",
        detail: "landscape_70x70",
        steps: vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "w=70&h=70&mode=max&rotate=90&watermark_red_dot=true".to_string(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn test_negatives_in_command_string() {
    visual_check_bitmap! {
        source: "test_inputs/red-leaf.jpg",
        detail: "red_leaf_negative_height",
        steps: vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "h=-100&maxwidth=2&mode=crop".to_string(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn test_jpeg_crop() {
    visual_check_bitmap! {
        source: "test_inputs/waterhouse.jpg",
        detail: "waterhouse_100x200",
        steps: vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "width=100&height=200&mode=crop".to_owned(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn decode_cmyk_jpeg() {
    visual_check_bitmap! {
        source: "test_inputs/cmyk_logo.jpg",
        detail: "logo_passthrough",
        steps: vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "".to_owned(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
        // CMYK→RGB conversion differs by up to 3 levels across SIMD paths
        // (AVX512 vs NEON vs AVX2). Perceptually identical.
        tolerance: Tolerance {
            max_delta: 3,
            min_similarity: 95.0,
            max_pixels_different: 1.0,
            ..Tolerance::exact()
        },
    }
}

#[test]
fn decode_rgb_with_cmyk_profile_jpeg() {
    visual_check_bitmap! {
        source: "test_inputs/wrenches.jpg",
        detail: "wrenches_ignore_icc",
        steps: vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "ignore_icc_errors=true".to_owned(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn test_crop_with_preshrink() {
    visual_check_bitmap! {
        source: "https://resizer-images.s3.amazonaws.com/private/cropissue.jpg",
        detail: "170x220_crop",
        steps: vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "w=170&h=220&mode=crop&scale=both&crop=449,0,-472,0".to_owned(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
        tolerance: Tolerance::off_by_one(),
    }
}

// ─── Alpha channel regression tests ─────────────────────────────────────

/// Verify that JPEG decode + scale produces fully opaque output (alpha=255).
///
/// JPEG has no alpha channel. The pipeline creates BGRA bitmaps with
/// alpha_meaningful=false. Some mozjpeg SIMD paths leave alpha=0 (from
/// zero-initialized canvas), others write 0xFF. The test framework
/// normalizes unused alpha to 255 before checksumming, but this test
/// verifies the raw bitmap directly.
#[test]
fn test_jpeg_alpha_is_opaque() {
    let source_url = format!(
        "https://s3-us-west-2.amazonaws.com/imageflow-resources/{}",
        "test_inputs/orientation/Landscape_1.jpg"
    );

    let mut context = Context::create().unwrap();
    let capture_id = 0;
    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Constrain(Constraint {
            mode: ConstraintMode::Within,
            w: Some(200),
            h: Some(200),
            hints: None,
            gravity: None,
            canvas_color: None,
        }),
        Node::CaptureBitmapKey { capture_id },
    ];

    build_steps(&mut context, &steps, vec![IoTestEnum::Url(source_url)], None, false).unwrap();

    let bitmap_key = context.get_captured_bitmap_key(capture_id).expect("no bitmap produced");
    let bitmaps = context.borrow_bitmaps().unwrap();
    let mut bm = bitmaps.try_borrow_mut(bitmap_key).unwrap();
    let mut window = bm.get_window_u8().unwrap();

    // Normalize alpha (same as the test framework does before checksumming)
    window.normalize_unused_alpha().unwrap();

    // Verify every pixel's alpha is 255
    for line in window.scanlines_bgra().unwrap() {
        for pix in line.row() {
            assert_eq!(
                pix.a, 255,
                "JPEG-sourced pixel has alpha={}, expected 255 after normalization.",
                pix.a
            );
        }
    }
}
