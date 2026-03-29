//! Test: do mozjpeg and zenjpeg produce the same pixels for the same JPEG?
//!
//! This isolates JPEG decoder differences from CMS, pipeline, and encoder.
//! If decoders produce different pixels, ICC test failures are decoder-caused.

use std::path::Path;

/// Decode a JPEG with mozjpeg (v2 decoder) and return raw BGRA pixels + dimensions.
#[cfg(feature = "c-codecs")]
fn decode_mozjpeg(jpeg_bytes: &[u8]) -> (Vec<u8>, u32, u32) {
    let mut ctx = imageflow_core::Context::create().unwrap();

    // Force v2 backend — io provided via Build001.io below
    ctx.force_backend = Some(imageflow_core::Backend::V2);

    let steps = vec![
        imageflow_types::Node::Decode { io_id: 0, commands: None },
        imageflow_types::Node::CaptureBitmapKey { capture_id: 0 },
    ];

    let _ = ctx
        .build_1(imageflow_types::Build001 {
            builder_config: None,
            io: vec![imageflow_types::IoObject {
                io_id: 0,
                direction: imageflow_types::IoDirection::In,
                io: imageflow_types::IoEnum::ByteArray(jpeg_bytes.to_vec()),
            }],
            framewise: imageflow_types::Framewise::Steps(steps),
        })
        .unwrap();

    let bitmap_key = ctx.get_captured_bitmap_key(0).unwrap();
    let bitmaps = ctx.borrow_bitmaps().unwrap();
    let mut bm = bitmaps.try_borrow_mut(bitmap_key).unwrap();
    let window = bm.get_window_u8().unwrap();
    let w = window.w();
    let h = window.h();
    let stride = window.info().t_stride() as usize;
    let bpp = 4; // BGRA

    // Copy pixel data row by row (stride may differ from w*bpp)
    let mut pixels = Vec::with_capacity(w as usize * h as usize * bpp);
    for y in 0..h {
        let row_start = y as usize * stride;
        let row_end = row_start + w as usize * bpp;
        pixels.extend_from_slice(&window.get_slice()[row_start..row_end]);
    }
    (pixels, w, h)
}

/// Decode a JPEG with zenjpeg (zen decoder) and return raw RGB pixels + dimensions.
#[cfg(feature = "zen-pipeline")]
fn decode_zenjpeg(jpeg_bytes: &[u8]) -> (Vec<u8>, u32, u32) {
    let registry = zencodecs::AllowedFormats::all();
    let output =
        zencodecs::DecodeRequest::new(jpeg_bytes).with_registry(&registry).decode().unwrap();

    let w = output.width();
    let h = output.height();
    let desc = output.descriptor();
    let bpp = desc.bytes_per_pixel();
    eprintln!("[zenjpeg] {}x{} {}bpp {:?}", w, h, bpp, desc);
    let pixels = output.pixels();
    (pixels.contiguous_bytes().to_vec(), w, h)
}

#[test]
#[cfg(all(feature = "c-codecs", feature = "zen-pipeline"))]
fn jpeg_decoder_raw_pixel_comparison() {
    // Load a test JPEG
    let jpeg_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join(".image-cache/sources/imageflow-resources/test_inputs/wide-gamut/rec-2020-pq/flickr_2a68670c58131566.jpg");

    if !jpeg_path.exists() {
        eprintln!("skipping: test image not cached at {}", jpeg_path.display());
        return;
    }

    let jpeg_bytes = std::fs::read(&jpeg_path).unwrap();

    let (mozjpeg_pixels, mw, mh) = decode_mozjpeg(&jpeg_bytes);
    let (zenjpeg_pixels, zw, zh) = decode_zenjpeg(&jpeg_bytes);

    assert_eq!((mw, mh), (zw, zh), "dimensions differ");

    let w = mw as usize;
    let h = mh as usize;

    // mozjpeg is BGRA (4 bytes), zenjpeg is RGB (3 bytes)
    // Compare R,G,B channels only
    let moz_bpp = 4;
    let zen_bpp = zenjpeg_pixels.len() / (w * h);
    eprintln!(
        "mozjpeg: {} bytes ({}bpp), zenjpeg: {} bytes ({}bpp), {}x{}",
        mozjpeg_pixels.len(),
        moz_bpp,
        zenjpeg_pixels.len(),
        zen_bpp,
        w,
        h
    );

    let mut max_delta = [0u8; 3];
    let mut sum_delta = [0u64; 3];
    let mut diff_count = 0u64;
    let total = (w * h) as u64;

    for y in 0..h {
        for x in 0..w {
            let moff = (y * w + x) * moz_bpp;
            let zoff = (y * w + x) * zen_bpp;

            // mozjpeg is BGRA: [B, G, R, A]
            let mr = mozjpeg_pixels[moff + 2];
            let mg = mozjpeg_pixels[moff + 1];
            let mb = mozjpeg_pixels[moff + 0];

            // zenjpeg is RGB: [R, G, B]
            let zr = zenjpeg_pixels[zoff + 0];
            let zg = zenjpeg_pixels[zoff + 1];
            let zb = zenjpeg_pixels[zoff + 2];

            let dr = mr.abs_diff(zr);
            let dg = mg.abs_diff(zg);
            let db = mb.abs_diff(zb);

            if dr > max_delta[0] {
                max_delta[0] = dr;
            }
            if dg > max_delta[1] {
                max_delta[1] = dg;
            }
            if db > max_delta[2] {
                max_delta[2] = db;
            }

            sum_delta[0] += dr as u64;
            sum_delta[1] += dg as u64;
            sum_delta[2] += db as u64;

            if dr > 1 || dg > 1 || db > 1 {
                diff_count += 1;
            }
        }
    }

    let avg_r = sum_delta[0] as f64 / total as f64;
    let avg_g = sum_delta[1] as f64 / total as f64;
    let avg_b = sum_delta[2] as f64 / total as f64;

    eprintln!("=== JPEG DECODER COMPARISON (before CMS) ===");
    eprintln!("Max delta:  R={} G={} B={}", max_delta[0], max_delta[1], max_delta[2]);
    eprintln!("Avg delta:  R={:.2} G={:.2} B={:.2}", avg_r, avg_g, avg_b);
    eprintln!(
        "Pixels > 1: {}/{} ({:.1}%)",
        diff_count,
        total,
        diff_count as f64 / total as f64 * 100.0
    );

    // Also test a normal sRGB JPEG for comparison.
    let srgb_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join(".image-cache/sources/imageflow-resources/test_inputs/wide-gamut/srgb-reference/canon_eos_5d_mark_iv/wmc_81b268fc64ea796c.jpg");
    if srgb_path.exists() {
        let srgb_bytes = std::fs::read(&srgb_path).unwrap();
        let (moz_px, _, _) = decode_mozjpeg(&srgb_bytes);
        let (zen_px, zw2, zh2) = decode_zenjpeg(&srgb_bytes);
        let w2 = zw2 as usize;
        let h2 = zh2 as usize;
        let zen_bpp2 = zen_px.len() / (w2 * h2);
        let mut max_d2 = [0u8; 3];
        for y in 0..h2 {
            for x in 0..w2 {
                let moff = (y * w2 + x) * 4;
                let zoff = (y * w2 + x) * zen_bpp2;
                let dr = moz_px[moff + 2].abs_diff(zen_px[zoff]);
                let dg = moz_px[moff + 1].abs_diff(zen_px[zoff + 1]);
                let db = moz_px[moff + 0].abs_diff(zen_px[zoff + 2]);
                if dr > max_d2[0] {
                    max_d2[0] = dr;
                }
                if dg > max_d2[1] {
                    max_d2[1] = dg;
                }
                if db > max_d2[2] {
                    max_d2[2] = db;
                }
            }
        }
        eprintln!("=== sRGB JPEG (Canon 5D) ===");
        eprintln!("Max delta: R={} G={} B={}", max_d2[0], max_d2[1], max_d2[2]);
    }

    // This test DOCUMENTS the decoder difference — it's expected to show
    // some delta. The key question is whether the delta explains the
    // post-CMS ICC test failures (delta ~186 for Rec.2020).
    //
    // If max_delta here is ~1-2, then the CMS amplifies it to 186.
    // If max_delta here is ~50+, then the decoder diff is the primary cause.
}
