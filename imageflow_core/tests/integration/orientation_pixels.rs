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
