//! Integration test for the zen streaming pipeline via v1/zen-build endpoint.
//!
//! Requires both `zen-pipeline` and `c-codecs` features (c-codecs for Context,
//! zen-pipeline for the zen endpoint).

#![cfg(all(feature = "zen-pipeline", feature = "c-codecs"))]

use imageflow_core::{Context, JsonResponse};
use imageflow_types as s;
use imageflow_types::*;

/// Helper: send a JSON request to Context and assert success.
fn send_json(ctx: &mut Context, method: &str, json: &serde_json::Value) -> JsonResponse {
    let json_bytes = serde_json::to_vec(json).unwrap();
    let response = imageflow_core::json::invoke(ctx, method, &json_bytes).unwrap();
    if !response.status_2xx() {
        let body = std::str::from_utf8(response.response_json.as_ref()).unwrap_or("(invalid utf8)");
        panic!("{method} failed with status {}: {body}", response.status_code);
    }
    response
}

/// Generate a minimal valid JPEG for testing.
fn make_test_jpeg(w: u32, h: u32) -> Vec<u8> {
    let mut pixels = vec![128u8; (w * h * 4) as usize];
    // Simple gradient so the image isn't uniform.
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            pixels[i] = (x * 255 / w) as u8;
            pixels[i + 1] = (y * 255 / h) as u8;
        }
    }

    let descriptor = zenpixels::PixelDescriptor::RGBA8_SRGB;
    let stride = (w * 4) as usize;
    let ps = zenpixels::PixelSlice::new(&pixels, w, h, stride, descriptor).unwrap();
    zencodecs::EncodeRequest::new(zencodecs::ImageFormat::Jpeg)
        .with_quality(85.0)
        .encode(ps, false)
        .unwrap()
        .into_vec()
}

#[test]
fn zen_build_jpeg_resize() {
    let jpeg_bytes = make_test_jpeg(400, 300);
    let hex_input = hex::encode(&jpeg_bytes);

    let build_request = serde_json::json!({
        "io": [
            {"io_id": 0, "direction": "in", "io": {"bytes_hex": hex_input}},
            {"io_id": 1, "direction": "out", "io": "output_buffer"}
        ],
        "framewise": {
            "steps": [
                {"decode": {"io_id": 0}},
                {"constrain": {"mode": "within", "w": 200, "h": 150}},
                {"encode": {"io_id": 1, "preset": {"mozjpeg": {"quality": 80}}}}
            ]
        }
    });

    let mut ctx = Context::create().unwrap();
    let _response = send_json(&mut ctx, "v1/zen-build", &build_request);

    // Verify output buffer exists and contains valid JPEG.
    let output = ctx.take_output_buffer(1).unwrap();
    assert!(output.len() > 100, "output too small: {} bytes", output.len());
    assert_eq!(&output[..2], &[0xFF, 0xD8], "not a JPEG");

    // Probe the output to verify dimensions.
    let info = zencodecs::from_bytes(&output).unwrap();
    assert!(info.width <= 200, "width {} > 200", info.width);
    assert!(info.height <= 150, "height {} > 150", info.height);
}

#[test]
fn zen_build_format_auto_select() {
    let jpeg_bytes = make_test_jpeg(100, 100);
    let hex_input = hex::encode(&jpeg_bytes);

    // Use Auto preset — should select JPEG for opaque input.
    let build_request = serde_json::json!({
        "io": [
            {"io_id": 0, "direction": "in", "io": {"bytes_hex": hex_input}},
            {"io_id": 1, "direction": "out", "io": "output_buffer"}
        ],
        "framewise": {
            "steps": [
                {"decode": {"io_id": 0}},
                {"encode": {"io_id": 1, "preset": {
                    "auto": {
                        "quality_profile": "high",
                        "allow": {"jpeg": true, "png": true, "gif": true}
                    }
                }}}
            ]
        }
    });

    let mut ctx = Context::create().unwrap();
    let _response = send_json(&mut ctx, "v1/zen-build", &build_request);

    let output = ctx.take_output_buffer(1).unwrap();
    assert!(output.len() > 50, "output too small");
    // Auto should select JPEG for opaque input.
    assert_eq!(&output[..2], &[0xFF, 0xD8], "expected JPEG for opaque input");
}

#[test]
fn zen_build_passthrough_no_ops() {
    let jpeg_bytes = make_test_jpeg(100, 100);
    let hex_input = hex::encode(&jpeg_bytes);

    // Decode + encode with no processing steps.
    let build_request = serde_json::json!({
        "io": [
            {"io_id": 0, "direction": "in", "io": {"bytes_hex": hex_input}},
            {"io_id": 1, "direction": "out", "io": "output_buffer"}
        ],
        "framewise": {
            "steps": [
                {"decode": {"io_id": 0}},
                {"encode": {"io_id": 1, "preset": {"mozjpeg": {"quality": 90}}}}
            ]
        }
    });

    let mut ctx = Context::create().unwrap();
    let _response = send_json(&mut ctx, "v1/zen-build", &build_request);

    let output = ctx.take_output_buffer(1).unwrap();
    assert!(output.len() > 50);

    let info = zencodecs::from_bytes(&output).unwrap();
    assert_eq!(info.width, 100);
    assert_eq!(info.height, 100);
}

// ─── Tests using zen_execute_1 (same pattern as integration test suite) ───

/// Execute through zen_execute_1 using the same Context IO pattern as the
/// existing integration test suite (add_copied_input_buffer + execute_1).
#[test]
fn zen_execute_1_resize() {
    let jpeg_bytes = make_test_jpeg(400, 300);

    let mut ctx = Context::create().unwrap();
    ctx.add_copied_input_buffer(0, &jpeg_bytes).unwrap();
    // Don't add output buffer — zen_execute_inner creates it.

    let result = ctx
        .zen_execute_1(s::Execute001 {
            framewise: s::Framewise::Steps(vec![
                s::Node::Decode { io_id: 0, commands: None },
                s::Node::Constrain(s::Constraint {
                    mode: s::ConstraintMode::Within,
                    w: Some(200),
                    h: Some(150),
                    hints: None,
                    gravity: None,
                    canvas_color: None,
                }),
                s::Node::Encode {
                    io_id: 1,
                    preset: s::EncoderPreset::Mozjpeg {
                        quality: Some(80),
                        progressive: Some(true),
                        matte: None,
                    },
                },
            ]),
            graph_recording: None,
            security: None,
        })
        .unwrap();

    // Verify we got a JobResult.
    match result {
        s::ResponsePayload::JobResult(jr) => {
            assert_eq!(jr.encodes.len(), 1);
            assert!(jr.encodes[0].w <= 200);
            assert!(jr.encodes[0].h <= 150);
        }
        other => panic!("expected JobResult, got {other:?}"),
    }

    // Verify output buffer is accessible.
    let output = ctx.take_output_buffer(1).unwrap();
    assert!(output.len() > 100);
    assert_eq!(&output[..2], &[0xFF, 0xD8]);
}

#[test]
fn zen_execute_1_flip_rotate() {
    let jpeg_bytes = make_test_jpeg(200, 100);

    let mut ctx = Context::create().unwrap();
    ctx.add_copied_input_buffer(0, &jpeg_bytes).unwrap();
    // Don't add output buffer — zen_execute_inner creates it.

    let result = ctx
        .zen_execute_1(s::Execute001 {
            framewise: s::Framewise::Steps(vec![
                s::Node::Decode { io_id: 0, commands: None },
                s::Node::FlipH,
                s::Node::Rotate90,
                s::Node::Encode {
                    io_id: 1,
                    preset: s::EncoderPreset::Mozjpeg {
                        quality: Some(85),
                        progressive: None,
                        matte: None,
                    },
                },
            ]),
            graph_recording: None,
            security: None,
        })
        .unwrap();

    match result {
        s::ResponsePayload::JobResult(jr) => {
            assert_eq!(jr.encodes.len(), 1);
            // 200x100 rotated 90° → 100x200.
            assert_eq!(jr.encodes[0].w, 100);
            assert_eq!(jr.encodes[0].h, 200);
        }
        other => panic!("expected JobResult, got {other:?}"),
    }
}

/// Red square watermark over green canvas — easy to verify visually.
/// Tests that the Materialize-based watermark compositing actually modifies pixels.
#[test]
fn zen_watermark_red_on_green() {
    use std::collections::HashMap;

    // Create a 200x200 solid green PNG.
    fn make_solid_png(w: u32, h: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        let descriptor = zenpixels::PixelDescriptor::RGBA8_SRGB;
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        for i in 0..(w * h) as usize {
            pixels[i * 4] = r;
            pixels[i * 4 + 1] = g;
            pixels[i * 4 + 2] = b;
            pixels[i * 4 + 3] = 255;
        }
        let stride = (w * 4) as usize;
        let ps = zenpixels::PixelSlice::new(&pixels, w, h, stride, descriptor).unwrap();
        zencodecs::EncodeRequest::new(zencodecs::ImageFormat::Png)
            .with_lossless(true)
            .encode(ps, true)
            .unwrap()
            .into_vec()
    }

    let green_png = make_solid_png(200, 200, 0, 255, 0);
    let red_png = make_solid_png(50, 50, 255, 0, 0);

    let mut io_buffers = HashMap::new();
    io_buffers.insert(0, green_png);
    io_buffers.insert(1, red_png);

    let steps = vec![
        s::Node::Decode { io_id: 0, commands: None },
        s::Node::Watermark(s::Watermark {
            io_id: 1,
            gravity: Some(s::ConstraintGravity::Center),
            fit_box: None,
            fit_mode: Some(s::WatermarkConstraintMode::Within),
            min_canvas_width: None,
            min_canvas_height: None,
            opacity: Some(1.0),
            hints: None,
        }),
        s::Node::Encode {
            io_id: 2,
            preset: s::EncoderPreset::Libpng { depth: None, matte: None, zlib_compression: None },
        },
    ];

    let framewise = s::Framewise::Steps(steps);
    let security = s::ExecutionSecurity::sane_defaults();

    let result =
        imageflow_core::zen::execute_framewise(&framewise, &io_buffers, &security).unwrap();
    assert_eq!(result.encode_results.len(), 1);

    let output = &result.encode_results[0];
    assert_eq!(output.width, 200);
    assert_eq!(output.height, 200);

    // Save for visual inspection
    std::fs::write("/tmp/wm_red_on_green.png", &output.bytes).unwrap();
    eprintln!("Wrote /tmp/wm_red_on_green.png ({} bytes)", output.bytes.len());

    // Decode the output via v2 context (handles palette PNGs correctly).
    // zencodecs' direct decoder may not expand 1-bit palette to RGBA.
    let mut decode_ctx = Context::create().unwrap();
    decode_ctx.add_copied_input_buffer(0, &output.bytes).unwrap();
    let capture_id = 0;
    decode_ctx
        .execute_1(s::Execute001 {
            framewise: s::Framewise::Steps(vec![
                s::Node::Decode { io_id: 0, commands: None },
                s::Node::CaptureBitmapKey { capture_id },
            ]),
            graph_recording: None,
            security: None,
        })
        .unwrap();

    let bitmaps = decode_ctx.borrow_bitmaps().unwrap();
    let bitmap_key = decode_ctx.get_captured_bitmap_key(capture_id).unwrap();
    let mut bm = bitmaps.try_borrow_mut(bitmap_key).unwrap();
    let window = bm.get_window_u8().unwrap();
    let _w = window.w() as usize;
    let _h = window.h() as usize;

    // v2 decodes to BGRA, so channels are B=0, G=1, R=2, A=3
    let row_100 = window.row(100).unwrap();
    let center_b = row_100[100 * 4];
    let center_g = row_100[100 * 4 + 1];
    let center_r = row_100[100 * 4 + 2];
    eprintln!("Center pixel (100,100): R={center_r} G={center_g} B={center_b}");

    let row_0 = window.row(0).unwrap();
    let corner_b = row_0[0];
    let corner_g = row_0[1];
    let corner_r = row_0[2];
    eprintln!("Corner pixel (0,0): R={corner_r} G={corner_g} B={corner_b}");

    // The center should be red (watermark was composited)
    assert!(center_r > 200, "center R={center_r}, expected >200 (red watermark)");
    assert!(center_g < 50, "center G={center_g}, expected <50 (red watermark)");

    // The corner should be green (untouched)
    assert!(corner_g > 200, "corner G={corner_g}, expected >200 (green canvas)");
    assert!(corner_r < 50, "corner R={corner_r}, expected <50 (green canvas)");
}

/// Red 50% alpha watermark over blue background — tests alpha compositing.
/// Runs both v2 and zen, compares pixel-for-pixel.
#[test]
fn zen_watermark_red_alpha_on_blue() {
    use std::collections::HashMap;

    fn make_solid_png(w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Vec<u8> {
        let descriptor = zenpixels::PixelDescriptor::RGBA8_SRGB;
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        for i in 0..(w * h) as usize {
            pixels[i * 4] = r;
            pixels[i * 4 + 1] = g;
            pixels[i * 4 + 2] = b;
            pixels[i * 4 + 3] = a;
        }
        let stride = (w * 4) as usize;
        let ps = zenpixels::PixelSlice::new(&pixels, w, h, stride, descriptor).unwrap();
        zencodecs::EncodeRequest::new(zencodecs::ImageFormat::Png)
            .with_lossless(true)
            .encode(ps, true)
            .unwrap()
            .into_vec()
    }

    let blue_png = make_solid_png(200, 200, 0, 0, 255, 255);
    let red_half_png = make_solid_png(200, 200, 255, 0, 0, 128); // 50% alpha

    // --- Zen pipeline ---
    let mut zen_io = HashMap::new();
    zen_io.insert(0, blue_png.clone());
    zen_io.insert(1, red_half_png.clone());

    let steps = vec![
        s::Node::Decode { io_id: 0, commands: None },
        s::Node::Watermark(s::Watermark {
            io_id: 1,
            gravity: Some(s::ConstraintGravity::Center),
            fit_box: None,
            fit_mode: Some(s::WatermarkConstraintMode::Within),
            min_canvas_width: None,
            min_canvas_height: None,
            opacity: Some(1.0),
            hints: None,
        }),
        s::Node::Encode {
            io_id: 2,
            preset: s::EncoderPreset::Libpng { depth: None, matte: None, zlib_compression: None },
        },
    ];

    let zen_result = imageflow_core::zen::execute_framewise(
        &s::Framewise::Steps(steps.clone()),
        &zen_io,
        &s::ExecutionSecurity::sane_defaults(),
    )
    .unwrap();

    let zen_bytes = &zen_result.encode_results[0].bytes;

    // --- V2 pipeline ---
    let mut v2_ctx = Context::create().unwrap();
    v2_ctx.force_backend = Some(imageflow_core::Backend::V2);
    v2_ctx.add_copied_input_buffer(0, &blue_png).unwrap();
    v2_ctx.add_copied_input_buffer(1, &red_half_png).unwrap();
    v2_ctx.add_output_buffer(2).unwrap();
    v2_ctx
        .execute_1(s::Execute001 {
            framewise: s::Framewise::Steps(steps),
            graph_recording: None,
            security: None,
        })
        .unwrap();
    let v2_bytes = v2_ctx.take_output_buffer(2).unwrap();

    // Decode both with v2 decoder (handles palette PNGs)
    fn decode_bgra(ctx_bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut ctx = Context::create().unwrap();
        ctx.add_copied_input_buffer(0, ctx_bytes).unwrap();
        let result = ctx
            .execute_1(s::Execute001 {
                framewise: s::Framewise::Steps(vec![
                    s::Node::Decode { io_id: 0, commands: None },
                    s::Node::CaptureBitmapKey { capture_id: 0 },
                ]),
                graph_recording: None,
                security: None,
            })
            .unwrap();
        let bitmaps = ctx.borrow_bitmaps().unwrap();
        let key = ctx.get_captured_bitmap_key(0).unwrap();
        let mut bm = bitmaps.try_borrow_mut(key).unwrap();
        let window = bm.get_window_u8().unwrap();
        let w = window.w() as u32;
        let h = window.h() as u32;
        let mut data = Vec::new();
        for y in 0..h as usize {
            let row = window.row(y).unwrap();
            data.extend_from_slice(&row[..w as usize * 4]);
        }
        (w, h, data)
    }

    let (zw, zh, zen_pixels) = decode_bgra(zen_bytes);
    let (vw, vh, v2_pixels) = decode_bgra(&v2_bytes);

    eprintln!("Zen: {zw}x{zh}, V2: {vw}x{vh}");

    // Sample center pixel (BGRA layout)
    let zen_center = 100 * zw as usize * 4 + 100 * 4;
    let v2_center = 100 * vw as usize * 4 + 100 * 4;

    let (zb, zg, zr, za) = (
        zen_pixels[zen_center],
        zen_pixels[zen_center + 1],
        zen_pixels[zen_center + 2],
        zen_pixels[zen_center + 3],
    );
    let (vb, vg, vr, va) = (
        v2_pixels[v2_center],
        v2_pixels[v2_center + 1],
        v2_pixels[v2_center + 2],
        v2_pixels[v2_center + 3],
    );

    eprintln!("Center pixel (100,100):");
    eprintln!("  V2:  R={vr} G={vg} B={vb} A={va}");
    eprintln!("  Zen: R={zr} G={zg} B={zb} A={za}");
    eprintln!(
        "  Delta: R={} G={} B={} A={}",
        (zr as i16 - vr as i16).abs(),
        (zg as i16 - vg as i16).abs(),
        (zb as i16 - vb as i16).abs(),
        (za as i16 - va as i16).abs(),
    );

    // Expected: red(255,0,0) at 50% alpha over blue(0,0,255)
    // Porter-Duff source-over in sRGB: out = src*sa + dst*(1-sa)
    // In linear: linearize both, blend, delinearize
    // The exact values depend on whether compositing is in sRGB or linear
    eprintln!("Expected (sRGB space):  R={} G=0 B={}", 255 * 128 / 255, 255 * 127 / 255);

    // Assert they're close (within 2)
    assert!(
        (zr as i16 - vr as i16).abs() <= 2
            && (zg as i16 - vg as i16).abs() <= 2
            && (zb as i16 - vb as i16).abs() <= 2,
        "Zen and V2 center pixels differ by more than 2:\n  V2:  R={vr} G={vg} B={vb}\n  Zen: R={zr} G={zg} B={zb}"
    );
}

/// Full-frame watermark WITH resize — 100x100 red at 50% alpha resized to cover 400x300 blue canvas.
/// This exercises the watermark resize path (zenresize Robidoux) + compositing.
#[test]
fn zen_watermark_fullframe_resized() {
    use std::collections::HashMap;

    fn make_solid_png(w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Vec<u8> {
        let descriptor = zenpixels::PixelDescriptor::RGBA8_SRGB;
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        for i in 0..(w * h) as usize {
            pixels[i * 4] = r;
            pixels[i * 4 + 1] = g;
            pixels[i * 4 + 2] = b;
            pixels[i * 4 + 3] = a;
        }
        let stride = (w * 4) as usize;
        let ps = zenpixels::PixelSlice::new(&pixels, w, h, stride, descriptor).unwrap();
        zencodecs::EncodeRequest::new(zencodecs::ImageFormat::Png)
            .with_lossless(true)
            .encode(ps, true)
            .unwrap()
            .into_vec()
    }

    // Blue canvas 400x300, red watermark 100x100 at 50% alpha — will be resized to fill
    let blue_png = make_solid_png(400, 300, 0, 0, 255, 255);
    let red_png = make_solid_png(100, 100, 255, 0, 0, 128);

    let steps = vec![
        s::Node::Decode { io_id: 0, commands: None },
        s::Node::Watermark(s::Watermark {
            io_id: 1,
            gravity: Some(s::ConstraintGravity::Center),
            fit_box: None,                                       // full canvas
            fit_mode: Some(s::WatermarkConstraintMode::Distort), // force exact fill
            min_canvas_width: None,
            min_canvas_height: None,
            opacity: Some(1.0),
            hints: None,
        }),
        s::Node::Encode {
            io_id: 2,
            preset: s::EncoderPreset::Libpng { depth: None, matte: None, zlib_compression: None },
        },
    ];

    // --- Zen ---
    let mut zen_io = HashMap::new();
    zen_io.insert(0, blue_png.clone());
    zen_io.insert(1, red_png.clone());
    let zen_result = imageflow_core::zen::execute_framewise(
        &s::Framewise::Steps(steps.clone()),
        &zen_io,
        &s::ExecutionSecurity::sane_defaults(),
    )
    .unwrap();
    let zen_bytes = &zen_result.encode_results[0].bytes;

    // --- V2 ---
    let mut v2_ctx = Context::create().unwrap();
    v2_ctx.force_backend = Some(imageflow_core::Backend::V2);
    v2_ctx.add_copied_input_buffer(0, &blue_png).unwrap();
    v2_ctx.add_copied_input_buffer(1, &red_png).unwrap();
    v2_ctx.add_output_buffer(2).unwrap();
    v2_ctx
        .execute_1(s::Execute001 {
            framewise: s::Framewise::Steps(steps),
            graph_recording: None,
            security: None,
        })
        .unwrap();
    let v2_bytes = v2_ctx.take_output_buffer(2).unwrap();

    // Decode both
    fn decode_bgra(ctx_bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut ctx = Context::create().unwrap();
        ctx.add_copied_input_buffer(0, ctx_bytes).unwrap();
        ctx.execute_1(s::Execute001 {
            framewise: s::Framewise::Steps(vec![
                s::Node::Decode { io_id: 0, commands: None },
                s::Node::CaptureBitmapKey { capture_id: 0 },
            ]),
            graph_recording: None,
            security: None,
        })
        .unwrap();
        let bitmaps = ctx.borrow_bitmaps().unwrap();
        let key = ctx.get_captured_bitmap_key(0).unwrap();
        let mut bm = bitmaps.try_borrow_mut(key).unwrap();
        let window = bm.get_window_u8().unwrap();
        let w = window.w() as u32;
        let h = window.h() as u32;
        let mut data = Vec::new();
        for y in 0..h as usize {
            let row = window.row(y).unwrap();
            data.extend_from_slice(&row[..w as usize * 4]);
        }
        (w, h, data)
    }

    let (zw, zh, zen_px) = decode_bgra(zen_bytes);
    let (vw, vh, v2_px) = decode_bgra(&v2_bytes);
    eprintln!("Zen: {zw}x{zh}, V2: {vw}x{vh}");
    assert_eq!((zw, zh), (vw, vh), "dimensions differ");

    // Compare every pixel, find max delta
    let mut max_dr = 0i16;
    let mut max_dg = 0i16;
    let mut max_db = 0i16;
    let mut max_da = 0i16;
    let mut diff_count = 0u32;
    let total = (zw * zh) as usize;
    for i in 0..total {
        let off = i * 4;
        // BGRA layout
        let dr = (zen_px[off + 2] as i16 - v2_px[off + 2] as i16).abs();
        let dg = (zen_px[off + 1] as i16 - v2_px[off + 1] as i16).abs();
        let db = (zen_px[off] as i16 - v2_px[off] as i16).abs();
        let da = (zen_px[off + 3] as i16 - v2_px[off + 3] as i16).abs();
        if dr > 0 || dg > 0 || db > 0 || da > 0 {
            diff_count += 1;
        }
        max_dr = max_dr.max(dr);
        max_dg = max_dg.max(dg);
        max_db = max_db.max(db);
        max_da = max_da.max(da);
    }

    // Sample corners and center
    let sample = |x: usize, y: usize| {
        let off = (y * zw as usize + x) * 4;
        let (zb, zg, zr, za) = (zen_px[off], zen_px[off + 1], zen_px[off + 2], zen_px[off + 3]);
        let (vb, vg, vr, va) = (v2_px[off], v2_px[off + 1], v2_px[off + 2], v2_px[off + 3]);
        eprintln!(
            "  ({x},{y}): V2=({vr},{vg},{vb},{va}) Zen=({zr},{zg},{zb},{za}) Δ=({},{},{},{})",
            (zr as i16 - vr as i16).abs(),
            (zg as i16 - vg as i16).abs(),
            (zb as i16 - vb as i16).abs(),
            (za as i16 - va as i16).abs()
        );
    };

    eprintln!("Max delta: R={max_dr} G={max_dg} B={max_db} A={max_da}");
    eprintln!(
        "Pixels differing: {diff_count}/{total} ({:.1}%)",
        diff_count as f64 / total as f64 * 100.0
    );
    eprintln!("Pixel samples:");
    sample(0, 0);
    sample(200, 150);
    sample(399, 299);
    sample(50, 50);
    sample(350, 250);

    assert!(
        max_dr <= 2 && max_dg <= 2 && max_db <= 2,
        "Full-frame resized watermark: max delta R={max_dr} G={max_dg} B={max_db} exceeds 2"
    );
}
