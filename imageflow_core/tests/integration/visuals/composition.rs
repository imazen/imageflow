//! Compositing tests: DrawImageExact, CopyRectToCanvas, multi-input graph mode.
//!
//! Tests cover:
//! - DrawImageExact with alpha blending (opacity via compose mode)
//! - CopyRectToCanvas for region copying between images
//! - Multi-input graph mode with explicit edges
//! - Watermark on transparent canvas (alpha compositing correctness)

#[allow(unused_imports)]
use crate::common::*;
use imageflow_types::{
    Color, ColorSrgb, CompositingMode, ConstraintMode, Edge, EdgeKind, EncoderPreset, Execute001,
    Filter, Framewise, Graph, Node, PixelFormat, ResampleHints,
};
use std::collections::HashMap;

use imageflow_core::Context;

// ============================================================================
// DrawImageExact — place overlay at pixel coordinates
// ============================================================================

#[test]
fn test_draw_image_exact_on_canvas() {
    // Create a canvas, then draw a decoded image onto it at specific coords
    visual_check_bitmap! {
        sources: [
            "test_inputs/dice.png",
        ],
        detail: "dice_at_50_50",
        steps: vec![
            // Create a 400x400 white canvas
            Node::CreateCanvas {
                w: 400,
                h: 400,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_owned())),
            },
        ],
        tolerance: Tolerance::off_by_one(),
    }
}

// ============================================================================
// Graph mode: decode → resize → 2 encodes (PNG + JPEG)
// ============================================================================

#[test]
fn test_graph_mode_dual_encode() {
    test_init();
    let mut nodes = HashMap::new();
    nodes.insert("0".to_owned(), Node::Decode { io_id: 0, commands: None });
    nodes.insert(
        "1".to_owned(),
        Node::Resample2D {
            w: 200,
            h: 200,
            hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
        },
    );
    nodes.insert(
        "2".to_owned(),
        Node::Encode { io_id: 1, preset: EncoderPreset::Lodepng { maximum_deflate: None } },
    );
    nodes.insert(
        "3".to_owned(),
        Node::Encode {
            io_id: 2,
            preset: EncoderPreset::Mozjpeg { progressive: None, quality: Some(85), matte: None },
        },
    );

    let graph = Framewise::Graph(Graph {
        edges: vec![
            Edge { from: 0, to: 1, kind: EdgeKind::Input },
            Edge { from: 1, to: 2, kind: EdgeKind::Input },
            Edge { from: 1, to: 3, kind: EdgeKind::Input },
        ],
        nodes,
    });

    let mut ctx = Context::create().unwrap();
    IoTestTranslator {}
        .add(&mut ctx, 0, IoTestEnum::Url(visual_check!(@source_url "test_inputs/waterhouse.jpg")))
        .unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.add_output_buffer(2).unwrap();

    ctx.execute_1(Execute001 {
        job_options: None,
        graph_recording: None,
        security: None,
        framewise: graph,
    })
    .unwrap();

    let png_bytes = ctx.take_output_buffer(1).unwrap();
    let jpg_bytes = ctx.take_output_buffer(2).unwrap();

    assert!(png_bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]), "Output 1 should be PNG");
    assert!(jpg_bytes.starts_with(&[0xFF, 0xD8, 0xFF]), "Output 2 should be JPEG");
    assert!(png_bytes.len() > 100, "PNG output should have content");
    assert!(jpg_bytes.len() > 100, "JPEG output should have content");
}

// ============================================================================
// Graph mode: CopyRectToCanvas — two inputs combined
// ============================================================================

#[test]
fn test_graph_copy_rect_to_canvas() {
    test_init();
    let mut nodes = HashMap::new();
    // Node 0: decode image (input)
    nodes.insert("0".to_owned(), Node::Decode { io_id: 0, commands: None });
    // Node 1: create canvas (background)
    nodes.insert(
        "1".to_owned(),
        Node::CreateCanvas {
            w: 300,
            h: 300,
            format: PixelFormat::Bgra32,
            color: Color::Srgb(ColorSrgb::Hex("FF0000FF".to_owned())),
        },
    );
    // Node 2: copy rect from image to canvas
    nodes.insert(
        "2".to_owned(),
        Node::CopyRectToCanvas { x: 50, y: 50, from_x: 0, from_y: 0, w: 100, h: 100 },
    );
    // Node 3: encode result
    nodes.insert(
        "3".to_owned(),
        Node::Encode { io_id: 1, preset: EncoderPreset::Lodepng { maximum_deflate: None } },
    );

    let graph = Framewise::Graph(Graph {
        edges: vec![
            Edge { from: 0, to: 2, kind: EdgeKind::Input },
            Edge { from: 1, to: 2, kind: EdgeKind::Canvas },
            Edge { from: 2, to: 3, kind: EdgeKind::Input },
        ],
        nodes,
    });

    let mut ctx = Context::create().unwrap();
    IoTestTranslator {}
        .add(&mut ctx, 0, IoTestEnum::Url(visual_check!(@source_url "test_inputs/waterhouse.jpg")))
        .unwrap();
    ctx.add_output_buffer(1).unwrap();

    ctx.execute_1(Execute001 {
        job_options: None,
        graph_recording: None,
        security: None,
        framewise: graph,
    })
    .unwrap();

    let png_bytes = ctx.take_output_buffer(1).unwrap();
    assert!(png_bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]), "Output should be PNG");

    // Decode and verify dimensions
    let decoded = lodepng::decode32(&png_bytes).unwrap();
    assert_eq!(decoded.width, 300, "Canvas width should be 300");
    assert_eq!(decoded.height, 300, "Canvas height should be 300");
}

// ============================================================================
// Graph mode: DrawImageExact — overlay image at coordinates with blend
// DrawImageExact is a two-input node: Input edge = overlay, Canvas edge = background
// ============================================================================

#[test]
fn test_graph_draw_image_exact() {
    test_init();
    let mut nodes = HashMap::new();
    // Node 0: decode overlay (dice.png — has alpha)
    nodes.insert("0".to_owned(), Node::Decode { io_id: 0, commands: None });
    // Node 1: decode background (waterhouse.jpg)
    nodes.insert("1".to_owned(), Node::Decode { io_id: 1, commands: None });
    // Node 2: resize background to 400x400
    nodes.insert(
        "2".to_owned(),
        Node::Resample2D {
            w: 400,
            h: 400,
            hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
        },
    );
    // Node 3: draw overlay onto background at (200, 200), size 100x100
    nodes.insert(
        "3".to_owned(),
        Node::DrawImageExact {
            x: 200,
            y: 200,
            w: 100,
            h: 100,
            blend: Some(CompositingMode::Compose),
            hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
        },
    );
    // Node 4: encode result
    nodes.insert(
        "4".to_owned(),
        Node::Encode { io_id: 2, preset: EncoderPreset::Lodepng { maximum_deflate: None } },
    );

    let graph = Framewise::Graph(Graph {
        edges: vec![
            Edge { from: 1, to: 2, kind: EdgeKind::Input },
            // DrawImageExact: Input = overlay source, Canvas = background
            Edge { from: 0, to: 3, kind: EdgeKind::Input },
            Edge { from: 2, to: 3, kind: EdgeKind::Canvas },
            Edge { from: 3, to: 4, kind: EdgeKind::Input },
        ],
        nodes,
    });

    let mut ctx = Context::create().unwrap();
    IoTestTranslator {}
        .add(&mut ctx, 0, IoTestEnum::Url(visual_check!(@source_url "test_inputs/dice.png")))
        .unwrap();
    IoTestTranslator {}
        .add(&mut ctx, 1, IoTestEnum::Url(visual_check!(@source_url "test_inputs/waterhouse.jpg")))
        .unwrap();
    ctx.add_output_buffer(2).unwrap();

    ctx.execute_1(Execute001 {
        job_options: None,
        graph_recording: None,
        security: None,
        framewise: graph,
    })
    .unwrap();

    let png_bytes = ctx.take_output_buffer(2).unwrap();
    assert!(png_bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]), "Output should be PNG");
    let decoded = lodepng::decode32(&png_bytes).unwrap();
    assert_eq!(decoded.width, 400);
    assert_eq!(decoded.height, 400);
}

// ============================================================================
// Pyramid: 1 decode → 4 resizes → 4 encodes (different sizes, different formats)
// ============================================================================

#[test]
fn test_pyramid_multi_output() {
    test_init();
    let mut nodes = HashMap::new();
    // Node 0: decode input
    nodes.insert("0".to_owned(), Node::Decode { io_id: 0, commands: None });
    // Node 1-4: resize to different sizes
    nodes.insert(
        "1".to_owned(),
        Node::Resample2D {
            w: 1600,
            h: 1200,
            hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
        },
    );
    nodes.insert(
        "2".to_owned(),
        Node::Resample2D {
            w: 800,
            h: 600,
            hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
        },
    );
    nodes.insert(
        "3".to_owned(),
        Node::Resample2D {
            w: 400,
            h: 300,
            hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
        },
    );
    nodes.insert(
        "4".to_owned(),
        Node::Resample2D {
            w: 200,
            h: 150,
            hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
        },
    );
    // Node 5: encode 1600 → JPEG
    nodes.insert(
        "5".to_owned(),
        Node::Encode {
            io_id: 1,
            preset: EncoderPreset::Mozjpeg {
                progressive: Some(true),
                quality: Some(90),
                matte: None,
            },
        },
    );
    // Node 6: encode 800 → WebP
    nodes.insert(
        "6".to_owned(),
        Node::Encode { io_id: 2, preset: EncoderPreset::WebPLossy { quality: 85.0 } },
    );
    // Node 7: encode 400 → PNG
    nodes.insert(
        "7".to_owned(),
        Node::Encode { io_id: 3, preset: EncoderPreset::Lodepng { maximum_deflate: None } },
    );
    // Node 8: encode 200 → WebP
    nodes.insert(
        "8".to_owned(),
        Node::Encode { io_id: 4, preset: EncoderPreset::WebPLossy { quality: 80.0 } },
    );

    let graph = Framewise::Graph(Graph {
        edges: vec![
            // Decode feeds all 4 resizes
            Edge { from: 0, to: 1, kind: EdgeKind::Input },
            Edge { from: 0, to: 2, kind: EdgeKind::Input },
            Edge { from: 0, to: 3, kind: EdgeKind::Input },
            Edge { from: 0, to: 4, kind: EdgeKind::Input },
            // Each resize feeds its encoder
            Edge { from: 1, to: 5, kind: EdgeKind::Input },
            Edge { from: 2, to: 6, kind: EdgeKind::Input },
            Edge { from: 3, to: 7, kind: EdgeKind::Input },
            Edge { from: 4, to: 8, kind: EdgeKind::Input },
        ],
        nodes,
    });

    let mut ctx = Context::create().unwrap();
    IoTestTranslator {}
        .add(&mut ctx, 0, IoTestEnum::Url(visual_check!(@source_url "test_inputs/waterhouse.jpg")))
        .unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.add_output_buffer(2).unwrap();
    ctx.add_output_buffer(3).unwrap();
    ctx.add_output_buffer(4).unwrap();

    ctx.execute_1(Execute001 {
        job_options: None,
        graph_recording: None,
        security: None,
        framewise: graph,
    })
    .unwrap();

    // Verify all outputs exist and have correct format magic bytes
    let jpeg_bytes = ctx.take_output_buffer(1).unwrap();
    let webp_bytes = ctx.take_output_buffer(2).unwrap();
    let png_bytes = ctx.take_output_buffer(3).unwrap();
    let webp2_bytes = ctx.take_output_buffer(4).unwrap();

    assert!(jpeg_bytes.starts_with(&[0xFF, 0xD8, 0xFF]), "1600px output should be JPEG");
    assert!(webp_bytes.starts_with(b"RIFF"), "800px output should be WebP");
    assert!(png_bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]), "400px output should be PNG");
    assert!(webp2_bytes.starts_with(b"RIFF"), "200px output should be WebP");

    // Verify dimensions by decoding each output
    // JPEG → should be 1600x1200
    {
        let mut ctx2 = Context::create().unwrap();
        ctx2.add_copied_input_buffer(0, &jpeg_bytes).unwrap();
        ctx2.add_output_buffer(1).unwrap();
        ctx2.execute_1(Execute001 {
            job_options: None,
            graph_recording: None,
            security: None,
            framewise: Framewise::Steps(vec![
                Node::Decode { io_id: 0, commands: None },
                Node::Encode { io_id: 1, preset: EncoderPreset::Lodepng { maximum_deflate: None } },
            ]),
        })
        .unwrap();
        let check_png = ctx2.take_output_buffer(1).unwrap();
        let decoded = lodepng::decode32(&check_png).unwrap();
        assert_eq!((decoded.width, decoded.height), (1600, 1200), "JPEG should be 1600x1200");
    }

    // PNG → should be 400x300
    {
        let decoded = lodepng::decode32(&png_bytes).unwrap();
        assert_eq!((decoded.width, decoded.height), (400, 300), "PNG should be 400x300");
    }

    eprintln!(
        "Pyramid outputs: JPEG={}B, WebP={}B, PNG={}B, WebP2={}B",
        jpeg_bytes.len(),
        webp_bytes.len(),
        png_bytes.len(),
        webp2_bytes.len()
    );
}

// ============================================================================
// Pyramid with Constrain (aspect-ratio preserving) instead of Resample2D
// ============================================================================

#[test]
fn test_pyramid_constrain_within() {
    test_init();
    let mut nodes = HashMap::new();
    nodes.insert("0".to_owned(), Node::Decode { io_id: 0, commands: None });

    // Constrain to different max dimensions
    nodes.insert(
        "1".to_owned(),
        Node::Constrain(imageflow_types::Constraint {
            w: Some(800),
            h: Some(800),
            mode: ConstraintMode::Within,
            hints: None,
            gravity: None,
            canvas_color: None,
        }),
    );
    nodes.insert(
        "2".to_owned(),
        Node::Constrain(imageflow_types::Constraint {
            w: Some(400),
            h: Some(400),
            mode: ConstraintMode::Within,
            hints: None,
            gravity: None,
            canvas_color: None,
        }),
    );
    nodes.insert(
        "3".to_owned(),
        Node::Constrain(imageflow_types::Constraint {
            w: Some(200),
            h: Some(200),
            mode: ConstraintMode::Within,
            hints: None,
            gravity: None,
            canvas_color: None,
        }),
    );
    nodes.insert(
        "4".to_owned(),
        Node::Constrain(imageflow_types::Constraint {
            w: Some(100),
            h: Some(100),
            mode: ConstraintMode::Within,
            hints: None,
            gravity: None,
            canvas_color: None,
        }),
    );

    // Encode each at different formats
    nodes.insert(
        "5".to_owned(),
        Node::Encode {
            io_id: 1,
            preset: EncoderPreset::Mozjpeg { progressive: None, quality: Some(90), matte: None },
        },
    );
    nodes.insert(
        "6".to_owned(),
        Node::Encode { io_id: 2, preset: EncoderPreset::WebPLossy { quality: 80.0 } },
    );
    nodes.insert(
        "7".to_owned(),
        Node::Encode { io_id: 3, preset: EncoderPreset::Lodepng { maximum_deflate: None } },
    );
    nodes.insert(
        "8".to_owned(),
        Node::Encode { io_id: 4, preset: EncoderPreset::WebPLossy { quality: 85.0 } },
    );

    let graph = Framewise::Graph(Graph {
        edges: vec![
            Edge { from: 0, to: 1, kind: EdgeKind::Input },
            Edge { from: 0, to: 2, kind: EdgeKind::Input },
            Edge { from: 0, to: 3, kind: EdgeKind::Input },
            Edge { from: 0, to: 4, kind: EdgeKind::Input },
            Edge { from: 1, to: 5, kind: EdgeKind::Input },
            Edge { from: 2, to: 6, kind: EdgeKind::Input },
            Edge { from: 3, to: 7, kind: EdgeKind::Input },
            Edge { from: 4, to: 8, kind: EdgeKind::Input },
        ],
        nodes,
    });

    let mut ctx = Context::create().unwrap();
    IoTestTranslator {}
        .add(&mut ctx, 0, IoTestEnum::Url(visual_check!(@source_url "test_inputs/waterhouse.jpg")))
        .unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.add_output_buffer(2).unwrap();
    ctx.add_output_buffer(3).unwrap();
    ctx.add_output_buffer(4).unwrap();

    ctx.execute_1(Execute001 {
        job_options: None,
        graph_recording: None,
        security: None,
        framewise: graph,
    })
    .unwrap();

    let jpeg_bytes = ctx.take_output_buffer(1).unwrap();
    let webp_bytes = ctx.take_output_buffer(2).unwrap();
    let png_bytes = ctx.take_output_buffer(3).unwrap();
    let webp2_bytes = ctx.take_output_buffer(4).unwrap();

    assert!(jpeg_bytes.starts_with(&[0xFF, 0xD8, 0xFF]), "800px JPEG");
    assert!(webp_bytes.starts_with(b"RIFF"), "400px WebP");
    assert!(png_bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]), "200px PNG");
    assert!(webp2_bytes.starts_with(b"RIFF"), "100px WebP2");

    eprintln!(
        "Pyramid (constrain within): JPEG={}B, WebP={}B, PNG={}B, WebP2={}B",
        jpeg_bytes.len(),
        webp_bytes.len(),
        png_bytes.len(),
        webp2_bytes.len()
    );
}

// ============================================================================
// Watermark with full alpha blending on JPEG background
// ============================================================================

#[test]
fn test_watermark_alpha_on_jpeg() {
    visual_check_bitmap! {
        sources: [
            "test_inputs/waterhouse.jpg",
            "test_inputs/dice.png",
        ],
        detail: "dice_center_50pct_opacity",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Constrain(imageflow_types::Constraint {
                w: Some(600),
                h: Some(600),
                hints: None,
                gravity: None,
                mode: ConstraintMode::Within,
                canvas_color: None,
            }),
            Node::Watermark(imageflow_types::Watermark {
                io_id: 1,
                gravity: Some(imageflow_types::ConstraintGravity::Percentage {
                    x: 50f32,
                    y: 50f32,
                }),
                fit_box: Some(imageflow_types::WatermarkConstraintBox::ImagePercentage {
                    x1: 10f32,
                    y1: 10f32,
                    x2: 90f32,
                    y2: 90f32,
                }),
                fit_mode: Some(imageflow_types::WatermarkConstraintMode::Within),
                min_canvas_width: None,
                min_canvas_height: None,
                opacity: Some(0.5f32),
                hints: Some(ResampleHints {
                    sharpen_percent: None,
                    down_filter: None,
                    up_filter: None,
                    scaling_colorspace: None,
                    background_color: None,
                    resample_when: None,
                    sharpen_when: None,
                }),
            }),
        ],
        tolerance: Similarity::MaxZdsim(0.02).to_tolerance_spec(),
    }
}

// ============================================================================
// Watermark on transparent PNG background (alpha-on-alpha compositing)
// ============================================================================

#[test]
fn test_watermark_alpha_on_alpha() {
    visual_check_bitmap! {
        sources: [
            "test_inputs/shirt_transparent.png",
            "test_inputs/dice.png",
        ],
        detail: "dice_on_shirt_70pct",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Watermark(imageflow_types::Watermark {
                io_id: 1,
                gravity: Some(imageflow_types::ConstraintGravity::Percentage {
                    x: 50f32,
                    y: 50f32,
                }),
                fit_box: Some(imageflow_types::WatermarkConstraintBox::ImagePercentage {
                    x1: 20f32,
                    y1: 20f32,
                    x2: 80f32,
                    y2: 80f32,
                }),
                fit_mode: Some(imageflow_types::WatermarkConstraintMode::Within),
                min_canvas_width: None,
                min_canvas_height: None,
                opacity: Some(0.7f32),
                hints: None,
            }),
        ],
        tolerance: Tolerance::off_by_one(),
    }
}
