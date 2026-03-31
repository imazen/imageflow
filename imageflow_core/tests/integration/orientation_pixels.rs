//! Pixel-level orientation verification.
//!
//! Uses a synthetic PNG gradient to verify that ApplyOrientation is applied
//! exactly once (not zero times, not twice) through both v2 and zen backends.
//!
//! A left-to-right red gradient is decoded, then ApplyOrientation is applied.
//! After FlipH (flag=2), the gradient should be reversed: left R > right R.
//! After Rotate90 (flag=6), dimensions should swap.

use imageflow_core::Context;
use imageflow_types as s;
use crate::common;

/// Run a pipeline with ApplyOrientation and check pixel content.
///
/// Creates a gradient, runs it through Constrain + ApplyOrientation,
/// and checks that the gradient direction matches the expected orientation.
fn check_orientation_pixels(backend: imageflow_core::Backend, exif_flag: i32, label: &str) {
    let w = 80u32;
    let h = 40u32;

    // Build gradient as PNG bytes (lossless)
    let png_bytes = {
        let mut png_data = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png_data, w, h);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            let mut rgba = vec![0u8; (w * h * 4) as usize];
            for y in 0..h {
                for x in 0..w {
                    let i = (y * w + x) as usize * 4;
                    rgba[i] = (x * 255 / (w - 1).max(1)) as u8;     // R gradient
                    rgba[i + 1] = (y * 255 / (h - 1).max(1)) as u8; // G gradient
                    rgba[i + 2] = 0;
                    rgba[i + 3] = 255;
                }
            }
            writer.write_image_data(&rgba).unwrap();
        }
        png_data
    };

    let mut ctx = Context::create().unwrap();
    ctx.force_backend = Some(backend);
    ctx.add_copied_input_buffer(0, &png_bytes).unwrap();

    let capture_id = 0;
    let steps = vec![
        s::Node::Decode { io_id: 0, commands: None },
        s::Node::ApplyOrientation { flag: exif_flag },
        s::Node::CaptureBitmapKey { capture_id },
    ];

    let result = ctx.execute_1(s::Execute001 {
        graph_recording: None,
        security: None,
        framewise: s::Framewise::Steps(steps),
        job_options: None,
    });
    result.unwrap();

    let bitmap_key = ctx.get_captured_bitmap_key(capture_id)
        .unwrap_or_else(|| panic!("[{label}] no captured bitmap"));
    let bitmaps = ctx.borrow_bitmaps().unwrap();
    let mut bm = bitmaps.try_borrow_mut(bitmap_key).unwrap();
    let window = bm.get_window_u8().unwrap();
    let ow = window.w();
    let oh = window.h();

    // Read BGRA pixels at corners
    let stride = window.info().t_stride() as usize;
    let slice = window.get_slice();
    let px = |x: u32, y: u32| -> [u8; 4] {
        let off = y as usize * stride + x as usize * 4;
        [slice[off], slice[off+1], slice[off+2], slice[off+3]] // B, G, R, A
    };

    let top_left = px(0, 0);
    let top_right = px(ow - 1, 0);
    let bot_left = px(0, oh - 1);

    // R channel is at index 2 in BGRA
    let tl_r = top_left[2];
    let tr_r = top_right[2];
    let tl_g = top_left[1];
    let bl_g = bot_left[1];

    match exif_flag {
        1 => {
            // Identity: R increases left→right, G increases top→bottom
            assert!(tr_r > tl_r + 100, "[{label}] Identity: R should increase L→R, tl_r={tl_r} tr_r={tr_r}");
            assert!(bl_g > tl_g + 100, "[{label}] Identity: G should increase T→B, tl_g={tl_g} bl_g={bl_g}");
        }
        2 => {
            // FlipH: R gradient reversed (right→left), G unchanged
            assert!(tl_r > tr_r + 100, "[{label}] FlipH: R should increase R→L, tl_r={tl_r} tr_r={tr_r}");
            assert!(bl_g > tl_g + 100, "[{label}] FlipH: G should still increase T→B, tl_g={tl_g} bl_g={bl_g}");
        }
        6 => {
            // Rotate90: dims swap. Original R(L→R) becomes R(T→B), G(T→B) becomes G(R→L)
            assert_eq!((ow, oh), (h, w), "[{label}] Rotate90 should swap dims");
            // After rot90 CW: top_left was bottom_left of original (R=0, G=255)
            assert!(tl_r < 55, "[{label}] Rotate90: top_left R should be low, got {tl_r}");
        }
        _ => panic!("unsupported flag {exif_flag} in pixel check"),
    }
}

#[test]
fn orientation_pixels_v2_identity() {
    check_orientation_pixels(imageflow_core::Backend::V2, 1, "v2/identity");
}

#[test]
fn orientation_pixels_v2_flip_h() {
    check_orientation_pixels(imageflow_core::Backend::V2, 2, "v2/flip_h");
}

#[test]
fn orientation_pixels_v2_rotate90() {
    check_orientation_pixels(imageflow_core::Backend::V2, 6, "v2/rotate90");
}

#[cfg(feature = "zen-pipeline")]
#[test]
fn orientation_pixels_zen_identity() {
    check_orientation_pixels(imageflow_core::Backend::Zen, 1, "zen/identity");
}

#[cfg(feature = "zen-pipeline")]
#[test]
fn orientation_pixels_zen_flip_h() {
    check_orientation_pixels(imageflow_core::Backend::Zen, 2, "zen/flip_h");
}

#[cfg(feature = "zen-pipeline")]
#[test]
fn orientation_pixels_zen_rotate90() {
    check_orientation_pixels(imageflow_core::Backend::Zen, 6, "zen/rotate90");
}

// ═══════════════════════════════════════════════════════════════════════
// JPEG EXIF auto-orient tests
//
// These use real JPEG files with EXIF orientation tags from S3.
// The pipeline uses Decode + Constrain (no explicit ApplyOrientation).
// The backend must auto-detect EXIF orientation and apply it.
// ═══════════════════════════════════════════════════════════════════════

/// Fetch a test JPEG from S3 and return bytes.
fn fetch_test_jpeg(name: &str) -> Vec<u8> {
    let url = format!(
        "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/{name}"
    );
    common::get_url_bytes_with_retry(&url).unwrap()
}

/// Run Decode + Constrain (no explicit ApplyOrientation) and return captured BGRA pixels.
fn decode_constrain_capture(
    backend: imageflow_core::Backend,
    jpeg_bytes: &[u8],
    max_w: u32,
    max_h: u32,
) -> (Vec<u8>, u32, u32) {
    let mut ctx = Context::create().unwrap();
    ctx.force_backend = Some(backend);
    ctx.add_copied_input_buffer(0, jpeg_bytes).unwrap();

    let capture_id = 0;
    let result = ctx.execute_1(s::Execute001 {
        graph_recording: None,
        security: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Constrain(s::Constraint {
                mode: s::ConstraintMode::Within,
                w: Some(max_w),
                h: Some(max_h),
                hints: None,
                gravity: None,
                canvas_color: None,
            }),
            s::Node::CaptureBitmapKey { capture_id },
        ]),
        job_options: None,
    });
    result.unwrap();

    let bitmap_key = ctx.get_captured_bitmap_key(capture_id).unwrap();
    let bitmaps = ctx.borrow_bitmaps().unwrap();
    let mut bm = bitmaps.try_borrow_mut(bitmap_key).unwrap();
    let mut window = bm.get_window_u8().unwrap();
    window.normalize_unused_alpha().unwrap();
    let w = window.w();
    let h = window.h();
    let stride = window.info().t_stride() as usize;

    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h as usize {
        let row = &window.get_slice()[y * stride..y * stride + w as usize * 4];
        pixels.extend_from_slice(row);
    }
    (pixels, w, h)
}

/// Compare v2 and zen output dimensions and pixel similarity for a JPEG with EXIF flag.
fn compare_backends_exif(jpeg_name: &str, max_dim: u32) {
    let bytes = fetch_test_jpeg(jpeg_name);

    let (v2_px, v2_w, v2_h) = decode_constrain_capture(
        imageflow_core::Backend::V2, &bytes, max_dim, max_dim,
    );
    let (zen_px, zen_w, zen_h) = decode_constrain_capture(
        imageflow_core::Backend::Zen, &bytes, max_dim, max_dim,
    );

    // Dimensions must match (both should auto-orient the same way)
    assert_eq!(
        (v2_w, v2_h), (zen_w, zen_h),
        "{jpeg_name}: v2 dims {v2_w}x{v2_h} != zen dims {zen_w}x{zen_h}"
    );

    // Pixel similarity: compute max delta across all pixels
    let mut max_delta: u8 = 0;
    let mut diff_count: u64 = 0;
    let total = v2_px.len();
    for (a, b) in v2_px.iter().zip(zen_px.iter()) {
        let d = (*a as i16 - *b as i16).unsigned_abs() as u8;
        if d > max_delta {
            max_delta = d;
        }
        if d > 0 {
            diff_count += 1;
        }
    }
    let diff_pct = diff_count as f64 / total as f64 * 100.0;
    eprintln!(
        "{jpeg_name}: {v2_w}x{v2_h}, max_delta={max_delta}, {diff_pct:.1}% pixels differ"
    );

    // Allow decoder rounding differences but not structural mismatches.
    // If orientation is wrong, max_delta will be very large (>100).
    assert!(
        max_delta < 100,
        "{jpeg_name}: max_delta={max_delta} — likely orientation mismatch between v2 and zen"
    );
}

// Landscape images: EXIF flags 1-8 (all have the same scene, different orientation)
// Flag 1 = identity, 2 = FlipH, 3 = Rotate180, 4 = FlipV,
// 5 = Transpose, 6 = Rotate90, 7 = Transverse, 8 = Rotate270

#[test]
fn exif_auto_orient_landscape_1() { compare_backends_exif("Landscape_1.jpg", 70); }
#[test]
fn exif_auto_orient_landscape_2() { compare_backends_exif("Landscape_2.jpg", 70); }
#[test]
fn exif_auto_orient_landscape_3() { compare_backends_exif("Landscape_3.jpg", 70); }
#[test]
fn exif_auto_orient_landscape_4() { compare_backends_exif("Landscape_4.jpg", 70); }
#[test]
fn exif_auto_orient_landscape_5() { compare_backends_exif("Landscape_5.jpg", 70); }
#[test]
fn exif_auto_orient_landscape_6() { compare_backends_exif("Landscape_6.jpg", 70); }
#[test]
fn exif_auto_orient_landscape_7() { compare_backends_exif("Landscape_7.jpg", 70); }
#[test]
fn exif_auto_orient_landscape_8() { compare_backends_exif("Landscape_8.jpg", 70); }

// ═══════════════════════════════════════════════════════════════════════
// ICC profile pixel comparison — v2 vs zen
// ═══════════════════════════════════════════════════════════════════════

fn fetch_test_input(path: &str) -> Vec<u8> {
    let url = format!(
        "https://s3-us-west-2.amazonaws.com/imageflow-resources/{path}"
    );
    common::get_url_bytes_with_retry(&url).unwrap()
}

fn compare_backends_command(input_path: &str, command: &str, label: &str) {
    let bytes = fetch_test_input(input_path);

    let run = |backend: imageflow_core::Backend| -> (Vec<u8>, u32, u32) {
        let mut ctx = Context::create().unwrap();
        ctx.force_backend = Some(backend);
        ctx.add_copied_input_buffer(0, &bytes).unwrap();

        let capture_id = 0;
        ctx.execute_1(s::Execute001 {
            graph_recording: None,
            security: None,
            framewise: s::Framewise::Steps(vec![
                s::Node::CommandString {
                    kind: s::CommandStringKind::ImageResizer4,
                    value: command.to_string(),
                    decode: Some(0),
                    encode: None,
                    watermarks: None,
                },
                s::Node::CaptureBitmapKey { capture_id },
            ]),
            job_options: None,
        }).unwrap();

        let bitmap_key = ctx.get_captured_bitmap_key(capture_id).unwrap();
        let bitmaps = ctx.borrow_bitmaps().unwrap();
        let mut bm = bitmaps.try_borrow_mut(bitmap_key).unwrap();
        let mut window = bm.get_window_u8().unwrap();
        window.normalize_unused_alpha().unwrap();
        let w = window.w();
        let h = window.h();
        let stride = window.info().t_stride() as usize;
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h as usize {
            let row = &window.get_slice()[y * stride..y * stride + w as usize * 4];
            pixels.extend_from_slice(row);
        }
        (pixels, w, h)
    };

    let (v2_px, v2_w, v2_h) = run(imageflow_core::Backend::V2);
    let (zen_px, zen_w, zen_h) = run(imageflow_core::Backend::Zen);

    assert_eq!(
        (v2_w, v2_h), (zen_w, zen_h),
        "{label}: dims v2={v2_w}x{v2_h} zen={zen_w}x{zen_h}"
    );

    let mut max_delta: u8 = 0;
    let mut diff_count: u64 = 0;
    for (a, b) in v2_px.iter().zip(zen_px.iter()) {
        let d = (*a as i16 - *b as i16).unsigned_abs() as u8;
        if d > max_delta { max_delta = d; }
        if d > 0 { diff_count += 1; }
    }
    let diff_pct = diff_count as f64 / v2_px.len() as f64 * 100.0;
    eprintln!("{label}: {v2_w}x{v2_h}, max_delta={max_delta}, {diff_pct:.1}% differ");

    // Decoder diff should be small. ICC diff would be huge.
    if max_delta >= 10 {
        panic!("{label}: max_delta={max_delta}, {diff_pct:.1}% differ — v2 and zen produce very different output");
    }
}

#[test]
fn icc_parity_srgb_canon5d() {
    compare_backends_command(
        "test_inputs/wide-gamut/srgb-reference/canon_eos_5d_mark_iv/wmc_81b268fc64ea796c.jpg",
        "w=300&format=png",
        "sRGB Canon 5D",
    );
}

#[test]
fn icc_parity_adobe_rgb() {
    compare_backends_command(
        "test_inputs/wide-gamut/adobe-rgb/flickr_092650e9e8211233.jpg",
        "w=300&format=png",
        "Adobe RGB",
    );
}

#[test]
fn icc_parity_display_p3() {
    compare_backends_command(
        "test_inputs/wide-gamut/display-p3/flickr_403aa5efb8efe6e8.jpg",
        "w=300&format=png",
        "Display P3",
    );
}

#[test]
fn icc_parity_rec2020() {
    compare_backends_command(
        "test_inputs/wide-gamut/rec-2020-pq/flickr_2a68670c58131566.jpg",
        "w=300&format=png",
        "Rec 2020",
    );
}

#[test]
fn icc_parity_prophoto() {
    compare_backends_command(
        "test_inputs/wide-gamut/prophoto-rgb/flickr_0d2d634cf46df137.jpg",
        "w=300&format=png",
        "ProPhoto RGB",
    );
}

// Portrait images: same set of flags
#[test]
fn exif_auto_orient_portrait_1() { compare_backends_exif("Portrait_1.jpg", 70); }
#[test]
fn exif_auto_orient_portrait_2() { compare_backends_exif("Portrait_2.jpg", 70); }
#[test]
fn exif_auto_orient_portrait_6() { compare_backends_exif("Portrait_6.jpg", 70); }
#[test]
fn exif_auto_orient_portrait_8() { compare_backends_exif("Portrait_8.jpg", 70); }
