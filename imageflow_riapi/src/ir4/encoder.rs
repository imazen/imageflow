use crate::ir4::parsing::*;
use imageflow_types::AllowedFormats;
use imageflow_types::build_killbits::{codec_priority, CodecPriority};
use imageflow_types::{self as s, *};

/// The build-time codec-priority flavor that RIAPI translates under.
/// Exposed so hosts can log the priority that RIAPI observed — the
/// value flows through to the encoder dispatcher (via the encoder
/// hints this module sets) and shows up in the
/// `codec_substitution.codec_priority` field of any substitution
/// annotation emitted for the resulting request.
pub fn active_codec_priority() -> CodecPriority {
    codec_priority()
}

pub fn read_allowed_formats(i: &Instructions) -> AllowedFormats {
    // Don't default to false — if not specified, leave as None to inherit from web_safe
    let webp = i.accept_webp;
    let jxl = i.accept_jxl;
    let avif = i.accept_avif;
    let custom_color_profiles = i.accept_color_profiles.unwrap_or(false);

    let mut base = AllowedFormats::web_safe().set_color_profiles(custom_color_profiles);

    // Override with explicit user settings (None means use base default)
    if let Some(v) = webp {
        base.webp = Some(v);
    }
    if let Some(v) = jxl {
        base.jxl = Some(v);
    }
    if let Some(v) = avif {
        base.avif = Some(v);
    }

    if !base.any_formats_enabled() {
        panic!("No formats enabled");
    }
    base
}

pub(crate) fn calculate_encoder_preset(i: &Instructions) -> s::EncoderPreset {
    // qp affects the default format.
    let target_format_default = match i.qp.is_some() {
        true => OutputFormat::Auto,
        false => OutputFormat::Keep,
    };
    let target_format = i.format.unwrap_or(target_format_default);
    // We already handled applying the matte in the layout/resize step.
    let matte = None; //= i.bgcolor_srgb.map(|v| Color::Srgb(ColorSrgb::Hex(v.to_rrggbbaa_string())));

    match target_format.to_output_image_format() {
        None => {
            let default_qp = i
                .quality
                .map(|v| QualityProfile::Percent(v as f32))
                .unwrap_or(QualityProfile::High);

            s::EncoderPreset::Auto {
                quality_profile: i.qp.unwrap_or(default_qp),
                quality_profile_dpr: i.qp_dpr,
                matte,
                lossless: i.lossless,
                allow: Some(read_allowed_formats(i)),
            }
        }
        Some(format) => {
            s::EncoderPreset::Format {
                quality_profile: i.qp, // We don't force a default quality profile when format is explicitly specified, to maintain compatibility with older code
                quality_profile_dpr: i.qp_dpr,
                format,
                lossless: i.lossless,
                matte,
                allow: Some(read_allowed_formats(i)),
                encoder_hints: Some(read_encoder_hints(i)),
            }
        }
    }
}

fn read_encoder_hints(i: &Instructions) -> s::EncoderHints {
    let priority = codec_priority();
    s::EncoderHints {
        jpeg: Some(s::JpegEncoderHints {
            progressive: i.jpeg_progressive,
            quality: i.jpeg_quality.map(|v| v as f32).or(i.quality.map(|v| v as f32)),
            // JPEG mimic style.
            //
            // Explicit user flags win: `&jpeg.li=true` forces jpegli,
            // `&jpeg.turbo=true` forces libjpeg-turbo-shape output.
            // When the caller hasn't asked for either and we're on a
            // V2-priority build, bias toward LibjpegTurbo so the
            // dispatcher picks the legacy C backend — the dispatcher
            // treats `Some(LibjpegTurbo)` as a hint, not a hard
            // requirement, so killbits still have the final say.
            // V3 leaves the mimic blank (default) so the
            // priority-aware dispatch picks ZenJpeg.
            mimic: if i.jpeg_li == Some(true) {
                Some(JpegEncoderStyle::Jpegli)
            } else if i.jpeg_turbo == Some(true) {
                Some(JpegEncoderStyle::LibjpegTurbo)
            } else {
                match priority {
                    CodecPriority::V3ZenFirst => None,
                    CodecPriority::V2ClassicFirst => Some(JpegEncoderStyle::LibjpegTurbo),
                }
            },
            //TODO: Subsampling is ignored. deprecate it or implement it
        }),
        png: Some(s::PngEncoderHints {
            lossless: i.png_lossless, //Some(i.png_lossless.unwrap_or(i.png_libpng == Some(true) || i.png_quality.is_none())),
            quality: i.png_quality.map(|v| v as f32),
            min_quality: i.png_min_quality.map(|v| v as f32),
            quantization_speed: i.png_quantization_speed,
            hint_max_deflate: i.png_max_deflate,
            // PNG mimic style. Same logic as the JPEG branch: explicit
            // `&png.libpng=true` wins; on V2 default to the libpng
            // mimic so the dispatcher emits libpng-compatible bytes;
            // on V3 leave it blank so the priority-aware dispatch
            // picks ZenPng.
            mimic: if i.png_libpng == Some(true) {
                Some(PngEncoderStyle::Libpng)
            } else {
                match priority {
                    CodecPriority::V3ZenFirst => None,
                    CodecPriority::V2ClassicFirst => Some(PngEncoderStyle::Libpng),
                }
            },
        }),
        webp: Some(s::WebpEncoderHints {
            lossless: i.webp_lossless,
            quality: i.webp_quality.or(i.quality.map(|v| v as f32)),
        }),
        gif: Some(s::GifEncoderHints {}),
    }
}

//     pub jxl_distance: Option<f64>,// recommend 0.5 to 3.0 (96.68 jpeg equiv), default 1, full range 0..25
//     pub jxl_effort: Option<u8>,//clamped to reasonable values 0..7, 8+ blocked
//     pub jxl_quality: Option<f32>, // similar to jpeg quality, 0..100
//     //#[deprecated(since = "0.1.0", note = "replaced with shared &lossless")]
//     pub jxl_lossless: Option<BoolKeep>,// replaced with shared &lossless

//     pub avif_quality: Option<f32>,
//     pub avif_speed: Option<u8>, // 3..10, 1 and 2 are blocked for being too slow.

#[cfg(test)]
mod tests {
    use super::*;
    use imageflow_types::build_killbits::{CodecPriority, CodecPriorityGuard};

    // Serializes priority-dependent tests in this module. The override
    // is process-wide — concurrent tests that set it would race.
    static PRIORITY_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn bare_instructions() -> Instructions {
        Instructions::default()
    }

    #[test]
    fn jpeg_mimic_blank_under_v3_priority() {
        let _lock = PRIORITY_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let _g = CodecPriorityGuard::install(CodecPriority::V3ZenFirst);
        let hints = read_encoder_hints(&bare_instructions());
        let jpeg = hints.jpeg.expect("jpeg hints present");
        assert_eq!(jpeg.mimic, None, "V3 default: leave mimic blank so dispatcher picks ZenJpeg");
    }

    #[test]
    fn jpeg_mimic_libjpeg_under_v2_priority() {
        let _lock = PRIORITY_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let _g = CodecPriorityGuard::install(CodecPriority::V2ClassicFirst);
        let hints = read_encoder_hints(&bare_instructions());
        let jpeg = hints.jpeg.expect("jpeg hints present");
        assert_eq!(
            jpeg.mimic,
            Some(JpegEncoderStyle::LibjpegTurbo),
            "V2: prefer libjpeg-turbo mimic so dispatcher picks MozJpeg(c)"
        );
    }

    #[test]
    fn jpeg_explicit_li_wins_over_priority() {
        let _lock = PRIORITY_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let _g = CodecPriorityGuard::install(CodecPriority::V2ClassicFirst);
        let mut i = bare_instructions();
        i.jpeg_li = Some(true);
        let hints = read_encoder_hints(&i);
        let jpeg = hints.jpeg.expect("jpeg hints present");
        assert_eq!(
            jpeg.mimic,
            Some(JpegEncoderStyle::Jpegli),
            "explicit jpeg.li=true must win over priority default"
        );
    }

    #[test]
    fn png_mimic_blank_under_v3_priority() {
        let _lock = PRIORITY_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let _g = CodecPriorityGuard::install(CodecPriority::V3ZenFirst);
        let hints = read_encoder_hints(&bare_instructions());
        let png = hints.png.expect("png hints present");
        assert_eq!(png.mimic, None, "V3 default: ZenPng is picked by dispatcher");
    }

    #[test]
    fn png_mimic_libpng_under_v2_priority() {
        let _lock = PRIORITY_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let _g = CodecPriorityGuard::install(CodecPriority::V2ClassicFirst);
        let hints = read_encoder_hints(&bare_instructions());
        let png = hints.png.expect("png hints present");
        assert_eq!(
            png.mimic,
            Some(PngEncoderStyle::Libpng),
            "V2: prefer libpng mimic so dispatcher picks libpng/lodepng"
        );
    }

    #[test]
    fn png_explicit_libpng_wins_over_priority() {
        let _lock = PRIORITY_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let _g = CodecPriorityGuard::install(CodecPriority::V3ZenFirst);
        let mut i = bare_instructions();
        i.png_libpng = Some(true);
        let hints = read_encoder_hints(&i);
        let png = hints.png.expect("png hints present");
        assert_eq!(
            png.mimic,
            Some(PngEncoderStyle::Libpng),
            "explicit png.libpng=true must win over priority default"
        );
    }

    #[test]
    fn active_codec_priority_tracks_override() {
        let _lock = PRIORITY_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        {
            let _g = CodecPriorityGuard::install(CodecPriority::V2ClassicFirst);
            assert_eq!(active_codec_priority(), CodecPriority::V2ClassicFirst);
        }
        // Released — back to V3 default.
        assert_eq!(active_codec_priority(), CodecPriority::V3ZenFirst);
    }
}
