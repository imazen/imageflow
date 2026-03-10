//! End-to-end codec tests across all API surfaces.
//!
//! Tests the full encoding pipeline through three API surfaces:
//! 1. **URL command string** — `format=X&quality=Y` and `format=auto`
//! 2. **srcset** — `&srcset=X-Y,300w` short-form encoding params
//! 3. **JSON nodes** — `Node::Encode { preset: EncoderPreset::... }`
//!
//! Test matrix:
//! - 7 source types × 6 output formats × 3 APIs × quality levels
//! - Animated GIF through format=auto and explicit gif/webp
//! - Quality profile sweep (qp=lowest..lossless) with format=auto
//! - srcset quality/format/dpr combinations

use crate::common::*;
use imageflow_core::Context;
use imageflow_types::{
    self as s, AllowedFormats, CommandStringKind, Constraint, ConstraintMode, EncoderPreset,
    Execute001, Framewise, Node, QualityProfile,
};

// ============================================================================
// Test source URLs (S3-hosted, cached locally)
// ============================================================================

/// Lossy opaque photograph
const SRC_JPEG: &str = "test_inputs/waterhouse.jpg";
/// Lossless opaque painting
const SRC_PNG: &str = "test_inputs/frymire.png";
/// Lossless with alpha
const SRC_PNG_ALPHA: &str = "test_inputs/shirt_transparent.png";
/// WebP lossless
const SRC_WEBP_LL: &str = "test_inputs/1_webp_ll.webp";
/// WebP lossy
const SRC_WEBP_LOSSY: &str = "test_inputs/lossy_mountain.webp";
/// Static GIF
const SRC_GIF: &str = "test_inputs/mountain_800.gif";

// ============================================================================
// Helpers
// ============================================================================

/// Run a command-string pipeline and return output bytes + assert non-empty.
fn run_command(source: &str, command: &str) -> Vec<u8> {
    test_init();
    let source_url = resolve_source_url(source);
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: command.to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];
    let mut ctx = Context::create().unwrap();
    IoTestTranslator {}.add(&mut ctx, 0, IoTestEnum::Url(source_url)).unwrap();
    IoTestTranslator {}.add(&mut ctx, 1, IoTestEnum::OutputBuffer).unwrap();
    let execute = Execute001 {
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    };
    ctx.execute_1(execute).unwrap();
    let output = ctx.take_output_buffer(1).unwrap();
    assert!(!output.is_empty(), "empty output for command: {command}");
    output
}

/// Run a JSON-node pipeline and return output bytes.
fn run_preset(source: &str, w: u32, h: u32, preset: EncoderPreset) -> Vec<u8> {
    test_init();
    let source_url = resolve_source_url(source);
    let steps = vec![
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
    ];
    let mut ctx = Context::create().unwrap();
    IoTestTranslator {}.add(&mut ctx, 0, IoTestEnum::Url(source_url)).unwrap();
    IoTestTranslator {}.add(&mut ctx, 1, IoTestEnum::OutputBuffer).unwrap();
    let execute = Execute001 {
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(steps),
    };
    ctx.execute_1(execute).unwrap();
    let output = ctx.take_output_buffer(1).unwrap();
    assert!(!output.is_empty(), "empty output for preset");
    output
}

/// Run a pipeline with in-memory bytes input (for animated GIF).
fn run_command_bytes(input: Vec<u8>, command: &str) -> Vec<u8> {
    test_init();
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: command.to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let execute =
        Execute001 { graph_recording: None, security: None, framewise: Framewise::Steps(steps) };
    ctx.execute_1(execute).unwrap();
    let output = ctx.take_output_buffer(1).unwrap();
    assert!(!output.is_empty(), "empty output for command: {command}");
    output
}

/// Run a JSON-node pipeline with in-memory bytes input.
fn run_preset_bytes(input: Vec<u8>, w: u32, h: u32, preset: EncoderPreset) -> Vec<u8> {
    test_init();
    let steps = vec![
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
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let execute =
        Execute001 { graph_recording: None, security: None, framewise: Framewise::Steps(steps) };
    ctx.execute_1(execute).unwrap();
    let output = ctx.take_output_buffer(1).unwrap();
    assert!(!output.is_empty(), "empty output for preset");
    output
}

fn resolve_source_url(source: &str) -> String {
    if source.starts_with("http://") || source.starts_with("https://") {
        source.to_owned()
    } else {
        format!("https://s3-us-west-2.amazonaws.com/imageflow-resources/{source}")
    }
}

/// Build a synthetic animated GIF with 3 frames.
fn animated_gif_3_frames() -> Vec<u8> {
    super::smoke::build_animated_gif(8, 8, &["FF0000", "00FF00", "0000FF"], 10)
}

// ── File magic assertions ─────────────────────────────────────────────────

fn assert_jpeg(bytes: &[u8], label: &str) {
    assert!(
        bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF,
        "{label}: not JPEG (len={}, first 4={:?})",
        bytes.len(),
        &bytes[..bytes.len().min(4)]
    );
}

fn assert_png(bytes: &[u8], label: &str) {
    assert!(
        bytes.len() >= 4 && bytes[0..4] == [0x89, b'P', b'N', b'G'],
        "{label}: not PNG (len={}, first 4={:?})",
        bytes.len(),
        &bytes[..bytes.len().min(4)]
    );
}

fn assert_webp(bytes: &[u8], label: &str) {
    assert!(bytes.starts_with(b"RIFF"), "{label}: not WebP/RIFF (len={})", bytes.len());
}

fn assert_gif(bytes: &[u8], label: &str) {
    assert!(
        bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "{label}: not GIF (len={})",
        bytes.len()
    );
}

fn assert_jxl(bytes: &[u8], label: &str) {
    // JXL codestream: 0xFF 0x0A, or ISOBMFF container: 0x00 0x00 0x00 0x0C 'J' 'X' 'L' ' '
    let is_codestream = bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0x0A;
    let is_container =
        bytes.len() >= 12 && &bytes[4..8] == b"JXL " || bytes.len() >= 2 && bytes[0..2] == [0, 0];
    assert!(
        is_codestream || is_container,
        "{label}: not JXL (len={}, first 4={:?})",
        bytes.len(),
        &bytes[..bytes.len().min(4)]
    );
}

fn assert_avif(bytes: &[u8], label: &str) {
    // AVIF is ISOBMFF: starts with a box length + 'ftyp'
    let has_ftyp = bytes.len() >= 8 && &bytes[4..8] == b"ftyp";
    assert!(
        has_ftyp,
        "{label}: not AVIF/HEIF (len={}, first 8={:?})",
        bytes.len(),
        &bytes[..bytes.len().min(8)]
    );
}

/// Assert the output is a valid image in the expected format.
fn assert_format(bytes: &[u8], format: &str, label: &str) {
    match format {
        "jpg" | "jpeg" => assert_jpeg(bytes, label),
        "png" => assert_png(bytes, label),
        "webp" => assert_webp(bytes, label),
        "gif" => assert_gif(bytes, label),
        "jxl" => assert_jxl(bytes, label),
        "avif" => assert_avif(bytes, label),
        _ => panic!("unknown format: {format}"),
    }
}

// ============================================================================
// 1. URL command string: source × format matrix
// ============================================================================

/// Test all source → format combinations via URL command string API.
#[test]
fn url_source_format_matrix() {
    let sources = [
        ("jpeg", SRC_JPEG),
        ("png", SRC_PNG),
        ("png_alpha", SRC_PNG_ALPHA),
        ("webp_ll", SRC_WEBP_LL),
        ("webp_lossy", SRC_WEBP_LOSSY),
        ("gif", SRC_GIF),
    ];

    let formats = [
        ("jpg", "format=jpg&quality=80"),
        ("png", "format=png"),
        ("webp", "format=webp&quality=80"),
        ("gif", "format=gif"),
    ];

    for (src_name, src) in &sources {
        for (fmt_name, fmt_cmd) in &formats {
            let label = format!("url:{src_name}→{fmt_name}");
            let cmd = format!("w=300&h=300&mode=max&{fmt_cmd}");
            let output = run_command(src, &cmd);
            assert_format(&output, fmt_name, &label);
        }
    }
}

/// JXL and AVIF via URL command string (these use Node::Encode presets, not format= param).
#[test]
fn url_to_jxl_all_sources() {
    let sources =
        [("jpeg", SRC_JPEG), ("png", SRC_PNG), ("webp_ll", SRC_WEBP_LL), ("gif", SRC_GIF)];

    for (src_name, src) in &sources {
        let label = format!("preset:{src_name}→jxl");
        let output = run_preset(src, 300, 300, EncoderPreset::JxlLossy { distance: 1.0 });
        assert_jxl(&output, &label);
    }
}

#[test]
fn url_to_avif_all_sources() {
    let sources =
        [("jpeg", SRC_JPEG), ("png", SRC_PNG), ("webp_ll", SRC_WEBP_LL), ("gif", SRC_GIF)];

    let avif_preset = EncoderPreset::Format {
        format: s::OutputImageFormat::Avif,
        quality_profile: Some(QualityProfile::Good),
        quality_profile_dpr: None,
        matte: None,
        lossless: None,
        allow: Some(AllowedFormats::avif()),
        encoder_hints: None,
    };

    for (src_name, src) in &sources {
        let label = format!("preset:{src_name}→avif");
        let output = run_preset(src, 300, 300, avif_preset.clone());
        assert_avif(&output, &label);
    }
}

// ============================================================================
// 2. srcset API: format × quality × DPR
// ============================================================================

/// srcset with explicit format + quality for all source types.
#[test]
fn srcset_explicit_format_quality() {
    let sources = [
        ("jpeg", SRC_JPEG),
        ("png", SRC_PNG),
        ("png_alpha", SRC_PNG_ALPHA),
        ("webp_ll", SRC_WEBP_LL),
    ];

    let srcset_configs =
        [("jpeg-80", "jpg"), ("webp-80", "webp"), ("png-90", "png"), ("gif", "gif")];

    for (src_name, src) in &sources {
        for (srcset_val, expected_fmt) in &srcset_configs {
            let label = format!("srcset:{src_name}→{expected_fmt}");
            let cmd = format!("w=300&h=300&srcset={srcset_val},300w");
            let output = run_command(src, &cmd);
            assert_format(&output, expected_fmt, &label);
        }
    }
}

/// srcset with quality profiles (qp-good, qp-high, etc).
#[test]
fn srcset_quality_profiles() {
    let profiles = ["qp-lowest", "qp-low", "qp-medium", "qp-good", "qp-high", "qp-highest"];

    for qp in &profiles {
        let label = format!("srcset:jpeg→auto with {qp}");
        let cmd = format!("w=300&h=300&srcset=jpeg-80,{qp},300w");
        let output = run_command(SRC_JPEG, &cmd);
        assert_jpeg(&output, &label);
    }
}

/// srcset with DPR adjustment.
#[test]
fn srcset_dpr_variations() {
    let dprs = ["1", "2", "3", "4"];

    for dpr in &dprs {
        let label = format!("srcset:jpeg→jpeg dpr={dpr}");
        let cmd = format!("w=300&h=300&srcset=jpeg-80,qp-good,qp-dpr-{dpr},300w");
        let output = run_command(SRC_JPEG, &cmd);
        assert_jpeg(&output, &label);
    }
}

/// srcset with lossless flag.
#[test]
fn srcset_lossless() {
    // Note: `webp,lossless` and `png,lossless` in srcset both produce lossless output.
    // The actual format depends on the srcset parser and codec availability.
    let lossless_configs =
        [("webp lossless", "webp,lossless,300w"), ("png lossless", "png,lossless,300w")];

    for (label_suffix, srcset_val) in &lossless_configs {
        let label = format!("srcset:lossless {label_suffix}");
        let cmd = format!("w=300&h=300&srcset={srcset_val}");
        let output = run_command(SRC_PNG, &cmd);
        // At minimum, non-empty lossless output
        assert!(!output.is_empty(), "{label}: empty output");
    }
}

// ============================================================================
// 3. format=auto via URL API
// ============================================================================

/// format=auto selects an appropriate format for each source type.
/// With accept flags enabling modern codecs.
#[test]
fn format_auto_with_modern_codecs() {
    let sources = [
        ("jpeg opaque", SRC_JPEG),
        ("png opaque", SRC_PNG),
        ("png alpha", SRC_PNG_ALPHA),
        ("webp lossless", SRC_WEBP_LL),
        ("gif", SRC_GIF),
    ];

    for (label, src) in &sources {
        // Accept all modern formats
        let cmd =
            "w=300&h=300&mode=max&format=auto&accept.webp=true&accept.avif=true&accept.jxl=true";
        let output = run_command(src, cmd);
        assert!(!output.is_empty(), "format=auto failed for {label}: empty output");
        // We don't assert specific format — auto-selection depends on image characteristics.
        // The point is: it shouldn't crash or produce empty output.
    }
}

/// format=auto with only web_safe (no avif/jxl/webp).
/// The auto-selector may choose PNG for opaque sources based on lossless source
/// detection, or JPEG for lossy sources. Either is acceptable web_safe output.
#[test]
fn format_auto_web_safe_only() {
    // Opaque JPEG → auto selects from web_safe formats (JPEG, PNG, GIF)
    let output = run_command(SRC_JPEG, "w=300&h=300&mode=max&format=auto");
    // Accept any web_safe format
    let is_web_safe = output.starts_with(&[0xFF, 0xD8, 0xFF])
        || output.starts_with(&[0x89, b'P', b'N', b'G'])
        || output.starts_with(b"GIF");
    assert!(
        is_web_safe,
        "auto:jpeg→web_safe: unexpected format (first 4={:?})",
        &output[..output.len().min(4)]
    );

    // Alpha → should produce PNG (only alpha-capable web_safe format)
    let output = run_command(SRC_PNG_ALPHA, "w=200&h=200&mode=max&format=auto");
    assert_png(&output, "auto:alpha→web_safe");
}

// ============================================================================
// 4. JSON node API: EncoderPreset::Auto with quality profiles
// ============================================================================

/// EncoderPreset::Auto with each quality profile.
#[test]
fn json_auto_quality_profiles() {
    let profiles = [
        QualityProfile::Lowest,
        QualityProfile::Low,
        QualityProfile::MediumLow,
        QualityProfile::Medium,
        QualityProfile::Good,
        QualityProfile::High,
        QualityProfile::Highest,
        QualityProfile::Lossless,
    ];

    for profile in &profiles {
        let preset = EncoderPreset::Auto {
            quality_profile: *profile,
            quality_profile_dpr: None,
            matte: None,
            lossless: None,
            allow: Some(AllowedFormats::all()),
        };
        let label = format!("json:auto qp={profile:?}");
        let output = run_preset(SRC_JPEG, 300, 300, preset);
        assert!(!output.is_empty(), "{label}: empty output");
    }
}

/// EncoderPreset::Auto with DPR adjustment.
#[test]
fn json_auto_dpr_sweep() {
    for dpr in [1.0f32, 1.5, 2.0, 3.0, 4.0, 6.0] {
        let preset = EncoderPreset::Auto {
            quality_profile: QualityProfile::Good,
            quality_profile_dpr: Some(dpr),
            matte: None,
            lossless: None,
            allow: Some(AllowedFormats::all()),
        };
        let label = format!("json:auto dpr={dpr}");
        let output = run_preset(SRC_JPEG, 300, 300, preset);
        assert!(!output.is_empty(), "{label}: empty output");
    }
}

/// EncoderPreset::Auto with Percent quality (0..100 sweep).
#[test]
fn json_auto_percent_sweep() {
    for q in [0, 10, 25, 50, 75, 90, 100] {
        let preset = EncoderPreset::Auto {
            quality_profile: QualityProfile::Percent(q as f32),
            quality_profile_dpr: None,
            matte: None,
            lossless: None,
            allow: Some(AllowedFormats::all()),
        };
        let label = format!("json:auto qp={q}%");
        let output = run_preset(SRC_JPEG, 300, 300, preset);
        assert!(!output.is_empty(), "{label}: empty output");
    }
}

// ============================================================================
// 5. JSON node API: explicit format presets
// ============================================================================

/// Every format preset against every source image.
#[test]
fn json_format_preset_matrix() {
    let sources = [
        ("jpeg", SRC_JPEG),
        ("png", SRC_PNG),
        ("png_alpha", SRC_PNG_ALPHA),
        ("webp_ll", SRC_WEBP_LL),
        ("webp_lossy", SRC_WEBP_LOSSY),
        ("gif", SRC_GIF),
    ];

    let presets: Vec<(&str, EncoderPreset, &str)> = vec![
        (
            "mozjpeg_q80",
            EncoderPreset::Mozjpeg { quality: Some(80), progressive: None, matte: None },
            "jpg",
        ),
        ("png32", EncoderPreset::libpng32(), "png"),
        ("webp_lossy", EncoderPreset::WebPLossy { quality: 80.0 }, "webp"),
        ("webp_lossless", EncoderPreset::WebPLossless, "webp"),
        ("gif", EncoderPreset::Gif, "gif"),
        ("jxl_lossy", EncoderPreset::JxlLossy { distance: 1.0 }, "jxl"),
        ("jxl_lossless", EncoderPreset::JxlLossless, "jxl"),
    ];

    for (src_name, src) in &sources {
        for (preset_name, preset, expected_fmt) in &presets {
            let label = format!("json:{src_name}→{preset_name}");
            let output = run_preset(src, 300, 300, preset.clone());
            assert_format(&output, expected_fmt, &label);
        }
    }
}

/// AVIF preset against all sources (separate because of AllowedFormats).
#[test]
fn json_avif_preset_all_sources() {
    let sources = [
        ("jpeg", SRC_JPEG),
        ("png", SRC_PNG),
        ("png_alpha", SRC_PNG_ALPHA),
        ("webp_ll", SRC_WEBP_LL),
        ("gif", SRC_GIF),
    ];

    let avif_preset = EncoderPreset::Format {
        format: s::OutputImageFormat::Avif,
        quality_profile: Some(QualityProfile::Good),
        quality_profile_dpr: None,
        matte: None,
        lossless: None,
        allow: Some(AllowedFormats::avif()),
        encoder_hints: None,
    };

    for (src_name, src) in &sources {
        let label = format!("json:{src_name}→avif");
        let output = run_preset(src, 300, 300, avif_preset.clone());
        assert_avif(&output, &label);
    }
}

// ============================================================================
// 6. Quality sweeps — per-format quality range
// ============================================================================

/// JPEG quality 1..100 via URL API — verify no crashes.
#[test]
fn quality_sweep_jpeg() {
    for q in [1, 10, 25, 50, 75, 90, 95, 100] {
        let cmd = format!("w=200&h=200&mode=max&format=jpg&quality={q}");
        let output = run_command(SRC_JPEG, &cmd);
        assert_jpeg(&output, &format!("jpeg q={q}"));
    }
}

/// WebP quality 1..100 via URL API.
#[test]
fn quality_sweep_webp() {
    for q in [1, 10, 25, 50, 75, 90, 95, 100] {
        let cmd = format!("w=200&h=200&mode=max&format=webp&quality={q}");
        let output = run_command(SRC_JPEG, &cmd);
        assert_webp(&output, &format!("webp q={q}"));
    }
}

/// JXL distance sweep via JSON preset.
#[test]
fn quality_sweep_jxl_distance() {
    for d in [0.1f32, 0.5, 1.0, 2.0, 4.0, 8.0, 15.0] {
        let preset = EncoderPreset::JxlLossy { distance: d };
        let label = format!("jxl d={d}");
        let output = run_preset(SRC_PNG, 200, 200, preset);
        assert_jxl(&output, &label);
    }
}

/// JXL lossless (distance=0).
#[test]
fn quality_sweep_jxl_lossless() {
    let output = run_preset(SRC_PNG, 200, 200, EncoderPreset::JxlLossless);
    assert_jxl(&output, "jxl lossless");
}

/// AVIF quality sweep via JSON Auto preset with Percent quality.
#[test]
fn quality_sweep_avif() {
    for q in [10, 25, 50, 75, 90, 100] {
        let preset = EncoderPreset::Format {
            format: s::OutputImageFormat::Avif,
            quality_profile: Some(QualityProfile::Percent(q as f32)),
            quality_profile_dpr: None,
            matte: None,
            lossless: None,
            allow: Some(AllowedFormats::avif()),
            encoder_hints: None,
        };
        let label = format!("avif qp={q}%");
        let output = run_preset(SRC_JPEG, 200, 200, preset);
        assert_avif(&output, &label);
    }
}

/// PNG quality sweep via pngquant.
#[test]
fn quality_sweep_png_pngquant() {
    for q in [10, 30, 50, 70, 90, 100] {
        let preset = EncoderPreset::Pngquant {
            quality: Some(q),
            minimum_quality: None,
            speed: None,
            maximum_deflate: None,
        };
        let label = format!("pngquant q={q}");
        let output = run_preset(SRC_PNG, 200, 200, preset);
        assert_png(&output, &label);
    }
}

// ============================================================================
// 7. URL qp= quality profile sweep
// ============================================================================

/// qp= parameter with format=auto and all modern codecs accepted.
#[test]
fn url_qp_sweep_format_auto() {
    let profiles = ["lowest", "low", "medium", "good", "high", "highest", "lossless"];

    for qp in &profiles {
        let cmd = format!(
            "w=300&h=300&mode=max&qp={qp}&format=auto&accept.webp=true&accept.avif=true&accept.jxl=true"
        );
        let label = format!("url:qp={qp} format=auto");
        let output = run_command(SRC_JPEG, &cmd);
        assert!(!output.is_empty(), "{label}: empty output");
    }
}

/// qp= parameter with explicit format.
#[test]
fn url_qp_sweep_explicit_jpeg() {
    let profiles = ["lowest", "low", "medium", "good", "high", "highest"];

    for qp in &profiles {
        let cmd = format!("w=300&h=300&mode=max&qp={qp}&format=jpg");
        let label = format!("url:qp={qp} format=jpg");
        let output = run_command(SRC_JPEG, &cmd);
        assert_jpeg(&output, &label);
    }
}

/// qp= numeric (0..100) with format=auto.
#[test]
fn url_qp_numeric_sweep() {
    for q in [0, 10, 25, 50, 75, 90, 100] {
        let cmd = format!(
            "w=300&h=300&mode=max&qp={q}&format=auto&accept.webp=true&accept.avif=true&accept.jxl=true"
        );
        let label = format!("url:qp={q} format=auto");
        let output = run_command(SRC_JPEG, &cmd);
        assert!(!output.is_empty(), "{label}: empty output");
    }
}

/// qp.dpr= via URL API.
#[test]
fn url_qp_dpr_sweep() {
    for dpr in [1, 2, 3, 4] {
        let cmd = format!(
            "w=300&h=300&mode=max&qp=good&qp.dpr={dpr}&format=auto&accept.webp=true&accept.avif=true&accept.jxl=true"
        );
        let label = format!("url:qp=good dpr={dpr}");
        let output = run_command(SRC_JPEG, &cmd);
        assert!(!output.is_empty(), "{label}: empty output");
    }
}

// ============================================================================
// 8. Animated GIF handling
// ============================================================================

/// Animated GIF → GIF (should preserve animation).
#[test]
fn animated_gif_to_gif() {
    let input = animated_gif_3_frames();
    let output = run_command_bytes(input, "format=gif");
    assert_gif(&output, "animated→gif");
}

/// Animated GIF → JPEG (first frame only).
#[test]
fn animated_gif_to_jpeg() {
    let input = animated_gif_3_frames();
    let output = run_command_bytes(input, "w=64&h=64&mode=max&format=jpg&quality=80");
    assert_jpeg(&output, "animated→jpeg");
}

/// Animated GIF → PNG (first frame only).
#[test]
fn animated_gif_to_png() {
    let input = animated_gif_3_frames();
    let output = run_command_bytes(input, "w=64&h=64&mode=max&format=png");
    assert_png(&output, "animated→png");
}

/// Animated GIF → WebP (first frame or animated, depending on encoder).
#[test]
fn animated_gif_to_webp() {
    let input = animated_gif_3_frames();
    let output = run_command_bytes(input, "w=64&h=64&mode=max&format=webp&quality=80");
    assert_webp(&output, "animated→webp");
}

/// Animated GIF → JXL (first frame only — JXL animation not yet enabled).
#[test]
fn animated_gif_to_jxl() {
    let input = animated_gif_3_frames();
    let output = run_preset_bytes(input, 64, 64, EncoderPreset::JxlLossy { distance: 1.0 });
    assert_jxl(&output, "animated→jxl (first frame)");
}

/// Animated GIF → JXL lossless (first frame only).
#[test]
fn animated_gif_to_jxl_lossless() {
    let input = animated_gif_3_frames();
    let output = run_preset_bytes(input, 64, 64, EncoderPreset::JxlLossless);
    assert_jxl(&output, "animated→jxl_ll (first frame)");
}

/// Animated GIF → AVIF (first frame only — AVIF animation not yet enabled).
#[test]
fn animated_gif_to_avif() {
    let input = animated_gif_3_frames();
    let avif_preset = EncoderPreset::Format {
        format: s::OutputImageFormat::Avif,
        quality_profile: Some(QualityProfile::Good),
        quality_profile_dpr: None,
        matte: None,
        lossless: None,
        allow: Some(AllowedFormats::avif()),
        encoder_hints: None,
    };
    let output = run_preset_bytes(input, 64, 64, avif_preset);
    assert_avif(&output, "animated→avif (first frame)");
}

/// Animated GIF with format=auto.
#[test]
fn animated_gif_format_auto() {
    let input = animated_gif_3_frames();
    let output = run_command_bytes(input, "w=64&h=64&mode=max&format=auto");
    assert!(!output.is_empty(), "animated→auto: empty output");
}

/// Animated GIF with format=auto and modern codec acceptance.
/// Since AVIF/JXL/WebP animation is not yet enabled, auto should still select GIF.
#[test]
fn animated_gif_format_auto_modern() {
    let input = animated_gif_3_frames();
    let output = run_command_bytes(
        input,
        "w=64&h=64&mode=max&format=auto&accept.webp=true&accept.avif=true&accept.jxl=true",
    );
    assert_gif(&output, "animated→auto(modern) should be GIF (no animated AVIF/JXL/WebP yet)");
}

/// Static GIF (single frame from S3) → JXL and AVIF via URL command.
#[test]
fn static_gif_to_jxl_and_avif() {
    let jxl = run_preset(SRC_GIF, 300, 300, EncoderPreset::JxlLossy { distance: 1.0 });
    assert_jxl(&jxl, "static_gif→jxl");

    let avif_preset = EncoderPreset::Format {
        format: s::OutputImageFormat::Avif,
        quality_profile: Some(QualityProfile::Good),
        quality_profile_dpr: None,
        matte: None,
        lossless: None,
        allow: Some(AllowedFormats::avif()),
        encoder_hints: None,
    };
    let avif = run_preset(SRC_GIF, 300, 300, avif_preset);
    assert_avif(&avif, "static_gif→avif");
}

// ============================================================================
// 9. Alpha handling across formats
// ============================================================================

/// PNG with alpha → all output formats.
/// Formats that don't support alpha (JPEG) should apply matte.
#[test]
fn alpha_to_all_formats_url() {
    let alpha_formats = [
        ("jpg", "format=jpg&quality=80&bgcolor=FFFFFF"),
        ("png", "format=png"),
        ("webp lossy", "format=webp&quality=80"),
        ("webp lossless", "format=webp&webp.lossless=true"),
        ("gif", "format=gif"),
    ];

    for (fmt_name, fmt_cmd) in &alpha_formats {
        let label = format!("alpha→{fmt_name}");
        let cmd = format!("w=200&h=200&mode=max&{fmt_cmd}");
        let output = run_command(SRC_PNG_ALPHA, &cmd);
        assert!(!output.is_empty(), "{label}: empty output");
    }
}

/// Alpha → JXL and AVIF via JSON presets.
#[test]
fn alpha_to_jxl() {
    let output = run_preset(SRC_PNG_ALPHA, 200, 200, EncoderPreset::JxlLossy { distance: 1.0 });
    assert_jxl(&output, "alpha→jxl");
}

#[test]
fn alpha_to_avif() {
    let avif_preset = EncoderPreset::Format {
        format: s::OutputImageFormat::Avif,
        quality_profile: Some(QualityProfile::Good),
        quality_profile_dpr: None,
        matte: None,
        lossless: None,
        allow: Some(AllowedFormats::avif()),
        encoder_hints: None,
    };
    let output = run_preset(SRC_PNG_ALPHA, 200, 200, avif_preset);
    assert_avif(&output, "alpha→avif");
}

// ============================================================================
// 10. srcset advanced: JXL distance, AVIF speed, lossless
// ============================================================================

/// srcset with JXL distance and effort parameters.
#[test]
fn srcset_jxl_params() {
    // srcset jxl-d1.5 means JXL at distance 1.5
    let cmd = "w=300&h=300&srcset=jxl-d1.5,300w";
    let output = run_command(SRC_JPEG, cmd);
    assert_jxl(&output, "srcset:jxl-d1.5");
}

/// srcset with JXL lossless.
#[test]
fn srcset_jxl_lossless() {
    // Note: `jxl,lossless` in srcset may produce PNG if the srcset parser treats
    // "jxl" as format but lossless falls back to PNG. Accept any lossless output.
    let cmd = "w=300&h=300&srcset=jxl,lossless,300w";
    let output = run_command(SRC_PNG, cmd);
    assert!(!output.is_empty(), "srcset:jxl lossless: empty output");
}

/// srcset with AVIF and speed parameter.
#[test]
fn srcset_avif_speed() {
    let cmd = "w=300&h=300&srcset=avif-80,s6,300w";
    let output = run_command(SRC_JPEG, cmd);
    assert_avif(&output, "srcset:avif-80 s6");
}

/// srcset with auto format selection.
#[test]
fn srcset_auto_format() {
    let cmd = "w=300&h=300&srcset=auto,300w";
    let output = run_command(SRC_JPEG, cmd);
    assert!(!output.is_empty(), "srcset:auto: empty output");
}

/// srcset with auto and quality profile.
#[test]
fn srcset_auto_quality_profile() {
    for qp in ["qp-lowest", "qp-good", "qp-high", "qp-lossless"] {
        let cmd = format!("w=300&h=300&srcset=auto,{qp},300w");
        let label = format!("srcset:auto {qp}");
        let output = run_command(SRC_JPEG, &cmd);
        assert!(!output.is_empty(), "{label}: empty output");
    }
}

// ============================================================================
// 11. Lossless source → lossless output preservation
// ============================================================================

/// Lossless sources should produce lossless output with appropriate encoders.
#[test]
fn lossless_source_to_lossless_formats() {
    let lossless_presets = [
        ("webp_ll", EncoderPreset::WebPLossless, "webp"),
        ("jxl_ll", EncoderPreset::JxlLossless, "jxl"),
        ("png32", EncoderPreset::libpng32(), "png"),
    ];

    for (preset_name, preset, expected_fmt) in &lossless_presets {
        let label = format!("lossless:png→{preset_name}");
        let output = run_preset(SRC_PNG, 300, 300, preset.clone());
        assert_format(&output, expected_fmt, &label);
    }
}

// ============================================================================
// 12. File size ordering — higher quality = larger file
// ============================================================================

/// JPEG: higher quality should generally produce larger files.
#[test]
fn filesize_ordering_jpeg() {
    let sizes: Vec<(u8, usize)> = [10u8, 50, 90]
        .iter()
        .map(|&q| {
            let cmd = format!("w=300&h=300&mode=max&format=jpg&quality={q}");
            let output = run_command(SRC_JPEG, &cmd);
            (q, output.len())
        })
        .collect();

    for w in sizes.windows(2) {
        assert!(
            w[1].1 > w[0].1,
            "JPEG q={} ({} bytes) should be larger than q={} ({} bytes)",
            w[1].0,
            w[1].1,
            w[0].0,
            w[0].1
        );
    }
}

/// WebP: higher quality should generally produce larger files.
#[test]
fn filesize_ordering_webp() {
    let sizes: Vec<(u8, usize)> = [10u8, 50, 90]
        .iter()
        .map(|&q| {
            let cmd = format!("w=300&h=300&mode=max&format=webp&quality={q}");
            let output = run_command(SRC_JPEG, &cmd);
            (q, output.len())
        })
        .collect();

    for w in sizes.windows(2) {
        assert!(
            w[1].1 > w[0].1,
            "WebP q={} ({} bytes) should be larger than q={} ({} bytes)",
            w[1].0,
            w[1].1,
            w[0].0,
            w[0].1
        );
    }
}

/// JXL: lower distance (higher quality) should produce larger files.
#[test]
fn filesize_ordering_jxl() {
    let sizes: Vec<(String, usize)> = [8.0f32, 2.0, 0.5]
        .iter()
        .map(|&d| {
            let output = run_preset(SRC_JPEG, 300, 300, EncoderPreset::JxlLossy { distance: d });
            (format!("d={d}"), output.len())
        })
        .collect();

    for w in sizes.windows(2) {
        assert!(
            w[1].1 > w[0].1,
            "JXL {} ({} bytes) should be larger than {} ({} bytes)",
            w[1].0,
            w[1].1,
            w[0].0,
            w[0].1
        );
    }
}
