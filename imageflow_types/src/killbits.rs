//! Format killbits: fine-grained per-format decode/encode gating.
//!
//! This module defines the types used by the three-layer codec killbits system:
//!
//! 1. **Build-time ceiling** (`build_killbits::COMPILE_DENY_*`) — formats this
//!    build refuses to decode/encode regardless of runtime config.
//! 2. **Trusted policy** (`Context::trusted_policy`) — set once via
//!    `v1/context/set_policy`; narrows the build-time ceiling but cannot widen it.
//! 3. **Job-level narrowing** — the existing `security` field on
//!    `Build001`/`Execute001` gains a `formats` sub-field. Job-level killbits
//!    may only **deny**; they can never widen what trusted policy allows.
//!
//! See `build_killbits.rs` for the compile-time arrays.

use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
#[cfg(feature = "schema-export")]
use utoipa::ToSchema;

/// The set of image formats that can be referenced by killbits.
///
/// `#[non_exhaustive]` so new formats can be added without semver breakage to
/// downstream matches. Serialized as `snake_case` strings.
#[non_exhaustive]
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    Jpeg,
    Png,
    Gif,
    Webp,
    Avif,
    Jxl,
    Heic,
    Bmp,
    Tiff,
    Pnm,
}

impl ImageFormat {
    /// All known formats, useful for iterating the full grid.
    pub const ALL: &'static [ImageFormat] = &[
        ImageFormat::Jpeg,
        ImageFormat::Png,
        ImageFormat::Gif,
        ImageFormat::Webp,
        ImageFormat::Avif,
        ImageFormat::Jxl,
        ImageFormat::Heic,
        ImageFormat::Bmp,
        ImageFormat::Tiff,
        ImageFormat::Pnm,
    ];

    /// snake_case form of this format's name.
    pub fn as_snake(self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "jpeg",
            ImageFormat::Png => "png",
            ImageFormat::Gif => "gif",
            ImageFormat::Webp => "webp",
            ImageFormat::Avif => "avif",
            ImageFormat::Jxl => "jxl",
            ImageFormat::Heic => "heic",
            ImageFormat::Bmp => "bmp",
            ImageFormat::Tiff => "tiff",
            ImageFormat::Pnm => "pnm",
        }
    }
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_snake())
    }
}

/// Operation a killbit applies to.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Op {
    Decode,
    Encode,
}

/// Named encoders that can be individually allowed/denied via the
/// codec-level killbits grid.
///
/// These are the wire-side names of the specific `NamedEncoders` variants
/// implemented by `imageflow_core`. This type intentionally lists every
/// known encoder — feature gating is handled at the core layer, so the
/// enum stays "purely nominal" on the types side. An encoder being
/// listed here doesn't mean the current build provides it; that question
/// is answered by the runtime net_support grid.
///
/// `#[non_exhaustive]` so new backends can be added without semver breakage.
#[non_exhaustive]
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum NamedEncoderName {
    MozjpegEncoder,
    ZenJpegEncoder,
    MozjpegRsEncoder,
    LibpngEncoder,
    LodepngEncoder,
    PngquantEncoder,
    ZenPngEncoder,
    /// ZenPng configured with the `quantize` (zenquant) backend for palette
    /// reduction. Distinct wire identity from `ZenPngEncoder` so the
    /// substitution priority table can order it explicitly vs the
    /// imagequant-backed sibling and vs pngquant.
    ZenPngZenquantEncoder,
    /// ZenPng configured with the `imagequant` backend for palette
    /// reduction. Shares pngquant's quantization kernel but goes through
    /// the zenpng encoding path.
    ZenPngImagequantEncoder,
    WebpEncoder,
    ZenWebpEncoder,
    GifEncoder,
    ZenGifEncoder,
    ZenAvifEncoder,
    ZenJxlEncoder,
    ZenBmpEncoder,
}

impl NamedEncoderName {
    /// All variants, stable ordering for grid iteration.
    pub const ALL: &'static [NamedEncoderName] = &[
        NamedEncoderName::MozjpegEncoder,
        NamedEncoderName::ZenJpegEncoder,
        NamedEncoderName::MozjpegRsEncoder,
        NamedEncoderName::LibpngEncoder,
        NamedEncoderName::LodepngEncoder,
        NamedEncoderName::PngquantEncoder,
        NamedEncoderName::ZenPngEncoder,
        NamedEncoderName::ZenPngZenquantEncoder,
        NamedEncoderName::ZenPngImagequantEncoder,
        NamedEncoderName::WebpEncoder,
        NamedEncoderName::ZenWebpEncoder,
        NamedEncoderName::GifEncoder,
        NamedEncoderName::ZenGifEncoder,
        NamedEncoderName::ZenAvifEncoder,
        NamedEncoderName::ZenJxlEncoder,
        NamedEncoderName::ZenBmpEncoder,
    ];

    /// snake_case form for wire serialization and error messages.
    pub fn as_snake(self) -> &'static str {
        match self {
            NamedEncoderName::MozjpegEncoder => "mozjpeg_encoder",
            NamedEncoderName::ZenJpegEncoder => "zen_jpeg_encoder",
            NamedEncoderName::MozjpegRsEncoder => "mozjpeg_rs_encoder",
            NamedEncoderName::LibpngEncoder => "libpng_encoder",
            NamedEncoderName::LodepngEncoder => "lodepng_encoder",
            NamedEncoderName::PngquantEncoder => "pngquant_encoder",
            NamedEncoderName::ZenPngEncoder => "zen_png_encoder",
            NamedEncoderName::ZenPngZenquantEncoder => "zen_png_zenquant_encoder",
            NamedEncoderName::ZenPngImagequantEncoder => "zen_png_imagequant_encoder",
            NamedEncoderName::WebpEncoder => "webp_encoder",
            NamedEncoderName::ZenWebpEncoder => "zen_webp_encoder",
            NamedEncoderName::GifEncoder => "gif_encoder",
            NamedEncoderName::ZenGifEncoder => "zen_gif_encoder",
            NamedEncoderName::ZenAvifEncoder => "zen_avif_encoder",
            NamedEncoderName::ZenJxlEncoder => "zen_jxl_encoder",
            NamedEncoderName::ZenBmpEncoder => "zen_bmp_encoder",
        }
    }

    /// The `ImageFormat` this encoder produces. Used by the net_support
    /// grid to derive format availability from codec availability.
    pub fn image_format(self) -> ImageFormat {
        match self {
            NamedEncoderName::MozjpegEncoder
            | NamedEncoderName::ZenJpegEncoder
            | NamedEncoderName::MozjpegRsEncoder => ImageFormat::Jpeg,
            NamedEncoderName::LibpngEncoder
            | NamedEncoderName::LodepngEncoder
            | NamedEncoderName::PngquantEncoder
            | NamedEncoderName::ZenPngEncoder
            | NamedEncoderName::ZenPngZenquantEncoder
            | NamedEncoderName::ZenPngImagequantEncoder => ImageFormat::Png,
            NamedEncoderName::WebpEncoder | NamedEncoderName::ZenWebpEncoder => ImageFormat::Webp,
            NamedEncoderName::GifEncoder | NamedEncoderName::ZenGifEncoder => ImageFormat::Gif,
            NamedEncoderName::ZenAvifEncoder => ImageFormat::Avif,
            NamedEncoderName::ZenJxlEncoder => ImageFormat::Jxl,
            NamedEncoderName::ZenBmpEncoder => ImageFormat::Bmp,
        }
    }
}

impl std::fmt::Display for NamedEncoderName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_snake())
    }
}

/// Named decoders that can be individually allowed/denied via the
/// codec-level killbits grid. See `NamedEncoderName` for the design
/// rationale (nominal enum, feature gating handled at the core layer).
#[non_exhaustive]
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum NamedDecoderName {
    MozjpegRsDecoder,
    ImageRsJpegDecoder,
    ZenJpegDecoder,
    LibpngDecoder,
    ImageRsPngDecoder,
    ZenPngDecoder,
    GifRsDecoder,
    ZenGifDecoder,
    WebpDecoder,
    ZenWebpDecoder,
    ZenAvifDecoder,
    ZenJxlDecoder,
    ZenBmpDecoder,
}

impl NamedDecoderName {
    pub const ALL: &'static [NamedDecoderName] = &[
        NamedDecoderName::MozjpegRsDecoder,
        NamedDecoderName::ImageRsJpegDecoder,
        NamedDecoderName::ZenJpegDecoder,
        NamedDecoderName::LibpngDecoder,
        NamedDecoderName::ImageRsPngDecoder,
        NamedDecoderName::ZenPngDecoder,
        NamedDecoderName::GifRsDecoder,
        NamedDecoderName::ZenGifDecoder,
        NamedDecoderName::WebpDecoder,
        NamedDecoderName::ZenWebpDecoder,
        NamedDecoderName::ZenAvifDecoder,
        NamedDecoderName::ZenJxlDecoder,
        NamedDecoderName::ZenBmpDecoder,
    ];

    pub fn as_snake(self) -> &'static str {
        match self {
            NamedDecoderName::MozjpegRsDecoder => "mozjpeg_rs_decoder",
            NamedDecoderName::ImageRsJpegDecoder => "image_rs_jpeg_decoder",
            NamedDecoderName::ZenJpegDecoder => "zen_jpeg_decoder",
            NamedDecoderName::LibpngDecoder => "libpng_decoder",
            NamedDecoderName::ImageRsPngDecoder => "image_rs_png_decoder",
            NamedDecoderName::ZenPngDecoder => "zen_png_decoder",
            NamedDecoderName::GifRsDecoder => "gif_rs_decoder",
            NamedDecoderName::ZenGifDecoder => "zen_gif_decoder",
            NamedDecoderName::WebpDecoder => "webp_decoder",
            NamedDecoderName::ZenWebpDecoder => "zen_webp_decoder",
            NamedDecoderName::ZenAvifDecoder => "zen_avif_decoder",
            NamedDecoderName::ZenJxlDecoder => "zen_jxl_decoder",
            NamedDecoderName::ZenBmpDecoder => "zen_bmp_decoder",
        }
    }

    pub fn image_format(self) -> ImageFormat {
        match self {
            NamedDecoderName::MozjpegRsDecoder
            | NamedDecoderName::ImageRsJpegDecoder
            | NamedDecoderName::ZenJpegDecoder => ImageFormat::Jpeg,
            NamedDecoderName::LibpngDecoder
            | NamedDecoderName::ImageRsPngDecoder
            | NamedDecoderName::ZenPngDecoder => ImageFormat::Png,
            NamedDecoderName::GifRsDecoder | NamedDecoderName::ZenGifDecoder => ImageFormat::Gif,
            NamedDecoderName::WebpDecoder | NamedDecoderName::ZenWebpDecoder => ImageFormat::Webp,
            NamedDecoderName::ZenAvifDecoder => ImageFormat::Avif,
            NamedDecoderName::ZenJxlDecoder => ImageFormat::Jxl,
            NamedDecoderName::ZenBmpDecoder => ImageFormat::Bmp,
        }
    }
}

impl std::fmt::Display for NamedDecoderName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_snake())
    }
}

/// Reason a specific-codec `EncoderPreset` was served by a different
/// codec than the one the caller named.
///
/// Appears in [`CodecSubstitutionAnnotation::reason`]. The enum is
/// `#[non_exhaustive]` so new reasons can be added without a semver
/// break. Serialized as `snake_case` strings.
#[non_exhaustive]
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum SubstitutionReason {
    /// The requested codec was denied by trusted/job `deny_encoders` /
    /// `deny_decoders`.
    CodecKillbitsDenyEncoders,
    /// The requested codec wasn't in the trusted/job `allow_encoders` /
    /// `allow_decoders` list.
    CodecKillbitsAllowEncodersExcludes,
    /// The build didn't compile in the requested codec (feature gate).
    CompileFeatureMissing,
    /// The build-time `COMPILE_DENY_*` list denies the format family.
    CompileCodecConstDenied,
    /// The codec isn't in the runtime `enabled_codecs` registry.
    NotRegistered,
}

impl SubstitutionReason {
    /// snake_case form for wire serialization, error messages, and the
    /// `codec_substitution.reason` field on responses.
    pub fn as_snake(self) -> &'static str {
        match self {
            SubstitutionReason::CodecKillbitsDenyEncoders => "codec_killbits.deny_encoders",
            SubstitutionReason::CodecKillbitsAllowEncodersExcludes => {
                "codec_killbits.allow_encoders_excludes"
            }
            SubstitutionReason::CompileFeatureMissing => "compile.feature_missing",
            SubstitutionReason::CompileCodecConstDenied => "compile.codec_const_denied",
            SubstitutionReason::NotRegistered => "not_registered",
        }
    }
}

impl std::fmt::Display for SubstitutionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_snake())
    }
}

/// Annotation attached to a per-encode-step response when the dispatcher
/// served a specific-codec `EncoderPreset` by routing to a different
/// codec for the same wire format.
///
/// Surfaces on [`crate::EncodeResult::annotations`] — one per encoded
/// image — and never represents an error. The output bytes are valid for
/// the advertised format; this annotation only tells the caller *which*
/// backend produced them and why the requested one was skipped.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct CodecSubstitutionAnnotation {
    /// Wire name of the codec the `EncoderPreset` named
    /// (e.g. `mozjpeg_encoder`).
    pub requested: NamedEncoderName,
    /// Wire name of the codec that actually produced output
    /// (e.g. `zen_jpeg_encoder`).
    pub actual: NamedEncoderName,
    /// Why the dispatcher skipped the requested codec.
    pub reason: SubstitutionReason,
    /// Build-time codec-priority flavor that selected the substitute
    /// order. `"v3_zen_first"` on upstream / V3 forks,
    /// `"v2_classic_first"` on V2 forks. The field is informational —
    /// callers can log it to distinguish an unexpected pick driven by
    /// priority from one driven by killbits.
    #[serde(default = "default_codec_priority_wire")]
    pub codec_priority: String,
    /// Human-readable translation notes, one per field the preset carried
    /// that was remapped onto the substitute codec's configuration
    /// (e.g. `"preset.quality → zen.quality"`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_translations: Vec<String>,
    /// Field values from the request that were dropped because the
    /// substitute codec doesn't support them
    /// (e.g. `"preset.zlib_compression"` on the lodepng fallback path).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dropped_fields: Vec<String>,
}

/// Serde default for [`CodecSubstitutionAnnotation::codec_priority`]
/// so older JSON payloads lacking the field deserialize as the V3
/// upstream default. Not called in normal construction paths — the
/// dispatcher always fills the field from
/// `build_killbits::codec_priority()`.
fn default_codec_priority_wire() -> String {
    crate::build_killbits::CODEC_PRIORITY_DEFAULT.as_snake().to_string()
}

/// Mirror of [`CodecSubstitutionAnnotation`] for the decode side.
///
/// Surfaces on [`crate::DecodeResult::annotations`] when a decoder
/// substitution happened (e.g. `zen_jpeg_decoder` was denied, so the
/// dispatcher used `mozjpeg_decoder` instead). Shape mirrors the encoder
/// annotation for UI consistency.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct DecoderSubstitutionAnnotation {
    pub requested: NamedDecoderName,
    pub actual: NamedDecoderName,
    pub reason: SubstitutionReason,
}

/// Bag of forward-extensible annotations attached to a single encoded
/// image in [`crate::EncodeResult`].
///
/// Each field is optional; new annotation kinds can be added without
/// breaking callers that deserialize older messages. Callers should
/// treat unknown fields as ignorable.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct EncodeAnnotations {
    /// Set iff the dispatcher substituted the requested codec with a
    /// different one that produces the same wire format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec_substitution: Option<CodecSubstitutionAnnotation>,
}

impl EncodeAnnotations {
    /// Convenience: `true` if at least one annotation field is populated.
    pub fn is_empty(&self) -> bool {
        self.codec_substitution.is_none()
    }
}

/// Decode-side companion to [`EncodeAnnotations`].
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct DecodeAnnotations {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec_substitution: Option<DecoderSubstitutionAnnotation>,
}

impl DecodeAnnotations {
    pub fn is_empty(&self) -> bool {
        self.codec_substitution.is_none()
    }
}

/// Codec-level killbits: allow/deny individual named encoders and
/// decoders.
///
/// Sits alongside `FormatKillbits` on `ExecutionSecurity`. Resolution
/// order:
///
/// 1. Build-time ceiling (compile gates) picks which named codecs exist.
/// 2. Trusted policy may `allow_*` (positive list — this is the only way
///    to *allow* a compiled codec that was absent from the ambient grid,
///    though today any compiled-in codec is available by default) or
///    `deny_*`.
/// 3. Job-level requests may only `deny_*` — no `allow_*`, same rule as
///    `FormatKillbits`.
///
/// When the grid denies every encoder for a format, the format itself
/// flips to "encode: false" in `v1/context/get_net_support`, with
/// `reasons: ["no_available_encoder"]` on the format entry.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct CodecKillbits {
    /// Allow only these encoders; all other encoders are denied.
    /// Mutually exclusive with `deny_encoders`. Trusted-policy-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_encoders: Option<Vec<NamedEncoderName>>,
    /// Deny these encoders. Mutually exclusive with `allow_encoders`.
    /// Allowed at any layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_encoders: Option<Vec<NamedEncoderName>>,
    /// Allow only these decoders; all other decoders are denied.
    /// Mutually exclusive with `deny_decoders`. Trusted-policy-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_decoders: Option<Vec<NamedDecoderName>>,
    /// Deny these decoders. Mutually exclusive with `allow_decoders`.
    /// Allowed at any layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_decoders: Option<Vec<NamedDecoderName>>,
}

/// Errors produced by `CodecKillbits::validate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecKillbitsValidationError {
    /// `allow_encoders` and `deny_encoders` set simultaneously.
    AllowAndDenyEncoders,
    /// `allow_decoders` and `deny_decoders` set simultaneously.
    AllowAndDenyDecoders,
}

impl std::fmt::Display for CodecKillbitsValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecKillbitsValidationError::AllowAndDenyEncoders => {
                f.write_str("pick allow or deny for encoders, not both")
            }
            CodecKillbitsValidationError::AllowAndDenyDecoders => {
                f.write_str("pick allow or deny for decoders, not both")
            }
        }
    }
}

impl std::error::Error for CodecKillbitsValidationError {}

/// Errors produced by `CodecKillbits::validate_job_level`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecKillbitsJobLevelError {
    /// Job-level request tried to use `allow_encoders` / `allow_decoders`.
    /// Layer 3 may only *narrow*.
    JobLevelMayOnlyDeny,
}

impl std::fmt::Display for CodecKillbitsJobLevelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecKillbitsJobLevelError::JobLevelMayOnlyDeny => f.write_str(
                "job-level security may only deny, never allow (layer 3 narrows only)",
            ),
        }
    }
}

impl std::error::Error for CodecKillbitsJobLevelError {}

impl CodecKillbits {
    /// Check mutual-exclusion invariants. Does not check feature
    /// availability — that happens at trusted-policy set time.
    pub fn validate(&self) -> Result<(), CodecKillbitsValidationError> {
        if self.allow_encoders.is_some() && self.deny_encoders.is_some() {
            return Err(CodecKillbitsValidationError::AllowAndDenyEncoders);
        }
        if self.allow_decoders.is_some() && self.deny_decoders.is_some() {
            return Err(CodecKillbitsValidationError::AllowAndDenyDecoders);
        }
        Ok(())
    }

    /// Validate layer-3 constraint: no allow-lists at the job level.
    pub fn validate_job_level(&self) -> Result<(), CodecKillbitsJobLevelError> {
        if self.allow_encoders.is_some() || self.allow_decoders.is_some() {
            return Err(CodecKillbitsJobLevelError::JobLevelMayOnlyDeny);
        }
        Ok(())
    }

    /// Decide whether `codec` is permitted for encoding given a
    /// baseline (the set of compiled-in encoders provides the ambient
    /// grid; callers pass `base_allowed = true` when that's the case).
    pub fn encoder_allowed(&self, codec: NamedEncoderName, base_allowed: bool) -> bool {
        if !base_allowed {
            return false;
        }
        if let Some(allow) = &self.allow_encoders {
            return allow.contains(&codec);
        }
        if let Some(deny) = &self.deny_encoders {
            return !deny.contains(&codec);
        }
        true
    }

    /// Mirror of `encoder_allowed` for decoders.
    pub fn decoder_allowed(&self, codec: NamedDecoderName, base_allowed: bool) -> bool {
        if !base_allowed {
            return false;
        }
        if let Some(allow) = &self.allow_decoders {
            return allow.contains(&codec);
        }
        if let Some(deny) = &self.deny_decoders {
            return !deny.contains(&codec);
        }
        true
    }

    /// Intersect two codec killbits blocks. Output is a deny-only form so
    /// it's safe to apply as a job-level narrowing.
    pub fn intersect(trusted: &CodecKillbits, job: &CodecKillbits) -> CodecKillbits {
        // Start from "all allowed" and apply trusted, then job. Any codec
        // denied by either layer stays denied.
        let encoders_denied: Vec<NamedEncoderName> = NamedEncoderName::ALL
            .iter()
            .copied()
            .filter(|&c| !trusted.encoder_allowed(c, true) || !job.encoder_allowed(c, true))
            .collect();
        let decoders_denied: Vec<NamedDecoderName> = NamedDecoderName::ALL
            .iter()
            .copied()
            .filter(|&c| !trusted.decoder_allowed(c, true) || !job.decoder_allowed(c, true))
            .collect();
        CodecKillbits {
            allow_encoders: None,
            deny_encoders: if encoders_denied.is_empty() { None } else { Some(encoders_denied) },
            allow_decoders: None,
            deny_decoders: if decoders_denied.is_empty() { None } else { Some(decoders_denied) },
        }
    }
}

/// Per-format decode/encode permissions.
///
/// Used inside `FormatKillbits::formats` to express a full allow/deny grid in
/// one request body.
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct FormatPermissions {
    #[serde(default)]
    pub decode: bool,
    #[serde(default)]
    pub encode: bool,
}

/// Killbits for decode/encode on a per-format basis.
///
/// Three mutually exclusive request shapes:
///
/// - `allow_decode` / `allow_encode`: the listed formats are the *only* ones
///   permitted. Anything else is denied. (Trusted-policy layer only — rejected
///   at job level.)
/// - `deny_decode` / `deny_encode`: the listed formats are denied. Everything
///   else carries over from the layer above.
/// - `formats`: an explicit table of `{format: {decode, encode}}` pairs.
///   (Trusted-policy layer only — job level may set fields to `false` only.)
///
/// Validation at deserialize time rejects mixing forms (e.g., `allow_decode`
/// together with `deny_decode`, or a list form together with `formats`).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct FormatKillbits {
    /// Allow only these for decode (rejects any format not listed).
    /// Mutually exclusive with `deny_decode` and with `formats`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_decode: Option<Vec<ImageFormat>>,
    /// Deny these for decode. Mutually exclusive with `allow_decode`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_decode: Option<Vec<ImageFormat>>,
    /// Allow only these for encode. Mutually exclusive with `deny_encode` and
    /// with `formats`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_encode: Option<Vec<ImageFormat>>,
    /// Deny these for encode. Mutually exclusive with `allow_encode`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_encode: Option<Vec<ImageFormat>>,
    /// Per-format table form. Mutually exclusive with the list forms above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formats: Option<BTreeMap<ImageFormat, FormatPermissions>>,
}

/// Errors produced by `FormatKillbits::validate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KillbitsValidationError {
    /// `allow_decode` and `deny_decode` set simultaneously.
    AllowAndDenyDecode,
    /// `allow_encode` and `deny_encode` set simultaneously.
    AllowAndDenyEncode,
    /// `allow_*` / `deny_*` list forms mixed with `formats` table form.
    ListAndTableMixed,
}

impl std::fmt::Display for KillbitsValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KillbitsValidationError::AllowAndDenyDecode => {
                f.write_str("pick allow or deny for decode, not both")
            }
            KillbitsValidationError::AllowAndDenyEncode => {
                f.write_str("pick allow or deny for encode, not both")
            }
            KillbitsValidationError::ListAndTableMixed => {
                f.write_str("pick a single form (allow/deny lists OR formats table)")
            }
        }
    }
}

impl std::error::Error for KillbitsValidationError {}

/// Errors produced by `FormatKillbits::validate_job_level`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobLevelKillbitsError {
    /// Job-level request tried to use `allow_decode` / `allow_encode` or set a
    /// table entry's `decode`/`encode` to `true`. Layer 3 may only *narrow*.
    JobLevelMayOnlyDeny,
}

impl std::fmt::Display for JobLevelKillbitsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobLevelKillbitsError::JobLevelMayOnlyDeny => f.write_str(
                "job-level security may only deny, never allow (layer 3 narrows only)",
            ),
        }
    }
}

impl std::error::Error for JobLevelKillbitsError {}

impl FormatKillbits {
    /// Check mutual-exclusion invariants. Does not check compile-ceiling or
    /// feature availability — that happens at trusted-policy set time.
    pub fn validate(&self) -> Result<(), KillbitsValidationError> {
        if self.allow_decode.is_some() && self.deny_decode.is_some() {
            return Err(KillbitsValidationError::AllowAndDenyDecode);
        }
        if self.allow_encode.is_some() && self.deny_encode.is_some() {
            return Err(KillbitsValidationError::AllowAndDenyEncode);
        }
        let has_list = self.allow_decode.is_some()
            || self.deny_decode.is_some()
            || self.allow_encode.is_some()
            || self.deny_encode.is_some();
        if has_list && self.formats.is_some() {
            return Err(KillbitsValidationError::ListAndTableMixed);
        }
        Ok(())
    }

    /// Validate additional job-level constraints (layer 3): no allow-lists and
    /// no table entries that set `decode: true` / `encode: true`.
    pub fn validate_job_level(&self) -> Result<(), JobLevelKillbitsError> {
        if self.allow_decode.is_some() || self.allow_encode.is_some() {
            return Err(JobLevelKillbitsError::JobLevelMayOnlyDeny);
        }
        if let Some(table) = &self.formats {
            for perms in table.values() {
                if perms.decode || perms.encode {
                    return Err(JobLevelKillbitsError::JobLevelMayOnlyDeny);
                }
            }
        }
        Ok(())
    }

    /// Produce the normalized grid this killbits block permits, *given* a
    /// starting grid of available formats. `base_allowed` is the set of
    /// formats the layer above has already permitted.
    ///
    /// Returns a grid of `(format, op) → bool` for every format in
    /// `ImageFormat::ALL`. The result is always a subset of `base_allowed`.
    pub fn apply_to(&self, base_allowed: &FormatGrid) -> FormatGrid {
        let mut out = FormatGrid::none();
        for &f in ImageFormat::ALL {
            // Start from base.
            let base_decode = base_allowed.decode(f);
            let base_encode = base_allowed.encode(f);
            let (d, e) = match (&self.allow_decode, &self.deny_decode,
                                &self.allow_encode, &self.deny_encode,
                                &self.formats) {
                (_, _, _, _, Some(table)) => {
                    // Table form: a format not in the table stays at base.
                    // Entry in table: decode/encode = entry value AND base.
                    let entry = table.get(&f).copied().unwrap_or(FormatPermissions {
                        decode: base_decode,
                        encode: base_encode,
                    });
                    (entry.decode && base_decode, entry.encode && base_encode)
                }
                (allow_d, deny_d, allow_e, deny_e, None) => {
                    let d = match (allow_d, deny_d) {
                        (Some(list), _) => base_decode && list.contains(&f),
                        (_, Some(list)) => base_decode && !list.contains(&f),
                        (None, None) => base_decode,
                    };
                    let e = match (allow_e, deny_e) {
                        (Some(list), _) => base_encode && list.contains(&f),
                        (_, Some(list)) => base_encode && !list.contains(&f),
                        (None, None) => base_encode,
                    };
                    (d, e)
                }
            };
            out.set(f, Op::Decode, d);
            out.set(f, Op::Encode, e);
        }
        out
    }

    /// Intersect two killbit blocks. Used to combine trusted policy with
    /// job-level requests: `effective = trusted.intersect(&job)`.
    ///
    /// Only produces `deny_*` forms (never `allow_*`), so the result is safe
    /// to apply as a job-level narrowing of whatever was available.
    pub fn intersect(trusted: &FormatKillbits, job: &FormatKillbits) -> FormatKillbits {
        // Compute a grid where everything starts as allowed, then apply
        // trusted, then job. The output grid is turned into a table form so
        // nothing is lost.
        let base = FormatGrid::all_allowed();
        let after_trusted = trusted.apply_to(&base);
        let after_job = job.apply_to(&after_trusted);
        after_job.to_table_killbits()
    }
}

/// A per-format decode/encode truth table over every known format.
///
/// Used internally to compute effective killbits grids and to produce the
/// `net_support` view exposed by `v1/context/get_net_support`.
#[derive(Clone, PartialEq, Debug)]
pub struct FormatGrid {
    entries: BTreeMap<ImageFormat, FormatPermissions>,
}

impl Default for FormatGrid {
    fn default() -> Self {
        Self::none()
    }
}

impl FormatGrid {
    /// A grid with every format denied for both decode and encode.
    pub fn none() -> Self {
        let mut entries = BTreeMap::new();
        for &f in ImageFormat::ALL {
            entries.insert(f, FormatPermissions { decode: false, encode: false });
        }
        FormatGrid { entries }
    }

    /// A grid with every format allowed for both decode and encode. Useful as
    /// the starting point for intersection before filtering through layers.
    pub fn all_allowed() -> Self {
        let mut entries = BTreeMap::new();
        for &f in ImageFormat::ALL {
            entries.insert(f, FormatPermissions { decode: true, encode: true });
        }
        FormatGrid { entries }
    }

    pub fn decode(&self, f: ImageFormat) -> bool {
        self.entries.get(&f).map(|p| p.decode).unwrap_or(false)
    }
    pub fn encode(&self, f: ImageFormat) -> bool {
        self.entries.get(&f).map(|p| p.encode).unwrap_or(false)
    }
    pub fn get(&self, f: ImageFormat, op: Op) -> bool {
        match op {
            Op::Decode => self.decode(f),
            Op::Encode => self.encode(f),
        }
    }
    pub fn set(&mut self, f: ImageFormat, op: Op, value: bool) {
        let entry = self.entries.entry(f).or_default();
        match op {
            Op::Decode => entry.decode = value,
            Op::Encode => entry.encode = value,
        }
    }

    /// Borrow the underlying entries map.
    pub fn entries(&self) -> &BTreeMap<ImageFormat, FormatPermissions> {
        &self.entries
    }

    /// Produce a `FormatKillbits` in table form that represents this grid.
    pub fn to_table_killbits(&self) -> FormatKillbits {
        FormatKillbits {
            allow_decode: None,
            deny_decode: None,
            allow_encode: None,
            deny_encode: None,
            formats: Some(self.entries.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_base() -> FormatGrid {
        FormatGrid::all_allowed()
    }

    #[test]
    fn validate_accepts_empty() {
        assert!(FormatKillbits::default().validate().is_ok());
    }

    #[test]
    fn validate_rejects_allow_and_deny_decode() {
        let kb = FormatKillbits {
            allow_decode: Some(vec![ImageFormat::Jpeg]),
            deny_decode: Some(vec![ImageFormat::Png]),
            ..Default::default()
        };
        assert_eq!(kb.validate(), Err(KillbitsValidationError::AllowAndDenyDecode));
    }

    #[test]
    fn validate_rejects_allow_and_deny_encode() {
        let kb = FormatKillbits {
            allow_encode: Some(vec![ImageFormat::Jpeg]),
            deny_encode: Some(vec![ImageFormat::Png]),
            ..Default::default()
        };
        assert_eq!(kb.validate(), Err(KillbitsValidationError::AllowAndDenyEncode));
    }

    #[test]
    fn validate_rejects_list_and_table_mixed() {
        let mut table = BTreeMap::new();
        table.insert(ImageFormat::Jpeg, FormatPermissions { decode: true, encode: false });
        let kb = FormatKillbits {
            deny_decode: Some(vec![ImageFormat::Png]),
            formats: Some(table),
            ..Default::default()
        };
        assert_eq!(kb.validate(), Err(KillbitsValidationError::ListAndTableMixed));
    }

    #[test]
    fn validate_job_level_rejects_allow_decode() {
        let kb = FormatKillbits {
            allow_decode: Some(vec![ImageFormat::Jpeg]),
            ..Default::default()
        };
        assert_eq!(kb.validate_job_level(), Err(JobLevelKillbitsError::JobLevelMayOnlyDeny));
    }

    #[test]
    fn validate_job_level_rejects_allow_encode() {
        let kb = FormatKillbits {
            allow_encode: Some(vec![ImageFormat::Jpeg]),
            ..Default::default()
        };
        assert_eq!(kb.validate_job_level(), Err(JobLevelKillbitsError::JobLevelMayOnlyDeny));
    }

    #[test]
    fn validate_job_level_rejects_table_true() {
        let mut table = BTreeMap::new();
        table.insert(ImageFormat::Jpeg, FormatPermissions { decode: true, encode: false });
        let kb = FormatKillbits { formats: Some(table), ..Default::default() };
        assert_eq!(kb.validate_job_level(), Err(JobLevelKillbitsError::JobLevelMayOnlyDeny));
    }

    #[test]
    fn validate_job_level_accepts_table_all_false() {
        let mut table = BTreeMap::new();
        table.insert(ImageFormat::Jpeg, FormatPermissions { decode: false, encode: false });
        let kb = FormatKillbits { formats: Some(table), ..Default::default() };
        assert!(kb.validate_job_level().is_ok());
    }

    #[test]
    fn validate_job_level_accepts_deny_lists() {
        let kb = FormatKillbits {
            deny_decode: Some(vec![ImageFormat::Jpeg]),
            deny_encode: Some(vec![ImageFormat::Png]),
            ..Default::default()
        };
        assert!(kb.validate_job_level().is_ok());
    }

    #[test]
    fn apply_allowlist_decode() {
        let kb = FormatKillbits {
            allow_decode: Some(vec![ImageFormat::Jpeg, ImageFormat::Png]),
            ..Default::default()
        };
        let grid = kb.apply_to(&all_base());
        assert!(grid.decode(ImageFormat::Jpeg));
        assert!(grid.decode(ImageFormat::Png));
        assert!(!grid.decode(ImageFormat::Avif));
        // Encode untouched.
        assert!(grid.encode(ImageFormat::Jpeg));
        assert!(grid.encode(ImageFormat::Avif));
    }

    #[test]
    fn apply_denylist_encode() {
        let kb = FormatKillbits {
            deny_encode: Some(vec![ImageFormat::Avif, ImageFormat::Jxl]),
            ..Default::default()
        };
        let grid = kb.apply_to(&all_base());
        assert!(grid.encode(ImageFormat::Jpeg));
        assert!(!grid.encode(ImageFormat::Avif));
        assert!(!grid.encode(ImageFormat::Jxl));
        // Decode untouched.
        assert!(grid.decode(ImageFormat::Avif));
    }

    #[test]
    fn apply_table() {
        let mut table = BTreeMap::new();
        table.insert(ImageFormat::Jpeg, FormatPermissions { decode: true, encode: false });
        table.insert(ImageFormat::Avif, FormatPermissions { decode: false, encode: false });
        let kb = FormatKillbits { formats: Some(table), ..Default::default() };
        let grid = kb.apply_to(&all_base());
        assert!(grid.decode(ImageFormat::Jpeg));
        assert!(!grid.encode(ImageFormat::Jpeg));
        assert!(!grid.decode(ImageFormat::Avif));
        assert!(!grid.encode(ImageFormat::Avif));
        // Unlisted formats fall through to base.
        assert!(grid.decode(ImageFormat::Png));
        assert!(grid.encode(ImageFormat::Png));
    }

    #[test]
    fn codec_killbits_validate_accepts_empty() {
        assert!(CodecKillbits::default().validate().is_ok());
    }

    #[test]
    fn codec_killbits_validate_rejects_allow_and_deny_encoders() {
        let kb = CodecKillbits {
            allow_encoders: Some(vec![NamedEncoderName::MozjpegEncoder]),
            deny_encoders: Some(vec![NamedEncoderName::ZenJpegEncoder]),
            ..Default::default()
        };
        assert_eq!(kb.validate(), Err(CodecKillbitsValidationError::AllowAndDenyEncoders));
    }

    #[test]
    fn codec_killbits_validate_rejects_allow_and_deny_decoders() {
        let kb = CodecKillbits {
            allow_decoders: Some(vec![NamedDecoderName::MozjpegRsDecoder]),
            deny_decoders: Some(vec![NamedDecoderName::ZenJpegDecoder]),
            ..Default::default()
        };
        assert_eq!(kb.validate(), Err(CodecKillbitsValidationError::AllowAndDenyDecoders));
    }

    #[test]
    fn codec_killbits_validate_job_level_rejects_allow_encoders() {
        let kb = CodecKillbits {
            allow_encoders: Some(vec![NamedEncoderName::MozjpegEncoder]),
            ..Default::default()
        };
        assert_eq!(
            kb.validate_job_level(),
            Err(CodecKillbitsJobLevelError::JobLevelMayOnlyDeny)
        );
    }

    #[test]
    fn codec_killbits_validate_job_level_rejects_allow_decoders() {
        let kb = CodecKillbits {
            allow_decoders: Some(vec![NamedDecoderName::MozjpegRsDecoder]),
            ..Default::default()
        };
        assert_eq!(
            kb.validate_job_level(),
            Err(CodecKillbitsJobLevelError::JobLevelMayOnlyDeny)
        );
    }

    #[test]
    fn codec_killbits_validate_job_level_accepts_deny_lists() {
        let kb = CodecKillbits {
            deny_encoders: Some(vec![NamedEncoderName::MozjpegEncoder]),
            deny_decoders: Some(vec![NamedDecoderName::MozjpegRsDecoder]),
            ..Default::default()
        };
        assert!(kb.validate_job_level().is_ok());
    }

    #[test]
    fn codec_killbits_deny_list_blocks_named() {
        let kb = CodecKillbits {
            deny_encoders: Some(vec![NamedEncoderName::MozjpegEncoder]),
            ..Default::default()
        };
        assert!(!kb.encoder_allowed(NamedEncoderName::MozjpegEncoder, true));
        assert!(kb.encoder_allowed(NamedEncoderName::ZenJpegEncoder, true));
    }

    #[test]
    fn codec_killbits_allow_list_is_exclusive() {
        let kb = CodecKillbits {
            allow_encoders: Some(vec![NamedEncoderName::MozjpegEncoder]),
            ..Default::default()
        };
        assert!(kb.encoder_allowed(NamedEncoderName::MozjpegEncoder, true));
        assert!(!kb.encoder_allowed(NamedEncoderName::ZenJpegEncoder, true));
    }

    #[test]
    fn codec_killbits_respects_base_allowed() {
        let kb = CodecKillbits::default();
        assert!(kb.encoder_allowed(NamedEncoderName::MozjpegEncoder, true));
        assert!(!kb.encoder_allowed(NamedEncoderName::MozjpegEncoder, false));
    }

    #[test]
    fn codec_killbits_intersect_unions_denies() {
        let trusted = CodecKillbits {
            deny_encoders: Some(vec![NamedEncoderName::MozjpegEncoder]),
            ..Default::default()
        };
        let job = CodecKillbits {
            deny_encoders: Some(vec![NamedEncoderName::ZenJpegEncoder]),
            ..Default::default()
        };
        let merged = CodecKillbits::intersect(&trusted, &job);
        assert!(!merged.encoder_allowed(NamedEncoderName::MozjpegEncoder, true));
        assert!(!merged.encoder_allowed(NamedEncoderName::ZenJpegEncoder, true));
        assert!(merged.encoder_allowed(NamedEncoderName::MozjpegRsEncoder, true));
    }

    #[test]
    fn codec_killbits_intersect_allow_list_narrows_to_intersection() {
        // Trusted allow-list: [moz, zen_jpeg]. Job deny: zen_jpeg.
        // Result should still only allow moz (intersection).
        let trusted = CodecKillbits {
            allow_encoders: Some(vec![
                NamedEncoderName::MozjpegEncoder,
                NamedEncoderName::ZenJpegEncoder,
            ]),
            ..Default::default()
        };
        let job = CodecKillbits {
            deny_encoders: Some(vec![NamedEncoderName::ZenJpegEncoder]),
            ..Default::default()
        };
        let merged = CodecKillbits::intersect(&trusted, &job);
        assert!(merged.encoder_allowed(NamedEncoderName::MozjpegEncoder, true));
        assert!(!merged.encoder_allowed(NamedEncoderName::ZenJpegEncoder, true));
        // Encoder outside trusted's allow-list stays denied.
        assert!(!merged.encoder_allowed(NamedEncoderName::MozjpegRsEncoder, true));
    }

    #[test]
    fn substitution_reason_serializes_snake_case() {
        // serde's rename_all=snake_case stringifies the variant name in
        // lowercase underscore form. `CodecKillbitsDenyEncoders` →
        // `codec_killbits_deny_encoders` (no dot). The `as_snake()`
        // helper produces the dotted reason-string used in error bodies.
        assert_eq!(
            serde_json::to_string(&SubstitutionReason::CodecKillbitsDenyEncoders).unwrap(),
            "\"codec_killbits_deny_encoders\""
        );
        assert_eq!(
            serde_json::to_string(&SubstitutionReason::CompileFeatureMissing).unwrap(),
            "\"compile_feature_missing\""
        );
        // Round-trip.
        let parsed: SubstitutionReason =
            serde_json::from_str("\"codec_killbits_allow_encoders_excludes\"").unwrap();
        assert_eq!(parsed, SubstitutionReason::CodecKillbitsAllowEncodersExcludes);
    }

    #[test]
    fn substitution_reason_as_snake_uses_dot_form() {
        assert_eq!(
            SubstitutionReason::CodecKillbitsDenyEncoders.as_snake(),
            "codec_killbits.deny_encoders"
        );
        assert_eq!(
            SubstitutionReason::CompileFeatureMissing.as_snake(),
            "compile.feature_missing"
        );
        assert_eq!(SubstitutionReason::NotRegistered.as_snake(), "not_registered");
    }

    #[test]
    fn codec_substitution_annotation_round_trips_through_serde() {
        let ann = CodecSubstitutionAnnotation {
            requested: NamedEncoderName::MozjpegEncoder,
            actual: NamedEncoderName::ZenJpegEncoder,
            reason: SubstitutionReason::CodecKillbitsDenyEncoders,
            codec_priority: "v3_zen_first".to_string(),
            field_translations: vec![
                "preset.quality → zen.quality".to_string(),
                "preset.progressive → zen.progressive".to_string(),
            ],
            dropped_fields: vec![],
        };
        let json = serde_json::to_string(&ann).unwrap();
        let back: CodecSubstitutionAnnotation = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ann);
        // Shape check: wire names are snake_case strings.
        assert!(json.contains("\"requested\":\"mozjpeg_encoder\""), "json: {}", json);
        assert!(json.contains("\"actual\":\"zen_jpeg_encoder\""), "json: {}", json);
        assert!(json.contains("\"reason\":\"codec_killbits_deny_encoders\""), "json: {}", json);
        assert!(json.contains("\"codec_priority\":\"v3_zen_first\""), "json: {}", json);
    }

    #[test]
    fn codec_substitution_annotation_older_payload_defaults_priority() {
        // A JSON payload generated before the `codec_priority` field
        // existed must still deserialize — serde falls back to the
        // V3 default wire form.
        let older_json = r#"{
            "requested": "mozjpeg_encoder",
            "actual": "zen_jpeg_encoder",
            "reason": "codec_killbits_deny_encoders"
        }"#;
        let parsed: CodecSubstitutionAnnotation = serde_json::from_str(older_json).unwrap();
        assert_eq!(parsed.codec_priority, "v3_zen_first");
    }

    #[test]
    fn codec_substitution_annotation_accepts_v2_priority_wire() {
        let json = r#"{
            "requested": "zen_jpeg_encoder",
            "actual": "mozjpeg_encoder",
            "reason": "not_registered",
            "codec_priority": "v2_classic_first"
        }"#;
        let parsed: CodecSubstitutionAnnotation = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.codec_priority, "v2_classic_first");
    }

    #[test]
    fn encode_annotations_omits_empty_fields() {
        let empty = EncodeAnnotations::default();
        assert!(empty.is_empty());
        let json = serde_json::to_string(&empty).unwrap();
        // `codec_substitution` is `None` and uses `skip_serializing_if`.
        assert_eq!(json, "{}");
    }

    #[test]
    fn encode_annotations_with_substitution_round_trips() {
        let ann = EncodeAnnotations {
            codec_substitution: Some(CodecSubstitutionAnnotation {
                requested: NamedEncoderName::PngquantEncoder,
                actual: NamedEncoderName::LodepngEncoder,
                reason: SubstitutionReason::CodecKillbitsDenyEncoders,
                codec_priority: "v3_zen_first".to_string(),
                field_translations: vec!["preset.quality → (dropped)".to_string()],
                dropped_fields: vec!["preset.quality".to_string()],
            }),
        };
        let json = serde_json::to_string(&ann).unwrap();
        let back: EncodeAnnotations = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ann);
        assert!(!ann.is_empty());
    }

    #[test]
    fn intersect_combines_deny_lists() {
        // trusted: deny avif encode
        let trusted = FormatKillbits {
            deny_encode: Some(vec![ImageFormat::Avif]),
            ..Default::default()
        };
        // job: deny webp encode
        let job = FormatKillbits {
            deny_encode: Some(vec![ImageFormat::Webp]),
            ..Default::default()
        };
        let combined = FormatKillbits::intersect(&trusted, &job);
        let grid = combined.apply_to(&all_base());
        // Both avif and webp encode must be denied.
        assert!(!grid.encode(ImageFormat::Avif));
        assert!(!grid.encode(ImageFormat::Webp));
        // Jpeg encode is still allowed.
        assert!(grid.encode(ImageFormat::Jpeg));
        // Decode for all remains allowed.
        assert!(grid.decode(ImageFormat::Avif));
        assert!(grid.decode(ImageFormat::Webp));
        assert!(grid.decode(ImageFormat::Jpeg));
    }
}
