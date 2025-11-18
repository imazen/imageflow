//! Format selection integration tests with magic byte verification
//! Tests the interaction between allow/accept parameters and explicit format selection

#[macro_use]
extern crate imageflow_core;
#[macro_use]
extern crate lazy_static;
extern crate imageflow_helpers as hlp;
extern crate serde_json;

pub mod common;
use crate::common::*;

use imageflow_core::{Context,Result, here};
use imageflow_types::*;

// ========== Test Image Constants ==========

// Helper to generate tiny test images on demand using imageflow's own encoder
// This ensures valid image data without hardcoding bytes
fn generate_tiny_jpeg() -> Result<Vec<u8>> {
    let mut context = Context::create().unwrap();

    IoTestTranslator {}
        .add(&mut context, 1, IoTestEnum::OutputBuffer)
        .unwrap();

    let execute = Execute001 {
        graph_recording: None,
        security: None,
        framewise: Framewise::Steps(vec![
            Node::CreateCanvas {
                w: 1,
                h: 1,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FF0000FF".to_owned())),
            },
            Node::Encode {
                io_id: 1,
                preset: EncoderPreset::Format {
                    format: OutputImageFormat::Jpeg,
                    quality_profile: None,
                    quality_profile_dpr: None,
                    lossless: None,
                    matte: None,
                    allow: None,
                    encoder_hints: None,
                },
            },
        ]),
    };

    context.execute_1(execute).map_err(|e| e.at(here!()))?;
    context.get_output_buffer_slice(1).map_err(|e| e.at(here!())).map(|slice| slice.to_vec())
}

fn generate_tiny_png(with_alpha: bool) -> Result<Vec<u8>> {
    let mut context = Context::create().unwrap();

    IoTestTranslator {}
        .add(&mut context, 1, IoTestEnum::OutputBuffer)
        .unwrap();

    let format = if with_alpha {
        PixelFormat::Bgra32
    } else {
        PixelFormat::Bgr32  // RGB without alpha
    };

    let execute = Execute001 {
        graph_recording: None,
        security: None,
        framewise: Framewise::Steps(vec![
            Node::CreateCanvas {
                w: 1,
                h: 1,
                format,
                color: Color::Srgb(ColorSrgb::Hex("FF0000FF".to_owned())),
            },
            Node::Encode {
                io_id: 1,
                preset: EncoderPreset::Format {
                    format: OutputImageFormat::Png,
                    quality_profile: None,
                    quality_profile_dpr: None,
                    lossless: Some(BoolKeep::True),
                    matte: None,
                    allow: None,
                    encoder_hints: None,
                },
            },
        ]),
    };

    context.execute_1(execute).map_err(|e| e.at(here!()))?;
    context.get_output_buffer_slice(1).map_err(|e| e.at(here!())).map(|slice| slice.to_vec())
}

fn generate_tiny_gif() -> Result<Vec<u8>> {
    let mut context = Context::create().unwrap();

    IoTestTranslator {}
        .add(&mut context, 1, IoTestEnum::OutputBuffer)
        .unwrap();

    let execute = Execute001 {
        graph_recording: None,
        security: None,
        framewise: Framewise::Steps(vec![
            Node::CreateCanvas {
                w: 1,
                h: 1,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FF0000FF".to_owned())),
            },
            Node::Encode {
                io_id: 1,
                preset: EncoderPreset::Format {
                    format: OutputImageFormat::Gif,
                    quality_profile: None,
                    quality_profile_dpr: None,
                    lossless: None,
                    matte: None,
                    allow: None,
                    encoder_hints: None,
                },
            },
        ]),
    };

    context.execute_1(execute).map_err(|e| e.at(here!()))?;
    context.get_output_buffer_slice(1).map_err(|e| e.at(here!())).map(|slice| slice.to_vec())
}

// ========== Helper Functions ==========

/// Helper to check magic bytes and identify actual file format
fn check_magic_bytes(bytes: &[u8]) -> &'static str {
    if bytes.len() < 12 {
        return "too_short";
    }

    // JPEG: FF D8 FF
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return "jpeg";
    }

    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return "png";
    }

    // GIF: GIF89a or GIF87a
    if bytes.starts_with(b"GIF89a") || bytes.starts_with(b"GIF87a") {
        return "gif";
    }

    // WebP: RIFF....WEBP
    if bytes.starts_with(b"RIFF") && bytes.len() >= 12 && &bytes[8..12] == b"WEBP" {
        return "webp";
    }

    // AVIF: ....ftyp....avif (4 bytes size, then ftyp, then brand)
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" && &bytes[8..12] == b"avif" {
        return "avif";
    }

    "unknown"
}

/// Test RIAPI format selection with a command string
/// Returns the detected format via magic bytes (e.g., "avif", "jpeg", "png")
/// If source_bytes is None, uses a tiny 8x8 red canvas
pub fn test_format_selection_riapi_with_source(
    command: &str,
    source_bytes: Option<&[u8]>,
) -> Result<String> {
    use imageflow_core::Context;
    use imageflow_types as s;

    let mut context = Context::create().unwrap();

    let steps = if let Some(bytes) = source_bytes {
        // Use provided source image
        IoTestTranslator {}
            .add(&mut context, 0, IoTestEnum::ByteArray(bytes.to_vec()))
            .unwrap();
        vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: command.to_owned(),
            decode: Some(0),
            encode: Some(1),
            watermarks: None,
        }]
    } else {
        // Use canvas (no source)
        vec![
            Node::CreateCanvas {
                w: 8,
                h: 8,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FF0000FF".to_owned())),
            },
            Node::CommandString {
                kind: CommandStringKind::ImageResizer4,
                value: command.to_owned(),
                decode: None,
                encode: Some(1),
                watermarks: None,
            },
        ]
    };

    IoTestTranslator {}
        .add(&mut context, 1, IoTestEnum::OutputBuffer)
        .unwrap();

    let build = s::Execute001 {
        security: None,
        graph_recording: None,
        framewise: s::Framewise::Steps(steps),
    };

    context.execute_1(build).map_err(|e| e.at(here!()))?;

    let bytes = context.get_output_buffer_slice(1).map_err(|e| e.at(here!()))?;
    Ok(check_magic_bytes(bytes).to_owned())
}

/// Test RIAPI format selection with a command string (no source image)
pub fn test_format_selection_riapi(command: &str) -> Result<String> {
    test_format_selection_riapi_with_source(command, None).map_err(|e| e.at(here!()))
}

/// Test JSON API format selection with an EncoderPreset
/// Returns the detected format via magic bytes (e.g., "avif", "jpeg", "png")
/// If source_bytes is None, uses a tiny 8x8 red canvas
pub fn test_format_selection_json_with_source(
    preset: EncoderPreset,
    source_bytes: Option<&[u8]>,
) -> Result<String> {
    use imageflow_core::Context;
    use imageflow_types as s;

    let mut context = Context::create().unwrap();

    let steps = if let Some(bytes) = source_bytes {
        // Use provided source image
        IoTestTranslator {}
            .add(&mut context, 0, IoTestEnum::ByteArray(bytes.to_vec()))
            .unwrap();
        vec![
            Node::Decode {
                io_id: 0,
                commands: None,
            },
            Node::Encode {
                io_id: 1,
                preset,
            },
        ]
    } else {
        // Use canvas (no source)
        vec![
            Node::CreateCanvas {
                w: 8,
                h: 8,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FF0000FF".to_owned())),
            },
            Node::Encode {
                io_id: 1,
                preset,
            },
        ]
    };

    IoTestTranslator {}
        .add(&mut context, 1, IoTestEnum::OutputBuffer)
        .unwrap();

    let build = s::Execute001 {
        security: None,
        graph_recording: None,
        framewise: s::Framewise::Steps(steps),
    };

    context.execute_1(build).map_err(|e| e.at(here!()))?;

    let bytes = context.get_output_buffer_slice(1).map_err(|e| e.at(here!()))?;
    Ok(check_magic_bytes(bytes).to_owned())
}

/// Test JSON API format selection with an EncoderPreset (no source image)
pub fn test_format_selection_json(preset: EncoderPreset) -> Result<String> {
    test_format_selection_json_with_source(preset, None).map_err(|e|e.at(here!()))
}

// ========== JSON API Tests ==========

#[test]
fn test_json_format_explicit_avif_with_allow() {
    use imageflow_types as s;

    let format = test_format_selection_json(s::EncoderPreset::Format {
        format: OutputImageFormat::Avif,
        quality_profile: None,
        quality_profile_dpr: None,
        lossless: None,
        matte: None,
        allow: Some(s::AllowedFormats {
            avif: Some(true),
            ..s::AllowedFormats::none()
        }),
        encoder_hints: None,
    }).unwrap();

    assert_eq!(
        format, "avif",
        "Expected AVIF output with format=avif + allow.avif=true"
    );
}

#[test]
fn test_json_format_explicit_avif_without_allow() {
    use imageflow_types as s;

    let format = test_format_selection_json(s::EncoderPreset::Format {
        format: OutputImageFormat::Avif,
        quality_profile: None,
        quality_profile_dpr: None,
        lossless: None,
        matte: None,
        allow: Some(s::AllowedFormats {
            avif: Some(false), // Explicitly disallow AVIF
            ..s::AllowedFormats::web_safe()
        }),
        encoder_hints: None,
    }).unwrap();

    // DOCUMENTS CURRENT BEHAVIOR: explicit format bypasses allow check
    assert_eq!(
        format, "avif",
        "Current behavior: explicit format=avif bypasses allow.avif=false"
    );
}

#[test]
fn test_json_format_auto_with_avif_allowed() {
    use imageflow_types as s;

    let format = test_format_selection_json(s::EncoderPreset::Auto {
        quality_profile: s::QualityProfile::High,
        quality_profile_dpr: None,
        lossless: None,
        matte: None,
        allow: Some(s::AllowedFormats {
            avif: Some(true),
            ..s::AllowedFormats::web_safe()
        }),
    }).unwrap();

    assert_eq!(
        format, "avif",
        "Auto format should select AVIF when allow.avif=true"
    );
}

#[test]
fn test_json_format_auto_without_avif_allowed() {
    use imageflow_types as s;

    let format = test_format_selection_json(s::EncoderPreset::Auto {
        quality_profile: s::QualityProfile::High,
        quality_profile_dpr: None,
        lossless: None,
        matte: None,
        allow: Some(s::AllowedFormats::web_safe()), // No AVIF
    }).unwrap();

    assert_eq!(
        format, "jpeg",
        "Auto format should fall back to JPEG when AVIF not in allow list"
    );
}

#[test]
fn test_json_format_explicit_png() {
    use imageflow_types as s;

    let format = test_format_selection_json(s::EncoderPreset::Format {
        format: OutputImageFormat::Png,
        quality_profile: None,
        quality_profile_dpr: None,
        lossless: None,
        matte: None,
        allow: None,
        encoder_hints: None,
    }).unwrap();

    assert_eq!(format, "png", "Explicit format=png should produce PNG");
}

#[test]
fn test_json_format_explicit_webp() {
    use imageflow_types as s;

    let format = test_format_selection_json(s::EncoderPreset::Format {
        format: OutputImageFormat::Webp,
        quality_profile: None,
        quality_profile_dpr: None,
        lossless: None,
        matte: None,
        allow: Some(s::AllowedFormats {
            webp: Some(true),
            ..s::AllowedFormats::none()
        }),
        encoder_hints: None,
    }).unwrap();

    assert_eq!(format, "webp", "Explicit format=webp should produce WebP");
}

#[test]
fn test_json_format_auto_with_webp_only() {
    use imageflow_types as s;

    let format = test_format_selection_json(s::EncoderPreset::Auto {
        quality_profile: s::QualityProfile::High,
        quality_profile_dpr: None,
        lossless: None,
        matte: None,
        allow: Some(s::AllowedFormats {
            webp: Some(true),
            ..s::AllowedFormats::web_safe()
        }),
    }).unwrap();

    assert_eq!(
        format, "webp",
        "Auto format with only allow.webp=true should select WebP over JPEG"
    );
}

#[test]
fn test_json_format_auto_with_avif_and_webp() {
    use imageflow_types as s;

    let format = test_format_selection_json(s::EncoderPreset::Auto {
        quality_profile: s::QualityProfile::High,
        quality_profile_dpr: None,
        lossless: None,
        matte: None,
        allow: Some(s::AllowedFormats {
            avif: Some(true),
            webp: Some(true),
            ..s::AllowedFormats::web_safe()
        }),
    }).unwrap() ;

    assert_eq!(
        format, "avif",
        "Auto format with both allow.avif and allow.webp should prefer AVIF"
    );
}

// ========== RIAPI Tests ==========

#[test]
fn test_riapi_format_avif_with_accept() {
    let format = test_format_selection_riapi("format=avif&accept.avif=1").unwrap();

    assert_eq!(
        format, "avif",
        "RIAPI format=avif with accept.avif=1 should produce AVIF"
    );
}

#[test]
fn test_riapi_format_avif_without_accept() {
    let format = test_format_selection_riapi("format=avif").unwrap();

    assert_eq!(
        format, "avif",
        "Current behavior: RIAPI format=avif works without accept.avif"
    );
}

#[test]
fn test_riapi_format_auto_with_accept_avif() {
    let format = test_format_selection_riapi("format=auto&accept.avif=1").unwrap();

    assert_eq!(
        format, "avif",
        "RIAPI format=auto with accept.avif=1 should select AVIF"
    );
}

#[test]
fn test_riapi_format_auto_without_accept_avif() {
    let format = test_format_selection_riapi("format=auto").unwrap();

    assert_eq!(
        format, "jpeg",
        "RIAPI format=auto on opaque created canvas image without accept.avif should fall back to JPEG"
    );
}

#[test]
fn test_riapi_format_auto_with_webp_only() {
    let format = test_format_selection_riapi("format=auto&accept.webp=1").unwrap();

    assert_eq!(
        format, "webp",
        "RIAPI format=auto with accept.webp=1 should select WebP over JPEG"
    );
}

#[test]
fn test_riapi_format_auto_with_avif_and_webp() {
    let format = test_format_selection_riapi("format=auto&accept.avif=1&accept.webp=1").unwrap();

    assert_eq!(
        format, "avif",
        "RIAPI format=auto with both accept.avif and accept.webp should prefer AVIF over WebP"
    );
}

#[test]
fn test_riapi_format_png_explicit() {
    let format = test_format_selection_riapi("format=png").unwrap() ;

    assert_eq!(format, "png", "RIAPI format=png should produce PNG");
}

#[test]
fn test_riapi_format_jpeg_explicit() {
    let format = test_format_selection_riapi("format=jpg").unwrap() ;

    assert_eq!(format, "jpeg", "RIAPI format=jpg should produce JPEG");
}

#[test]
fn test_riapi_format_webp_explicit() {
    let format = test_format_selection_riapi("format=webp").unwrap();

    assert_eq!(format, "webp", "RIAPI format=webp should produce WebP");
}

#[test]
fn test_riapi_format_webp_with_quality() {
    let format = test_format_selection_riapi("format=webp&webp.quality=75").unwrap();

    assert_eq!(
        format, "webp",
        "RIAPI format=webp with quality parameter should produce WebP"
    );
}

#[test]
fn test_riapi_format_avif_with_quality_and_speed() {
    let format = test_format_selection_riapi("format=avif&avif.quality=80&avif.speed=6").unwrap();

    assert_eq!(
        format, "avif",
        "RIAPI format=avif with quality and speed parameters should produce AVIF"
    )           ;
}

// ========== Source Format Tests (format=auto behavior) ==========

#[test]
fn test_json_auto_from_jpeg_source() {
    use imageflow_types as s;

    let format = test_format_selection_json_with_source(
        s::EncoderPreset::Auto {
            quality_profile: s::QualityProfile::High,
            quality_profile_dpr: None,
            lossless: None,
            matte: None,
            allow: Some(s::AllowedFormats {
                avif: Some(true),
                ..s::AllowedFormats::web_safe()
            }),
        },
        Some(&generate_tiny_jpeg().unwrap()),
    ).unwrap()  ;

    assert_eq!(
        format, "avif",
        "Auto format from JPEG source with allow.avif should select AVIF"
    );
}

#[test]
fn test_json_auto_from_png_alpha_source() {
    use imageflow_types as s;

    let format = test_format_selection_json_with_source(
        s::EncoderPreset::Auto {
            quality_profile: s::QualityProfile::High,
            quality_profile_dpr: None,
            lossless: None,
            matte: None,
            allow: Some(s::AllowedFormats {
                avif: Some(true),
                webp: Some(true),
                ..s::AllowedFormats::web_safe()
            }),
        },
        Some(&generate_tiny_png(true).unwrap()),
    ).unwrap()  ;

    // PNG with alpha should prefer WebP for lossless alpha over AVIF
    assert_eq!(
        format, "webp",
        "Auto format from PNG with alpha should prefer WebP for lossless alpha compression"
    );
}

#[test]
fn test_json_auto_from_png_source() {
    use imageflow_types as s;

    let format = test_format_selection_json_with_source(
        s::EncoderPreset::Auto {
            quality_profile: s::QualityProfile::High,
            quality_profile_dpr: None,
            lossless: None,
            matte: None,
            allow: Some(s::AllowedFormats {
                avif: Some(true),
                ..s::AllowedFormats::web_safe()
            }),
        },
        Some(&generate_tiny_png(false).unwrap()),
    ).unwrap()  ;

    assert_eq!(
        format, "png",
        "Auto format from PNG without alpha with allow.avif should select PNG to preserve likely losslessness"
    );
}

#[test]
fn test_json_auto_from_animated_gif_source() {
    use imageflow_types as s;

    let format = test_format_selection_json_with_source(
        s::EncoderPreset::Auto {
            quality_profile: s::QualityProfile::High,
            quality_profile_dpr: None,
            lossless: None,
            matte: None,
            allow: Some(s::AllowedFormats {
                avif: Some(true),
                webp: Some(true),
                ..s::AllowedFormats::web_safe()
            }),
        },
        Some(&TINY_ANIMATED_GIF),
    ).unwrap()  ;

    // GIF source should preserve as GIF (animation capability)
    assert_eq!(
        format, "gif",
        "Auto format from GIF source should preserve as GIF for animation capability" // (until we implement webp animation)
    );
}

#[test]
fn test_riapi_auto_from_jpeg_source() {
    let format =
        test_format_selection_riapi_with_source("format=auto&accept.avif=1", Some(&generate_tiny_jpeg().unwrap())).unwrap()  ;

    assert_eq!(
        format, "avif",
        "RIAPI auto from JPEG source with accept.avif should select AVIF"
    );
}

#[test]
fn test_riapi_auto_from_png_alpha_source() {
    let format = test_format_selection_riapi_with_source(
        "format=auto&accept.avif=1&accept.webp=1",
        Some(&generate_tiny_png(true).unwrap()),
    ).unwrap()  ;

    assert_eq!(
        format, "webp",
        "RIAPI auto from PNG with alpha should prefer WebP"
    );
}

#[test]
fn test_riapi_auto_from_gif_source() {
    let format = test_format_selection_riapi_with_source(
        "format=auto&accept.avif=1&accept.webp=1",
        Some(&generate_tiny_gif().unwrap()),
    ).unwrap()  ;

    assert_eq!(
        format, "avif",
        "RIAPI auto from single-frame GIF source should switch to avif"
    );
}

#[test]
fn test_riapi_keep_from_gif_source() {
    let format = test_format_selection_riapi_with_source(
        "format=keep&accept.avif=1&accept.webp=1",
        Some(&generate_tiny_gif().unwrap()),
    ).unwrap()  ;

    assert_eq!(
        format, "gif",
        "RIAPI format=keep from GIF source should preserve as GIF"
    );
}


// Test CreateCanvas with different alpha characteristics

#[test]
fn test_json_auto_from_canvas_opaque() {
    use imageflow_types as s;

    // CreateCanvas defaults to opaque (alpha not meaningful)
    let format = test_format_selection_json(s::EncoderPreset::Auto {
        quality_profile: s::QualityProfile::High,
        quality_profile_dpr: None,
        lossless: None,
        matte: None,
        allow: Some(s::AllowedFormats {
            avif: Some(true),
            ..s::AllowedFormats::web_safe()
        }),
    }).unwrap();

    // Documents current behavior with CreateCanvas
    assert_eq!(
        format, "avif",
        "Current behavior: Auto from CreateCanvas defaults to avif (no source format to reference)"
    );
}

#[test]
fn test_riapi_auto_from_canvas() {
    // RIAPI CreateCanvas via format=auto without source
    let format = test_format_selection_riapi("format=auto&accept.avif=1").unwrap();

    // Documents current behavior
    assert_eq!(
        format, "avif",
        "Current behavior: RIAPI auto from CreateCanvas defaults to avif (no source format to reference)"
    );
}


static TINY_ANIMATED_GIF: [u8; TINY_ANIMATED_GIF_LEN] = [
  0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0xf0, 0x00,
  0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x21, 0xf9, 0x04, 0x04, 0x00,
  0x00, 0x00, 0x00, 0x21, 0xff, 0x0b, 0x4e, 0x45, 0x54, 0x53, 0x43, 0x41,
  0x50, 0x45, 0x32, 0x2e, 0x30, 0x03, 0x01, 0x00, 0x00, 0x00, 0x21, 0xff,
  0x0b, 0x49, 0x6d, 0x61, 0x67, 0x65, 0x4d, 0x61, 0x67, 0x69, 0x63, 0x6b,
  0x0e, 0x67, 0x61, 0x6d, 0x6d, 0x61, 0x3d, 0x30, 0x2e, 0x34, 0x35, 0x34,
  0x35, 0x34, 0x35, 0x00, 0x2c, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01,
  0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x21, 0xf9, 0x04, 0x04, 0x00,
  0x00, 0x00, 0x00, 0x21, 0xff, 0x0b, 0x49, 0x6d, 0x61, 0x67, 0x65, 0x4d,
  0x61, 0x67, 0x69, 0x63, 0x6b, 0x0e, 0x67, 0x61, 0x6d, 0x6d, 0x61, 0x3d,
  0x30, 0x2e, 0x34, 0x35, 0x34, 0x35, 0x34, 0x35, 0x00, 0x2c, 0x00, 0x00,
  0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x80, 0xff, 0xff, 0xff, 0x00, 0x00,
  0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x21, 0xf9, 0x04, 0x04, 0x00, 0x00,
  0x00, 0x00, 0x21, 0xff, 0x0b, 0x49, 0x6d, 0x61, 0x67, 0x65, 0x4d, 0x61,
  0x67, 0x69, 0x63, 0x6b, 0x0e, 0x67, 0x61, 0x6d, 0x6d, 0x61, 0x3d, 0x30,
  0x2e, 0x34, 0x35, 0x34, 0x35, 0x34, 0x35, 0x00, 0x2c, 0x00, 0x00, 0x00,
  0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3b
];
pub const TINY_ANIMATED_GIF_LEN: usize = 204;
