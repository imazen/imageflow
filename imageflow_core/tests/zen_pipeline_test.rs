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
