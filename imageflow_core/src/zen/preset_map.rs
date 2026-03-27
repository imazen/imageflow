//! Map v2 [`EncoderPreset`] to zencodecs [`CodecIntent`].
//!
//! Each legacy encoder preset (Mozjpeg, LibjpegTurbo, Libpng, etc.)
//! maps to a `CodecIntent` with the appropriate format, quality, and
//! per-codec hints. The `Auto` and `Format` presets map more directly.

use std::collections::BTreeMap;

use imageflow_types::{AllowedFormats, BoolKeep, EncoderPreset, OutputImageFormat, QualityProfile};
use zencodecs::{
    BoolKeep as ZenBoolKeep, CodecIntent, FormatChoice, FormatSet, PerCodecHints,
    QualityProfile as ZenQualityProfile,
};

use super::translate::TranslateError;

/// Result of mapping an EncoderPreset.
pub struct PresetMapping {
    /// The codec intent for format/quality resolution.
    pub intent: CodecIntent,
    /// The original format if explicitly specified (for legacy presets).
    pub explicit_format: Option<zencodecs::ImageFormat>,
}

/// Map a v2 EncoderPreset to a zencodecs CodecIntent.
pub fn map_preset(preset: &EncoderPreset) -> Result<PresetMapping, TranslateError> {
    match preset {
        EncoderPreset::Auto { quality_profile, quality_profile_dpr, matte, lossless, allow } => {
            let mut intent = CodecIntent::default();
            intent.format = Some(FormatChoice::Auto);
            intent.quality_profile = Some(map_quality_profile(quality_profile));
            intent.quality_dpr = *quality_profile_dpr;
            intent.lossless = lossless.map(map_bool_keep);

            if let Some(allow) = allow {
                intent.allowed = map_allowed_formats(allow);
            }

            if let Some(matte) = matte {
                intent.matte = color_to_rgb(matte);
            }

            Ok(PresetMapping { intent, explicit_format: None })
        }

        EncoderPreset::Format {
            format,
            quality_profile,
            quality_profile_dpr,
            matte,
            lossless,
            allow,
            encoder_hints,
        } => {
            let mut intent = CodecIntent::default();
            // Handle Keep format: use FormatChoice::Keep so the format is
            // resolved from the source image at encode time.
            let (format_choice, explicit_format) = if *format == OutputImageFormat::Keep {
                (FormatChoice::Keep, None)
            } else {
                let zen_format = map_output_format(format);
                (FormatChoice::Specific(zen_format), Some(zen_format))
            };
            intent.format = Some(format_choice);
            intent.quality_profile = quality_profile.as_ref().map(map_quality_profile);
            intent.quality_dpr = *quality_profile_dpr;
            intent.lossless = lossless.map(map_bool_keep);

            if let Some(matte) = matte {
                intent.matte = color_to_rgb(matte);
            }

            // Map encoder hints BEFORE lossless defaults so per-codec
            // lossless hints (webp.lossless=true) are available.
            if let Some(hints) = encoder_hints {
                intent.hints = map_encoder_hints(hints);

                // When no quality profile is set, extract per-codec quality
                // from encoder hints as the fallback. This handles the common
                // case of `format=webp&quality=5` where the generic `quality=`
                // param flows through per-codec hints but not through `qp=`.
                if intent.quality_profile.is_none() {
                    // For Keep, look at PNG hints since that's the most common
                    // use case (png.quality/png.min_quality on a PNG source).
                    let resolved_format = explicit_format.unwrap_or(zencodecs::ImageFormat::Png);
                    let codec_quality = match resolved_format {
                        zencodecs::ImageFormat::WebP => hints.webp.as_ref().and_then(|w| w.quality),
                        zencodecs::ImageFormat::Jpeg => hints.jpeg.as_ref().and_then(|j| j.quality),
                        zencodecs::ImageFormat::Png => hints.png.as_ref().and_then(|p| p.quality),
                        _ => None,
                    };
                    if let Some(q) = codec_quality {
                        intent.quality_fallback = Some(q);
                    }
                }
            }

            if let Some(allow) = allow {
                intent.allowed = map_allowed_formats(allow);
            }

            // Defaults: lossless for inherently-lossless formats,
            // and honor per-codec lossless hints. Only applies when the
            // format is explicitly specified (not Keep, which resolves later).
            if intent.lossless.is_none() {
                if let Some(zen_format) = explicit_format {
                    match zen_format {
                        zencodecs::ImageFormat::Png => {
                            // If PNG quality hint is present, the user wants
                            // quantized (lossy) PNG — don't default to lossless.
                            // This mirrors v2 behavior where `png.quality=N`
                            // triggers pngquant.
                            let has_png_quality = intent.hints.png.contains_key("quality")
                                || intent.hints.png.contains_key("min_quality");
                            if !has_png_quality {
                                intent.lossless = Some(zencodecs::BoolKeep::True);
                            }
                        }
                        zencodecs::ImageFormat::Gif => {
                            intent.lossless = Some(zencodecs::BoolKeep::True);
                        }
                        zencodecs::ImageFormat::WebP => {
                            if intent.hints.webp.get("lossless").is_some_and(|v| v == "true") {
                                intent.lossless = Some(zencodecs::BoolKeep::True);
                            }
                        }
                        _ => {}
                    }
                }
            }

            Ok(PresetMapping { intent, explicit_format })
        }

        // ─── Legacy presets: map to explicit format + quality ───
        EncoderPreset::Mozjpeg { quality, progressive, matte } => {
            let mut intent = CodecIntent::default();
            intent.format = Some(FormatChoice::Specific(zencodecs::ImageFormat::Jpeg));
            if let Some(q) = quality {
                intent.quality_fallback = Some(*q as f32);
            }
            let mut jpeg_hints = BTreeMap::new();
            // Tell zencodecs/zenjpeg to use mozjpeg-compatible encoder profile.
            let preset = match progressive {
                Some(true) => "mozjpeg_progressive",
                Some(false) => "mozjpeg_baseline",
                None => "mozjpeg_progressive", // mozjpeg default is progressive
            };
            jpeg_hints.insert("preset".into(), preset.into());
            intent.hints.jpeg = jpeg_hints;
            if let Some(matte) = matte {
                intent.matte = color_to_rgb(matte);
            }
            Ok(PresetMapping { intent, explicit_format: Some(zencodecs::ImageFormat::Jpeg) })
        }

        EncoderPreset::LibjpegTurbo { quality, progressive, optimize_huffman_coding, matte } => {
            let mut intent = CodecIntent::default();
            intent.format = Some(FormatChoice::Specific(zencodecs::ImageFormat::Jpeg));
            if let Some(q) = quality {
                intent.quality_fallback = Some(*q as f32);
            }
            let mut jpeg_hints = BTreeMap::new();
            if let Some(p) = progressive {
                jpeg_hints.insert("progressive".into(), p.to_string());
            }
            if !jpeg_hints.is_empty() {
                intent.hints.jpeg = jpeg_hints;
            }
            if let Some(matte) = matte {
                intent.matte = color_to_rgb(matte);
            }
            Ok(PresetMapping { intent, explicit_format: Some(zencodecs::ImageFormat::Jpeg) })
        }

        EncoderPreset::Libpng { depth, matte, zlib_compression } => {
            let mut intent = CodecIntent::default();
            intent.format = Some(FormatChoice::Specific(zencodecs::ImageFormat::Png));
            intent.lossless = Some(ZenBoolKeep::True);
            if let Some(matte) = matte {
                intent.matte = color_to_rgb(matte);
            }
            Ok(PresetMapping { intent, explicit_format: Some(zencodecs::ImageFormat::Png) })
        }

        EncoderPreset::Pngquant { quality, minimum_quality, speed, maximum_deflate } => {
            let mut intent = CodecIntent::default();
            intent.format = Some(FormatChoice::Specific(zencodecs::ImageFormat::Png));
            if let Some(q) = quality {
                intent.quality_fallback = Some(*q as f32);
            }
            let mut png_hints = BTreeMap::new();
            if let Some(mq) = minimum_quality {
                png_hints.insert("min_quality".into(), mq.to_string());
            }
            if let Some(s) = speed {
                png_hints.insert("speed".into(), s.to_string());
            }
            if !png_hints.is_empty() {
                intent.hints.png = png_hints;
            }
            Ok(PresetMapping { intent, explicit_format: Some(zencodecs::ImageFormat::Png) })
        }

        EncoderPreset::Lodepng { maximum_deflate } => {
            let mut intent = CodecIntent::default();
            intent.format = Some(FormatChoice::Specific(zencodecs::ImageFormat::Png));
            intent.lossless = Some(ZenBoolKeep::True);
            Ok(PresetMapping { intent, explicit_format: Some(zencodecs::ImageFormat::Png) })
        }

        EncoderPreset::WebPLossy { quality } => {
            let mut intent = CodecIntent::default();
            intent.format = Some(FormatChoice::Specific(zencodecs::ImageFormat::WebP));
            intent.quality_fallback = Some(*quality);
            intent.lossless = Some(ZenBoolKeep::False);
            Ok(PresetMapping { intent, explicit_format: Some(zencodecs::ImageFormat::WebP) })
        }

        EncoderPreset::WebPLossless => {
            let mut intent = CodecIntent::default();
            intent.format = Some(FormatChoice::Specific(zencodecs::ImageFormat::WebP));
            intent.lossless = Some(ZenBoolKeep::True);
            Ok(PresetMapping { intent, explicit_format: Some(zencodecs::ImageFormat::WebP) })
        }

        EncoderPreset::Gif => {
            let mut intent = CodecIntent::default();
            intent.format = Some(FormatChoice::Specific(zencodecs::ImageFormat::Gif));
            Ok(PresetMapping { intent, explicit_format: Some(zencodecs::ImageFormat::Gif) })
        }
    }
}

// ─── Mapping helpers ───

fn map_quality_profile(qp: &QualityProfile) -> ZenQualityProfile {
    match qp {
        QualityProfile::Lowest => ZenQualityProfile::Lowest,
        QualityProfile::Low => ZenQualityProfile::Low,
        QualityProfile::MediumLow => ZenQualityProfile::MediumLow,
        QualityProfile::Medium => ZenQualityProfile::Medium,
        QualityProfile::Good => ZenQualityProfile::Good,
        QualityProfile::High => ZenQualityProfile::High,
        QualityProfile::Highest => ZenQualityProfile::Highest,
        QualityProfile::Lossless => ZenQualityProfile::Lossless,
        QualityProfile::Percent(p) => ZenQualityProfile::from_quality(*p),
    }
}

fn map_bool_keep(bk: BoolKeep) -> ZenBoolKeep {
    match bk {
        BoolKeep::True => ZenBoolKeep::True,
        BoolKeep::False => ZenBoolKeep::False,
        BoolKeep::Keep => ZenBoolKeep::Keep,
    }
}

fn map_output_format(format: &OutputImageFormat) -> zencodecs::ImageFormat {
    match format {
        OutputImageFormat::Webp => zencodecs::ImageFormat::WebP,
        OutputImageFormat::Jpeg | OutputImageFormat::Jpg => zencodecs::ImageFormat::Jpeg,
        OutputImageFormat::Png => zencodecs::ImageFormat::Png,
        OutputImageFormat::Gif => zencodecs::ImageFormat::Gif,
        OutputImageFormat::Avif => zencodecs::ImageFormat::Avif,
        OutputImageFormat::Jxl => zencodecs::ImageFormat::Jxl,
        OutputImageFormat::Keep => zencodecs::ImageFormat::Jpeg, // fallback; select.rs handles Keep
        #[allow(unreachable_patterns)]
        _ => zencodecs::ImageFormat::Jpeg,
    }
}

fn map_allowed_formats(allow: &AllowedFormats) -> FormatSet {
    let allow = allow.clone().expand_sets();
    let mut set = FormatSet::EMPTY;
    if allow.jpeg == Some(true) {
        set = set.with(zencodecs::ImageFormat::Jpeg);
    }
    if allow.png == Some(true) {
        set = set.with(zencodecs::ImageFormat::Png);
    }
    if allow.gif == Some(true) {
        set = set.with(zencodecs::ImageFormat::Gif);
    }
    if allow.webp == Some(true) {
        set = set.with(zencodecs::ImageFormat::WebP);
    }
    if allow.avif == Some(true) {
        set = set.with(zencodecs::ImageFormat::Avif);
    }
    if allow.jxl == Some(true) {
        set = set.with(zencodecs::ImageFormat::Jxl);
    }
    set
}

fn map_encoder_hints(hints: &imageflow_types::EncoderHints) -> PerCodecHints {
    let mut result = PerCodecHints::default();

    if let Some(ref jpeg) = hints.jpeg {
        let mut m = BTreeMap::new();
        if let Some(q) = jpeg.quality {
            m.insert("quality".into(), q.to_string());
        }
        if let Some(p) = jpeg.progressive {
            m.insert("progressive".into(), p.to_string());
        }
        result.jpeg = m;
    }

    if let Some(ref webp) = hints.webp {
        let mut m = BTreeMap::new();
        if let Some(q) = webp.quality {
            m.insert("quality".into(), q.to_string());
        }
        if let Some(ref lossless) = webp.lossless {
            m.insert("lossless".into(), format!("{lossless}"));
        }
        result.webp = m;
    }

    if let Some(ref png) = hints.png {
        let mut m = BTreeMap::new();
        if let Some(q) = png.quality {
            m.insert("quality".into(), q.to_string());
        }
        if let Some(mq) = png.min_quality {
            m.insert("min_quality".into(), mq.to_string());
        }
        result.png = m;
    }

    result
}

fn color_to_rgb(color: &imageflow_types::Color) -> Option<[u8; 3]> {
    match color {
        imageflow_types::Color::Transparent => None,
        imageflow_types::Color::Black => Some([0, 0, 0]),
        imageflow_types::Color::Srgb(imageflow_types::ColorSrgb::Hex(hex)) => parse_hex_rgb(hex),
    }
}

fn parse_hex_rgb(hex: &str) -> Option<[u8; 3]> {
    let hex = hex.trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some([r, g, b])
    } else {
        None
    }
}
