use crate::ir4::parsing::*;
use imageflow_types::AllowedFormats;
use imageflow_types::{self as s, *};

pub fn read_allowed_formats(i: &Instructions) -> AllowedFormats {
    // It wasn't a single format, so we need to figure out the options
    let webp = i.accept_webp.unwrap_or(false);
    let jxl = i.accept_jxl.unwrap_or(false);
    let avif = i.accept_avif.unwrap_or(false);
    let custom_color_profiles = i.accept_color_profiles.unwrap_or(false);

    let allowed_formats = AllowedFormats {
        webp: Some(webp),
        jxl: Some(jxl),
        avif: Some(avif),
        ..AllowedFormats::web_safe().set_color_profiles(custom_color_profiles)
    };
    if !allowed_formats.any_formats_enabled() {
        panic!("No formats enabled");
    }
    allowed_formats
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
    s::EncoderHints {
        jpeg: Some(s::JpegEncoderHints {
            progressive: i.jpeg_progressive,
            quality: i.jpeg_quality.map(|v| v as f32).or(i.quality.map(|v| v as f32)),
            mimic: if i.jpeg_li == Some(true) {
                Some(JpegEncoderStyle::Jpegli)
            } else if i.jpeg_turbo == Some(true) {
                Some(JpegEncoderStyle::LibjpegTurbo)
            } else {
                Some(JpegEncoderStyle::Default)
            },
            //TODO: Subsampling is ignored. deprecate it or implement it
        }),
        png: Some(s::PngEncoderHints {
            lossless: i.png_lossless, //Some(i.png_lossless.unwrap_or(i.png_libpng == Some(true) || i.png_quality.is_none())),
            quality: i.png_quality.map(|v| v as f32),
            min_quality: i.png_min_quality.map(|v| v as f32),
            quantization_speed: i.png_quantization_speed,
            hint_max_deflate: i.png_max_deflate,
            mimic: if i.png_libpng == Some(true) {
                Some(PngEncoderStyle::Libpng)
            } else {
                Some(PngEncoderStyle::Default)
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
