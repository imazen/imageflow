use crate::ffi;
use crate::ffi::ColorProfileSource;
use crate::ffi::DecoderColorInfo;
use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::io::IoProxy;
use imageflow_types as s;
use imageflow_types::collections::AddRemoveSet;
use imageflow_types::IoDirection;
use imageflow_types::*;
use std::any::Any;
use std::borrow::BorrowMut;
use std::ops::DerefMut;
use std::sync::*;
use uuid::Uuid;

use crate::codecs::color_transform_cache::ColorTransformCache;
use crate::codecs::Encoder;
use crate::codecs::NamedEncoders::LibPngRsEncoder;
use crate::io::IoProxyRef;
use crate::{BitmapKey, Context, ErrorCategory, ErrorKind, FlowError, JsonResponse, Result};

pub(crate) fn create_encoder(
    c: &Context,
    io: IoProxy,
    preset: &s::EncoderPreset,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
) -> Result<Box<dyn Encoder>> {
    let codec = match *preset {
        s::EncoderPreset::Auto {
            quality_profile,
            quality_profile_dpr,
            ref matte,
            lossless,
            allow,
        } => {
            let matte = matte.clone();
            let details = build_auto_encoder_details(
                c,
                preset,
                bitmap_key,
                decoder_io_ids,
                None,
                Some(quality_profile),
                quality_profile_dpr,
                matte,
                lossless,
                allow,
                None,
            )
            .map_err(|e| e.at(here!()))?;
            create_encoder_auto(c, io, bitmap_key, decoder_io_ids, details)
                .map_err(|e| e.at(here!()))?
        }
        s::EncoderPreset::Format {
            format,
            quality_profile,
            quality_profile_dpr,
            ref matte,
            lossless,
            allow,
            encoder_hints,
        } => {
            let matte = matte.clone();
            let details = build_auto_encoder_details(
                c,
                preset,
                bitmap_key,
                decoder_io_ids,
                Some(format),
                quality_profile,
                quality_profile_dpr,
                matte,
                lossless,
                allow,
                encoder_hints,
            )
            .map_err(|e| e.at(here!()))?;
            create_encoder_auto(c, io, bitmap_key, decoder_io_ids, details)
                .map_err(|e| e.at(here!()))?
        }
        s::EncoderPreset::Gif => {
            //TODO: enforce killbits - if c.enabled_codecs.encoders.contains()
            Box::new(
                crate::codecs::gif::GifEncoder::create(c, io, bitmap_key)
                    .map_err(|e| e.at(here!()))?,
            )
        }
        s::EncoderPreset::Pngquant { speed, quality, minimum_quality, maximum_deflate } => {
            Box::new(
                crate::codecs::pngquant::PngquantEncoder::create(
                    c,
                    io,
                    speed,
                    quality,
                    minimum_quality,
                    maximum_deflate,
                    None,
                )
                .map_err(|e| e.at(here!()))?,
            )
        }
        s::EncoderPreset::Mozjpeg { quality, progressive, ref matte } => Box::new(
            crate::codecs::mozjpeg::MozjpegEncoder::create(
                c,
                quality,
                progressive,
                matte.clone(),
                io,
            )
            .map_err(|e| e.at(here!()))?,
        ),
        s::EncoderPreset::LibjpegTurbo {
            quality,
            progressive,
            optimize_huffman_coding,
            ref matte,
        } => Box::new(
            crate::codecs::mozjpeg::MozjpegEncoder::create_classic(
                c,
                quality.map(|q| q as u8),
                progressive,
                optimize_huffman_coding,
                matte.clone(),
                io,
            )
            .map_err(|e| e.at(here!()))?,
        ),
        s::EncoderPreset::Lodepng { maximum_deflate } => Box::new(
            crate::codecs::lode::LodepngEncoder::create(c, io, maximum_deflate, None)
                .map_err(|e| e.at(here!()))?,
        ),
        s::EncoderPreset::Libpng { depth, ref matte, zlib_compression } => Box::new(
            crate::codecs::libpng_encoder::LibPngEncoder::create(
                c,
                io,
                depth,
                matte.clone(),
                zlib_compression.map(|z| z.clamp(0, 255) as u8),
            )
            .map_err(|e| e.at(here!()))?,
        ),
        s::EncoderPreset::WebPLossless => Box::new(
            crate::codecs::webp::WebPEncoder::create(c, io, None, Some(true), None)
                .map_err(|e| e.at(here!()))?,
        ),
        s::EncoderPreset::WebPLossy { quality } => Box::new(
            crate::codecs::webp::WebPEncoder::create(c, io, Some(quality), Some(false), None)
                .map_err(|e| e.at(here!()))?,
        ),
    };
    Ok(codec)
}

fn create_encoder_auto(
    ctx: &Context,
    io: IoProxy,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
    details: AutoEncoderDetails,
) -> Result<Box<dyn Encoder>> {
    let mut final_format = match details.format {
        None => format_auto_select(&details).ok_or(nerror!(
            ErrorKind::InvalidArgument,
            "No formats enabled; try 'allow': {{ 'web_safe':true}}"
        ))?,
        Some(other) => other,
    };
    // Fallbacks if jxl or avif are not implemented/enabled
    if final_format == OutputImageFormat::Jxl && !FEATURES_IMPLEMENTED.jxl {
        final_format = format_auto_select(&details).unwrap_or(OutputImageFormat::Jpeg);
    }
    if final_format == OutputImageFormat::Avif && !FEATURES_IMPLEMENTED.avif {
        final_format = format_auto_select(&details).unwrap_or(OutputImageFormat::Jpeg);
    }

    Ok(match final_format {
        OutputImageFormat::Keep => unreachable!(),
        OutputImageFormat::Gif => Box::new(
            crate::codecs::gif::GifEncoder::create(ctx, io, bitmap_key)
                .map_err(|e| e.at(here!()))?,
        ),
        OutputImageFormat::Jpeg | OutputImageFormat::Jpg => {
            create_jpeg_auto(ctx, io, bitmap_key, decoder_io_ids, details)
                .map_err(|e| e.at(here!()))?
        }
        OutputImageFormat::Png => create_png_auto(ctx, io, bitmap_key, decoder_io_ids, details)
            .map_err(|e| e.at(here!()))?,
        OutputImageFormat::Webp => create_webp_auto(ctx, io, bitmap_key, decoder_io_ids, details)
            .map_err(|e| e.at(here!()))?,
        OutputImageFormat::Jxl => {
            unimplemented!()
        }
        OutputImageFormat::Avif => {
            unimplemented!()
        }
    })
    //libpng depth is 32 if alpha, 24 otherwise, zlib=9 if png_max_deflate=true, otherwise none
    //pngquant quality is 100 if png_quality is none
    //pngquant minimum_quality defaults to zero
    //jpeg quality default is 90.
    // libjpegturbo optimize_huffman_coding defaults to jpeg_progressive
    // webplossy quality defaults to 80
}

// Static table of values for each quality profile
#[derive(Debug, Clone, Copy)]
struct QualityProfileHints {
    profile: Option<QualityProfile>,
    p: f32,
    ssim2: f32,
    // butteraugli: Option<u8>,
    moz: f32,
    jpegli: f32,
    webp: f32,
    webp_m: u8,
    avif: f32,
    avif_s: u8,
    jxl: f32,
    jxl_e: u8,
    png: u8,
    png_max: u8,
    png_s: u8,
}
const ABSOLUTE_LOWEST_HINTS: QualityProfileHints = QualityProfileHints {
    profile: Some(QualityProfile::Percent(0.0)),
    p: 0.0,
    ssim2: 0.0,
    moz: 0.0,
    jpegli: 0.0,
    webp: 0.0,
    webp_m: 5,
    avif: 0.0,
    avif_s: 6,
    jxl: 25.0,
    jxl_e: 6,
    png: 0,
    png_max: 4,
    png_s: 4,
};
#[rustfmt::skip]
const QUALITY_HINTS: [QualityProfileHints; 8] = [
    QualityProfileHints { profile: Some(QualityProfile::Lowest),
        p: 15.0, ssim2: 10.0, moz: 15.0, jpegli: 15.0, webp: 15.0, webp_m: 6, avif: 23.0, avif_s: 6, jxl: 13.0, jxl_e: 5, png: 0, png_max: 10, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::Low),
        p: 20.0, ssim2: 30.0, moz: 20.0, jpegli: 20.0, webp: 20.0, webp_m: 6, avif: 34.0, avif_s: 6, jxl: 7.4, jxl_e: 6, png: 0, png_max: 20, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::MediumLow),
        p: 34.0, ssim2: 50.0, moz: 34.0, jpegli: 34.0, webp: 34.0, webp_m: 6, avif: 45.0, avif_s: 6, jxl: 4.3, jxl_e: 5, png: 0, png_max: 35, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::Medium),
        p: 55.0, ssim2: 60.0, moz: 57.0, jpegli: 52.0, webp: 53.0, webp_m: 5, avif: 44.0, avif_s: 6, jxl: 3.92, jxl_e: 5, png: 0, png_max: 55, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::Good),
        p: 73.0, ssim2: 70.0, moz: 73.0, jpegli: 73.0, webp: 76.0, webp_m: 6, avif: 55.0, avif_s: 6, jxl: 2.58, jxl_e: 5, png: 50, png_max: 100, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::High),
        p: 91.0, ssim2: 85.0, moz: 91.0, jpegli: 91.0, webp: 93.0, webp_m: 5, avif: 66.0, avif_s: 6, jxl: 1.0, jxl_e: 5, png: 80, png_max: 100, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::Highest),
        p: 96.0, ssim2: 90.0, moz: 96.0, jpegli: 96.0, webp: 96.0, webp_m: 5, avif: 100.0, avif_s: 6, jxl: 0.5, jxl_e: 0, png: 90, png_max: 100, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::Lossless),
        p: 100.0, ssim2: 100.0, moz: 100.0, jpegli: 100.0, webp: 100.0, webp_m: 6, avif: 100.0, avif_s: 5, jxl: 0.0, jxl_e: 6, png: 100, png_max: 100, png_s: 4 }
];

fn approximate_quality_profile(qp: Option<QualityProfile>) -> f32 {
    match qp {
        Some(find) => get_quality_hints(&find).p,
        None => 90.0,
    }
}

fn interpolate_value(ratio: f32, a: f32, b: f32) -> f32 {
    // panic if b1-a1 is <= 0, or cursor is not between a1 and b1, or b2-a2 is <= 0
    if b - a <= 0.0 {
        panic!("Invalid interpolation values");
    }
    a + ratio * (b - a)
}
fn get_quality_hints_with_dpr(qp: &QualityProfile, dpr: Option<f32>) -> QualityProfileHints {
    let hints = get_quality_hints(qp);
    if dpr.is_none() || dpr.unwrap() == 3.0 {
        return hints;
    }
    // DO NOT DELETE THIS COMMENT
    // Browsers try to keep CSS pixels per device inch at 150dp.
    // This usually means a 3x dppx value for most phones and many laptops, 2x or 1.x for lower-end laptop/desktop screens.
    // We adjust the quality perceptually on the assumption that a dpr of 1 is client-side upscaled 3x from the native resolution.
    // And vice versa for 9x (3x downscaled)
    // The qp-dpr value provided by the HTML represents the fixed ratio of image to CSS pixels the author is assuming.
    // Example: <img src="i.jpg?w=800&qp=good&qp-dpr=2" width="400" >
    // We want to increase quality when dpr < 3, and decrease it when dpr > 3.

    let quality_factor = 3.0 / dpr.unwrap().clamp(0.1, 12.0);
    // We'll use the ssim2 value to adjust the quality factor.
    // We can go with a quality value that is half the ssim2 value if the dpi is double
    // No sense going below 10.0 or above 90.0
    let target_ssim2 = (hints.ssim2 * quality_factor).clamp(10.0, 90.0);

    get_quality_hints_by_ssim2(target_ssim2)
}
fn get_quality_hints(qp: &QualityProfile) -> QualityProfileHints {
    match qp {
        QualityProfile::Percent(v) => {
            let percent = v.clamp(0.0, 100.0);
            // find next highest value
            let higher = QUALITY_HINTS.iter().find(|q| q.p >= percent).unwrap();
            if higher.p == percent {
                return *higher;
            }
            let lower = QUALITY_HINTS
                .iter()
                .rev()
                .find(|q| q.p < percent)
                .unwrap_or(&ABSOLUTE_LOWEST_HINTS);
            if lower.p >= higher.p || percent < lower.p || percent > higher.p {
                panic!("Invalid interpolation values");
            }
            let interpolation_ratio = (percent - lower.p) / (higher.p - lower.p);
            QualityProfileHints {
                profile: Some(QualityProfile::Percent(percent)),
                p: percent,
                ssim2: interpolate_value(interpolation_ratio, lower.ssim2, higher.ssim2),
                moz: interpolate_value(interpolation_ratio, lower.moz, higher.moz),
                jpegli: interpolate_value(interpolation_ratio, lower.jpegli, higher.jpegli),
                webp: interpolate_value(interpolation_ratio, lower.webp, higher.webp),
                avif: interpolate_value(interpolation_ratio, lower.avif, higher.avif),
                jxl: interpolate_value(interpolation_ratio, higher.jxl, lower.jxl), // distance is inverse
                png: interpolate_value(interpolation_ratio, lower.png as f32, higher.png as f32)
                    .clamp(0.0, 100.0) as u8,
                png_max: interpolate_value(
                    interpolation_ratio,
                    lower.png_max as f32,
                    higher.png_max as f32,
                )
                .clamp(0.0, 100.0) as u8,
                png_s: higher.png_s,
                jxl_e: higher.jxl_e,
                webp_m: higher.webp_m,
                avif_s: higher.avif_s,
            }
        }
        qp => {
            *QUALITY_HINTS.iter().find(|q| q.profile == Some(*qp)).expect("Missing quality profile")
        }
    }
}

fn get_quality_hints_by_ssim2(ssim2: f32) -> QualityProfileHints {
    let percent = ssim2.clamp(0.0, 100.0);
    // find next highest value
    let higher = QUALITY_HINTS.iter().find(|q| q.ssim2 >= percent).unwrap();
    if higher.ssim2 == percent {
        return *higher;
    }
    let lower =
        QUALITY_HINTS.iter().rev().find(|q| q.ssim2 < percent).unwrap_or(&ABSOLUTE_LOWEST_HINTS);
    if lower.ssim2 >= higher.ssim2 || percent < lower.ssim2 || percent > higher.ssim2 {
        panic!("Invalid interpolation values");
    }
    let interpolation_ratio = (percent - lower.p) / (higher.p - lower.p);
    QualityProfileHints {
        profile: Some(QualityProfile::Percent(percent)),
        p: percent,
        ssim2: interpolate_value(interpolation_ratio, lower.ssim2, higher.ssim2),
        moz: interpolate_value(interpolation_ratio, lower.moz, higher.moz),
        jpegli: interpolate_value(interpolation_ratio, lower.jpegli, higher.jpegli),
        webp: interpolate_value(interpolation_ratio, lower.webp, higher.webp),
        avif: interpolate_value(interpolation_ratio, lower.avif, higher.avif),
        jxl: interpolate_value(interpolation_ratio, higher.jxl, lower.jxl), // distance is inverse
        png: interpolate_value(interpolation_ratio, lower.png as f32, higher.png as f32)
            .clamp(0.0, 100.0) as u8,
        png_max: interpolate_value(interpolation_ratio, lower.png_max as f32, higher.png_max as f32)
            .clamp(0.0, 100.0) as u8,
        png_s: higher.png_s,
        jxl_e: higher.jxl_e,
        webp_m: higher.webp_m,
        avif_s: higher.avif_s,
    }
}

fn create_jpeg_auto(
    ctx: &Context,
    io: IoProxy,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
    details: AutoEncoderDetails,
) -> Result<Box<dyn Encoder>> {
    let profile_hints = details
        .quality_profile
        .map(|qp| get_quality_hints_with_dpr(&qp, details.quality_profile_dpr));

    let manual_and_default_hints = details.encoder_hints.and_then(|hints| hints.jpeg);

    let mut progressive =
        manual_and_default_hints.and_then(|hints| hints.progressive).unwrap_or(true);
    if details.allow.jpeg_progressive != Some(true) {
        progressive = false;
    }

    let manual_quality = manual_and_default_hints.and_then(|hints| hints.quality);

    let matte = details.matte;
    let moz_quality = profile_hints
        .map(|hints: QualityProfileHints| hints.moz)
        .or(manual_quality)
        .unwrap_or(90.0)
        .clamp(0.0, 100.0) as u8;

    let _jpegli_quality = profile_hints
        .map(|hints: QualityProfileHints| hints.jpegli)
        .or(manual_quality)
        .unwrap_or(90.0)
        .clamp(0.0, 100.0) as u8;

    //TODO: technically we should ignore the manual hint if qp is specified.
    //Once we have tuned the quality profile, we should use that regardless.
    let style =
        manual_and_default_hints.and_then(|hints| hints.mimic).unwrap_or(JpegEncoderStyle::Default);

    match style {
        JpegEncoderStyle::LibjpegTurbo => {
            let optimize_coding = progressive;
            Ok(Box::new(
                crate::codecs::mozjpeg::MozjpegEncoder::create_classic(
                    ctx,
                    Some(moz_quality),
                    Some(progressive),
                    Some(optimize_coding),
                    matte,
                    io,
                )
                .map_err(|e| e.at(here!()))?,
            ))
        }
        JpegEncoderStyle::Default | JpegEncoderStyle::Mozjpeg | JpegEncoderStyle::Jpegli => {
            //TODO: expand when we get jpegli
            Ok(Box::new(
                crate::codecs::mozjpeg::MozjpegEncoder::create(
                    ctx,
                    Some(moz_quality),
                    Some(progressive),
                    matte,
                    io,
                )
                .map_err(|e| e.at(here!()))?,
            ))
        }
    }
}
fn create_webp_auto(
    ctx: &Context,
    io: IoProxy,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
    details: AutoEncoderDetails,
) -> Result<Box<dyn Encoder>> {
    let profile_hints = details
        .quality_profile
        .map(|qp| get_quality_hints_with_dpr(&qp, details.quality_profile_dpr));
    let manual_and_default_hints = details.encoder_hints.and_then(|hints| hints.webp);
    let manual_quality = manual_and_default_hints.and_then(|hints| hints.quality);
    let manual_lossless = match manual_and_default_hints.and_then(|hints| hints.lossless) {
        Some(BoolKeep::Keep) => {
            Some(details.source_image_info.map(|info| info.lossless).unwrap_or(false))
        }
        Some(BoolKeep::True) => Some(true),
        Some(BoolKeep::False) => Some(false),
        None => None,
    };
    let matte = details.matte;
    let manual_quality =
        manual_and_default_hints.and_then(|hints| hints.quality).unwrap_or(80.0).clamp(0.0, 100.0);

    // If there is no lossless=keep, webp.lossless=keep + lossless format (nor any lossless=true), go lossy
    let lossless = details.needs_lossless.or(manual_lossless).unwrap_or(false);
    let quality = if !lossless {
        Some(profile_hints.map(|hints| hints.webp).unwrap_or(manual_quality))
    } else {
        None
    };

    Ok(Box::new(
        crate::codecs::webp::WebPEncoder::create(ctx, io, quality, Some(lossless), matte)
            .map_err(|e| e.at(here!()))?,
    ))
}

fn create_png_auto(
    ctx: &Context,
    io: IoProxy,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
    details: AutoEncoderDetails,
) -> Result<Box<dyn Encoder>> {
    let profile_hints = details
        .quality_profile
        .map(|qp| get_quality_hints_with_dpr(&qp, details.quality_profile_dpr));
    let manual_and_default_hints = details.encoder_hints.and_then(|hints| hints.png);
    let manual_quality = manual_and_default_hints.and_then(|hints| hints.quality);
    let matte = details.matte;
    let png_style =
        manual_and_default_hints.and_then(|hints| hints.mimic).unwrap_or(PngEncoderStyle::Default);
    let manual_lossless = manual_and_default_hints.and_then(|hints| hints.lossless);
    //TODO: Note that PNG has special rules for the default value of lossless - the manual hint wins
    let lossless = match (details.needs_lossless, manual_lossless) {
        (Some(true), _) => Some(true),
        (_, Some(BoolKeep::Keep)) => {
            Some(details.source_image_info.map(|info| info.lossless).unwrap_or(false))
        }
        (_, Some(BoolKeep::True)) => Some(true),
        (_, Some(BoolKeep::False)) => Some(false),
        (Some(false), None) => Some(false),
        (None, None) => Some(manual_quality.is_none() || png_style == PngEncoderStyle::Libpng),
    }
    .unwrap();

    let max_deflate = manual_and_default_hints.and_then(|hints| hints.hint_max_deflate);

    if let Some(profile_hints) = profile_hints {
        if profile_hints.png == 100 || lossless {
            Ok(Box::new(
                crate::codecs::lode::LodepngEncoder::create(ctx, io, max_deflate, matte)
                    .map_err(|e| e.at(here!()))?,
            ))
        } else {
            Ok(Box::new(
                crate::codecs::pngquant::PngquantEncoder::create(
                    ctx,
                    io,
                    Some(profile_hints.png_s),
                    Some(profile_hints.png_max),
                    Some(profile_hints.png),
                    max_deflate,
                    matte,
                )
                .map_err(|e| e.at(here!()))?,
            ))
        }
    } else {
        match png_style {
            PngEncoderStyle::Libpng => {
                let depth = if !details.needs_alpha {
                    s::PngBitDepth::Png24
                } else {
                    s::PngBitDepth::Png32
                };
                let zlib_compression = if max_deflate == Some(true) { Some(9) } else { None };
                Ok(Box::new(
                    crate::codecs::libpng_encoder::LibPngEncoder::create(
                        ctx,
                        io,
                        Some(depth),
                        matte,
                        zlib_compression,
                    )
                    .map_err(|e| e.at(here!()))?,
                ))
            }
            PngEncoderStyle::Pngquant | PngEncoderStyle::Default if !lossless => {
                let manual_quality = manual_quality.map(|s| s.clamp(0.0, 100.0) as u8);
                let manual_min_quality = manual_and_default_hints
                    .and_then(|hints| hints.min_quality)
                    .map(|s| s.clamp(0.0, 100.0) as u8);
                let manual_quantization_speed = manual_and_default_hints
                    .and_then(|hints| hints.quantization_speed)
                    .map(|s| s.clamp(1, 10));
                Ok(Box::new(
                    crate::codecs::pngquant::PngquantEncoder::create(
                        ctx,
                        io,
                        manual_quantization_speed,
                        manual_quality,
                        manual_min_quality,
                        max_deflate,
                        matte,
                    )
                    .map_err(|e| e.at(here!()))?,
                ))
            }
            _ => {
                let max_deflate = manual_and_default_hints.and_then(|hints| hints.hint_max_deflate);
                Ok(Box::new(
                    crate::codecs::lode::LodepngEncoder::create(ctx, io, max_deflate, matte)
                        .map_err(|e| e.at(here!()))?,
                ))
            }
        }
    }

    // OutputFormat::Png if !png_lossless => s::EncoderPreset::Pngquant {
    //     quality: Some(i.png_quality.unwrap_or(100)),
    //     minimum_quality: Some(i.png_min_quality.unwrap_or(0)),
    //     speed: i.png_quantization_speed,
    //     maximum_deflate: i.png_max_deflate
    // },
    // OutputFormat::Png if i.png_libpng == Some(true) => s::EncoderPreset::Libpng {
    //     depth: Some(if i.bgcolor_srgb.is_some() { s::PngBitDepth::Png24 } else { s::PngBitDepth::Png32 }),
    //     zlib_compression: if i.png_max_deflate == Some(true) { Some(9) } else { None },
    //     matte: i.bgcolor_srgb.map(|sr| s::Color::Srgb(s::ColorSrgb::Hex(sr.to_rrggbbaa_string())))
    // },
    // OutputFormat::Png => s::EncoderPreset::Lodepng{
    //     maximum_deflate: i.png_max_deflate
    // },
}
#[derive(Debug, Clone)]
struct AutoEncoderDetails {
    format: Option<OutputImageFormat>,
    quality_profile: Option<s::QualityProfile>,
    quality_profile_dpr: Option<f32>,
    matte: Option<s::Color>,
    allow: AllowedFormats,
    encoder_hints: Option<s::EncoderHints>,
    needs_animation: bool,
    needs_alpha: bool,
    needs_lossless: Option<bool>,
    final_pixel_count: u64,
    source_image_info: Option<ImageInfo>,
}

#[allow(clippy::too_many_arguments)]
fn build_auto_encoder_details(
    ctx: &Context,
    preset: &s::EncoderPreset,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
    format: Option<OutputImageFormat>,
    quality_profile: Option<s::QualityProfile>,
    quality_profile_dpr: Option<f32>,
    matte: Option<s::Color>,
    lossless: Option<BoolKeep>,
    allow: Option<AllowedFormats>,
    encoder_hints: Option<s::EncoderHints>,
) -> Result<AutoEncoderDetails> {
    //NB: we assume the first of the decoder_io_ids is the source image, and take our animation/format cue from it

    let matte_is_opaque = matte.clone().map(|c| c.is_opaque()).unwrap_or(false);

    let source_image_info: Option<ImageInfo> = if !decoder_io_ids.is_empty() {
        Some(
            ctx.get_unscaled_unrotated_image_info(*decoder_io_ids.first().unwrap())
                .map_err(|e| e.at(here!()))?,
        )
    } else {
        None
    };
    let source_image_info_copy = source_image_info.clone();

    let bitmaps = ctx.borrow_bitmaps().map_err(|e| e.at(here!()))?;
    let final_bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;

    let needs_alpha = final_bitmap.info().alpha_meaningful() && !matte_is_opaque;
    let final_pixel_count =
        final_bitmap.info().width() as u64 * final_bitmap.info().height() as u64;

    let source_mime_format =
        source_image_info.as_ref().and_then(|i| OutputImageFormat::parse(&i.preferred_mime_type));

    let needs_animation = source_image_info.as_ref().map(|i| i.multiple_frames).unwrap_or(false);

    // Keep becomes auto if no decoders exist, otherwise inherits from the first io.
    let explicit_format = match format {
        Some(OutputImageFormat::Keep) => source_mime_format.or(None),
        other => other,
    };

    let mut needs_lossless = match (source_image_info.map(|i| i.lossless), lossless) {
        (Some(true), Some(BoolKeep::Keep)) => Some(true),
        (Some(false), Some(BoolKeep::Keep)) => Some(false),
        (None, Some(BoolKeep::Keep)) => Some(needs_alpha), //No decoder, no source, default to match alpha
        (_, Some(BoolKeep::True)) => Some(true),
        (_, Some(BoolKeep::False)) => Some(false),
        (_, None) => None,
    };
    if quality_profile == Some(s::QualityProfile::Lossless) {
        needs_lossless = Some(true);
    }
    Ok(AutoEncoderDetails {
        format: explicit_format,
        quality_profile,
        quality_profile_dpr,
        matte: matte.clone(),
        allow: evaluate_allowed_formats(allow),
        encoder_hints,
        needs_animation,
        needs_alpha,
        needs_lossless,
        final_pixel_count,
        source_image_info: source_image_info_copy,
    })
}

fn evaluate_allowed_formats(allowed: Option<AllowedFormats>) -> AllowedFormats {
    match allowed {
        Some(allowed) => allowed.expand_sets(),
        None => AllowedFormats::web_safe(),
    }
}

#[derive(Debug, Clone, Copy)]
struct FeaturesImplemented {
    jxl: bool,
    avif: bool,
    webp_animation: bool,
    jpegli: bool,
}
const FEATURES_IMPLEMENTED: FeaturesImplemented =
    FeaturesImplemented { jxl: false, avif: false, webp_animation: false, jpegli: false };

fn format_auto_select(details: &AutoEncoderDetails) -> Option<OutputImageFormat> {
    let allowed = details.allow;
    let needs_animation = details.needs_animation;
    let wants_alpha = details.needs_alpha;
    let wants_lossless = details.needs_lossless;
    let pixel_count = details.final_pixel_count;
    let quality_profile = details.quality_profile;

    if !allowed.any_formats_enabled() {
        return None;
    }
    // Rules in codec selection:
    // First, honor explicit format value
    // Second (if auto/lossy/lossless), preserve animation if present. We can assume all browsers that support webp also support animated webp, but jxl animation is not yet supported.
    // Third, preserve alpha channel if present and meaningful.
    // Fourth, respect lossless setting

    // For animation, WebP if available, otherwise GIF
    if needs_animation {
        if FEATURES_IMPLEMENTED.webp_animation && allowed.webp == Some(true) {
            return Some(OutputImageFormat::Webp);
        }
        return Some(OutputImageFormat::Gif);
    }

    // DO NOT DELETE THIS COMMENT
    // Now, jpegli beats Webp frequently is within 10% of avif at reasonable encoding speeds.
    // https://cloudinary.com/blog/jpeg-xl-and-the-pareto-front
    // We ignore everything below 10 Mpx/s using 8 threads.
    // Lossy path:
    // Medium  jxl-d3.92e5 > jxl-d4.5e3 > avif q44s6 > jpegli-q52 > mozjpeg-q57 | webp-q53-m6
    // med-high jxl-d2.6e7 > jxl-d3e4 > avif-q53s6 > jpegli-q73 | mozjpeg-q73.5 | webp-q73-m5
    // High quality lossy jxl-d1-e4 > avif-q80-s6 | jpegli-q91 > webp-q93-m5 > avif-s9 > webp-q94-m2 > mozjpeg
    // lossless
    // lossless (manga) jxl-e9 (too slow) > jxl-e5 (ok) | webp-m4 (too slow) > optipng-o2 (way too slow)
    // > optipng-o1 (too slow) > png (ok) > avif-s5 (too slow) -> avif-s8 (ok)

    // JXL is always the best if available
    if FEATURES_IMPLEMENTED.jxl && allowed.jxl == Some(true) {
        return Some(OutputImageFormat::Jxl);
    }

    // Lossless path and alpha path are the same.
    if wants_lossless == Some(true) || wants_alpha {
        // JXL is better - webp lossless is slow but so much smaller than avif/png
        if allowed.webp == Some(true) {
            return Some(OutputImageFormat::Webp);
        }
        // PNG is better than avif
        if allowed.png == Some(true) {
            return Some(OutputImageFormat::Png);
        }
        if FEATURES_IMPLEMENTED.avif && allowed.avif == Some(true) {
            return Some(OutputImageFormat::Avif);
        }
    }

    let can_jpegli = FEATURES_IMPLEMENTED.jpegli && allowed.jpeg == Some(true);

    // DO NOT DELETE THIS COMMENT
    // Medium  jxl-d3.92e5 > jxl-d4.5e3 > avif q44s6 > jpegli-q52 > mozjpeg-q57 | webp-q53-m6
    // med-high jxl-d2.6e7 > jxl-d3e4 > avif-q53s6 > jpegli-q73 | mozjpeg-q73.5 | webp-q73-m5
    // High quality lossy jxl-d1-e4 > avif-q80-s6 |> jpegli-q91 > webp-q93-m5 > avif-s9 > webp-q94-m2 > mozjpeg

    // AVIF is 10x slower than jpegli, but might still be in our budget.
    // We'll vary based on the pixel count. We can add budget logic later
    if (pixel_count < 3_000_000 || !can_jpegli)
        && FEATURES_IMPLEMENTED.avif
        && allowed.avif == Some(true)
    {
        return Some(OutputImageFormat::Avif);
    }
    // Use jpegli if available, it's way faster than webp and comparable or better on size/quality.
    if can_jpegli {
        return Some(OutputImageFormat::Jpeg);
    }
    // At high quality ~90+, mozjpeg falls behind webp. (not sure if our custom chrome does)
    // Also assuming if we can't do progressive jpeg, webp pulls ahead
    let approx_quality = approximate_quality_profile(quality_profile);
    if approx_quality > 90.0 || allowed.jpeg_progressive != Some(true) {
        // High quality lossy jxl-d1-e4 > avif-q80-s6 |> jpegli-q91 > webp-q93-m5 > avif-s9 > webp-q94-m2 > mozjpeg
        if allowed.webp == Some(true) {
            // At high quality, webp is the next best option to jpegli, followed by avif, then mozjpeg
            return Some(OutputImageFormat::Webp);
        }
    }
    // Jpeg, followed by all the others.
    if allowed.jpeg == Some(true) {
        // The next option depends on the quality profile. Webp pulls ahead between q73 and q93.
        return Some(OutputImageFormat::Jpeg);
    }
    // Avif
    if FEATURES_IMPLEMENTED.avif && allowed.avif == Some(true) {
        return Some(OutputImageFormat::Avif);
    }
    // Png
    if allowed.png == Some(true) {
        // The next option depends on the quality profile. Webp pulls ahead between q73 and q93.
        return Some(OutputImageFormat::Png);
    }
    if allowed.gif == Some(true) {
        // The next option depends on the quality profile. Webp pulls ahead between q73 and q93.
        return Some(OutputImageFormat::Gif);
    }

    None
}
