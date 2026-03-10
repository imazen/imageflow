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

use crate::codecs::EnabledCodecs;
use crate::codecs::Encoder;
use crate::codecs::NamedEncoders;
use crate::{BitmapKey, Context, ErrorCategory, ErrorKind, FlowError, JsonResponse, Result};

/// Versioned encoding engine logic. Controls format selection heuristics and
/// per-format config defaults.
///
/// Will move to `imageflow_riapi::version` once querystring version parsing is wired up.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EncodeEngineVersion {
    /// Mimic ImageResizer 4 tuning. Fidelity may improve over time.
    IR4,
    /// Imageflow 2.x defaults.
    V2,
    /// Preview / experimental logic that may change.
    Preview,
}

pub(crate) fn create_encoder(
    c: &Context,
    io: IoProxy,
    preset: &s::EncoderPreset,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
) -> Result<Box<dyn Encoder>> {
    let v = &EncodeEngineVersion::Preview;

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
                v,
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
                v,
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
        // Legacy codec-specific presets: select best encoder, build params, instantiate.
        s::EncoderPreset::Gif => {
            let enc =
                select_encoder_for_format(c, ImageFormat::Gif, false).map_err(|e| e.at(here!()))?;
            instantiate_encoder(c, enc, io, &EncoderParams { bitmap_key, ..Default::default() })
                .map_err(|e| e.at(here!()))?
        }
        s::EncoderPreset::Pngquant { speed, quality, minimum_quality, maximum_deflate } => {
            instantiate_encoder(
                c,
                NamedEncoders::PngQuantEncoder,
                io,
                &EncoderParams {
                    bitmap_key,
                    png_speed: speed,
                    png_target_quality: quality,
                    png_min_quality: minimum_quality,
                    max_deflate: maximum_deflate,
                    ..Default::default()
                },
            )
            .map_err(|e| e.at(here!()))?
        }
        s::EncoderPreset::Mozjpeg { quality, progressive, ref matte } => {
            let enc = select_encoder_for_format(c, ImageFormat::Jpeg, false)
                .map_err(|e| e.at(here!()))?;
            instantiate_encoder(
                c,
                enc,
                io,
                &EncoderParams {
                    bitmap_key,
                    quality: quality.map(|q| q as f32),
                    progressive,
                    matte: matte.clone(),
                    ..Default::default()
                },
            )
            .map_err(|e| e.at(here!()))?
        }
        s::EncoderPreset::LibjpegTurbo {
            quality,
            progressive,
            optimize_huffman_coding,
            ref matte,
        } => {
            let enc = select_encoder_for_format(c, ImageFormat::Jpeg, false)
                .map_err(|e| e.at(here!()))?;
            instantiate_encoder(
                c,
                enc,
                io,
                &EncoderParams {
                    bitmap_key,
                    quality: quality.map(|q| q as f32),
                    progressive,
                    optimize_huffman_coding,
                    classic_mode: true,
                    matte: matte.clone(),
                    ..Default::default()
                },
            )
            .map_err(|e| e.at(here!()))?
        }
        s::EncoderPreset::Lodepng { maximum_deflate } => instantiate_encoder(
            c,
            NamedEncoders::LodePngEncoder,
            io,
            &EncoderParams { bitmap_key, max_deflate: maximum_deflate, ..Default::default() },
        )
        .map_err(|e| e.at(here!()))?,
        s::EncoderPreset::Libpng { depth, ref matte, zlib_compression } => instantiate_encoder(
            c,
            NamedEncoders::LibPngRsEncoder,
            io,
            &EncoderParams {
                bitmap_key,
                png_bit_depth: depth,
                matte: matte.clone(),
                zlib_compression: zlib_compression.map(|z| z.clamp(0, 255) as u8),
                ..Default::default()
            },
        )
        .map_err(|e| e.at(here!()))?,
        s::EncoderPreset::WebPLossless => {
            let enc =
                select_encoder_for_format(c, ImageFormat::Webp, true).map_err(|e| e.at(here!()))?;
            instantiate_encoder(
                c,
                enc,
                io,
                &EncoderParams { bitmap_key, lossless: Some(true), ..Default::default() },
            )
            .map_err(|e| e.at(here!()))?
        }
        s::EncoderPreset::WebPLossy { quality } => {
            let enc = select_encoder_for_format(c, ImageFormat::Webp, false)
                .map_err(|e| e.at(here!()))?;
            instantiate_encoder(
                c,
                enc,
                io,
                &EncoderParams {
                    bitmap_key,
                    quality: Some(quality),
                    lossless: Some(false),
                    ..Default::default()
                },
            )
            .map_err(|e| e.at(here!()))?
        }
        s::EncoderPreset::JxlLossy { distance } => {
            let enc =
                select_encoder_for_format(c, ImageFormat::Jxl, false).map_err(|e| e.at(here!()))?;
            instantiate_encoder(
                c,
                enc,
                io,
                &EncoderParams {
                    bitmap_key,
                    jxl_distance: Some(distance),
                    lossless: Some(false),
                    ..Default::default()
                },
            )
            .map_err(|e| e.at(here!()))?
        }
        s::EncoderPreset::JxlLossless => {
            let enc =
                select_encoder_for_format(c, ImageFormat::Jxl, true).map_err(|e| e.at(here!()))?;
            instantiate_encoder(
                c,
                enc,
                io,
                &EncoderParams { bitmap_key, lossless: Some(true), ..Default::default() },
            )
            .map_err(|e| e.at(here!()))?
        }
    };
    Ok(codec)
}

// ── Unified encoder instantiation ────────────────────────────────────────────

/// Instantiate a concrete encoder from a `NamedEncoders` variant + format-specific params.
/// This is the single dispatch point — all encoder creation flows through here.
fn instantiate_encoder(
    c: &Context,
    encoder: NamedEncoders,
    io: IoProxy,
    params: &EncoderParams,
) -> Result<Box<dyn Encoder>> {
    match encoder {
        NamedEncoders::GifEncoder => Ok(Box::new(
            crate::codecs::gif::GifEncoder::create(c, io, params.bitmap_key)
                .map_err(|e| e.at(here!()))?,
        )),
        #[cfg(feature = "zen-codecs")]
        NamedEncoders::ZenGifEncoder => Ok(Box::new(
            crate::codecs::zen_encoder::ZenEncoder::create_gif(c, io, params.bitmap_key)
                .map_err(|e| e.at(here!()))?,
        )),
        #[cfg(feature = "zen-codecs")]
        NamedEncoders::ZenJpegEncoder => Ok(Box::new(
            crate::codecs::zen_encoder::ZenEncoder::create_jpeg(
                c,
                io,
                params.quality_u8(),
                params.progressive,
                params.matte.clone(),
            )
            .map_err(|e| e.at(here!()))?,
        )),
        #[cfg(feature = "c-codecs")]
        NamedEncoders::MozJpegEncoder => {
            if params.classic_mode {
                Ok(Box::new(
                    crate::codecs::mozjpeg::MozjpegEncoder::create_classic(
                        c,
                        params.quality_u8(),
                        params.progressive,
                        params.optimize_huffman_coding,
                        params.matte.clone(),
                        io,
                    )
                    .map_err(|e| e.at(here!()))?,
                ))
            } else {
                Ok(Box::new(
                    crate::codecs::mozjpeg::MozjpegEncoder::create(
                        c,
                        params.quality_u8(),
                        params.progressive,
                        params.matte.clone(),
                        io,
                    )
                    .map_err(|e| e.at(here!()))?,
                ))
            }
        }
        NamedEncoders::PngQuantEncoder => Ok(Box::new(
            crate::codecs::pngquant::PngquantEncoder::create(
                c,
                io,
                params.png_speed,
                params.png_target_quality,
                params.png_min_quality,
                params.max_deflate,
                params.matte.clone(),
            )
            .map_err(|e| e.at(here!()))?,
        )),
        NamedEncoders::LodePngEncoder => Ok(Box::new(
            crate::codecs::lode::LodepngEncoder::create(
                c,
                io,
                params.max_deflate,
                params.matte.clone(),
            )
            .map_err(|e| e.at(here!()))?,
        )),
        #[cfg(feature = "c-codecs")]
        NamedEncoders::LibPngRsEncoder => Ok(Box::new(
            crate::codecs::libpng_encoder::LibPngEncoder::create(
                c,
                io,
                params.png_bit_depth,
                params.matte.clone(),
                params.zlib_compression,
            )
            .map_err(|e| e.at(here!()))?,
        )),
        #[cfg(feature = "zen-codecs")]
        NamedEncoders::ZenWebPEncoder => Ok(Box::new(
            crate::codecs::zen_encoder::ZenEncoder::create_webp(
                c,
                io,
                params.quality_f32(),
                params.lossless_bool(),
                params.matte.clone(),
            )
            .map_err(|e| e.at(here!()))?,
        )),
        #[cfg(feature = "c-codecs")]
        NamedEncoders::WebPEncoder => Ok(Box::new(
            crate::codecs::webp::WebPEncoder::create(
                c,
                io,
                params.quality_f32(),
                params.lossless_bool(),
                params.matte.clone(),
            )
            .map_err(|e| e.at(here!()))?,
        )),
        #[cfg(feature = "zen-codecs")]
        NamedEncoders::ZenJxlEncoder => Ok(Box::new(
            crate::codecs::zen_encoder::ZenEncoder::create_jxl(
                c,
                io,
                params.jxl_distance,
                params.lossless.unwrap_or(false),
            )
            .map_err(|e| e.at(here!()))?,
        )),
        #[cfg(feature = "zen-codecs")]
        NamedEncoders::ZenAvifEncoder => Ok(Box::new(
            crate::codecs::zen_encoder::ZenEncoder::create_avif(
                c,
                io,
                params.quality_f32(),
                params.avif_speed,
                params.lossless.unwrap_or(false),
                params.matte.clone(),
            )
            .map_err(|e| e.at(here!()))?,
        )),
        #[allow(unreachable_patterns)]
        other => Err(nerror!(
            ErrorKind::CodecDisabledError,
            "Encoder {:?} is not compiled in (missing feature flag)",
            other
        )),
    }
}

/// Unified parameter bag for encoder instantiation.
/// Not all fields are used by every encoder — only the relevant ones are read.
#[derive(Debug, Clone, Default)]
struct EncoderParams {
    bitmap_key: BitmapKey,
    quality: Option<f32>,
    progressive: Option<bool>,
    optimize_huffman_coding: Option<bool>,
    classic_mode: bool,
    matte: Option<s::Color>,
    lossless: Option<bool>,
    jxl_distance: Option<f32>,
    avif_speed: Option<u8>,
    max_deflate: Option<bool>,
    png_speed: Option<u8>,
    png_target_quality: Option<u8>,
    png_min_quality: Option<u8>,
    png_bit_depth: Option<s::PngBitDepth>,
    zlib_compression: Option<u8>,
}

impl EncoderParams {
    fn quality_u8(&self) -> Option<u8> {
        self.quality.map(|q| q.clamp(0.0, 100.0) as u8)
    }
    fn quality_f32(&self) -> Option<f32> {
        self.quality
    }
    fn lossless_bool(&self) -> Option<bool> {
        self.lossless
    }
}

/// Select the best encoder for a format using the priority list + capabilities.
/// Returns error if no suitable encoder is enabled.
fn select_encoder_for_format(
    c: &Context,
    format: ImageFormat,
    lossless: bool,
) -> Result<NamedEncoders> {
    c.enabled_codecs.select_encoder(format, lossless).map(|(enc, _trace)| enc).ok_or_else(|| {
        nerror!(
            ErrorKind::CodecDisabledError,
            "No {:?} encoder is enabled (lossless={})",
            format,
            lossless
        )
    })
}

// ── Auto encoder dispatch ───────────────────────────────────────────────────

fn create_encoder_auto(
    ctx: &Context,
    io: IoProxy,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
    details: AutoEncoderDetails,
) -> Result<Box<dyn Encoder>> {
    let final_format = format_select_with_specified(details.format, &details, &ctx.enabled_codecs)
        .map_err(|e| e.at(here!()))?;

    let (encoder, params) = match final_format {
        OutputImageFormat::Keep => unreachable!(),
        OutputImageFormat::Gif => {
            let enc = select_encoder_for_format(ctx, ImageFormat::Gif, false)
                .map_err(|e| e.at(here!()))?;
            (enc, EncoderParams { bitmap_key, ..Default::default() })
        }
        OutputImageFormat::Jpeg | OutputImageFormat::Jpg => {
            build_jpeg_auto_params(ctx, bitmap_key, &details)?
        }
        OutputImageFormat::Png => build_png_auto_params(ctx, bitmap_key, &details)?,
        OutputImageFormat::Webp => build_webp_auto_params(ctx, bitmap_key, &details)?,
        OutputImageFormat::Jxl => build_jxl_auto_params(ctx, bitmap_key, &details)?,
        OutputImageFormat::Avif => build_avif_auto_params(ctx, bitmap_key, &details)?,
    };

    instantiate_encoder(ctx, encoder, io, &params).map_err(|e| e.at(here!()))
}

// ── Quality profile system ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
struct QualityProfileHints {
    profile: Option<QualityProfile>,
    p: f32,
    ssim2: f32,
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
impl QualityProfileHints {
    fn is_lossless(&self) -> bool {
        self.profile == Some(QualityProfile::Lossless)
    }
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
        p: 34.0, ssim2: 50.0, moz: 34.0, jpegli: 34.0, webp: 34.0, webp_m: 6, avif: 44.0, avif_s: 6, jxl: 4.3, jxl_e: 5, png: 0, png_max: 35, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::Medium),
        p: 55.0, ssim2: 60.0, moz: 57.0, jpegli: 52.0, webp: 53.0, webp_m: 5, avif: 45.0, avif_s: 6, jxl: 3.92, jxl_e: 5, png: 0, png_max: 55, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::Good),
        p: 73.0, ssim2: 70.0, moz: 73.0, jpegli: 73.0, webp: 76.0, webp_m: 6, avif: 55.0, avif_s: 6, jxl: 2.58, jxl_e: 5, png: 50, png_max: 100, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::High),
        p: 91.0, ssim2: 85.0, moz: 91.0, jpegli: 91.0, webp: 93.0, webp_m: 5, avif: 66.0, avif_s: 6, jxl: 1.0, jxl_e: 5, png: 80, png_max: 100, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::Highest),
        p: 96.0, ssim2: 90.0, moz: 96.0, jpegli: 96.0, webp: 96.0, webp_m: 5, avif: 100.0, avif_s: 6, jxl: 0.5, jxl_e: 0, png: 90, png_max: 100, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::Lossless),
        p: 100.0, ssim2: 100.0, moz: 100.0, jpegli: 100.0, webp: 100.0, webp_m: 6, avif: 100.0, avif_s: 5, jxl: 0.0, jxl_e: 6, png: 100, png_max: 100, png_s: 4 }
];

fn interpolate_value(ratio: f32, a: f32, b: f32) -> f32 {
    // If values are equal, no interpolation needed
    if (b - a).abs() < 0.0001 {
        return a;
    }
    // panic if b < a (values not monotonic)
    if b - a < 0.0 {
        panic!("Invalid interpolation values: {} < {}", b, a);
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
    let interpolation_ratio = (percent - lower.ssim2) / (higher.ssim2 - lower.ssim2);
    QualityProfileHints {
        profile: None,
        p: interpolate_value(interpolation_ratio, lower.p, higher.p),
        ssim2: percent,
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

// ── Per-format auto config + instantiation ──────────────────────────────────

fn build_jpeg_auto_params(
    ctx: &Context,
    bitmap_key: BitmapKey,
    details: &AutoEncoderDetails,
) -> Result<(NamedEncoders, EncoderParams)> {
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

    let matte = details.matte.clone();
    let moz_quality = profile_hints
        .map(|hints: QualityProfileHints| hints.moz)
        .or(manual_quality)
        .unwrap_or(90.0)
        .clamp(0.0, 100.0);

    let _jpegli_quality = profile_hints
        .map(|hints: QualityProfileHints| hints.jpegli)
        .or(manual_quality)
        .unwrap_or(90.0)
        .clamp(0.0, 100.0);

    let style =
        manual_and_default_hints.and_then(|hints| hints.mimic).unwrap_or(JpegEncoderStyle::Default);

    let enc =
        select_encoder_for_format(ctx, ImageFormat::Jpeg, false).map_err(|e| e.at(here!()))?;

    let classic_mode = matches!(style, JpegEncoderStyle::LibjpegTurbo);
    let optimize_coding = if classic_mode { Some(progressive) } else { None };

    Ok((
        enc,
        EncoderParams {
            bitmap_key,
            quality: Some(moz_quality),
            progressive: Some(progressive),
            optimize_huffman_coding: optimize_coding,
            classic_mode,
            matte,
            ..Default::default()
        },
    ))
}

#[derive(Debug, Clone)]
struct WebPEncodingDetails {
    lossy: Option<f32>,
    lossless: bool,
    matte: Option<Color>,
}

fn build_webp_auto_params(
    ctx: &Context,
    bitmap_key: BitmapKey,
    details: &AutoEncoderDetails,
) -> Result<(NamedEncoders, EncoderParams)> {
    let webp_details = match details.v {
        EncodeEngineVersion::Preview => config_webp_auto_preview(ctx, details.clone())?,
        _ => config_webp_auto_v2(ctx, details.clone())?,
    };
    let enc = select_encoder_for_format(ctx, ImageFormat::Webp, webp_details.lossless)
        .map_err(|e| e.at(here!()))?;
    Ok((
        enc,
        EncoderParams {
            bitmap_key,
            quality: webp_details.lossy,
            lossless: Some(webp_details.lossless),
            matte: webp_details.matte,
            ..Default::default()
        },
    ))
}

fn config_webp_auto_v2(ctx: &Context, details: AutoEncoderDetails) -> Result<WebPEncodingDetails> {
    let profile_hints = details
        .quality_profile
        .map(|qp| get_quality_hints_with_dpr(&qp, details.quality_profile_dpr));
    let manual_and_default_hints = details.encoder_hints.and_then(|hints| hints.webp);
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

    let lossless = details.legacy_needs_lossless.or(manual_lossless).unwrap_or(false);
    let quality = if !lossless {
        Some(profile_hints.map(|hints| hints.webp).unwrap_or(manual_quality))
    } else {
        None
    };

    Ok(WebPEncodingDetails { lossy: quality, lossless, matte })
}

fn config_webp_auto_preview(
    ctx: &Context,
    details: AutoEncoderDetails,
) -> Result<WebPEncodingDetails> {
    let profile_hints = details
        .quality_profile
        .map(|qp| get_quality_hints_with_dpr(&qp, details.quality_profile_dpr));
    let manual_and_default_hints = details.encoder_hints.and_then(|hints| hints.webp);
    let manual_quality = manual_and_default_hints.and_then(|hints| hints.quality);

    let manual_hint_lossless = BoolKeep::and_resolve(
        manual_and_default_hints.and_then(|hints| hints.lossless),
        details.source_lossless_capable,
    );

    let source_format = details.source_image_format;

    let good_defaults_for_source = match source_format {
        _ if details.source_lossless_capable == Some(true) => {
            get_quality_hints(&QualityProfile::Lossless)
        }
        Some(OutputImageFormat::Webp) => get_quality_hints(&QualityProfile::High),
        Some(OutputImageFormat::Jpeg) => get_quality_hints(&QualityProfile::High),
        Some(OutputImageFormat::Jxl) => get_quality_hints(&QualityProfile::High),
        _ => get_quality_hints(&QualityProfile::High),
    };

    let lossless_inferred_from_quality_and_source =
        manual_quality.is_none() && good_defaults_for_source.is_lossless();

    let lossless = details
        .lossless_setting
        .or(manual_hint_lossless)
        .unwrap_or(lossless_inferred_from_quality_and_source);

    let matte = details.matte;
    let quality = manual_and_default_hints
        .and_then(|hints| hints.quality)
        .unwrap_or(good_defaults_for_source.webp)
        .clamp(0.0, 100.0);

    let profile_or_manual_lossless =
        profile_hints.map(|hints| hints.is_lossless()).unwrap_or(lossless);
    let profile_or_manual_quality = if !profile_or_manual_lossless {
        profile_hints.map(|hints| hints.webp).or(Some(quality))
    } else {
        None
    };

    Ok(WebPEncodingDetails {
        lossy: profile_or_manual_quality,
        lossless: profile_or_manual_lossless,
        matte,
    })
}

fn build_avif_auto_params(
    ctx: &Context,
    bitmap_key: BitmapKey,
    details: &AutoEncoderDetails,
) -> Result<(NamedEncoders, EncoderParams)> {
    let profile_hints = details
        .quality_profile
        .map(|qp| get_quality_hints_with_dpr(&qp, details.quality_profile_dpr));

    let manual_and_default_hints = details.encoder_hints.and_then(|hints| hints.avif);
    let matte = details.matte.clone();

    let good_defaults = get_quality_hints(&QualityProfile::Good);

    let quality = profile_hints
        .map(|hints| hints.avif)
        .or_else(|| manual_and_default_hints.and_then(|hints| hints.quality))
        .unwrap_or(good_defaults.avif)
        .clamp(0.0, 100.0);

    let speed = profile_hints
        .map(|hints| hints.avif_s)
        .or_else(|| manual_and_default_hints.and_then(|hints| hints.speed))
        .unwrap_or(good_defaults.avif_s)
        .clamp(0, 10);

    let lossless =
        details.lossless_setting.unwrap_or(false) || details.legacy_needs_lossless.unwrap_or(false);

    let enc =
        select_encoder_for_format(ctx, ImageFormat::Avif, lossless).map_err(|e| e.at(here!()))?;
    Ok((
        enc,
        EncoderParams {
            bitmap_key,
            quality: Some(quality),
            avif_speed: Some(speed),
            lossless: Some(lossless),
            matte,
            ..Default::default()
        },
    ))
}

fn build_jxl_auto_params(
    ctx: &Context,
    bitmap_key: BitmapKey,
    details: &AutoEncoderDetails,
) -> Result<(NamedEncoders, EncoderParams)> {
    let profile_hints = details
        .quality_profile
        .map(|qp| get_quality_hints_with_dpr(&qp, details.quality_profile_dpr));

    let lossless =
        details.lossless_setting.unwrap_or(false) || details.legacy_needs_lossless.unwrap_or(false);

    let enc =
        select_encoder_for_format(ctx, ImageFormat::Jxl, lossless).map_err(|e| e.at(here!()))?;

    let jxl_distance =
        if lossless { None } else { Some(profile_hints.map(|h| h.jxl).unwrap_or(1.0)) };

    Ok((
        enc,
        EncoderParams { bitmap_key, lossless: Some(lossless), jxl_distance, ..Default::default() },
    ))
}

#[derive(Debug, Clone)]
enum PngEncodingDetails {
    LodePngLossless {
        max_deflate: Option<bool>,
        matte: Option<Color>,
    },
    PngQuant {
        speed: Option<u8>,
        target_quality: Option<u8>,
        minimum_quality: Option<u8>,
        max_deflate: Option<bool>,
        matte: Option<Color>,
    },
    LibPng {
        depth: Option<s::PngBitDepth>,
        matte: Option<Color>,
        zlib_compression: Option<u8>,
    },
}

fn build_png_auto_params(
    ctx: &Context,
    bitmap_key: BitmapKey,
    details: &AutoEncoderDetails,
) -> Result<(NamedEncoders, EncoderParams)> {
    let png_details = match details.v {
        EncodeEngineVersion::Preview => config_png_auto_preview(ctx, details.clone())?,
        _ => config_png_legacy(ctx, details.clone())?,
    };
    match png_details {
        PngEncodingDetails::LodePngLossless { max_deflate, matte } => Ok((
            NamedEncoders::LodePngEncoder,
            EncoderParams { bitmap_key, max_deflate, matte, ..Default::default() },
        )),
        PngEncodingDetails::PngQuant {
            speed,
            target_quality,
            minimum_quality,
            max_deflate,
            matte,
        } => Ok((
            NamedEncoders::PngQuantEncoder,
            EncoderParams {
                bitmap_key,
                png_speed: speed,
                png_target_quality: target_quality,
                png_min_quality: minimum_quality,
                max_deflate,
                matte,
                ..Default::default()
            },
        )),
        PngEncodingDetails::LibPng { depth, matte, zlib_compression } => Ok((
            NamedEncoders::LibPngRsEncoder,
            EncoderParams {
                bitmap_key,
                png_bit_depth: depth,
                matte,
                zlib_compression,
                ..Default::default()
            },
        )),
    }
}

fn config_png_auto_preview(
    ctx: &Context,
    details: AutoEncoderDetails,
) -> Result<PngEncodingDetails> {
    let profile_hints = details
        .quality_profile
        .map(|qp| get_quality_hints_with_dpr(&qp, details.quality_profile_dpr));
    let manual_and_default_hints = details.encoder_hints.and_then(|hints| hints.png);
    let manual_target_quality = manual_and_default_hints.and_then(|hints| hints.quality);
    let manual_min_quality = manual_and_default_hints.and_then(|hints| hints.min_quality);
    let matte = details.matte;
    let png_style =
        manual_and_default_hints.and_then(|hints| hints.mimic).unwrap_or(PngEncoderStyle::Default);

    let manual_hint_lossless = BoolKeep::and_resolve(
        manual_and_default_hints.and_then(|hints| hints.lossless),
        details.source_lossless_capable,
    );

    let source_format = details.source_image_format;

    let good_defaults_for_source = match source_format {
        _ if details.source_lossless_capable == Some(true) => {
            get_quality_hints(&QualityProfile::Lossless)
        }
        Some(OutputImageFormat::Webp) => get_quality_hints(&QualityProfile::High),
        Some(OutputImageFormat::Jpeg) => get_quality_hints(&QualityProfile::High),
        Some(OutputImageFormat::Jxl) => get_quality_hints(&QualityProfile::High),
        _ => get_quality_hints(&QualityProfile::Lossless),
    };

    let lossless_inferred_from_quality_and_source = manual_target_quality.is_none()
        && manual_min_quality.is_none()
        && good_defaults_for_source.is_lossless();

    let lossless = details
        .lossless_setting
        .or(manual_hint_lossless)
        .unwrap_or(lossless_inferred_from_quality_and_source);

    let max_deflate = manual_and_default_hints.and_then(|hints| hints.hint_max_deflate);

    if let Some(profile_hints) = profile_hints {
        if profile_hints.png == 100 || lossless {
            Ok(PngEncodingDetails::LodePngLossless { max_deflate, matte })
        } else {
            Ok(PngEncodingDetails::PngQuant {
                speed: Some(profile_hints.png_s),
                target_quality: Some(profile_hints.png_max),
                minimum_quality: Some(profile_hints.png),
                max_deflate,
                matte,
            })
        }
    } else {
        match png_style {
            PngEncoderStyle::Libpng => {
                let depth =
                    if !details.has_alpha { s::PngBitDepth::Png24 } else { s::PngBitDepth::Png32 };
                let zlib_compression = if max_deflate == Some(true) { Some(9) } else { None };
                Ok(PngEncodingDetails::LibPng { depth: Some(depth), matte, zlib_compression })
            }
            PngEncoderStyle::Pngquant | PngEncoderStyle::Default if !lossless => {
                let manual_target_quality = manual_target_quality
                    .unwrap_or(good_defaults_for_source.png_max as f32)
                    .clamp(0.0, 100.0) as u8;
                let manual_min_quality = manual_min_quality
                    .unwrap_or(good_defaults_for_source.png as f32)
                    .clamp(0.0, manual_target_quality.into())
                    as u8;
                let manual_quantization_speed = manual_and_default_hints
                    .and_then(|hints| hints.quantization_speed)
                    .unwrap_or(good_defaults_for_source.png_s)
                    .clamp(1, 10);
                Ok(PngEncodingDetails::PngQuant {
                    speed: Some(manual_quantization_speed),
                    target_quality: Some(manual_target_quality),
                    minimum_quality: Some(manual_min_quality),
                    max_deflate,
                    matte,
                })
            }
            _ => {
                let max_deflate = manual_and_default_hints.and_then(|hints| hints.hint_max_deflate);
                Ok(PngEncodingDetails::LodePngLossless { max_deflate, matte })
            }
        }
    }
}

fn config_png_legacy(ctx: &Context, details: AutoEncoderDetails) -> Result<PngEncodingDetails> {
    let profile_hints = details
        .quality_profile
        .map(|qp| get_quality_hints_with_dpr(&qp, details.quality_profile_dpr));
    let manual_and_default_hints = details.encoder_hints.and_then(|hints| hints.png);
    let manual_target_quality = manual_and_default_hints.and_then(|hints| hints.quality);
    let manual_min_quality = manual_and_default_hints.and_then(|hints| hints.min_quality);
    let matte = details.matte;
    let png_style =
        manual_and_default_hints.and_then(|hints| hints.mimic).unwrap_or(PngEncoderStyle::Default);
    let manual_lossless = manual_and_default_hints.and_then(|hints| hints.lossless);
    //TODO: Note that PNG has special rules for the default value of lossless - the manual hint wins
    let lossless = match (details.legacy_needs_lossless, manual_lossless) {
        (Some(true), _) => Some(true),
        (_, Some(BoolKeep::Keep)) => {
            Some(details.source_image_info.map(|info| info.lossless).unwrap_or(false))
        }
        (_, Some(BoolKeep::True)) => Some(true),
        (_, Some(BoolKeep::False)) => Some(false),
        (Some(false), None) => Some(false),
        (None, None) => {
            Some(manual_target_quality.is_none() || png_style == PngEncoderStyle::Libpng)
        }
    }
    .unwrap();

    let max_deflate = manual_and_default_hints.and_then(|hints| hints.hint_max_deflate);

    if let Some(profile_hints) = profile_hints {
        if profile_hints.png == 100 || lossless {
            Ok(PngEncodingDetails::LodePngLossless { max_deflate, matte })
        } else {
            Ok(PngEncodingDetails::PngQuant {
                speed: Some(profile_hints.png_s),
                target_quality: Some(profile_hints.png_max),
                minimum_quality: Some(profile_hints.png),
                max_deflate,
                matte,
            })
        }
    } else {
        match png_style {
            PngEncoderStyle::Libpng => {
                let depth =
                    if !details.has_alpha { s::PngBitDepth::Png24 } else { s::PngBitDepth::Png32 };
                let zlib_compression = if max_deflate == Some(true) { Some(9) } else { None };
                Ok(PngEncodingDetails::LibPng { depth: Some(depth), matte, zlib_compression })
            }
            PngEncoderStyle::Pngquant | PngEncoderStyle::Default if !lossless => {
                let manual_target_quality =
                    manual_target_quality.map(|s| s.clamp(0.0, 100.0) as u8);
                let manual_min_quality = manual_and_default_hints
                    .and_then(|hints| hints.min_quality)
                    .map(|s| s.clamp(0.0, 100.0) as u8);
                let manual_quantization_speed = manual_and_default_hints
                    .and_then(|hints| hints.quantization_speed)
                    .map(|s| s.clamp(1, 10));
                Ok(PngEncodingDetails::PngQuant {
                    speed: manual_quantization_speed,
                    target_quality: manual_target_quality,
                    minimum_quality: manual_min_quality,
                    max_deflate,
                    matte,
                })
            }
            _ => {
                let max_deflate = manual_and_default_hints.and_then(|hints| hints.hint_max_deflate);
                Ok(PngEncodingDetails::LodePngLossless { max_deflate, matte })
            }
        }
    }
}

// ── AutoEncoderDetails + builder ────────────────────────────────────────────

#[derive(Debug, Clone)]
struct AutoEncoderDetails {
    format_auto_mode: bool,
    format: Option<OutputImageFormat>,
    source_image_format: Option<OutputImageFormat>,
    quality_profile: Option<s::QualityProfile>,
    quality_profile_dpr: Option<f32>,
    matte: Option<s::Color>,
    allow: AllowedFormats,
    encoder_hints: Option<s::EncoderHints>,
    has_animation: bool,
    has_alpha: bool,
    source_lossless_capable: Option<bool>,
    lossless_setting: Option<bool>,
    legacy_needs_lossless: Option<bool>,
    final_pixel_count: u64,
    source_image_info: Option<ImageInfo>,
    v: EncodeEngineVersion,
}

#[allow(clippy::too_many_arguments)]
fn build_auto_encoder_details(
    ctx: &Context,
    preset: &s::EncoderPreset,
    v: &EncodeEngineVersion,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
    format: Option<OutputImageFormat>,
    quality_profile: Option<s::QualityProfile>,
    quality_profile_dpr: Option<f32>,
    matte: Option<s::Color>,
    lossless_setting: Option<BoolKeep>,
    allow: Option<AllowedFormats>,
    encoder_hints: Option<s::EncoderHints>,
) -> Result<AutoEncoderDetails> {
    //NB: we assume the first of the decoder_io_ids is the source image, and take our animation/format cue from it

    let matte_is_opaque = matte.clone().map(|c| c.is_opaque()).unwrap_or(false);

    let source_image_info: Option<ImageInfo> = if !decoder_io_ids.is_empty() {
        Some(
            // This logic may not be correct with watermarks if they are added before the decoder
            // But in Imageflow.Net, we make sure the main image is decoder 0
            ctx.get_unscaled_unrotated_image_info(*decoder_io_ids.first().unwrap())
                .map_err(|e| e.at(here!()))?,
        )
    } else {
        None
    };
    let source_image_info_copy = source_image_info.clone();

    let bitmaps = ctx.borrow_bitmaps().map_err(|e| e.at(here!()))?;
    let final_bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;

    let has_alpha = final_bitmap.info().alpha_meaningful() && !matte_is_opaque;
    let final_pixel_count =
        final_bitmap.info().width() as u64 * final_bitmap.info().height() as u64;

    let source_image_format =
        source_image_info.as_ref().and_then(|i| OutputImageFormat::parse(&i.preferred_mime_type));

    let has_animation = source_image_info.as_ref().map(|i| i.multiple_frames).unwrap_or(false);

    // Keep becomes auto if no decoders exist, otherwise inherits from the first io.
    let explicit_format = match format {
        Some(OutputImageFormat::Keep) => source_image_format,
        other => other,
    };

    let mut legacy_needs_lossless =
        match (source_image_info.as_ref().map(|i| i.lossless), lossless_setting) {
            (Some(true), Some(BoolKeep::Keep)) => Some(true),
            (Some(false), Some(BoolKeep::Keep)) => Some(false),
            (None, Some(BoolKeep::Keep)) => Some(has_alpha), //No decoder, no source, default to match alpha
            (_, Some(BoolKeep::True)) => Some(true),
            (_, Some(BoolKeep::False)) => Some(false),
            (_, None) => None,
        };

    let source_lossless_capable = source_image_info.as_ref().map(|i| i.lossless);
    let mut lossless_setting = BoolKeep::and_resolve(lossless_setting, source_lossless_capable);
    if quality_profile == Some(s::QualityProfile::Lossless) {
        lossless_setting = Some(true);
        legacy_needs_lossless = Some(true);
    }
    Ok(AutoEncoderDetails {
        format_auto_mode: format.is_none(),
        format: explicit_format,
        quality_profile,
        quality_profile_dpr,
        lossless_setting,
        source_image_format,
        matte: matte.clone(),
        allow: evaluate_allowed_formats(allow),
        encoder_hints,
        has_animation,
        has_alpha,
        legacy_needs_lossless,
        source_lossless_capable,
        final_pixel_count,
        source_image_info: source_image_info_copy,
        v: *v,
    })
}

fn evaluate_allowed_formats(allowed: Option<AllowedFormats>) -> AllowedFormats {
    match allowed {
        Some(allowed) => allowed.expand_sets(),
        None => AllowedFormats::web_safe(),
    }
}

// ── Format selection ────────────────────────────────────────────────────────

fn format_select_with_specified(
    mut specified_format: Option<OutputImageFormat>,
    details: &AutoEncoderDetails,
    codecs: &EnabledCodecs,
) -> Result<OutputImageFormat> {
    // Downgrade to auto-select if specified format has no encoder
    if let Some(fmt) = specified_format {
        if let Some(img_fmt) = fmt.to_image_format() {
            if !codecs.has_encoder_for_format(img_fmt) {
                specified_format = None;
            }
        }
    }
    match specified_format {
        Some(other) => Ok(other),
        None => {
            let selector = crate::codecs::CodecSelector::new(codecs);
            let criteria = crate::codecs::FormatCriteria {
                allowed: details.allow,
                has_alpha: details.has_alpha,
                has_animation: details.has_animation,
                lossless: details.lossless_setting.or(details.legacy_needs_lossless),
                source_lossless: details.source_lossless_capable,
                pixel_count: details.final_pixel_count,
                quality_profile: details.quality_profile,
            };
            selector.select_format(&criteria).map(|s| s.chosen).ok_or(nerror!(
                ErrorKind::InvalidArgument,
                "No formats enabled; try 'allow': {{ 'web_safe':true}}"
            ))
        }
    }
}

// Format selection logic is now in CodecSelector::select_format (mod.rs)
