use crate::ffi;
use crate::ffi::ColorProfileSource;
use crate::ffi::DecoderColorInfo;
use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::io::IoProxy;
use imageflow_riapi::version::EncodeEngineVersion;
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
    let v = &EncodeEngineVersion::Preview;

    let codec = match *preset {
        
        // Can only be triggered in RIAPI with format=auto, or qp=something when format is not specified.
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
        // Unless qp is specified or format=auto, RIAPI always uses this with format=keep or format=value
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
    let final_format =format_select_with_specified(details.format, &details)
    .map_err(|e| e.at(here!()))?;

    //eprintln!("AutoEncoderDetails: {:?}", details);
    

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
        OutputImageFormat::Avif => create_avif_auto(ctx, io, bitmap_key, decoder_io_ids, details)
            .map_err(|e| e.at(here!()))?,
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

fn approximate_quality_profile(qp: Option<QualityProfile>) -> f32 {
    match qp {
        Some(find) => get_quality_hints(&find).p,
        None => 90.0,
    }
}

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



#[derive(Debug, Clone)]
struct WebPEncodingDetails {
    lossy: Option<f32>,
    lossless: bool,
    matte: Option<Color>,
}
fn create_webp_auto(
    ctx: &Context,
    io: IoProxy,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
    details: AutoEncoderDetails,
) -> Result<Box<dyn Encoder>> {
    let params = match details.v {
        EncodeEngineVersion::Preview => config_webp_auto_preview(ctx, details)?,
        _ => config_webp_auto_v2(ctx, details)?
    };
    Ok(Box::new(
        crate::codecs::webp::WebPEncoder::create(ctx, io, params.lossy, Some(params.lossless), params.matte)
            .map_err(|e| e.at(here!()))?,
    ))
}



fn config_webp_auto_v2(
    ctx: &Context,
    details: AutoEncoderDetails,
) -> Result<WebPEncodingDetails> {
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
    let lossless = details.legacy_needs_lossless.or(manual_lossless).unwrap_or(false);
    let quality = if !lossless {
        Some(profile_hints.map(|hints| hints.webp).unwrap_or(manual_quality))
    } else {
        None
    };

    Ok(WebPEncodingDetails {
        lossy: quality,
        lossless,
        matte,
    })
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
    let manual_lossless = manual_and_default_hints.and_then(|hints| hints.lossless);

    let manual_hint_lossless = BoolKeep::and_resolve(
        manual_and_default_hints.and_then(|hints| hints.lossless),
         details.source_lossless_capable);

    let source_format = details.source_image_format;

    let good_defaults_for_source = match source_format {
        _ if details.source_lossless_capable == Some(true) => get_quality_hints(&QualityProfile::Lossless),
        Some(OutputImageFormat::Webp) => get_quality_hints(&QualityProfile::High),
        Some(OutputImageFormat::Jpeg) => get_quality_hints(&QualityProfile::High),
        Some(OutputImageFormat::Jxl) => get_quality_hints(&QualityProfile::High),
        _ => get_quality_hints(&QualityProfile::High),
    };

    let lossless_inferred_from_quality_and_source = manual_quality.is_none()
            && good_defaults_for_source.is_lossless();
    
    let lossless = details.lossless_setting.or(manual_hint_lossless).unwrap_or(lossless_inferred_from_quality_and_source);


    let matte = details.matte;
    let quality =
        manual_and_default_hints.and_then(|hints| hints.quality).unwrap_or(good_defaults_for_source.webp).clamp(0.0, 100.0);


    let profile_or_manual_lossless = profile_hints.map(|hints| hints.is_lossless()).unwrap_or(lossless);
    let profile_or_manual_quality =if !profile_or_manual_lossless { profile_hints.map(|hints| hints.webp).or(Some(quality)) } else { None };

   
    Ok(WebPEncodingDetails {
        lossy: profile_or_manual_quality,
        lossless: profile_or_manual_lossless,
        matte,
    })
}

fn create_avif_auto(
    ctx: &Context,
    io: IoProxy,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
    details: AutoEncoderDetails,
) -> Result<Box<dyn Encoder>> {
    let profile_hints = details
        .quality_profile
        .map(|qp| get_quality_hints_with_dpr(&qp, details.quality_profile_dpr));

    let manual_and_default_hints = details.encoder_hints.and_then(|hints| hints.avif);
    let matte = details.matte;

    let good_defaults = get_quality_hints(&QualityProfile::Good);
    
    // Get quality from profile hints or manual hints
    let quality = profile_hints
        .map(|hints| hints.avif)
        .or_else(|| manual_and_default_hints.and_then(|hints| hints.quality))
        .unwrap_or(good_defaults.avif)
        .clamp(0.0, 100.0);

    // Get speed from profile hints or manual hints
    let speed = profile_hints
        .map(|hints| hints.avif_s)
        .or_else(|| manual_and_default_hints.and_then(|hints| hints.speed))
        .unwrap_or(good_defaults.avif_s)
        .clamp(0, 10);

    // Get alpha quality if specified
    let alpha_quality = manual_and_default_hints.and_then(|hints| hints.alpha_quality);

    Ok(Box::new(
        crate::codecs::avif_encoder::AvifEncoder::create(
            ctx,
            io,
            Some(quality),
            Some(speed),
            alpha_quality,
            matte,
        )
        .map_err(|e| e.at(here!()))?,
    ))
}

#[derive(Debug, Clone)]
enum PngEncodingDetails{
    LodePngLossless{ max_deflate: Option<bool>, matte: Option<Color> },
    PngQuant{ speed: Option<u8>, target_quality: Option<u8>, minimum_quality: Option<u8>, max_deflate: Option<bool>, matte: Option<Color> },
    LibPng{ depth: Option<imageflow_types::PngBitDepth>, matte: Option<Color>, zlib_compression: Option<u8> }
}

fn create_png_auto(
    ctx: &Context,
    io: IoProxy,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
    details: AutoEncoderDetails,
) -> Result<Box<dyn Encoder>> {
    //eprintln!("Encoding parameters: {:?}", &details);
    let png_details = match details.v {
        EncodeEngineVersion::Preview => {
            config_png_auto_preview(ctx,  details)?
        }
        _ => {
            config_png_legacy(ctx, details)?
        }
    };
    //eprintln!("Final params: {:?}",&png_details);
    match png_details {
        PngEncodingDetails::LodePngLossless { max_deflate, matte } => {
            Ok(Box::new(
                crate::codecs::lode::LodepngEncoder::create(ctx, io, max_deflate, matte)
                    .map_err(|e| e.at(here!()))?,
            ))
        }
        PngEncodingDetails::PngQuant { speed, target_quality, minimum_quality, max_deflate, matte } => {
            Ok(Box::new(
                crate::codecs::pngquant::PngquantEncoder::create(
                    ctx,
                    io,
                    speed,
                    target_quality,
                    minimum_quality,
                    max_deflate,
                    matte,
                )
                .map_err(|e| e.at(here!()))?,
            ))
        }
        PngEncodingDetails::LibPng { depth, matte, zlib_compression } => {
            Ok(Box::new(
                crate::codecs::libpng_encoder::LibPngEncoder::create(
                    ctx,
                    io,
                    depth,
                    matte,
                    zlib_compression,
                )
                .map_err(|e| e.at(here!()))?,
            ))
        }
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
    let manual_min_quality = manual_and_default_hints
                    .and_then(|hints| hints.min_quality);
    let matte = details.matte;
    let png_style =
        manual_and_default_hints.and_then(|hints| hints.mimic).unwrap_or(PngEncoderStyle::Default);

    let manual_hint_lossless = BoolKeep::and_resolve(
        manual_and_default_hints.and_then(|hints| hints.lossless),
         details.source_lossless_capable);

    let source_format = details.source_image_format;

    let good_defaults_for_source = match source_format {
        _ if details.source_lossless_capable == Some(true) => get_quality_hints(&QualityProfile::Lossless),
        Some(OutputImageFormat::Webp) => get_quality_hints(&QualityProfile::High),
        Some(OutputImageFormat::Jpeg) => get_quality_hints(&QualityProfile::High),
        Some(OutputImageFormat::Jxl) => get_quality_hints(&QualityProfile::High),
        _ => get_quality_hints(&QualityProfile::Lossless),
    };


    let lossless_inferred_from_quality_and_source = manual_target_quality.is_none() && manual_min_quality.is_none()
            && good_defaults_for_source.is_lossless();
    
    let lossless = details.lossless_setting.or(manual_hint_lossless).unwrap_or(lossless_inferred_from_quality_and_source);

    let max_deflate = manual_and_default_hints.and_then(|hints| hints.hint_max_deflate);

    if let Some(profile_hints) = profile_hints {
        if profile_hints.png == 100 || lossless {
            Ok(PngEncodingDetails::LodePngLossless { max_deflate, matte })
        } else {
            Ok(PngEncodingDetails::PngQuant {
                speed: Some(profile_hints.png_s as u8),
                target_quality: Some(profile_hints.png_max as u8),
                minimum_quality: Some(profile_hints.png as u8),
                max_deflate,
                matte,
            })
        }
    } else {
        match png_style {
            PngEncoderStyle::Libpng => {
                let depth = if !details.has_alpha {
                    s::PngBitDepth::Png24
                } else {
                    s::PngBitDepth::Png32
                };
                let zlib_compression = if max_deflate == Some(true) { Some(9) } else { None };
                Ok(PngEncodingDetails::LibPng {
                    depth: Some(depth),
                    matte,
                    zlib_compression,
                })
            }
            PngEncoderStyle::Pngquant | PngEncoderStyle::Default if !lossless => {
                let manual_target_quality = manual_target_quality.unwrap_or(good_defaults_for_source.png_max as f32).clamp(0.0, 100.0) as u8;
                let manual_min_quality = manual_min_quality.unwrap_or(good_defaults_for_source.png as f32).clamp(0.0, manual_target_quality.into()) as u8;
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



fn config_png_legacy(
    ctx: &Context,
    details: AutoEncoderDetails,
) -> Result<PngEncodingDetails> {
    let profile_hints = details
        .quality_profile
        .map(|qp| get_quality_hints_with_dpr(&qp, details.quality_profile_dpr));
    let manual_and_default_hints = details.encoder_hints.and_then(|hints| hints.png);
    let manual_target_quality = manual_and_default_hints.and_then(|hints| hints.quality);
    let manual_min_quality = manual_and_default_hints
                    .and_then(|hints| hints.min_quality);
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
        (None, None) => Some(manual_target_quality.is_none() || png_style == PngEncoderStyle::Libpng),
    }
    .unwrap();

    let max_deflate = manual_and_default_hints.and_then(|hints| hints.hint_max_deflate);

    if let Some(profile_hints) = profile_hints {
        if profile_hints.png == 100 || lossless {
            Ok(PngEncodingDetails::LodePngLossless { max_deflate, matte })
        } else {
            Ok(PngEncodingDetails::PngQuant {
                speed: Some(profile_hints.png_s as u8),
                target_quality: Some(profile_hints.png_max as u8),
                minimum_quality: Some(profile_hints.png as u8),
                max_deflate,
                matte,
            })
        }
    } else {
        match png_style {
            PngEncoderStyle::Libpng => {
                let depth = if !details.has_alpha {
                    s::PngBitDepth::Png24
                } else {
                    s::PngBitDepth::Png32
                };
                let zlib_compression = if max_deflate == Some(true) { Some(9) } else { None };
                Ok(PngEncodingDetails::LibPng {
                    depth: Some(depth),
                    matte,
                    zlib_compression,
                })
            }
            PngEncoderStyle::Pngquant | PngEncoderStyle::Default if !lossless => {
                let manual_target_quality = manual_target_quality.map(|s| s.clamp(0.0, 100.0) as u8);
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

// Helps select an encoder when format=null caused by (qp present and format absent, or format=auto)
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

    let source_image_format = source_image_info
        .as_ref()
        .and_then(|i| OutputImageFormat::from_str(&i.preferred_mime_type));

    let has_animation = source_image_info.as_ref().map(|i| i.multiple_frames).unwrap_or(false);
    
    // Keep becomes auto if no decoders exist, otherwise inherits from the first io.
    let explicit_format = match format {
        Some(OutputImageFormat::Keep) => source_image_format.or(None),
        other => other,
    };

    let mut legacy_needs_lossless = match (source_image_info.as_ref().map(|i| i.lossless), lossless_setting) {
        (Some(true), Some(BoolKeep::Keep)) => Some(true),
        (Some(false), Some(BoolKeep::Keep)) => Some(false),
        (None, Some(BoolKeep::Keep)) => Some(has_alpha), //No decoder, no source, default to match alpha
        (_, Some(BoolKeep::True)) => Some(true),
        (_, Some(BoolKeep::False)) => Some(false),
        (_, None) => None
    };
    
    let source_lossless_capable = source_image_info.as_ref().map(|i| i.lossless).or(None);
    let mut lossless_setting = BoolKeep::and_resolve(lossless_setting , source_lossless_capable);
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
        v: *v
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
    avif_lossless: bool,
    avif_animation: bool,
    jpegli: bool,
}
const FEATURES_IMPLEMENTED: FeaturesImplemented =
    FeaturesImplemented { jxl: false, avif: true, webp_animation: false, avif_lossless: false, avif_animation: false, jpegli: false };

fn format_select_with_specified(mut specified_format: Option<OutputImageFormat>, details: &AutoEncoderDetails) -> Result<OutputImageFormat> {

    if specified_format == Some(OutputImageFormat::Jxl) && !FEATURES_IMPLEMENTED.jxl {
        specified_format = None;
    }
    if specified_format == Some(OutputImageFormat::Avif) && !FEATURES_IMPLEMENTED.avif {
        specified_format = None;
    }
    match specified_format {
        Some(other) => Ok(other),
        None => match details.v {
            EncodeEngineVersion::Preview => format_auto_select_preview(details).ok_or(nerror!(
            ErrorKind::InvalidArgument,
            "No formats enabled; try 'allow': {{ 'web_safe':true}}"
        )),
            _ => format_auto_select_legacy(details).ok_or(nerror!(
            ErrorKind::InvalidArgument,
            "No formats enabled; try 'allow': {{ 'web_safe':true}}"
        ))
        }
    }
}

fn format_auto_select_preview(details: &AutoEncoderDetails) -> Option<OutputImageFormat> {
    let allowed = details.allow;
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

    // For animation,Avif if available, otherwise WebP if available, otherwise GIF. A this time, we don't implement avif or webp animation
    if details.has_animation {
        if details.lossless_setting == Some(true) {
            // WebP has lossless animation
            if FEATURES_IMPLEMENTED.webp_animation && allowed.webp == Some(true) {
                return Some(OutputImageFormat::Webp);
            }
        }
        if FEATURES_IMPLEMENTED.avif_animation && allowed.avif == Some(true) {
            return Some(OutputImageFormat::Avif);
        }
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

    // JXL is always the best if available, regardless of lossless 
    if FEATURES_IMPLEMENTED.jxl && allowed.jxl == Some(true) {
        return Some(OutputImageFormat::Jxl);
    }

    let choose_lossless = match details.v {
        EncodeEngineVersion::Preview => details.lossless_setting == Some(true) || details.source_lossless_capable == Some(true),
        _ => details.legacy_needs_lossless.unwrap_or(details.has_alpha || details.source_lossless_capable == Some(true)),
    };

    // Lossless path: PNG and WebP are faster and often smaller than AVIF for lossless
    // We are assuming all lossless formats support alpha
    if choose_lossless {
        // webp lossless is slow but so much smaller than avif/png
        if allowed.webp == Some(true) {
            return Some(OutputImageFormat::Webp);
        }
        // PNG is actually better than avif for lossless
        if allowed.png == Some(true) {
            return Some(OutputImageFormat::Png);
        }
        // our AVIF encoder doesn't support lossless yet
        if FEATURES_IMPLEMENTED.avif_lossless && allowed.avif == Some(true) {
            return Some(OutputImageFormat::Avif);
        }
        // If png is disabled, we might end up with avif or jpeg or something..
    }

    // For lossy images with alpha, prefer AVIF over WebP over PNG (better compression)
    if details.has_alpha {
        // AVIF handles lossy alpha very well
        if FEATURES_IMPLEMENTED.avif && allowed.avif == Some(true) {
            return Some(OutputImageFormat::Avif);
        }
        // WebP is second best for lossy alpha
        if allowed.webp == Some(true) {
            return Some(OutputImageFormat::Webp);
        }
        // PNG works but doesn't compress as well as AVIF/WebP
        if allowed.png == Some(true) {
            return Some(OutputImageFormat::Png);
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
    // We already handled alpha/lossless/animation above. 
    if can_jpegli {
        return Some(OutputImageFormat::Jpeg);
    }
    // At high quality ~90+, mozjpeg falls behind webp. (not sure if our custom chroma does)
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
    // WebP, if jpeg is disabled.
    if allowed.webp == Some(true) {
        return Some(OutputImageFormat::Webp);
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



fn format_auto_select_legacy(details: &AutoEncoderDetails) -> Option<OutputImageFormat> {
    let allowed = details.allow;
    let has_animation = details.has_animation;
    let has_alpha = details.has_alpha;
    let lossless_setting = details.legacy_needs_lossless;
    let pixel_count = details.final_pixel_count;
    let quality_profile = details.quality_profile;

    eprintln!("Auto format selection: {:#?}", details);

    if !allowed.any_formats_enabled() {
        return None;
    }
    // Rules in codec selection:
    // First, honor explicit format value
    // Second (if auto/lossy/lossless), preserve animation if present. We can assume all browsers that support webp also support animated webp, but jxl animation is not yet supported.
    // Third, preserve alpha channel if present and meaningful.
    // Fourth, respect lossless setting

    // For animation, WebP if available, otherwise GIF
    if has_animation {
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

    if FEATURES_IMPLEMENTED.avif && allowed.avif == Some(true) {
        if details.lossless_setting != Some(true) && details.source_lossless_capable != Some(true) && has_alpha{
            return Some(OutputImageFormat::Avif);
        }
    }

    // Lossless path and alpha path are the same.
    if details.legacy_needs_lossless.unwrap_or(details.has_alpha || details.source_lossless_capable == Some(true)) {
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
