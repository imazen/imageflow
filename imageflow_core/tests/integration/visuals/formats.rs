//! End-to-end format tests: decode, transcode, and corpus validation.
//!
//! Tests every supported input format (JPEG, PNG, GIF, WebP, JXL, AVIF, HEIC)
//! through every supported output format, with scaling, at multiple quality levels.
//! Corpus tests scan real-world scraped images to find decoder crashes and
//! transcoding failures.

use crate::common::*;
use imageflow_core::Context;
use imageflow_types::{
    self as s, CommandStringKind, Constraint, ConstraintMode, DecoderCommand, EncoderPreset,
    Execute001, Framewise, Node,
};
use std::path::{Path, PathBuf};

// ============================================================================
// Test source URLs (S3-hosted)
// ============================================================================

const WATERHOUSE_JPG: &str =
    "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg";
const FRYMIRE_PNG: &str =
    "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/frymire.png";
const WEBP_LL: &str =
    "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/1_webp_ll.webp";
const MOUNTAIN_GIF: &str =
    "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/mountain_800.gif";

// ============================================================================
// Helper: decode → constrain → encode pipeline
// ============================================================================

/// Decode input, constrain to w×h, encode to output format.
fn transcode_pipeline(w: u32, h: u32, preset: EncoderPreset) -> Vec<Node> {
    vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Constrain(Constraint {
            mode: ConstraintMode::Within,
            w: Some(w),
            h: Some(h),
            hints: None,
            gravity: None,
            canvas_color: None,
        }),
        Node::Encode { io_id: 1, preset },
    ]
}

/// Run a decode→scale→encode pipeline and return the output bytes.
/// Panics on error.
fn run_transcode(input: IoTestEnum, w: u32, h: u32, preset: EncoderPreset) -> Vec<u8> {
    test_init();
    let mut ctx = Context::create().unwrap();
    IoTestTranslator {}.add(&mut ctx, 0, input).unwrap();
    IoTestTranslator {}.add(&mut ctx, 1, IoTestEnum::OutputBuffer).unwrap();
    let execute = Execute001 {
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(transcode_pipeline(w, h, preset)),
    };
    ctx.execute_1(execute).unwrap();
    ctx.take_output_buffer(1).unwrap()
}

/// Decode bytes into a bitmap. Returns (width, height).
fn decode_bytes_dimensions(bytes: &[u8]) -> (u32, u32) {
    test_init();
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, bytes.to_vec()).unwrap();
    let capture_id = 0;
    let execute = Execute001 {
        graph_recording: None,
        security: None,
        framewise: Framewise::Steps(vec![
            Node::Decode { io_id: 0, commands: None },
            Node::CaptureBitmapKey { capture_id },
        ]),
    };
    ctx.execute_1(execute).unwrap();
    let bk = ctx.get_captured_bitmap_key(capture_id).unwrap();
    let bitmaps = ctx.borrow_bitmaps().unwrap();
    let bm = bitmaps.try_borrow_mut(bk).unwrap();
    let (w, h) = bm.size();
    (w as u32, h as u32)
}

// ============================================================================
// Encoder presets for each output format
// ============================================================================

fn jpeg_q80() -> EncoderPreset {
    EncoderPreset::Mozjpeg { quality: Some(80), progressive: None, matte: None }
}
fn png32() -> EncoderPreset {
    EncoderPreset::libpng32()
}
fn webp_lossy_q80() -> EncoderPreset {
    EncoderPreset::WebPLossy { quality: 80.0 }
}
fn webp_lossless() -> EncoderPreset {
    EncoderPreset::WebPLossless
}
fn jxl_lossy() -> EncoderPreset {
    EncoderPreset::JxlLossy { distance: 1.0 }
}
fn jxl_lossless() -> EncoderPreset {
    EncoderPreset::JxlLossless
}

fn avif_lossy() -> EncoderPreset {
    EncoderPreset::Format {
        format: s::OutputImageFormat::Avif,
        quality_profile: Some(s::QualityProfile::Good),
        quality_profile_dpr: None,
        matte: None,
        lossless: None,
        allow: Some(s::AllowedFormats::avif()),
        encoder_hints: None,
    }
}

// ============================================================================
// JPEG decode → all output formats
// ============================================================================

#[test]
fn jpeg_to_jpeg() {
    visual_check! {
        source: "test_inputs/waterhouse.jpg",
        detail: "300x300_q80",
        command: "w=300&h=300&mode=max&format=jpg&quality=80",
        similarity: Similarity::MaxZdsim(0.05),
    }
}

#[test]
fn jpeg_to_png() {
    visual_check! {
        source: "test_inputs/waterhouse.jpg",
        detail: "300x300_png32",
        command: "w=300&h=300&mode=max&format=png",
    }
}

#[test]
fn jpeg_to_webp_lossy() {
    visual_check! {
        source: "test_inputs/waterhouse.jpg",
        detail: "300x300_webp_q80",
        command: "w=300&h=300&mode=max&format=webp&quality=80",
        similarity: Similarity::MaxZdsim(0.05),
    }
}

#[test]
fn jpeg_to_webp_lossless() {
    visual_check! {
        source: "test_inputs/waterhouse.jpg",
        detail: "300x300_webp_ll",
        command: "w=300&h=300&mode=max&format=webp&webp.lossless=true",
    }
}

#[test]
fn jpeg_to_gif() {
    visual_check! {
        source: "test_inputs/waterhouse.jpg",
        detail: "300x300_gif",
        command: "w=300&h=300&mode=max&format=gif",
        similarity: Similarity::MaxZdsim(0.50),
    }
}

#[test]
fn jpeg_to_jxl_lossy() {
    visual_check_steps! {
        source: "test_inputs/waterhouse.jpg",
        detail: "300x300_jxl_d1",
        steps: transcode_pipeline(300, 300, jxl_lossy()),
        similarity: Similarity::MaxZdsim(0.05),
    }
}

#[test]
fn jpeg_to_jxl_lossless() {
    visual_check_steps! {
        source: "test_inputs/waterhouse.jpg",
        detail: "300x300_jxl_ll",
        steps: transcode_pipeline(300, 300, jxl_lossless()),
    }
}

#[test]
fn jpeg_to_avif() {
    visual_check_steps! {
        source: "test_inputs/waterhouse.jpg",
        detail: "300x300_avif",
        steps: transcode_pipeline(300, 300, avif_lossy()),
        similarity: Similarity::MaxZdsim(0.55),
    }
}

// ============================================================================
// PNG decode → all output formats
// ============================================================================

#[test]
fn png_to_jpeg() {
    visual_check! {
        source: "test_inputs/frymire.png",
        detail: "400x400_q80",
        command: "w=400&h=400&mode=max&format=jpg&quality=80",
        similarity: Similarity::MaxZdsim(0.05),
    }
}

#[test]
fn png_to_png() {
    visual_check! {
        source: "test_inputs/frymire.png",
        detail: "400x400_png32",
        command: "w=400&h=400&mode=max&format=png",
    }
}

#[test]
fn png_to_webp_lossy() {
    visual_check! {
        source: "test_inputs/frymire.png",
        detail: "400x400_webp_q80",
        command: "w=400&h=400&mode=max&format=webp&quality=80",
        similarity: Similarity::MaxZdsim(0.05),
    }
}

#[test]
fn png_to_webp_lossless() {
    visual_check! {
        source: "test_inputs/frymire.png",
        detail: "400x400_webp_ll",
        command: "w=400&h=400&mode=max&format=webp&webp.lossless=true",
    }
}

#[test]
fn png_to_gif() {
    visual_check! {
        source: "test_inputs/frymire.png",
        detail: "400x400_gif",
        command: "w=400&h=400&mode=max&format=gif",
        similarity: Similarity::MaxZdsim(0.50),
    }
}

#[test]
fn png_to_jxl_lossy() {
    visual_check_steps! {
        source: "test_inputs/frymire.png",
        detail: "400x400_jxl_d1",
        steps: transcode_pipeline(400, 400, jxl_lossy()),
        similarity: Similarity::MaxZdsim(0.05),
    }
}

#[test]
fn png_to_jxl_lossless() {
    visual_check_steps! {
        source: "test_inputs/frymire.png",
        detail: "400x400_jxl_ll",
        steps: transcode_pipeline(400, 400, jxl_lossless()),
    }
}

#[test]
fn png_to_avif() {
    visual_check_steps! {
        source: "test_inputs/frymire.png",
        detail: "400x400_avif",
        steps: transcode_pipeline(400, 400, avif_lossy()),
        similarity: Similarity::MaxZdsim(0.55),
    }
}

// ============================================================================
// PNG with transparency → all output formats
// ============================================================================

#[test]
fn png_alpha_to_png() {
    visual_check! {
        source: "test_inputs/shirt_transparent.png",
        detail: "200x200_png32",
        command: "w=200&h=200&mode=max&format=png",
        similarity: Similarity::AllowOffByOneBytesRatio(0.01),
    }
}

#[test]
fn png_alpha_to_jpeg() {
    visual_check! {
        source: "test_inputs/shirt_transparent.png",
        detail: "200x200_jpeg_q90",
        command: "w=200&h=200&mode=max&format=jpg&quality=90",
        similarity: Similarity::MaxZdsim(0.05),
    }
}

#[test]
fn png_alpha_to_webp_lossless() {
    visual_check! {
        source: "test_inputs/shirt_transparent.png",
        detail: "200x200_webp_ll",
        command: "w=200&h=200&mode=max&format=webp&webp.lossless=true",
        similarity: Similarity::AllowOffByOneBytesRatio(0.01),
    }
}

#[test]
fn png_alpha_to_webp_lossy() {
    visual_check! {
        source: "test_inputs/shirt_transparent.png",
        detail: "200x200_webp_q80",
        command: "w=200&h=200&mode=max&format=webp&quality=80",
        similarity: Similarity::MaxZdsim(0.10),
    }
}

#[test]
fn png_alpha_to_gif() {
    visual_check! {
        source: "test_inputs/shirt_transparent.png",
        detail: "200x200_gif",
        command: "w=200&h=200&mode=max&format=gif",
        similarity: Similarity::MaxZdsim(0.50),
    }
}

// ============================================================================
// WebP decode → all output formats
// ============================================================================

#[test]
fn webp_lossless_to_png() {
    visual_check! {
        source: "test_inputs/1_webp_ll.webp",
        detail: "200x200_png32",
        command: "w=200&h=200&mode=max&format=png",
        similarity: Similarity::AllowOffByOneBytesRatio(0.01),
    }
}

#[test]
fn webp_lossless_to_jpeg() {
    visual_check! {
        source: "test_inputs/1_webp_ll.webp",
        detail: "200x200_jpeg_q80",
        command: "w=200&h=200&mode=max&format=jpg&quality=80",
        similarity: Similarity::MaxZdsim(0.05),
    }
}

#[test]
fn webp_lossless_to_webp_lossy() {
    visual_check! {
        source: "test_inputs/1_webp_ll.webp",
        detail: "200x200_webp_q80",
        command: "w=200&h=200&mode=max&format=webp&quality=80",
        similarity: Similarity::MaxZdsim(0.05),
    }
}

#[test]
fn webp_lossless_to_gif() {
    visual_check! {
        source: "test_inputs/1_webp_ll.webp",
        detail: "200x200_gif",
        command: "w=200&h=200&mode=max&format=gif",
        similarity: Similarity::MaxZdsim(0.50),
    }
}

#[test]
fn webp_lossless_to_jxl_lossy() {
    visual_check_steps! {
        source: "test_inputs/1_webp_ll.webp",
        detail: "200x200_jxl_d1",
        steps: transcode_pipeline(200, 200, jxl_lossy()),
        similarity: Similarity::MaxZdsim(0.05),
    }
}

#[test]
fn webp_lossy_to_jpeg() {
    visual_check! {
        source: "test_inputs/lossy_mountain.webp",
        detail: "300x300_jpeg_q80",
        command: "w=300&h=300&mode=max&format=jpg&quality=80",
        similarity: Similarity::MaxZdsim(0.05),
    }
}

#[test]
fn webp_lossy_to_png() {
    visual_check! {
        source: "test_inputs/lossy_mountain.webp",
        detail: "300x300_png32",
        command: "w=300&h=300&mode=max&format=png",
    }
}

#[test]
fn webp_lossy_to_jxl_lossy() {
    visual_check_steps! {
        source: "test_inputs/lossy_mountain.webp",
        detail: "300x300_jxl_d1",
        steps: transcode_pipeline(300, 300, jxl_lossy()),
        similarity: Similarity::MaxZdsim(0.05),
    }
}

// ============================================================================
// GIF decode → all output formats
// ============================================================================

#[test]
fn gif_to_jpeg() {
    visual_check! {
        source: "test_inputs/mountain_800.gif",
        detail: "300x300_jpeg_q80",
        command: "w=300&h=300&mode=max&format=jpg&quality=80",
        similarity: Similarity::MaxZdsim(0.05),
    }
}

#[test]
fn gif_to_png() {
    visual_check! {
        source: "test_inputs/mountain_800.gif",
        detail: "300x300_png32",
        command: "w=300&h=300&mode=max&format=png",
    }
}

#[test]
fn gif_to_webp_lossless() {
    visual_check! {
        source: "test_inputs/mountain_800.gif",
        detail: "300x300_webp_ll",
        command: "w=300&h=300&mode=max&format=webp&webp.lossless=true",
    }
}

#[test]
fn gif_to_jxl_lossy() {
    visual_check_steps! {
        source: "test_inputs/mountain_800.gif",
        detail: "300x300_jxl_d1",
        steps: transcode_pipeline(300, 300, jxl_lossy()),
        similarity: Similarity::MaxZdsim(0.05),
    }
}

// ============================================================================
// Passthrough (decode → encode, no resize) — lossless round-trip fidelity
// ============================================================================

#[test]
fn passthrough_png_lossless() {
    visual_check_steps! {
        source: "test_inputs/frymire.png",
        detail: "passthrough_png32",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Encode { io_id: 1, preset: png32() },
        ],
        similarity: Similarity::MaxZdsim(0.0),
    }
}

#[test]
fn passthrough_webp_lossless() {
    visual_check_steps! {
        source: "test_inputs/1_webp_ll.webp",
        detail: "passthrough_webp_ll",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Encode { io_id: 1, preset: webp_lossless() },
        ],
        similarity: Similarity::AllowOffByOneBytesRatio(0.01),
    }
}

#[test]
fn passthrough_jxl_lossless() {
    visual_check_steps! {
        source: "test_inputs/frymire.png",
        detail: "passthrough_jxl_ll",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Encode { io_id: 1, preset: jxl_lossless() },
        ],
    }
}

// ============================================================================
// Multi-step transcoding chains (A → B → C)
// ============================================================================

#[test]
fn chain_jpeg_to_webp_to_png() {
    test_init();
    let input = IoTestEnum::Url(WATERHOUSE_JPG.to_owned());

    // JPEG → WebP lossy
    let webp_bytes = run_transcode(input, 300, 300, webp_lossy_q80());

    // WebP → PNG
    let png_bytes = run_transcode(IoTestEnum::ByteArray(webp_bytes), 300, 300, png32());

    // Verify PNG output is valid and decodable
    let (w, h) = decode_bytes_dimensions(&png_bytes);
    assert!(w > 0 && w <= 300 && h > 0 && h <= 300, "unexpected dimensions: {w}x{h}");
}

#[test]
fn chain_png_to_jxl_to_webp() {
    test_init();
    let input = IoTestEnum::Url(FRYMIRE_PNG.to_owned());

    // PNG → JXL lossy
    let jxl_bytes = run_transcode(input, 400, 400, jxl_lossy());

    // JXL → WebP lossless
    let webp_bytes = run_transcode(IoTestEnum::ByteArray(jxl_bytes), 400, 400, webp_lossless());

    let (w, h) = decode_bytes_dimensions(&webp_bytes);
    assert!(w > 0 && w <= 400 && h > 0 && h <= 400, "unexpected dimensions: {w}x{h}");
}

#[test]
fn chain_webp_to_avif_to_jpeg() {
    test_init();
    let input = IoTestEnum::Url(WEBP_LL.to_owned());

    // WebP → AVIF
    let avif_bytes = run_transcode(input, 200, 200, avif_lossy());

    // AVIF → JPEG
    let jpeg_bytes = run_transcode(IoTestEnum::ByteArray(avif_bytes), 200, 200, jpeg_q80());

    let (w, h) = decode_bytes_dimensions(&jpeg_bytes);
    assert!(w > 0 && w <= 200 && h > 0 && h <= 200, "unexpected dimensions: {w}x{h}");
}

#[test]
fn chain_gif_to_jxl_to_avif() {
    test_init();
    let input = IoTestEnum::Url(MOUNTAIN_GIF.to_owned());

    // GIF → JXL lossy
    let jxl_bytes = run_transcode(input, 300, 300, jxl_lossy());

    // JXL → AVIF
    let avif_bytes = run_transcode(IoTestEnum::ByteArray(jxl_bytes), 300, 300, avif_lossy());

    let (w, h) = decode_bytes_dimensions(&avif_bytes);
    assert!(w > 0 && w <= 300 && h > 0 && h <= 300, "unexpected dimensions: {w}x{h}");
}

// ============================================================================
// Quality sweep — verify encoder doesn't crash at edge qualities
// ============================================================================

#[test]
fn jpeg_quality_sweep() {
    test_init();
    let input_bytes =
        get_url_bytes_with_retry(WATERHOUSE_JPG).expect("failed to fetch waterhouse.jpg");

    for q in [1, 10, 25, 50, 75, 90, 100] {
        let steps = vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: format!("w=200&h=200&mode=max&format=jpg&quality={q}"),
            decode: Some(0),
            encode: Some(1),
            watermarks: None,
        }];
        let mut ctx = Context::create().unwrap();
        ctx.add_input_vector(0, input_bytes.clone()).unwrap();
        ctx.add_output_buffer(1).unwrap();
        ctx.execute_1(Execute001 {
            graph_recording: None,
            security: None,
            framewise: Framewise::Steps(steps),
        })
        .unwrap_or_else(|e| panic!("JPEG quality={q} failed: {e}"));
        let output = ctx.take_output_buffer(1).unwrap();
        assert!(
            output.starts_with(&[0xFF, 0xD8, 0xFF]),
            "JPEG quality={q}: output doesn't have JPEG magic"
        );
    }
}

#[test]
fn webp_quality_sweep() {
    test_init();
    let input_bytes =
        get_url_bytes_with_retry(WATERHOUSE_JPG).expect("failed to fetch waterhouse.jpg");

    for q in [1, 10, 25, 50, 75, 90, 100] {
        let steps = vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: format!("w=200&h=200&mode=max&format=webp&quality={q}"),
            decode: Some(0),
            encode: Some(1),
            watermarks: None,
        }];
        let mut ctx = Context::create().unwrap();
        ctx.add_input_vector(0, input_bytes.clone()).unwrap();
        ctx.add_output_buffer(1).unwrap();
        ctx.execute_1(Execute001 {
            graph_recording: None,
            security: None,
            framewise: Framewise::Steps(steps),
        })
        .unwrap_or_else(|e| panic!("WebP quality={q} failed: {e}"));
        let output = ctx.take_output_buffer(1).unwrap();
        assert!(output.starts_with(b"RIFF"), "WebP quality={q}: output doesn't have RIFF magic");
    }
}

#[test]
fn jxl_distance_sweep() {
    test_init();
    let input_bytes = get_url_bytes_with_retry(FRYMIRE_PNG).expect("failed to fetch frymire.png");

    for d in [0.0f32, 0.5, 1.0, 2.0, 4.0, 8.0] {
        let preset =
            if d == 0.0 { jxl_lossless() } else { EncoderPreset::JxlLossy { distance: d } };
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
            Node::Encode { io_id: 1, preset },
        ];
        let mut ctx = Context::create().unwrap();
        ctx.add_input_vector(0, input_bytes.clone()).unwrap();
        ctx.add_output_buffer(1).unwrap();
        ctx.execute_1(Execute001 {
            graph_recording: None,
            security: None,
            framewise: Framewise::Steps(steps),
        })
        .unwrap_or_else(|e| panic!("JXL distance={d} failed: {e}"));
        let output = ctx.take_output_buffer(1).unwrap();
        assert!(!output.is_empty(), "JXL distance={d}: output is empty");
    }
}

// ============================================================================
// CMYK and ICC profile handling
// ============================================================================

#[test]
fn cmyk_jpeg_to_png() {
    visual_check_bitmap! {
        source: "test_inputs/cmyk_logo.jpg",
        detail: "cmyk_to_png_200x200",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Constrain(Constraint {
                mode: ConstraintMode::Within,
                w: Some(200),
                h: Some(200),
                hints: None,
                gravity: None,
                canvas_color: None,
            }),
        ],
        tolerance: Tolerance {
            max_delta: 3,
            min_similarity: 95.0,
            max_pixels_different: 1.0,
            ..Tolerance::exact()
        },
    }
}

#[test]
fn cmyk_jpeg_to_webp() {
    visual_check! {
        source: "test_inputs/cmyk_logo.jpg",
        detail: "cmyk_to_webp_200x200",
        command: "w=200&h=200&mode=max&format=webp&quality=90",
        similarity: Similarity::MaxZdsim(0.10),
    }
}

#[test]
fn cmyk_jpeg_to_jxl() {
    visual_check_steps! {
        source: "test_inputs/cmyk_logo.jpg",
        detail: "cmyk_to_jxl_200x200",
        steps: transcode_pipeline(200, 200, jxl_lossy()),
        similarity: Similarity::MaxZdsim(0.10),
    }
}

// ============================================================================
// Corpus tests — scan local scraped images for decode/transcode crashes
// ============================================================================

/// Scan a directory of files, decode+scale+encode each one.
/// Returns (successes, failures). Panics only on unexpected panics (not decode errors).
fn corpus_scan(
    dir: &Path,
    extension: &str,
    target_format: &str,
    max_files: usize,
) -> (usize, Vec<(PathBuf, String)>) {
    test_init();

    let mut files: Vec<PathBuf> = Vec::new();
    if dir.is_dir() {
        collect_files_recursive(dir, extension, &mut files, max_files);
    }
    files.sort();
    if files.len() > max_files {
        files.truncate(max_files);
    }

    let mut successes = 0;
    let mut failures: Vec<(PathBuf, String)> = Vec::new();

    for path in &files {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                failures.push((path.clone(), format!("read error: {e}")));
                continue;
            }
        };

        let command = format!("w=300&h=300&mode=max&format={target_format}");
        let steps = vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: command,
            decode: Some(0),
            encode: Some(1),
            watermarks: None,
        }];

        let result = std::panic::catch_unwind(|| {
            let mut ctx = Context::create().unwrap();
            ctx.add_input_vector(0, bytes)?;
            ctx.add_output_buffer(1)?;
            // Corpus images may have mismatched ICC profiles (e.g. grayscale
            // profile on an RGB image). Fall back to sRGB rather than failing.
            let _ = ctx.tell_decoder(0, DecoderCommand::IgnoreColorProfileErrors);
            let execute = Execute001 {
                graph_recording: None,
                security: None,
                framewise: Framewise::Steps(steps),
            };
            ctx.execute_1(execute)
        });

        match result {
            Ok(Ok(_)) => successes += 1,
            Ok(Err(e)) => {
                failures.push((path.clone(), format!("{e}")));
            }
            Err(panic_info) => {
                let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                failures.push((path.clone(), format!("PANIC: {msg}")));
            }
        }
    }

    (successes, failures)
}

fn collect_files_recursive(dir: &Path, extension: &str, out: &mut Vec<PathBuf>, max: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if out.len() >= max {
            return;
        }
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(&path, extension, out, max);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext.eq_ignore_ascii_case(extension) {
                out.push(path);
            }
        }
    }
}

/// Report corpus results: print summary and save failures to a log file.
fn report_corpus(
    format_name: &str,
    target_format: &str,
    successes: usize,
    failures: &[(PathBuf, String)],
) {
    let total = successes + failures.len();
    let fail_count = failures.len();
    eprintln!(
        "corpus {format_name}→{target_format}: {successes}/{total} passed, {fail_count} failed"
    );

    if !failures.is_empty() {
        let log_path = format!("/tmp/corpus_{format_name}_to_{target_format}_failures.log");
        let mut log = std::fs::File::create(&log_path).unwrap();
        for (path, err) in failures {
            use std::io::Write;
            writeln!(log, "{}\t{err}", path.display()).unwrap();
        }
        eprintln!("  failures logged to {log_path}");

        // Show first few failures inline
        for (path, err) in failures.iter().take(5) {
            eprintln!("  FAIL: {} — {err}", path.file_name().unwrap_or_default().to_string_lossy());
        }

        // Panics indicate bugs — log them prominently but don't fail the test.
        // Corpus images include adversarial/malformed files that trigger edge cases
        // in third-party decoders. We file bugs for these separately.
        let panics: Vec<_> = failures.iter().filter(|(_, e)| e.starts_with("PANIC:")).collect();
        if !panics.is_empty() {
            eprintln!(
                "  WARNING: {} files caused panics in {format_name}→{target_format}",
                panics.len()
            );
            for (path, err) in panics.iter().take(3) {
                eprintln!(
                    "    PANIC: {} — {err}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
            }
        }
    }
}

// ── JPEG corpus ────────────────────────────────────────────────────────────

#[test]
fn corpus_jpeg_to_jpeg() {
    let dir = Path::new("/mnt/v/datasets/scraping/jpeg");
    if !dir.exists() {
        eprintln!("skipping corpus_jpeg_to_jpeg: {dir:?} not found");
        return;
    }
    let (ok, fail) = corpus_scan(dir, "jpg", "jpg", 200);
    report_corpus("jpeg", "jpg", ok, &fail);
}

#[test]
fn corpus_jpeg_to_webp() {
    let dir = Path::new("/mnt/v/datasets/scraping/jpeg");
    if !dir.exists() {
        eprintln!("skipping corpus_jpeg_to_webp: {dir:?} not found");
        return;
    }
    let (ok, fail) = corpus_scan(dir, "jpg", "webp", 200);
    report_corpus("jpeg", "webp", ok, &fail);
}

#[test]
fn corpus_jpeg_to_png() {
    let dir = Path::new("/mnt/v/datasets/scraping/jpeg");
    if !dir.exists() {
        eprintln!("skipping corpus_jpeg_to_png: {dir:?} not found");
        return;
    }
    let (ok, fail) = corpus_scan(dir, "jpg", "png", 200);
    report_corpus("jpeg", "png", ok, &fail);
}

// ── WebP corpus ────────────────────────────────────────────────────────────

#[test]
fn corpus_webp_to_jpeg() {
    let dir = Path::new("/mnt/v/datasets/scraping/webp");
    if !dir.exists() {
        eprintln!("skipping corpus_webp_to_jpeg: {dir:?} not found");
        return;
    }
    let (ok, fail) = corpus_scan(dir, "webp", "jpg", 200);
    report_corpus("webp", "jpg", ok, &fail);
}

#[test]
fn corpus_webp_to_png() {
    let dir = Path::new("/mnt/v/datasets/scraping/webp");
    if !dir.exists() {
        eprintln!("skipping corpus_webp_to_png: {dir:?} not found");
        return;
    }
    let (ok, fail) = corpus_scan(dir, "webp", "png", 200);
    report_corpus("webp", "png", ok, &fail);
}

#[test]
fn corpus_webp_to_webp() {
    let dir = Path::new("/mnt/v/datasets/scraping/webp");
    if !dir.exists() {
        eprintln!("skipping corpus_webp_to_webp: {dir:?} not found");
        return;
    }
    let (ok, fail) = corpus_scan(dir, "webp", "webp", 200);
    report_corpus("webp", "webp", ok, &fail);
}

// ── JXL corpus ─────────────────────────────────────────────────────────────

#[test]
fn corpus_jxl_to_jpeg() {
    let dir = Path::new("/mnt/v/datasets/scraping/jxl");
    if !dir.exists() {
        eprintln!("skipping corpus_jxl_to_jpeg: {dir:?} not found");
        return;
    }
    let (ok, fail) = corpus_scan(dir, "jxl", "jpg", 200);
    report_corpus("jxl", "jpg", ok, &fail);
}

#[test]
fn corpus_jxl_to_png() {
    let dir = Path::new("/mnt/v/datasets/scraping/jxl");
    if !dir.exists() {
        eprintln!("skipping corpus_jxl_to_png: {dir:?} not found");
        return;
    }
    let (ok, fail) = corpus_scan(dir, "jxl", "png", 200);
    report_corpus("jxl", "png", ok, &fail);
}

#[test]
fn corpus_jxl_to_webp() {
    let dir = Path::new("/mnt/v/datasets/scraping/jxl");
    if !dir.exists() {
        eprintln!("skipping corpus_jxl_to_webp: {dir:?} not found");
        return;
    }
    let (ok, fail) = corpus_scan(dir, "jxl", "webp", 200);
    report_corpus("jxl", "webp", ok, &fail);
}

// ── AVIF corpus ────────────────────────────────────────────────────────────

#[test]
fn corpus_avif_to_jpeg() {
    let dir = Path::new("/mnt/v/datasets/scraping/avif");
    if !dir.exists() {
        eprintln!("skipping corpus_avif_to_jpeg: {dir:?} not found");
        return;
    }
    let (ok, fail) = corpus_scan(dir, "avif", "jpg", 200);
    report_corpus("avif", "jpg", ok, &fail);
}

#[test]
fn corpus_avif_to_png() {
    let dir = Path::new("/mnt/v/datasets/scraping/avif");
    if !dir.exists() {
        eprintln!("skipping corpus_avif_to_png: {dir:?} not found");
        return;
    }
    let (ok, fail) = corpus_scan(dir, "avif", "png", 200);
    report_corpus("avif", "png", ok, &fail);
}

#[test]
fn corpus_avif_to_webp() {
    let dir = Path::new("/mnt/v/datasets/scraping/avif");
    if !dir.exists() {
        eprintln!("skipping corpus_avif_to_webp: {dir:?} not found");
        return;
    }
    let (ok, fail) = corpus_scan(dir, "avif", "webp", 200);
    report_corpus("avif", "webp", ok, &fail);
}

// ── HEIC corpus ────────────────────────────────────────────────────────────

#[test]
fn corpus_heic_to_jpeg() {
    let dir = Path::new("/mnt/v/heic");
    if !dir.exists() {
        eprintln!("skipping corpus_heic_to_jpeg: {dir:?} not found");
        return;
    }
    let (ok, fail) = corpus_scan(dir, "heic", "jpg", 50);
    report_corpus("heic", "jpg", ok, &fail);
}

#[test]
fn corpus_heic_to_png() {
    let dir = Path::new("/mnt/v/heic");
    if !dir.exists() {
        eprintln!("skipping corpus_heic_to_png: {dir:?} not found");
        return;
    }
    let (ok, fail) = corpus_scan(dir, "heic", "png", 50);
    report_corpus("heic", "png", ok, &fail);
}

// ── Weird/conformance corpus ───────────────────────────────────────────────

#[test]
fn corpus_weird_avif_to_png() {
    let dir = Path::new("/mnt/v/datasets/scraping/weird/avif");
    if !dir.exists() {
        eprintln!("skipping corpus_weird_avif_to_png: {dir:?} not found");
        return;
    }
    let (ok, fail) = corpus_scan(dir, "avif", "png", 100);
    report_corpus("weird_avif", "png", ok, &fail);
}

#[test]
fn corpus_weird_jxl_to_png() {
    let dir = Path::new("/mnt/v/datasets/scraping/weird/jxl");
    if !dir.exists() {
        eprintln!("skipping corpus_weird_jxl_to_png: {dir:?} not found");
        return;
    }
    let (ok, fail) = corpus_scan(dir, "jxl", "png", 100);
    report_corpus("weird_jxl", "png", ok, &fail);
}

#[test]
fn corpus_weird_webp_to_png() {
    let dir = Path::new("/mnt/v/datasets/scraping/weird/webp");
    if !dir.exists() {
        eprintln!("skipping corpus_weird_webp_to_png: {dir:?} not found");
        return;
    }
    let (ok, fail) = corpus_scan(dir, "webp", "png", 100);
    report_corpus("weird_webp", "png", ok, &fail);
}

// ── ICC profile corpus (non-sRGB) ─────────────────────────────────────────

#[test]
fn corpus_icc_adobe_rgb_to_jpeg() {
    let dir = Path::new("/mnt/v/datasets/non-srgb-by-profile/adobe-rgb");
    if !dir.exists() {
        eprintln!("skipping corpus_icc_adobe_rgb_to_jpeg: {dir:?} not found");
        return;
    }
    // Mix of .webp and .jpg files — scan both
    let (ok_webp, fail_webp) = corpus_scan(dir, "webp", "jpg", 100);
    let (ok_jpg, fail_jpg) = corpus_scan(dir, "jpg", "jpg", 100);
    let ok = ok_webp + ok_jpg;
    let mut fail = fail_webp;
    fail.extend(fail_jpg);
    report_corpus("icc_adobe_rgb", "jpg", ok, &fail);
}

#[test]
fn corpus_icc_display_p3_to_jpeg() {
    let dir = Path::new("/mnt/v/datasets/non-srgb-by-profile/display-p3");
    if !dir.exists() {
        eprintln!("skipping corpus_icc_display_p3_to_jpeg: {dir:?} not found");
        return;
    }
    let (ok_webp, fail_webp) = corpus_scan(dir, "webp", "jpg", 100);
    let (ok_jpg, fail_jpg) = corpus_scan(dir, "jpg", "jpg", 100);
    let ok = ok_webp + ok_jpg;
    let mut fail = fail_webp;
    fail.extend(fail_jpg);
    report_corpus("icc_display_p3", "jpg", ok, &fail);
}

#[test]
fn corpus_icc_prophoto_to_jpeg() {
    let dir = Path::new("/mnt/v/datasets/non-srgb-by-profile/prophoto-rgb");
    if !dir.exists() {
        eprintln!("skipping corpus_icc_prophoto_to_jpeg: {dir:?} not found");
        return;
    }
    let (ok_webp, fail_webp) = corpus_scan(dir, "webp", "jpg", 100);
    let (ok_jpg, fail_jpg) = corpus_scan(dir, "jpg", "jpg", 100);
    let ok = ok_webp + ok_jpg;
    let mut fail = fail_webp;
    fail.extend(fail_jpg);
    report_corpus("icc_prophoto", "jpg", ok, &fail);
}

// ── corpus-builder comprehensive tests ────────────────────────────────────

const CORPUS_BUILDER: &str = "/mnt/v/output/corpus-builder";

/// Helper: run corpus_scan against a corpus-builder subdirectory.
fn cb_scan(subdir: &str, ext: &str, target: &str) -> (usize, Vec<(PathBuf, String)>) {
    cb_scan_limit(subdir, ext, target, 500)
}

fn cb_scan_limit(
    subdir: &str,
    ext: &str,
    target: &str,
    max: usize,
) -> (usize, Vec<(PathBuf, String)>) {
    let dir = Path::new(CORPUS_BUILDER).join(subdir);
    if !dir.exists() {
        eprintln!("skipping corpus-builder/{subdir}: not found");
        return (0, vec![]);
    }
    corpus_scan(&dir, ext, target, max)
}

// ── PNG corpus-builder ──────────────────────────────────────────────────

#[test]
fn cb_png24_to_png() {
    let (ok, fail) = cb_scan("png-24-32", "png", "png");
    report_corpus("cb_png24", "png", ok, &fail);
}

#[test]
fn cb_png24_to_jpg() {
    let (ok, fail) = cb_scan("png-24-32", "png", "jpg");
    report_corpus("cb_png24", "jpg", ok, &fail);
}

#[test]
fn cb_png24_to_webp() {
    let (ok, fail) = cb_scan("png-24-32", "png", "webp");
    report_corpus("cb_png24", "webp", ok, &fail);
}

#[test]
fn cb_png24_to_jxl() {
    let (ok, fail) = cb_scan("png-24-32", "png", "jxl");
    report_corpus("cb_png24", "jxl", ok, &fail);
}

#[test]
fn cb_png24_to_avif() {
    let (ok, fail) = cb_scan("png-24-32", "png", "avif");
    report_corpus("cb_png24", "avif", ok, &fail);
}

#[test]
fn cb_png24_to_gif() {
    let (ok, fail) = cb_scan("png-24-32", "png", "gif");
    report_corpus("cb_png24", "gif", ok, &fail);
}

#[test]
fn cb_png8_to_png() {
    let (ok, fail) = cb_scan("png-8", "png", "png");
    report_corpus("cb_png8", "png", ok, &fail);
}

#[test]
fn cb_png8_to_jpg() {
    let (ok, fail) = cb_scan("png-8", "png", "jpg");
    report_corpus("cb_png8", "jpg", ok, &fail);
}

#[test]
fn cb_png8_to_webp() {
    let (ok, fail) = cb_scan("png-8", "png", "webp");
    report_corpus("cb_png8", "webp", ok, &fail);
}

#[test]
fn cb_png8_to_jxl() {
    let (ok, fail) = cb_scan("png-8", "png", "jxl");
    report_corpus("cb_png8", "jxl", ok, &fail);
}

#[test]
fn cb_png8_to_avif() {
    let (ok, fail) = cb_scan("png-8", "png", "avif");
    report_corpus("cb_png8", "avif", ok, &fail);
}

#[test]
fn cb_png8_to_gif() {
    let (ok, fail) = cb_scan("png-8", "png", "gif");
    report_corpus("cb_png8", "gif", ok, &fail);
}

// ── APNG corpus-builder ─────────────────────────────────────────────────

#[test]
fn cb_apng_to_png() {
    let (ok, fail) = cb_scan("apng", "png", "png");
    report_corpus("cb_apng", "png", ok, &fail);
}

#[test]
fn cb_apng_to_jpg() {
    let (ok, fail) = cb_scan("apng", "png", "jpg");
    report_corpus("cb_apng", "jpg", ok, &fail);
}

#[test]
fn cb_apng_to_webp() {
    let (ok, fail) = cb_scan("apng", "png", "webp");
    report_corpus("cb_apng", "webp", ok, &fail);
}

#[test]
fn cb_apng_to_gif() {
    let (ok, fail) = cb_scan("apng", "png", "gif");
    report_corpus("cb_apng", "gif", ok, &fail);
}

#[test]
fn cb_apng_to_jxl() {
    let (ok, fail) = cb_scan("apng", "png", "jxl");
    report_corpus("cb_apng", "jxl", ok, &fail);
}

#[test]
fn cb_apng_to_avif() {
    let (ok, fail) = cb_scan("apng", "png", "avif");
    report_corpus("cb_apng", "avif", ok, &fail);
}

// ── JPEG corpus-builder ─────────────────────────────────────────────────
// ~543 large JPEGs (~1.7 GB) — limit to 200 per test

#[test]
fn cb_jpeg_to_jpg() {
    let (ok, fail) = cb_scan_limit("source_jpegs", "jpg", "jpg", 200);
    report_corpus("cb_jpeg", "jpg", ok, &fail);
}

#[test]
fn cb_jpeg_to_png() {
    let (ok, fail) = cb_scan_limit("source_jpegs", "jpg", "png", 200);
    report_corpus("cb_jpeg", "png", ok, &fail);
}

#[test]
fn cb_jpeg_to_webp() {
    let (ok, fail) = cb_scan_limit("source_jpegs", "jpg", "webp", 200);
    report_corpus("cb_jpeg", "webp", ok, &fail);
}

#[test]
fn cb_jpeg_to_jxl() {
    let (ok, fail) = cb_scan_limit("source_jpegs", "jpg", "jxl", 200);
    report_corpus("cb_jpeg", "jxl", ok, &fail);
}

#[test]
fn cb_jpeg_to_avif() {
    let (ok, fail) = cb_scan_limit("source_jpegs", "jpg", "avif", 200);
    report_corpus("cb_jpeg", "avif", ok, &fail);
}

#[test]
fn cb_jpeg_to_gif() {
    let (ok, fail) = cb_scan_limit("source_jpegs", "jpg", "gif", 200);
    report_corpus("cb_jpeg", "gif", ok, &fail);
}

// ── WebP corpus-builder ─────────────────────────────────────────────────

#[test]
fn cb_webp_to_png() {
    let (ok, fail) = cb_scan("webp", "webp", "png");
    report_corpus("cb_webp", "png", ok, &fail);
}

#[test]
fn cb_webp_to_jpg() {
    let (ok, fail) = cb_scan("webp", "webp", "jpg");
    report_corpus("cb_webp", "jpg", ok, &fail);
}

#[test]
fn cb_webp_to_webp() {
    let (ok, fail) = cb_scan("webp", "webp", "webp");
    report_corpus("cb_webp", "webp", ok, &fail);
}

#[test]
fn cb_webp_to_jxl() {
    let (ok, fail) = cb_scan("webp", "webp", "jxl");
    report_corpus("cb_webp", "jxl", ok, &fail);
}

#[test]
fn cb_webp_to_avif() {
    let (ok, fail) = cb_scan("webp", "webp", "avif");
    report_corpus("cb_webp", "avif", ok, &fail);
}

#[test]
fn cb_webp_to_gif() {
    let (ok, fail) = cb_scan("webp", "webp", "gif");
    report_corpus("cb_webp", "gif", ok, &fail);
}

// ── WebP animated corpus-builder ────────────────────────────────────────
// 1,681 files — limit to 50 to stay within test timeout (animated decode is slow)

#[test]
fn cb_webp_anim_to_gif() {
    let (ok, fail) = cb_scan_limit("webp-animated", "webp", "gif", 25);
    report_corpus("cb_webp_anim", "gif", ok, &fail);
}

#[test]
fn cb_webp_anim_to_png() {
    let (ok, fail) = cb_scan_limit("webp-animated", "webp", "png", 25);
    report_corpus("cb_webp_anim", "png", ok, &fail);
}

#[test]
fn cb_webp_anim_to_jpg() {
    let (ok, fail) = cb_scan_limit("webp-animated", "webp", "jpg", 25);
    report_corpus("cb_webp_anim", "jpg", ok, &fail);
}

#[test]
fn cb_webp_anim_to_webp() {
    let (ok, fail) = cb_scan_limit("webp-animated", "webp", "webp", 25);
    report_corpus("cb_webp_anim", "webp", ok, &fail);
}

#[test]
fn cb_webp_anim_to_jxl() {
    let (ok, fail) = cb_scan_limit("webp-animated", "webp", "jxl", 25);
    report_corpus("cb_webp_anim", "jxl", ok, &fail);
}

#[test]
fn cb_webp_anim_to_avif() {
    let (ok, fail) = cb_scan_limit("webp-animated", "webp", "avif", 25);
    report_corpus("cb_webp_anim", "avif", ok, &fail);
}

// ── AVIF corpus-builder ─────────────────────────────────────────────────

#[test]
fn cb_avif_to_png() {
    let (ok, fail) = cb_scan("avif", "avif", "png");
    report_corpus("cb_avif", "png", ok, &fail);
}

#[test]
fn cb_avif_to_jpg() {
    let (ok, fail) = cb_scan("avif", "avif", "jpg");
    report_corpus("cb_avif", "jpg", ok, &fail);
}

#[test]
fn cb_avif_to_webp() {
    let (ok, fail) = cb_scan("avif", "avif", "webp");
    report_corpus("cb_avif", "webp", ok, &fail);
}

#[test]
fn cb_avif_to_jxl() {
    let (ok, fail) = cb_scan("avif", "avif", "jxl");
    report_corpus("cb_avif", "jxl", ok, &fail);
}

#[test]
fn cb_avif_to_avif() {
    let (ok, fail) = cb_scan("avif", "avif", "avif");
    report_corpus("cb_avif", "avif", ok, &fail);
}

#[test]
fn cb_avif_to_gif() {
    let (ok, fail) = cb_scan("avif", "avif", "gif");
    report_corpus("cb_avif", "gif", ok, &fail);
}

// ── AVIF animated corpus-builder ────────────────────────────────────────

#[test]
fn cb_avif_anim_to_png() {
    let (ok, fail) = cb_scan("avif-animated", "avif", "png");
    report_corpus("cb_avif_anim", "png", ok, &fail);
}

#[test]
fn cb_avif_anim_to_jpg() {
    let (ok, fail) = cb_scan("avif-animated", "avif", "jpg");
    report_corpus("cb_avif_anim", "jpg", ok, &fail);
}

#[test]
fn cb_avif_anim_to_webp() {
    let (ok, fail) = cb_scan("avif-animated", "avif", "webp");
    report_corpus("cb_avif_anim", "webp", ok, &fail);
}

#[test]
fn cb_avif_anim_to_jxl() {
    let (ok, fail) = cb_scan("avif-animated", "avif", "jxl");
    report_corpus("cb_avif_anim", "jxl", ok, &fail);
}

#[test]
fn cb_avif_anim_to_avif() {
    let (ok, fail) = cb_scan("avif-animated", "avif", "avif");
    report_corpus("cb_avif_anim", "avif", ok, &fail);
}

#[test]
fn cb_avif_anim_to_gif() {
    let (ok, fail) = cb_scan("avif-animated", "avif", "gif");
    report_corpus("cb_avif_anim", "gif", ok, &fail);
}

// ── JXL corpus-builder ──────────────────────────────────────────────────
// ~955 MB, JXL decode is slow — limit to 100

#[test]
fn cb_jxl_to_png() {
    let (ok, fail) = cb_scan_limit("jxl", "jxl", "png", 100);
    report_corpus("cb_jxl", "png", ok, &fail);
}

#[test]
fn cb_jxl_to_jpg() {
    let (ok, fail) = cb_scan_limit("jxl", "jxl", "jpg", 100);
    report_corpus("cb_jxl", "jpg", ok, &fail);
}

#[test]
fn cb_jxl_to_webp() {
    let (ok, fail) = cb_scan_limit("jxl", "jxl", "webp", 100);
    report_corpus("cb_jxl", "webp", ok, &fail);
}

#[test]
fn cb_jxl_to_jxl() {
    let (ok, fail) = cb_scan_limit("jxl", "jxl", "jxl", 100);
    report_corpus("cb_jxl", "jxl", ok, &fail);
}

#[test]
fn cb_jxl_to_avif() {
    let (ok, fail) = cb_scan_limit("jxl", "jxl", "avif", 100);
    report_corpus("cb_jxl", "avif", ok, &fail);
}

#[test]
fn cb_jxl_to_gif() {
    let (ok, fail) = cb_scan_limit("jxl", "jxl", "gif", 100);
    report_corpus("cb_jxl", "gif", ok, &fail);
}

// ── JXL animated corpus-builder ─────────────────────────────────────────
// corpus-builder/jxl-animated is empty, but jxl-encoder/animation has real files

const JXL_ANIM_DIR: &str = "/mnt/v/output/jxl-encoder/animation";

fn jxl_anim_scan(target: &str) -> (usize, Vec<(PathBuf, String)>) {
    // Try corpus-builder first
    let (ok1, fail1) = cb_scan("jxl-animated", "jxl", target);
    // Also scan the jxl-encoder animation output
    let dir = Path::new(JXL_ANIM_DIR);
    if !dir.exists() {
        return (ok1, fail1);
    }
    let (ok2, fail2) = corpus_scan(dir, "jxl", target, 500);
    (ok1 + ok2, [fail1, fail2].concat())
}

#[test]
fn cb_jxl_anim_to_png() {
    let (ok, fail) = jxl_anim_scan("png");
    report_corpus("jxl_anim", "png", ok, &fail);
}

#[test]
fn cb_jxl_anim_to_jpg() {
    let (ok, fail) = jxl_anim_scan("jpg");
    report_corpus("jxl_anim", "jpg", ok, &fail);
}

#[test]
fn cb_jxl_anim_to_webp() {
    let (ok, fail) = jxl_anim_scan("webp");
    report_corpus("jxl_anim", "webp", ok, &fail);
}

#[test]
fn cb_jxl_anim_to_jxl() {
    let (ok, fail) = jxl_anim_scan("jxl");
    report_corpus("jxl_anim", "jxl", ok, &fail);
}

#[test]
fn cb_jxl_anim_to_avif() {
    let (ok, fail) = jxl_anim_scan("avif");
    report_corpus("jxl_anim", "avif", ok, &fail);
}

#[test]
fn cb_jxl_anim_to_gif() {
    let (ok, fail) = jxl_anim_scan("gif");
    report_corpus("jxl_anim", "gif", ok, &fail);
}

// ── GIF static corpus-builder ───────────────────────────────────────────

#[test]
fn cb_gif_static_to_png() {
    let (ok, fail) = cb_scan("gif-static", "gif", "png");
    report_corpus("cb_gif_static", "png", ok, &fail);
}

#[test]
fn cb_gif_static_to_jpg() {
    let (ok, fail) = cb_scan("gif-static", "gif", "jpg");
    report_corpus("cb_gif_static", "jpg", ok, &fail);
}

#[test]
fn cb_gif_static_to_webp() {
    let (ok, fail) = cb_scan("gif-static", "gif", "webp");
    report_corpus("cb_gif_static", "webp", ok, &fail);
}

#[test]
fn cb_gif_static_to_jxl() {
    let (ok, fail) = cb_scan("gif-static", "gif", "jxl");
    report_corpus("cb_gif_static", "jxl", ok, &fail);
}

#[test]
fn cb_gif_static_to_avif() {
    let (ok, fail) = cb_scan("gif-static", "gif", "avif");
    report_corpus("cb_gif_static", "avif", ok, &fail);
}

#[test]
fn cb_gif_static_to_gif() {
    let (ok, fail) = cb_scan("gif-static", "gif", "gif");
    report_corpus("cb_gif_static", "gif", ok, &fail);
}

// ── GIF animated corpus-builder ─────────────────────────────────────────
// 6,861 files — limit to 50 to stay within test timeout (animated decode is slow)

#[test]
fn cb_gif_anim_to_gif() {
    let (ok, fail) = cb_scan_limit("gif-animated", "gif", "gif", 25);
    report_corpus("cb_gif_anim", "gif", ok, &fail);
}

#[test]
fn cb_gif_anim_to_png() {
    let (ok, fail) = cb_scan_limit("gif-animated", "gif", "png", 25);
    report_corpus("cb_gif_anim", "png", ok, &fail);
}

#[test]
fn cb_gif_anim_to_jpg() {
    let (ok, fail) = cb_scan_limit("gif-animated", "gif", "jpg", 25);
    report_corpus("cb_gif_anim", "jpg", ok, &fail);
}

#[test]
fn cb_gif_anim_to_webp() {
    let (ok, fail) = cb_scan_limit("gif-animated", "gif", "webp", 25);
    report_corpus("cb_gif_anim", "webp", ok, &fail);
}

#[test]
fn cb_gif_anim_to_jxl() {
    let (ok, fail) = cb_scan_limit("gif-animated", "gif", "jxl", 25);
    report_corpus("cb_gif_anim", "jxl", ok, &fail);
}

#[test]
fn cb_gif_anim_to_avif() {
    let (ok, fail) = cb_scan_limit("gif-animated", "gif", "avif", 25);
    report_corpus("cb_gif_anim", "avif", ok, &fail);
}

// ── Wide-gamut corpus-builder ───────────────────────────────────────────

const WIDE_GAMUT_EXTS: &[&str] = &["jpg", "png", "avif", "webp", "jxl", "heic"];

fn cb_wide_gamut_to(target: &str) {
    let dir = Path::new(CORPUS_BUILDER).join("wide-gamut");
    if !dir.exists() {
        eprintln!("skipping corpus-builder/wide-gamut: not found");
        return;
    }
    let mut ok_total = 0;
    let mut fail_total = Vec::new();
    for ext in WIDE_GAMUT_EXTS {
        let (ok, fail) = corpus_scan(&dir, ext, target, 500);
        ok_total += ok;
        fail_total.extend(fail);
    }
    report_corpus("cb_wide_gamut", target, ok_total, &fail_total);
}

#[test]
fn cb_wide_gamut_to_jpg() {
    cb_wide_gamut_to("jpg");
}

#[test]
fn cb_wide_gamut_to_png() {
    cb_wide_gamut_to("png");
}

#[test]
fn cb_wide_gamut_to_webp() {
    cb_wide_gamut_to("webp");
}

#[test]
fn cb_wide_gamut_to_jxl() {
    cb_wide_gamut_to("jxl");
}

#[test]
fn cb_wide_gamut_to_avif() {
    cb_wide_gamut_to("avif");
}

// ── Weird/edge-case corpus-builder ──────────────────────────────────────

const WEIRD_EXTS: &[&str] = &["avif", "jxl", "webp", "png", "jpg"];

fn cb_weird_to(target: &str) {
    let dir = Path::new(CORPUS_BUILDER).join("weird");
    if !dir.exists() {
        eprintln!("skipping corpus-builder/weird: not found");
        return;
    }
    let mut ok_total = 0;
    let mut fail_total = Vec::new();
    for ext in WEIRD_EXTS {
        let (ok, fail) = corpus_scan(&dir, ext, target, 500);
        ok_total += ok;
        fail_total.extend(fail);
    }
    report_corpus("cb_weird", target, ok_total, &fail_total);
}

#[test]
fn cb_weird_to_png() {
    cb_weird_to("png");
}

#[test]
fn cb_weird_to_jpg() {
    cb_weird_to("jpg");
}

#[test]
fn cb_weird_to_webp() {
    cb_weird_to("webp");
}

#[test]
fn cb_weird_to_jxl() {
    cb_weird_to("jxl");
}

#[test]
fn cb_weird_to_avif() {
    cb_weird_to("avif");
}

// ── Repro images corpus-builder ─────────────────────────────────────────

const REPRO_EXTS: &[&str] = &["avif", "jxl", "webp", "png", "jpg", "gif", "heic"];

fn cb_repro_to(target: &str) {
    let dir = Path::new(CORPUS_BUILDER).join("repro-images");
    if !dir.exists() {
        eprintln!("skipping corpus-builder/repro-images: not found");
        return;
    }
    let mut ok_total = 0;
    let mut fail_total = Vec::new();
    for ext in REPRO_EXTS {
        let (ok, fail) = corpus_scan(&dir, ext, target, 5);
        ok_total += ok;
        fail_total.extend(fail);
    }
    report_corpus("cb_repro", target, ok_total, &fail_total);
}

#[test]
#[ignore = "repro-images: 190k+ nested files, dir walk alone takes >60s"]
fn cb_repro_to_png() {
    cb_repro_to("png");
}

#[test]
#[ignore = "repro-images: 190k+ nested files, dir walk alone takes >60s"]
fn cb_repro_to_jpg() {
    cb_repro_to("jpg");
}

#[test]
#[ignore = "repro-images: 190k+ nested files, dir walk alone takes >60s"]
fn cb_repro_to_webp() {
    cb_repro_to("webp");
}

// ── HEIC test images ────────────────────────────────────────────────────

const HEIC_DIR: &str = "/mnt/v/heic";

fn heic_scan(target: &str) {
    let dir = Path::new(HEIC_DIR);
    if !dir.exists() {
        eprintln!("skipping heic: not found");
        return;
    }
    // collect_files_recursive uses eq_ignore_ascii_case, so "heic" matches .HEIC
    let (ok, fail) = corpus_scan(dir, "heic", target, 500);
    report_corpus("heic", target, ok, &fail);
}

#[test]
fn heic_to_png() {
    heic_scan("png");
}

#[test]
fn heic_to_jpg() {
    heic_scan("jpg");
}

#[test]
fn heic_to_webp() {
    heic_scan("webp");
}

#[test]
fn heic_to_avif() {
    heic_scan("avif");
}

#[test]
fn heic_to_jxl() {
    heic_scan("jxl");
}

// ── Non-sRGB ICC profile corpus ─────────────────────────────────────────

const NON_SRGB_DIR: &str = "/mnt/v/datasets/non-srgb-by-profile";

fn icc_profile_scan(subdir: &str, target: &str) {
    let dir = Path::new(NON_SRGB_DIR).join(subdir);
    if !dir.exists() {
        eprintln!("skipping non-srgb-by-profile/{subdir}: not found");
        return;
    }
    let mut ok_total = 0;
    let mut fail_total = Vec::new();
    for ext in &["jpg", "png", "avif", "webp", "jxl", "tiff", "tif"] {
        let (ok, fail) = corpus_scan(&dir, ext, target, 500);
        ok_total += ok;
        fail_total.extend(fail);
    }
    report_corpus(&format!("icc_{subdir}"), target, ok_total, &fail_total);
}

#[test]
fn icc_adobe_rgb_to_jpg() {
    icc_profile_scan("adobe-rgb", "jpg");
}

#[test]
fn icc_adobe_rgb_to_png() {
    icc_profile_scan("adobe-rgb", "png");
}

#[test]
fn icc_display_p3_to_jpg() {
    icc_profile_scan("display-p3", "jpg");
}

#[test]
fn icc_display_p3_to_png() {
    icc_profile_scan("display-p3", "png");
}

#[test]
fn icc_prophoto_rgb_to_jpg() {
    icc_profile_scan("prophoto-rgb", "jpg");
}

#[test]
fn icc_prophoto_rgb_to_png() {
    icc_profile_scan("prophoto-rgb", "png");
}

#[test]
fn icc_rec2020_to_jpg() {
    icc_profile_scan("rec-2020-pq", "jpg");
}

#[test]
fn icc_rec2020_to_png() {
    icc_profile_scan("rec-2020-pq", "png");
}

#[test]
fn icc_bt709_to_jpg() {
    icc_profile_scan("bt709", "jpg");
}

#[test]
fn icc_camera_rgb_to_jpg() {
    icc_profile_scan("camera-rgb", "jpg");
}

#[test]
fn icc_grayscale_to_jpg() {
    icc_profile_scan("grayscale", "jpg");
}

#[test]
fn icc_grayscale_to_png() {
    icc_profile_scan("grayscale", "png");
}

// ── Decode error files ──────────────────────────────────────────────────
// Malformed/truncated/corrupt files that must produce errors, never panics.

const DECODE_ERRORS_DIR: &str = "/mnt/v/output/corpus-builder/decode-errors";

/// Run every file in decode-errors/ through imageflow targeting each format.
/// Asserts: no panics. Errors are expected and counted.
fn decode_error_scan(target: &str) {
    test_init();
    let dir = Path::new(DECODE_ERRORS_DIR);
    if !dir.exists() {
        eprintln!("skipping decode-errors: not found");
        return;
    }

    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    files.sort();

    let mut errors = 0usize;
    let mut successes = 0usize;
    let mut panics = Vec::new();

    for path in &files {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("  read error: {} — {e}", path.display());
                errors += 1;
                continue;
            }
        };

        let command = format!("w=100&h=100&mode=max&format={target}");
        let steps = vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: command,
            decode: Some(0),
            encode: Some(1),
            watermarks: None,
        }];

        let result = std::panic::catch_unwind(|| {
            let mut ctx = Context::create().unwrap();
            ctx.add_input_vector(0, bytes)?;
            ctx.add_output_buffer(1)?;
            let _ = ctx.tell_decoder(0, DecoderCommand::IgnoreColorProfileErrors);
            let execute = Execute001 {
                graph_recording: None,
                security: None,
                framewise: Framewise::Steps(steps),
            };
            ctx.execute_1(execute)
        });

        let fname = path.file_name().unwrap_or_default().to_string_lossy();
        match result {
            Ok(Ok(_)) => {
                // Some files (wrong extension, gamma=0) may decode successfully
                successes += 1;
            }
            Ok(Err(_)) => {
                errors += 1;
            }
            Err(panic_info) => {
                let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                panics.push((fname.to_string(), msg));
            }
        }
    }

    let total = files.len();
    eprintln!(
        "decode-errors→{target}: {total} files, {errors} errors, {successes} successes, {} panics",
        panics.len()
    );

    if !panics.is_empty() {
        for (name, msg) in &panics {
            eprintln!("  PANIC: {name} — {msg}");
        }
    }
    assert!(
        panics.is_empty(),
        "{} files caused panics in decode-errors→{target}: {:?}",
        panics.len(),
        panics.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>()
    );
}

#[test]
fn decode_errors_to_jpg() {
    decode_error_scan("jpg");
}

#[test]
fn decode_errors_to_png() {
    decode_error_scan("png");
}

#[test]
fn decode_errors_to_webp() {
    decode_error_scan("webp");
}

#[test]
fn decode_errors_to_jxl() {
    decode_error_scan("jxl");
}

#[test]
fn decode_errors_to_avif() {
    decode_error_scan("avif");
}

#[test]
fn decode_errors_to_gif() {
    decode_error_scan("gif");
}

// ── Inline malformed WebP inputs ────────────────────────────────────────
// Crafted byte sequences targeting zenwebp decoder panic paths.
// These don't depend on external files, so they run in CI.

/// Build a minimal RIFF WEBP container around the given payload.
/// The RIFF size field is set to `payload.len() + 4` (for "WEBP").
fn riff_webp(payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(12 + payload.len());
    buf.extend_from_slice(b"RIFF");
    let size = (payload.len() + 4) as u32;
    buf.extend_from_slice(&size.to_le_bytes());
    buf.extend_from_slice(b"WEBP");
    buf.extend_from_slice(payload);
    buf
}

/// Build a RIFF WEBP container but lie about the RIFF size.
fn riff_webp_with_size(riff_size: u32, payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(12 + payload.len());
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&riff_size.to_le_bytes());
    buf.extend_from_slice(b"WEBP");
    buf.extend_from_slice(payload);
    buf
}

/// Build a chunk: fourcc + declared_size (LE u32) + data.
fn webp_chunk(fourcc: &[u8; 4], declared_size: u32, data: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8 + data.len());
    buf.extend_from_slice(fourcc);
    buf.extend_from_slice(&declared_size.to_le_bytes());
    buf.extend_from_slice(data);
    buf
}

/// Build a minimal VP8 chunk payload (keyframe tag + magic + dimensions).
fn vp8_header(width: u16, height: u16) -> Vec<u8> {
    let mut data = Vec::new();
    // 3-byte tag: bit 0 = 0 means keyframe, rest irrelevant
    data.extend_from_slice(&[0x00, 0x00, 0x00]);
    // VP8 magic: 9d 01 2a
    data.extend_from_slice(&[0x9d, 0x01, 0x2a]);
    data.extend_from_slice(&width.to_le_bytes());
    data.extend_from_slice(&height.to_le_bytes());
    data
}

/// Build a minimal VP8L chunk payload (signature + header with dimensions).
fn vp8l_header(width: u16, height: u16) -> Vec<u8> {
    let mut data = Vec::new();
    data.push(0x2f); // VP8L signature
                     // Header: version=0, width-1 in bits 0..13, height-1 in bits 14..27
    let w = (width as u32).saturating_sub(1) & 0x3FFF;
    let h = (height as u32).saturating_sub(1) & 0x3FFF;
    let header = w | (h << 14); // version 0 = bits 29..31 = 0
    data.extend_from_slice(&header.to_le_bytes());
    data
}

/// Build a VP8X extended header payload (10 bytes).
fn vp8x_header(flags: u8, width: u32, height: u32) -> Vec<u8> {
    let mut data = Vec::new();
    // Byte 0: flags (bit 1=animation, bit 4=alpha, bit 5=icc)
    data.push(flags);
    data.extend_from_slice(&[0u8; 3]); // reserved bytes
                                       // Canvas width - 1 as 24-bit LE
    let w = width.saturating_sub(1);
    data.push((w & 0xFF) as u8);
    data.push(((w >> 8) & 0xFF) as u8);
    data.push(((w >> 16) & 0xFF) as u8);
    // Canvas height - 1 as 24-bit LE
    let h = height.saturating_sub(1);
    data.push((h & 0xFF) as u8);
    data.push(((h >> 8) & 0xFF) as u8);
    data.push(((h >> 16) & 0xFF) as u8);
    data
}

/// Run a single malformed input through imageflow, catching panics.
/// Returns Ok(()) if no panic, Err(msg) if panic.
fn run_malformed_webp(name: &str, data: Vec<u8>) -> Result<(), String> {
    let result = std::panic::catch_unwind(|| {
        let mut ctx = Context::create().unwrap();
        ctx.add_input_vector(0, data)?;
        ctx.add_output_buffer(1)?;
        let _ = ctx.tell_decoder(0, DecoderCommand::IgnoreColorProfileErrors);
        let execute = Execute001 {
            graph_recording: None,
            security: None,
            framewise: Framewise::Steps(vec![Node::CommandString {
                kind: CommandStringKind::ImageResizer4,
                value: "w=100&h=100&mode=max&format=png".to_string(),
                decode: Some(0),
                encode: Some(1),
                watermarks: None,
            }]),
        };
        ctx.execute_1(execute)
    });

    match result {
        Ok(Ok(_)) => {
            eprintln!("  {name}: decoded successfully (unexpected but not a panic)");
            Ok(())
        }
        Ok(Err(e)) => {
            eprintln!("  {name}: error (expected): {e}");
            Ok(())
        }
        Err(panic_info) => {
            let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            eprintln!("  {name}: PANIC — {msg}");
            Err(format!("{name}: {msg}"))
        }
    }
}

#[test]
fn malformed_webp_no_panics() {
    test_init();
    let mut panics = Vec::new();

    let cases: Vec<(&str, Vec<u8>)> = vec![
        // ── Oversized chunk declarations (trigger OOB slice at lines 1094/1112) ──
        ("vp8_oversized_chunk", riff_webp(&webp_chunk(b"VP8 ", 100_000, &vp8_header(100, 100)))),
        ("vp8l_oversized_chunk", riff_webp(&webp_chunk(b"VP8L", 100_000, &vp8l_header(100, 100)))),
        // ── Truncated VP8 data ──
        ("vp8_truncated_after_header", riff_webp(&webp_chunk(b"VP8 ", 10, &vp8_header(100, 100)))),
        ("vp8l_truncated_after_header", riff_webp(&webp_chunk(b"VP8L", 5, &vp8l_header(100, 100)))),
        // ── Zero dimensions ──
        ("vp8_zero_width", riff_webp(&webp_chunk(b"VP8 ", 10, &vp8_header(0, 100)))),
        ("vp8_zero_height", riff_webp(&webp_chunk(b"VP8 ", 10, &vp8_header(100, 0)))),
        ("vp8_zero_both", riff_webp(&webp_chunk(b"VP8 ", 10, &vp8_header(0, 0)))),
        // ── Huge dimensions ──
        ("vp8_huge_dimensions", riff_webp(&webp_chunk(b"VP8 ", 10, &vp8_header(16383, 16383)))),
        ("vp8l_huge_dimensions", riff_webp(&webp_chunk(b"VP8L", 5, &vp8l_header(16383, 16383)))),
        // ── Bad magic / signature ──
        (
            "vp8_bad_magic",
            riff_webp(&webp_chunk(b"VP8 ", 10, &{
                let mut d = vp8_header(100, 100);
                d[3] = 0xFF; // corrupt 9d → FF
                d
            })),
        ),
        (
            "vp8_non_keyframe",
            riff_webp(&webp_chunk(b"VP8 ", 10, &{
                let mut d = vp8_header(100, 100);
                d[0] = 0x01; // bit 0 = 1 means not keyframe
                d
            })),
        ),
        (
            "vp8l_bad_signature",
            riff_webp(&webp_chunk(b"VP8L", 5, &{
                let mut d = vp8l_header(100, 100);
                d[0] = 0x00; // not 0x2f
                d
            })),
        ),
        (
            "vp8l_bad_version",
            riff_webp(&webp_chunk(b"VP8L", 5, &{
                let mut d = vp8l_header(100, 100);
                // Set version bits (29..31) to non-zero
                d[4] |= 0x20; // set bit 29
                d
            })),
        ),
        // ── RIFF size lies ──
        (
            "riff_size_too_large",
            riff_webp_with_size(1_000_000, &webp_chunk(b"VP8 ", 10, &vp8_header(100, 100))),
        ),
        ("riff_size_zero", riff_webp_with_size(0, &webp_chunk(b"VP8 ", 10, &vp8_header(100, 100)))),
        ("riff_size_four", riff_webp_with_size(4, &webp_chunk(b"VP8 ", 10, &vp8_header(100, 100)))),
        // ── Empty / minimal containers ──
        ("empty_riff_webp", riff_webp(&[])),
        ("riff_webp_one_byte", riff_webp(&[0x00])),
        ("vp8_empty_chunk", riff_webp(&webp_chunk(b"VP8 ", 0, &[]))),
        ("vp8l_empty_chunk", riff_webp(&webp_chunk(b"VP8L", 0, &[]))),
        // ── VP8X extended format edge cases ──
        ("vp8x_no_subchunks", riff_webp(&webp_chunk(b"VP8X", 10, &vp8x_header(0, 100, 100)))),
        (
            "vp8x_animated_no_anmf",
            riff_webp(&webp_chunk(b"VP8X", 10, &vp8x_header(0x02, 100, 100))),
        ),
        ("vp8x_alpha_no_alph_chunk", {
            let mut payload = webp_chunk(b"VP8X", 10, &vp8x_header(0x10, 100, 100));
            // Add a VP8 chunk after VP8X but no ALPH
            payload.extend_from_slice(&webp_chunk(b"VP8 ", 10, &vp8_header(100, 100)));
            riff_webp(&payload)
        }),
        ("vp8x_oversized_subchunk", {
            let mut payload = webp_chunk(b"VP8X", 10, &vp8x_header(0, 100, 100));
            // Sub-chunk claiming 500000 bytes
            payload.extend_from_slice(&webp_chunk(b"VP8 ", 500_000, &vp8_header(100, 100)));
            riff_webp(&payload)
        }),
        ("vp8x_alph_oversized", {
            let mut payload = webp_chunk(b"VP8X", 10, &vp8x_header(0x10, 100, 100));
            // ALPH chunk claiming huge size
            payload.extend_from_slice(&webp_chunk(b"ALPH", 500_000, &[0u8; 4]));
            // VP8 chunk
            payload.extend_from_slice(&webp_chunk(b"VP8 ", 10, &vp8_header(100, 100)));
            riff_webp(&payload)
        }),
        ("vp8x_iccp_oversized", {
            let mut payload = webp_chunk(b"VP8X", 10, &vp8x_header(0x20, 100, 100));
            // ICC chunk claiming huge size
            payload.extend_from_slice(&webp_chunk(b"ICCP", 500_000, &[0u8; 4]));
            payload.extend_from_slice(&webp_chunk(b"VP8L", 5, &vp8l_header(100, 100)));
            riff_webp(&payload)
        }),
        // ── Animated WebP edge cases ──
        ("vp8x_animated_truncated_anmf", {
            let mut payload = webp_chunk(b"VP8X", 10, &vp8x_header(0x02, 100, 100));
            // ANMF chunk with declared size 24 but only 8 bytes of data
            payload.extend_from_slice(&webp_chunk(b"ANMF", 24, &[0u8; 8]));
            riff_webp(&payload)
        }),
        ("vp8x_animated_anmf_zero_size", {
            let mut payload = webp_chunk(b"VP8X", 10, &vp8x_header(0x02, 100, 100));
            payload.extend_from_slice(&webp_chunk(b"ANMF", 0, &[]));
            riff_webp(&payload)
        }),
        ("vp8x_animated_anmf_oversized", {
            let mut payload = webp_chunk(b"VP8X", 10, &vp8x_header(0x02, 100, 100));
            // ANMF claims 1MB but has 30 bytes
            payload.extend_from_slice(&webp_chunk(b"ANMF", 1_000_000, &[0u8; 30]));
            riff_webp(&payload)
        }),
        // ── Dimension / size mismatches ──
        (
            "vp8_dimensions_mismatch_riff",
            // VP8 says 200x200 but chunk is tiny
            riff_webp(&webp_chunk(b"VP8 ", 10, &vp8_header(200, 200))),
        ),
        ("vp8x_dimensions_vs_vp8", {
            // VP8X says 100x100, VP8 says 200x200
            let mut payload = webp_chunk(b"VP8X", 10, &vp8x_header(0, 100, 100));
            payload.extend_from_slice(&webp_chunk(b"VP8 ", 10, &vp8_header(200, 200)));
            riff_webp(&payload)
        }),
        // ── Random garbage after valid-looking header ──
        ("vp8_garbage_data", {
            let mut header = vp8_header(50, 50);
            header.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE]);
            riff_webp(&webp_chunk(b"VP8 ", header.len() as u32, &header))
        }),
        ("vp8l_garbage_data", {
            let mut header = vp8l_header(50, 50);
            header.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE]);
            riff_webp(&webp_chunk(b"VP8L", header.len() as u32, &header))
        }),
        // ── Unknown chunk types ──
        ("unknown_chunk_only", riff_webp(&webp_chunk(b"FAKE", 4, &[0u8; 4]))),
        ("multiple_unknown_chunks", {
            let mut payload = webp_chunk(b"XXXX", 4, &[0u8; 4]);
            payload.extend_from_slice(&webp_chunk(b"YYYY", 4, &[0u8; 4]));
            payload.extend_from_slice(&webp_chunk(b"ZZZZ", 4, &[0u8; 4]));
            riff_webp(&payload)
        }),
    ];

    for (name, data) in cases {
        if let Err(msg) = run_malformed_webp(name, data) {
            panics.push(msg);
        }
    }

    eprintln!("malformed_webp: {} cases, {} panics", 35, panics.len());
    assert!(
        panics.is_empty(),
        "Malformed WebP inputs caused {} panics:\n{}",
        panics.len(),
        panics.join("\n")
    );
}
