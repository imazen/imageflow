//! Build the `StaticInfoResponse` returned by `v1/static/info`.
//!
//! The response is derived purely from compile-time / load-time state —
//! zencodec's `ImageFormatRegistry`, `EnabledCodecs::default()`, and
//! each codec's `EncodeCapabilities` / `DecodeCapabilities`. It does
//! NOT depend on any `Context` field. Results are cached in a
//! `OnceLock<Arc<StaticInfoResponse>>` plus a `OnceLock<String>` for
//! the pre-serialized JSON, so repeat calls are a pointer load.
//!
//! Distinct from `v1/context/get_net_support`, which is `Context`-scoped
//! (depends on trusted policy + per-job narrowing).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use imageflow_types::killbits::ImageFormat as IfImageFormat;
use imageflow_types::static_info::{
    BuildInfo, CapsSummary, CodecAvailability, CodecRole, FormatAvailability, RiapiCategory,
    RiapiKeyInfo, RiapiSchema, RiapiValueKind, ServerRecommendations, StaticInfoResponse,
};

use crate::codecs::{EnabledCodecs, NamedDecoders, NamedEncoders};

static STATIC_INFO: OnceLock<Arc<StaticInfoResponse>> = OnceLock::new();
static STATIC_INFO_JSON: OnceLock<String> = OnceLock::new();

/// Return the cached parsed response, building it on first call.
pub fn get_static_info() -> Arc<StaticInfoResponse> {
    STATIC_INFO.get_or_init(|| Arc::new(build_static_info())).clone()
}

/// Return the cached pre-serialized JSON string. Cheap to call on the
/// hot path — subsequent calls are a single pointer load with no serde
/// work.
pub fn get_static_info_json() -> &'static str {
    STATIC_INFO_JSON.get_or_init(|| {
        let info = get_static_info();
        serde_json::to_string(&*info).expect("StaticInfoResponse must serialize")
    })
}

/// Construct the response from scratch. Not exposed publicly; callers
/// hit the cache via `get_static_info()`.
fn build_static_info() -> StaticInfoResponse {
    let enabled = EnabledCodecs::default();

    let build = build_info();
    let (formats_available, codecs) = build_formats_and_codecs(&enabled);
    let riapi = build_riapi_schema();
    let server_recommendations = build_server_recommendations();

    StaticInfoResponse {
        imageflow_version: imageflow_types::version::one_line_version(),
        build,
        formats_available,
        codecs,
        riapi,
        server_recommendations,
    }
}

// ===========================================================================
// Build info
// ===========================================================================

fn build_info() -> BuildInfo {
    let mut features: Vec<String> = Vec::new();
    if cfg!(feature = "c-codecs") {
        features.push("c-codecs".to_string());
    }
    if cfg!(feature = "zen-codecs") {
        features.push("zen-codecs".to_string());
    }
    if cfg!(feature = "schema-export") {
        features.push("schema-export".to_string());
    }
    if cfg!(feature = "json-schema") {
        features.push("json-schema".to_string());
    }

    let compile_deny_decode = imageflow_types::build_killbits::COMPILE_DENY_DECODE
        .iter()
        .map(|f| f.as_snake().to_string())
        .collect();
    let compile_deny_encode = imageflow_types::build_killbits::COMPILE_DENY_ENCODE
        .iter()
        .map(|f| f.as_snake().to_string())
        .collect();

    let codec_priority_default =
        imageflow_types::build_killbits::CODEC_PRIORITY_DEFAULT.as_snake().to_string();

    BuildInfo {
        features,
        compile_deny_decode,
        compile_deny_encode,
        codec_priority_default,
    }
}

// ===========================================================================
// Format + codec aggregation
// ===========================================================================

/// Returns the zencodec `ImageFormatDefinition` for each imageflow
/// `ImageFormat`, for the subset of formats zencodec knows about.
///
/// Only present under `zen-codecs`. Without that feature, we fall back
/// to a hand-maintained metadata table below.
#[cfg(feature = "zen-codecs")]
fn zc_def_for(f: IfImageFormat) -> Option<&'static zc::ImageFormatDefinition> {
    use zc::ImageFormat as Zf;
    let zf = match f {
        IfImageFormat::Jpeg => Zf::Jpeg,
        IfImageFormat::Png => Zf::Png,
        IfImageFormat::Gif => Zf::Gif,
        IfImageFormat::Webp => Zf::WebP,
        IfImageFormat::Avif => Zf::Avif,
        IfImageFormat::Jxl => Zf::Jxl,
        IfImageFormat::Heic => Zf::Heic,
        IfImageFormat::Bmp => Zf::Bmp,
        IfImageFormat::Tiff => Zf::Tiff,
        IfImageFormat::Pnm => Zf::Pnm,
        _ => return None,
    };
    zf.definition()
}

#[cfg(not(feature = "zen-codecs"))]
fn zc_def_for(_f: IfImageFormat) -> Option<&'static ()> {
    None
}

/// Hand-maintained format metadata, used as a fallback when zencodec
/// isn't compiled in (and as the only source for formats zencodec
/// doesn't define definitions for, should any arise).
struct FallbackFormatMetadata {
    display_name: &'static str,
    preferred_mime_type: &'static str,
    mime_types: &'static [&'static str],
    preferred_extension: &'static str,
    extensions: &'static [&'static str],
    supports_alpha: bool,
    supports_animation: bool,
    supports_lossless: bool,
    supports_lossy: bool,
    magic_bytes_needed: u32,
}

fn fallback_metadata(f: IfImageFormat) -> FallbackFormatMetadata {
    match f {
        IfImageFormat::Jpeg => FallbackFormatMetadata {
            display_name: "JPEG",
            preferred_mime_type: "image/jpeg",
            mime_types: &["image/jpeg"],
            preferred_extension: "jpg",
            extensions: &["jpg", "jpeg", "jpe", "jfif"],
            supports_alpha: false,
            supports_animation: false,
            supports_lossless: false,
            supports_lossy: true,
            magic_bytes_needed: 3,
        },
        IfImageFormat::Png => FallbackFormatMetadata {
            display_name: "PNG",
            preferred_mime_type: "image/png",
            mime_types: &["image/png"],
            preferred_extension: "png",
            extensions: &["png"],
            supports_alpha: true,
            supports_animation: false,
            supports_lossless: true,
            supports_lossy: false,
            magic_bytes_needed: 8,
        },
        IfImageFormat::Gif => FallbackFormatMetadata {
            display_name: "GIF",
            preferred_mime_type: "image/gif",
            mime_types: &["image/gif"],
            preferred_extension: "gif",
            extensions: &["gif"],
            supports_alpha: true,
            supports_animation: true,
            supports_lossless: true,
            supports_lossy: false,
            magic_bytes_needed: 6,
        },
        IfImageFormat::Webp => FallbackFormatMetadata {
            display_name: "WebP",
            preferred_mime_type: "image/webp",
            mime_types: &["image/webp"],
            preferred_extension: "webp",
            extensions: &["webp"],
            supports_alpha: true,
            supports_animation: true,
            supports_lossless: true,
            supports_lossy: true,
            magic_bytes_needed: 12,
        },
        IfImageFormat::Avif => FallbackFormatMetadata {
            display_name: "AVIF",
            preferred_mime_type: "image/avif",
            mime_types: &["image/avif"],
            preferred_extension: "avif",
            extensions: &["avif"],
            supports_alpha: true,
            supports_animation: true,
            supports_lossless: true,
            supports_lossy: true,
            magic_bytes_needed: 12,
        },
        IfImageFormat::Jxl => FallbackFormatMetadata {
            display_name: "JPEG XL",
            preferred_mime_type: "image/jxl",
            mime_types: &["image/jxl"],
            preferred_extension: "jxl",
            extensions: &["jxl"],
            supports_alpha: true,
            supports_animation: true,
            supports_lossless: true,
            supports_lossy: true,
            magic_bytes_needed: 12,
        },
        IfImageFormat::Heic => FallbackFormatMetadata {
            display_name: "HEIC",
            preferred_mime_type: "image/heic",
            mime_types: &["image/heic", "image/heif"],
            preferred_extension: "heic",
            extensions: &["heic", "heif"],
            supports_alpha: true,
            supports_animation: true,
            supports_lossless: true,
            supports_lossy: true,
            magic_bytes_needed: 12,
        },
        IfImageFormat::Bmp => FallbackFormatMetadata {
            display_name: "BMP",
            preferred_mime_type: "image/bmp",
            mime_types: &["image/bmp", "image/x-ms-bmp"],
            preferred_extension: "bmp",
            extensions: &["bmp"],
            supports_alpha: false,
            supports_animation: false,
            supports_lossless: true,
            supports_lossy: false,
            magic_bytes_needed: 2,
        },
        IfImageFormat::Tiff => FallbackFormatMetadata {
            display_name: "TIFF",
            preferred_mime_type: "image/tiff",
            mime_types: &["image/tiff"],
            preferred_extension: "tiff",
            extensions: &["tif", "tiff"],
            supports_alpha: true,
            supports_animation: false,
            supports_lossless: true,
            supports_lossy: true,
            magic_bytes_needed: 4,
        },
        IfImageFormat::Pnm => FallbackFormatMetadata {
            display_name: "PNM",
            preferred_mime_type: "image/x-portable-anymap",
            mime_types: &["image/x-portable-anymap"],
            preferred_extension: "pnm",
            extensions: &["pbm", "pgm", "ppm", "pnm", "pam", "pfm"],
            supports_alpha: false,
            supports_animation: false,
            supports_lossless: true,
            supports_lossy: false,
            magic_bytes_needed: 2,
        },
        _ => FallbackFormatMetadata {
            display_name: "Unknown",
            preferred_mime_type: "application/octet-stream",
            mime_types: &["application/octet-stream"],
            preferred_extension: "bin",
            extensions: &["bin"],
            supports_alpha: false,
            supports_animation: false,
            supports_lossless: false,
            supports_lossy: false,
            magic_bytes_needed: 0,
        },
    }
}

/// Collect format metadata via zencodec when available, else fall
/// back.
fn format_metadata(f: IfImageFormat) -> FallbackFormatMetadata {
    #[cfg(feature = "zen-codecs")]
    {
        if let Some(def) = zc_def_for(f) {
            return FallbackFormatMetadata {
                display_name: def.display_name,
                preferred_mime_type: def.preferred_mime_type,
                mime_types: def.mime_types,
                preferred_extension: def.preferred_extension,
                extensions: def.extensions,
                supports_alpha: def.supports_alpha,
                supports_animation: def.supports_animation,
                supports_lossless: def.supports_lossless,
                supports_lossy: def.supports_lossy,
                magic_bytes_needed: def.magic_bytes_needed as u32,
            };
        }
    }
    fallback_metadata(f)
}

fn build_formats_and_codecs(
    enabled: &EnabledCodecs,
) -> (BTreeMap<String, FormatAvailability>, BTreeMap<String, CodecAvailability>) {
    // Seed each known format with empty availability + metadata.
    let mut formats: BTreeMap<IfImageFormat, FormatAvailability> = BTreeMap::new();
    for &f in IfImageFormat::ALL {
        let meta = format_metadata(f);
        formats.insert(
            f,
            FormatAvailability {
                decode: false,
                encode: false,
                display_name: meta.display_name.to_string(),
                preferred_mime_type: meta.preferred_mime_type.to_string(),
                mime_types: meta.mime_types.iter().map(|s| s.to_string()).collect(),
                preferred_extension: meta.preferred_extension.to_string(),
                extensions: meta.extensions.iter().map(|s| s.to_string()).collect(),
                supports_alpha: meta.supports_alpha,
                supports_animation: meta.supports_animation,
                supports_lossless: meta.supports_lossless,
                supports_lossy: meta.supports_lossy,
                magic_bytes_needed: meta.magic_bytes_needed,
                encode_union: None,
                decode_union: None,
            },
        );
    }

    // Per-format capability accumulators. Keyed by imageflow's ImageFormat.
    let mut encode_acc: BTreeMap<IfImageFormat, CapsSummary> = BTreeMap::new();
    let mut decode_acc: BTreeMap<IfImageFormat, CapsSummary> = BTreeMap::new();

    let mut codecs: BTreeMap<String, CodecAvailability> = BTreeMap::new();

    // Walk enabled encoders.
    for &enc in enabled.encoders.iter() {
        let wire = enc.wire_name();
        let format = wire.image_format();
        let caps = encoder_caps(enc);

        if let Some(entry) = formats.get_mut(&format) {
            entry.encode = true;
        }
        let acc = encode_acc.entry(format).or_insert_with(CapsSummary::empty_for_union);
        acc.union_in_place(&caps);

        codecs.insert(
            wire.as_snake().to_string(),
            CodecAvailability {
                format,
                role: CodecRole::Encode,
                caps,
            },
        );
    }

    // Walk enabled decoders.
    for &dec in enabled.decoders.iter() {
        let wire = dec.wire_name();
        let format = wire.image_format();
        let caps = decoder_caps(dec);

        if let Some(entry) = formats.get_mut(&format) {
            entry.decode = true;
        }
        let acc = decode_acc.entry(format).or_insert_with(CapsSummary::empty_for_union);
        acc.union_in_place(&caps);

        codecs.insert(
            wire.as_snake().to_string(),
            CodecAvailability {
                format,
                role: CodecRole::Decode,
                caps,
            },
        );
    }

    // Collapse the empty-union sentinel (`threads_supported_range =
    // [u16::MAX, 0]`) when a format got no codecs. We only attach the
    // union if at least one codec contributed.
    for (f, fa) in formats.iter_mut() {
        if let Some(caps) = encode_acc.remove(f) {
            fa.encode_union = Some(caps);
        }
        if let Some(caps) = decode_acc.remove(f) {
            fa.decode_union = Some(caps);
        }
    }

    // Re-key the format map by snake-case name for the JSON wire form.
    let formats_available = formats
        .into_iter()
        .map(|(f, fa)| (f.as_snake().to_string(), fa))
        .collect();

    (formats_available, codecs)
}

// ===========================================================================
// Per-codec capability descriptors
// ===========================================================================

/// Static encoder capabilities. We route through zencodec's
/// `EncodeCapabilities` for zen codecs and hand-author matching shapes
/// for C codecs (which don't implement the zencodec trait hierarchy).
fn encoder_caps(enc: NamedEncoders) -> CapsSummary {
    match enc {
        NamedEncoders::GifEncoder => CapsSummary {
            icc: false,
            exif: false,
            xmp: false,
            cicp: false,
            lossy: false,
            lossless: true,
            hdr: false,
            gain_map: false,
            native_alpha: true,
            native_gray: false,
            native_16bit: false,
            native_f32: false,
            animation: true,
            push_rows: false,
            effort_range: None,
            quality_range: None,
            threads_supported_range: [1, 1],
        },
        #[cfg(feature = "c-codecs")]
        NamedEncoders::MozJpegEncoder => CapsSummary {
            icc: true,
            exif: true,
            xmp: true,
            cicp: false,
            lossy: true,
            lossless: false,
            hdr: false,
            gain_map: false,
            native_alpha: false,
            native_gray: true,
            native_16bit: false,
            native_f32: false,
            animation: false,
            push_rows: true,
            effort_range: None,
            quality_range: Some([0.0, 100.0]),
            threads_supported_range: [1, 1],
        },
        NamedEncoders::PngQuantEncoder => CapsSummary {
            icc: false,
            exif: false,
            xmp: false,
            cicp: false,
            lossy: true,
            lossless: false,
            hdr: false,
            gain_map: false,
            native_alpha: true,
            native_gray: false,
            native_16bit: false,
            native_f32: false,
            animation: false,
            push_rows: false,
            effort_range: Some([1, 10]),
            quality_range: Some([0.0, 100.0]),
            threads_supported_range: [1, 1],
        },
        NamedEncoders::LodePngEncoder => CapsSummary {
            icc: false,
            exif: false,
            xmp: false,
            cicp: false,
            lossy: false,
            lossless: true,
            hdr: false,
            gain_map: false,
            native_alpha: true,
            native_gray: true,
            native_16bit: true,
            native_f32: false,
            animation: false,
            push_rows: false,
            effort_range: None,
            quality_range: None,
            threads_supported_range: [1, 1],
        },
        #[cfg(feature = "c-codecs")]
        NamedEncoders::WebPEncoder => CapsSummary {
            icc: true,
            exif: true,
            xmp: true,
            cicp: false,
            lossy: true,
            lossless: true,
            hdr: false,
            gain_map: false,
            native_alpha: true,
            native_gray: false,
            native_16bit: false,
            native_f32: false,
            animation: false,
            push_rows: false,
            effort_range: Some([0, 6]),
            quality_range: Some([0.0, 100.0]),
            threads_supported_range: [1, 1],
        },
        #[cfg(feature = "c-codecs")]
        NamedEncoders::LibPngRsEncoder => CapsSummary {
            icc: true,
            exif: false,
            xmp: false,
            cicp: false,
            lossy: false,
            lossless: true,
            hdr: false,
            gain_map: false,
            native_alpha: true,
            native_gray: true,
            native_16bit: true,
            native_f32: false,
            animation: false,
            push_rows: true,
            effort_range: Some([0, 9]),
            quality_range: None,
            threads_supported_range: [1, 1],
        },
        #[cfg(feature = "zen-codecs")]
        NamedEncoders::ZenJpegEncoder => zc_encode_caps(
            <zenjpeg::JpegEncoderConfig as zc::encode::EncoderConfig>::capabilities(),
        ),
        #[cfg(feature = "zen-codecs")]
        NamedEncoders::ZenWebPEncoder => zc_encode_caps(
            <zenwebp::zencodec::WebpEncoderConfig as zc::encode::EncoderConfig>::capabilities(),
        ),
        #[cfg(feature = "zen-codecs")]
        NamedEncoders::ZenGifEncoder => zc_encode_caps(
            <zengif::GifEncoderConfig as zc::encode::EncoderConfig>::capabilities(),
        ),
        #[cfg(feature = "zen-codecs")]
        NamedEncoders::ZenPngEncoder
        | NamedEncoders::ZenPngZenquantEncoder
        | NamedEncoders::ZenPngImagequantEncoder => zc_encode_caps(
            <zenpng::PngEncoderConfig as zc::encode::EncoderConfig>::capabilities(),
        ),
        #[cfg(feature = "zen-codecs")]
        NamedEncoders::ZenAvifEncoder => zc_encode_caps(
            <zenavif::AvifEncoderConfig as zc::encode::EncoderConfig>::capabilities(),
        ),
        #[cfg(feature = "zen-codecs")]
        NamedEncoders::ZenJxlEncoder => zc_encode_caps(
            <zenjxl::JxlEncoderConfig as zc::encode::EncoderConfig>::capabilities(),
        ),
        #[cfg(feature = "zen-codecs")]
        NamedEncoders::ZenBmpEncoder => zc_encode_caps(
            <zenbitmaps::BmpEncoderConfig as zc::encode::EncoderConfig>::capabilities(),
        ),
        #[cfg(feature = "zen-codecs")]
        NamedEncoders::MozjpegRsEncoder => zc_encode_caps(
            <mozjpeg_rs::MozjpegEncoderConfig as zc::encode::EncoderConfig>::capabilities(),
        ),
    }
}

/// Static decoder capabilities. Mirrors `encoder_caps` for the decode
/// side.
fn decoder_caps(dec: NamedDecoders) -> CapsSummary {
    match dec {
        #[cfg(feature = "c-codecs")]
        NamedDecoders::MozJpegRsDecoder => CapsSummary {
            icc: true,
            exif: true,
            xmp: true,
            cicp: false,
            lossy: true,
            lossless: false,
            hdr: false,
            gain_map: false,
            native_alpha: false,
            native_gray: true,
            native_16bit: false,
            native_f32: false,
            animation: false,
            push_rows: true,
            effort_range: None,
            quality_range: None,
            threads_supported_range: [1, 1],
        },
        #[cfg(feature = "c-codecs")]
        NamedDecoders::ImageRsJpegDecoder => CapsSummary {
            icc: false,
            exif: false,
            xmp: false,
            cicp: false,
            lossy: true,
            lossless: false,
            hdr: false,
            gain_map: false,
            native_alpha: false,
            native_gray: true,
            native_16bit: false,
            native_f32: false,
            animation: false,
            push_rows: false,
            effort_range: None,
            quality_range: None,
            threads_supported_range: [1, 1],
        },
        NamedDecoders::ImageRsPngDecoder => CapsSummary {
            icc: true,
            exif: false,
            xmp: false,
            cicp: false,
            lossy: false,
            lossless: true,
            hdr: false,
            gain_map: false,
            native_alpha: true,
            native_gray: true,
            native_16bit: true,
            native_f32: false,
            animation: false,
            push_rows: false,
            effort_range: None,
            quality_range: None,
            threads_supported_range: [1, 1],
        },
        #[cfg(feature = "c-codecs")]
        NamedDecoders::LibPngRsDecoder => CapsSummary {
            icc: true,
            exif: false,
            xmp: false,
            cicp: false,
            lossy: false,
            lossless: true,
            hdr: false,
            gain_map: false,
            native_alpha: true,
            native_gray: true,
            native_16bit: true,
            native_f32: false,
            animation: false,
            push_rows: true,
            effort_range: None,
            quality_range: None,
            threads_supported_range: [1, 1],
        },
        NamedDecoders::GifRsDecoder => CapsSummary {
            icc: false,
            exif: false,
            xmp: false,
            cicp: false,
            lossy: false,
            lossless: true,
            hdr: false,
            gain_map: false,
            native_alpha: true,
            native_gray: false,
            native_16bit: false,
            native_f32: false,
            animation: true,
            push_rows: false,
            effort_range: None,
            quality_range: None,
            threads_supported_range: [1, 1],
        },
        #[cfg(feature = "c-codecs")]
        NamedDecoders::WebPDecoder => CapsSummary {
            icc: true,
            exif: true,
            xmp: true,
            cicp: false,
            lossy: true,
            lossless: true,
            hdr: false,
            gain_map: false,
            native_alpha: true,
            native_gray: false,
            native_16bit: false,
            native_f32: false,
            animation: false,
            push_rows: false,
            effort_range: None,
            quality_range: None,
            threads_supported_range: [1, 1],
        },
        #[cfg(feature = "zen-codecs")]
        NamedDecoders::ZenJpegDecoder => zc_decode_caps(
            <zenjpeg::JpegDecoderConfig as zc::decode::DecoderConfig>::capabilities(),
        ),
        #[cfg(feature = "zen-codecs")]
        NamedDecoders::ZenWebPDecoder => zc_decode_caps(
            <zenwebp::zencodec::WebpDecoderConfig as zc::decode::DecoderConfig>::capabilities(),
        ),
        #[cfg(feature = "zen-codecs")]
        NamedDecoders::ZenGifDecoder => zc_decode_caps(
            <zengif::GifDecoderConfig as zc::decode::DecoderConfig>::capabilities(),
        ),
        #[cfg(feature = "zen-codecs")]
        NamedDecoders::ZenPngDecoder => zc_decode_caps(
            <zenpng::PngDecoderConfig as zc::decode::DecoderConfig>::capabilities(),
        ),
        #[cfg(feature = "zen-codecs")]
        NamedDecoders::ZenAvifDecoder => zc_decode_caps(
            <zenavif::AvifDecoderConfig as zc::decode::DecoderConfig>::capabilities(),
        ),
        #[cfg(feature = "zen-codecs")]
        NamedDecoders::ZenJxlDecoder => zc_decode_caps(
            <zenjxl::JxlDecoderConfig as zc::decode::DecoderConfig>::capabilities(),
        ),
        #[cfg(feature = "zen-codecs")]
        NamedDecoders::ZenBmpDecoder => zc_decode_caps(
            <zenbitmaps::BmpDecoderConfig as zc::decode::DecoderConfig>::capabilities(),
        ),
    }
}

#[cfg(feature = "zen-codecs")]
fn zc_encode_caps(c: &zc::encode::EncodeCapabilities) -> CapsSummary {
    let (tmin, tmax) = c.threads_supported_range();
    CapsSummary {
        icc: c.icc(),
        exif: c.exif(),
        xmp: c.xmp(),
        cicp: c.cicp(),
        lossy: c.lossy(),
        lossless: c.lossless(),
        hdr: c.hdr(),
        gain_map: c.gain_map(),
        native_alpha: c.native_alpha(),
        native_gray: c.native_gray(),
        native_16bit: c.native_16bit(),
        native_f32: c.native_f32(),
        animation: c.animation(),
        push_rows: c.push_rows(),
        effort_range: c.effort_range(),
        quality_range: c.quality_range(),
        threads_supported_range: [tmin, tmax],
    }
}

#[cfg(feature = "zen-codecs")]
fn zc_decode_caps(c: &zc::decode::DecodeCapabilities) -> CapsSummary {
    let (tmin, tmax) = c.threads_supported_range();
    CapsSummary {
        icc: c.icc(),
        exif: c.exif(),
        xmp: c.xmp(),
        cicp: c.cicp(),
        // DecodeCapabilities has no lossy/lossless split; leave both
        // false so clients know the decoder doesn't discriminate.
        lossy: false,
        lossless: false,
        hdr: c.hdr(),
        gain_map: c.gain_map(),
        native_alpha: c.native_alpha(),
        native_gray: c.native_gray(),
        native_16bit: c.native_16bit(),
        native_f32: c.native_f32(),
        animation: c.animation(),
        // Treat row-level streaming decode as the analogue of push_rows.
        push_rows: c.streaming(),
        effort_range: None,
        quality_range: None,
        threads_supported_range: [tmin, tmax],
    }
}

// ===========================================================================
// RIAPI schema
// ===========================================================================

fn build_riapi_schema() -> RiapiSchema {
    // Reuse the canonical key list from imageflow_riapi. The existing
    // `v1/schema/riapi/latest/list_keys` endpoint is a thin wrapper
    // around this same source, so both endpoints stay in lockstep.
    let schema = imageflow_riapi::ir4::get_query_string_keys()
        .unwrap_or_else(|_| imageflow_types::json_messages::QueryStringSchema {
            key_names: Vec::new(),
        });

    let keys = schema
        .key_names
        .iter()
        .map(|name| annotate_riapi_key(name))
        .collect();

    RiapiSchema {
        keys,
        // The ir4 parser records unknown keys as warnings but proceeds
        // (see `ParseWarning::KeyNotRecognized`), so the end result is
        // that unknown keys are ignored in practice.
        ignores_unknown_keys: true,
    }
}

fn annotate_riapi_key(name: &str) -> RiapiKeyInfo {
    // Accept-header origin: only `accept.*` keys translate directly
    // from a media type. The edge layer is expected to synthesize
    // `accept.webp=1` etc.
    let accept_header_origin = match name {
        "accept.webp" => Some("image/webp".to_string()),
        "accept.avif" => Some("image/avif".to_string()),
        "accept.jxl" => Some("image/jxl".to_string()),
        _ => None,
    };

    let category = riapi_category(name);
    let cache_relevant = !matches!(category, RiapiCategory::Debug);
    let (accepts, enum_values) = riapi_value_shape(name);

    RiapiKeyInfo {
        name: name.to_string(),
        category,
        cache_relevant,
        accepts,
        enum_values,
        accept_header_origin,
    }
}

fn riapi_category(name: &str) -> RiapiCategory {
    // Exact-match categorization for keys that don't fit a prefix rule.
    match name {
        "w" | "h" | "width" | "height" | "maxwidth" | "maxheight" | "zoom" | "scale" | "mode"
        | "anchor" | "stretch" | "dpr" | "dppx" | "up.filter" | "down.filter" | "up.colorspace"
        | "down.colorspace" | "floatspace" | "srcset" | "short" | "thumbnail" => {
            return RiapiCategory::Resize;
        }
        "crop" | "cropxunits" | "cropyunits" | "c" | "c.gravity" | "trim.threshold"
        | "trim.percentpadding" => {
            return RiapiCategory::Crop;
        }
        "format" | "quality" | "encoder" | "lossless" | "subsampling" | "colors" | "dither"
        | "qp" | "qp.dpr" | "qp.dppx" => {
            return RiapiCategory::Output;
        }
        "autorotate" | "srotate" | "rotate" | "flip" | "sflip" | "frame" | "page"
        | "ignoreicc" | "ignore_icc_errors" | "decoder"
        | "decoder.min_precise_scaling_ratio"
        | "jpeg_idct_downscale_linear" => {
            return RiapiCategory::Source;
        }
        "404" | "bgcolor" | "paddingcolor" | "bordercolor" | "paddingwidth" | "paddingheight"
        | "margin" | "borderwidth" | "watermark" | "watermark_red_dot" | "s.roundcorners"
        | "preset" | "builder" | "process" | "cache" => {
            return RiapiCategory::Composition;
        }
        _ => {}
    }
    // Prefix rules.
    if let Some(prefix) = name.split('.').next() {
        match prefix {
            "accept" => return RiapiCategory::Negotiation,
            "f" | "s" | "a" => return RiapiCategory::Filter,
            "jpeg" | "png" | "webp" | "avif" | "jxl" => return RiapiCategory::Output,
            _ => {}
        }
    }
    RiapiCategory::Other
}

fn riapi_value_shape(name: &str) -> (RiapiValueKind, Vec<String>) {
    // Enums we care about (hand-audited). Everything else falls through
    // to a coarse integer/float/boolean/string guess from the key name.
    match name {
        "format" => {
            return (
                RiapiValueKind::Enum,
                ["auto", "keep", "jpeg", "png", "gif", "webp", "avif", "jxl"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            );
        }
        "mode" => {
            return (
                RiapiValueKind::Enum,
                ["max", "pad", "crop", "carve", "stretch"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            );
        }
        "anchor" => {
            return (
                RiapiValueKind::Enum,
                [
                    "topleft",
                    "topcenter",
                    "topright",
                    "middleleft",
                    "middlecenter",
                    "middleright",
                    "bottomleft",
                    "bottomcenter",
                    "bottomright",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            );
        }
        "flip" | "sflip" => {
            return (
                RiapiValueKind::Enum,
                ["none", "x", "y", "xy"].iter().map(|s| s.to_string()).collect(),
            );
        }
        _ => {}
    }

    // Integer keys.
    if matches!(
        name,
        "w" | "h"
            | "width"
            | "height"
            | "maxwidth"
            | "maxheight"
            | "rotate"
            | "srotate"
            | "frame"
            | "page"
            | "colors"
            | "quality"
            | "jpeg.quality"
            | "png.quality"
            | "png.min_quality"
            | "png.quantization_speed"
            | "png.max_deflate"
            | "webp.quality"
            | "avif.quality"
            | "avif.speed"
            | "jxl.effort"
            | "jxl.quality"
            | "paddingwidth"
            | "paddingheight"
            | "margin"
            | "borderwidth"
    ) {
        return (RiapiValueKind::Integer, Vec::new());
    }

    // Float keys.
    if matches!(
        name,
        "zoom"
            | "scale"
            | "dpr"
            | "dppx"
            | "qp.dpr"
            | "qp.dppx"
            | "trim.threshold"
            | "trim.percentpadding"
            | "f.sharpen"
            | "a.blur"
            | "a.sharpen"
            | "s.brightness"
            | "s.contrast"
            | "s.saturation"
            | "s.alpha"
            | "jxl.distance"
            | "decoder.min_precise_scaling_ratio"
    ) {
        return (RiapiValueKind::Float, Vec::new());
    }

    // Boolean keys.
    if matches!(
        name,
        "autorotate"
            | "lossless"
            | "webp.lossless"
            | "png.lossless"
            | "png.libpng"
            | "jxl.lossless"
            | "jpeg.progressive"
            | "jpeg.turbo"
            | "jpeg.li"
            | "ignoreicc"
            | "ignore_icc_errors"
            | "stretch"
            | "watermark_red_dot"
            | "s.invert"
            | "s.sepia"
            | "s.grayscale"
            | "accept.webp"
            | "accept.avif"
            | "accept.jxl"
            | "accept.color_profiles"
            | "jpeg_idct_downscale_linear"
    ) {
        return (RiapiValueKind::Boolean, Vec::new());
    }

    (RiapiValueKind::String, Vec::new())
}

// ===========================================================================
// Server recommendations
// ===========================================================================

fn build_server_recommendations() -> ServerRecommendations {
    let mut accept_header_translation = BTreeMap::new();
    accept_header_translation.insert("image/webp".to_string(), "accept.webp=1".to_string());
    accept_header_translation.insert("image/avif".to_string(), "accept.avif=1".to_string());
    accept_header_translation.insert("image/jxl".to_string(), "accept.jxl=1".to_string());

    ServerRecommendations {
        accept_header_translation,
        strip_from_cache_key: vec!["trace".to_string()],
        include_in_cache_key_prefix: vec![
            "accept.*".to_string(),
            "format".to_string(),
            "w".to_string(),
            "h".to_string(),
            "width".to_string(),
            "height".to_string(),
            "maxwidth".to_string(),
            "maxheight".to_string(),
            "dpr".to_string(),
            "dppx".to_string(),
            "quality".to_string(),
            "mode".to_string(),
            "crop".to_string(),
        ],
        vary_header: "Accept".to_string(),
        notes: vec![
            "Translate Accept header into accept.* RIAPI params at the edge and \
             include them in the cache key. This is a better-cardinality \
             substitute for Vary: Accept at the CDN layer; keep Vary: Accept for \
             client caches."
                .to_string(),
            "`v1/static/info` is cacheable forever — the response only changes \
             when the imageflow binary changes. Use `imageflow_version` as the \
             ETag."
                .to_string(),
        ],
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_cache_is_stable_across_calls() {
        let a = get_static_info_json() as *const str;
        let b = get_static_info_json() as *const str;
        // OnceLock hands back the same string slice every call — the
        // hot path is a pointer load, no serde work.
        assert_eq!(a, b);
    }

    #[test]
    fn parsed_cache_is_stable_across_calls() {
        let a = get_static_info();
        let b = get_static_info();
        // Arc points at the same allocation.
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn all_formats_are_represented() {
        let info = get_static_info();
        for &f in IfImageFormat::ALL {
            let key = f.as_snake();
            assert!(
                info.formats_available.contains_key(key),
                "format {} missing from formats_available",
                key
            );
        }
    }

    #[test]
    fn enabled_encoders_appear_in_codecs_table() {
        let info = get_static_info();
        let enabled = EnabledCodecs::default();
        for &enc in enabled.encoders.iter() {
            let name = enc.wire_name().as_snake();
            let entry = info.codecs.get(name).unwrap_or_else(|| {
                panic!("codec {} missing from codecs table", name);
            });
            assert_eq!(entry.role, CodecRole::Encode);
        }
    }

    #[test]
    fn enabled_decoders_appear_in_codecs_table() {
        let info = get_static_info();
        let enabled = EnabledCodecs::default();
        for &dec in enabled.decoders.iter() {
            let name = dec.wire_name().as_snake();
            let entry = info.codecs.get(name).unwrap_or_else(|| {
                panic!("codec {} missing from codecs table", name);
            });
            assert_eq!(entry.role, CodecRole::Decode);
        }
    }

    #[test]
    fn jpeg_encode_union_quality_is_0_to_100() {
        let info = get_static_info();
        // JPEG is always at least one of the configured encoders; the
        // union must include a 0-100 quality range.
        let jpeg = info.formats_available.get("jpeg").expect("jpeg format");
        if jpeg.encode {
            let union = jpeg.encode_union.as_ref().expect("encode union for enabled jpeg");
            let q = union.quality_range.expect("quality range on jpeg encode union");
            assert!(q[0] <= 0.0 + f32::EPSILON, "jpeg quality min should be 0: {q:?}");
            assert!(q[1] >= 100.0 - f32::EPSILON, "jpeg quality max should be 100: {q:?}");
        }
    }

    #[test]
    fn server_recommendations_translate_known_accept_headers() {
        let info = get_static_info();
        let m = &info.server_recommendations.accept_header_translation;
        assert_eq!(m.get("image/webp").map(String::as_str), Some("accept.webp=1"));
        assert_eq!(m.get("image/avif").map(String::as_str), Some("accept.avif=1"));
        assert_eq!(m.get("image/jxl").map(String::as_str), Some("accept.jxl=1"));
    }

    #[test]
    fn riapi_schema_has_accept_origin_for_accept_keys() {
        let info = get_static_info();
        let accept_webp = info
            .riapi
            .keys
            .iter()
            .find(|k| k.name == "accept.webp")
            .expect("accept.webp key present");
        assert_eq!(accept_webp.accept_header_origin.as_deref(), Some("image/webp"));
        assert_eq!(accept_webp.category, RiapiCategory::Negotiation);
        assert!(accept_webp.cache_relevant);
    }

    #[test]
    fn png_encode_and_decode_are_always_available() {
        let info = get_static_info();
        let png = info.formats_available.get("png").expect("png format");
        assert!(png.encode, "png encode should always be on (pngquant+lodepng)");
        assert!(png.decode, "png decode should always be on");
    }
}
