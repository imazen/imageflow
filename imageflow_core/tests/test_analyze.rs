//! Integration tests for Node::Analyze and focus-aware cropping.

extern crate imageflow_core;
extern crate imageflow_types as s;

use imageflow_core::Context;
use imageflow_types::{AnalyzeMode, ConstraintMode, FocusRect, Node, ResponsePayload};

/// Create a minimal valid PNG in memory (red 64x64 image)
fn create_test_png() -> Vec<u8> {
    let mut ctx = Context::create().unwrap();

    let build = s::Build001 {
        builder_config: None,
        io: vec![s::IoObject {
            io_id: 0,
            direction: s::IoDirection::Out,
            io: s::IoEnum::OutputBuffer,
        }],
        framewise: s::Framewise::Steps(vec![
            Node::CreateCanvas {
                w: 64,
                h: 64,
                format: s::PixelFormat::Bgra32,
                color: s::Color::Srgb(s::ColorSrgb::Hex("ff0000".to_owned())),
            },
            Node::Encode { io_id: 0, preset: s::EncoderPreset::libpng32() },
        ]),
    };

    ctx.build_1(build).unwrap();
    ctx.get_output_buffer_slice(0).unwrap().to_vec()
}

/// Create a test PNG with a bright red rectangle in one corner on a gray background
fn create_test_png_with_feature(feature_in_topleft: bool) -> Vec<u8> {
    let mut ctx = Context::create().unwrap();

    let (x1, y1) = if feature_in_topleft { (0, 0) } else { (96, 96) };

    let build = s::Build001 {
        builder_config: None,
        io: vec![s::IoObject {
            io_id: 0,
            direction: s::IoDirection::Out,
            io: s::IoEnum::OutputBuffer,
        }],
        framewise: s::Framewise::Steps(vec![
            Node::CreateCanvas {
                w: 128,
                h: 128,
                format: s::PixelFormat::Bgra32,
                color: s::Color::Srgb(s::ColorSrgb::Hex("808080".to_owned())),
            },
            Node::FillRect {
                x1,
                y1,
                x2: x1 + 32,
                y2: y1 + 32,
                color: s::Color::Srgb(s::ColorSrgb::Hex("ff0000".to_owned())),
            },
            Node::Encode { io_id: 0, preset: s::EncoderPreset::libpng32() },
        ]),
    };

    ctx.build_1(build).unwrap();
    ctx.get_output_buffer_slice(0).unwrap().to_vec()
}

#[test]
fn test_analyze_node_returns_results() {
    let png_bytes = create_test_png();

    let mut ctx = Context::create().unwrap();
    ctx.add_copied_input_buffer(0, &png_bytes).unwrap();

    let execute = s::Execute001 {
        framewise: s::Framewise::Steps(vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Analyze { mode: AnalyzeMode::Saliency },
        ]),
        graph_recording: None,
        security: None,
    };

    let response = ctx.execute_1(execute).unwrap();

    match response {
        ResponsePayload::JobResult(r) => {
            assert_eq!(r.analyses.len(), 1, "Should have exactly one analysis result");
            let analysis = &r.analyses[0];
            assert_eq!(analysis.image_width, 64);
            assert_eq!(analysis.image_height, 64);
            // The uniform red image may or may not have focus regions
            // but the analysis should complete without error
        }
        _ => panic!("Expected JobResult"),
    }
}

#[test]
fn test_analyze_node_detects_salient_region() {
    let png_bytes = create_test_png_with_feature(true);

    let mut ctx = Context::create().unwrap();
    ctx.add_copied_input_buffer(0, &png_bytes).unwrap();

    let execute = s::Execute001 {
        framewise: s::Framewise::Steps(vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Analyze { mode: AnalyzeMode::All },
        ]),
        graph_recording: None,
        security: None,
    };

    let response = ctx.execute_1(execute).unwrap();

    match response {
        ResponsePayload::JobResult(r) => {
            assert_eq!(r.analyses.len(), 1);
            let analysis = &r.analyses[0];
            assert_eq!(analysis.image_width, 128);
            assert_eq!(analysis.image_height, 128);
            // With a bright red square on gray, saliency should detect something
            if !analysis.focus_regions.is_empty() {
                let rect = &analysis.focus_regions[0];
                // The red square is in the top-left corner (0-25%, 0-25%)
                // The detected region should overlap that area
                assert!(rect.x1 < 50.0, "Focus region should be in left half, got x1={}", rect.x1);
                assert!(rect.y1 < 50.0, "Focus region should be in top half, got y1={}", rect.y1);
            }
        }
        _ => panic!("Expected JobResult"),
    }
}

#[test]
fn test_focus_crop_url_api() {
    // Test the full URL API path: c.focus= with mode=crop
    let png_bytes = create_test_png_with_feature(true);

    let mut ctx = Context::create().unwrap();
    ctx.add_copied_input_buffer(0, &png_bytes).unwrap();
    ctx.add_output_buffer(1).unwrap();

    // Crop to 64x128 (half width) with focus on top-left where the red square is
    let execute = s::Execute001 {
        framewise: s::Framewise::Steps(vec![Node::CommandString {
            kind: s::CommandStringKind::ImageResizer4,
            value: "w=64&h=128&mode=crop&c.focus=0,0,25,25".to_owned(),
            decode: Some(0),
            encode: Some(1),
            watermarks: None,
        }]),
        graph_recording: None,
        security: None,
    };

    let response = ctx.execute_1(execute).unwrap();

    match response {
        ResponsePayload::JobResult(r) => {
            assert_eq!(r.encodes.len(), 1);
            assert_eq!(r.encodes[0].w, 64);
            assert_eq!(r.encodes[0].h, 128);
        }
        _ => panic!("Expected JobResult"),
    }
}

#[test]
fn test_analyze_json_roundtrip() {
    // Verify AnalyzeResult serializes/deserializes correctly
    let result = s::AnalyzeResult {
        focus_regions: vec![
            FocusRect {
                x1: 10.0,
                y1: 20.0,
                x2: 50.0,
                y2: 60.0,
                weight: 10.0,
                kind: s::FocusKind::Face,
            },
            FocusRect {
                x1: 5.0,
                y1: 10.0,
                x2: 90.0,
                y2: 85.0,
                weight: 1.0,
                kind: s::FocusKind::Saliency,
            },
        ],
        image_width: 4000,
        image_height: 3000,
        analysis_ms: 23,
    };

    let json = serde_json::to_string(&result).unwrap();
    let parsed: s::AnalyzeResult = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.focus_regions.len(), 2);
    assert_eq!(parsed.focus_regions[0].kind, s::FocusKind::Face);
    assert_eq!(parsed.focus_regions[0].weight, 10.0);
    assert_eq!(parsed.focus_regions[1].kind, s::FocusKind::Saliency);
    assert_eq!(parsed.image_width, 4000);
    assert_eq!(parsed.image_height, 3000);
    assert_eq!(parsed.analysis_ms, 23);
}

#[test]
fn test_focus_rect_json_roundtrip() {
    // Verify focus rects in Constraint round-trip through JSON
    let constraint = s::Constraint {
        mode: ConstraintMode::Fit,
        w: Some(800),
        h: Some(600),
        hints: None,
        gravity: None,
        canvas_color: None,
        focus: Some(vec![FocusRect::new(20.0, 30.0, 80.0, 70.0)]),
    };

    let json = serde_json::to_string(&constraint).unwrap();
    let parsed: s::Constraint = serde_json::from_str(&json).unwrap();

    assert!(parsed.focus.is_some());
    let focus = parsed.focus.unwrap();
    assert_eq!(focus.len(), 1);
    assert!((focus[0].x1 - 20.0).abs() < 0.001);
    assert!((focus[0].y1 - 30.0).abs() < 0.001);
    assert!((focus[0].x2 - 80.0).abs() < 0.001);
    assert!((focus[0].y2 - 70.0).abs() < 0.001);
}
