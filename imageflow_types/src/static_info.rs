//! Types for the `v1/static/info` endpoint.
//!
//! Process-wide, compile-time-static introspection of this build's
//! format/codec availability, capability surface, and RIAPI schema. The
//! response changes only when the binary changes, so the endpoint is
//! safe to cache forever by clients.
//!
//! Distinct from `v1/context/get_net_support`, which is `Context`-scoped
//! and depends on the trusted policy + per-job narrowing.

use crate::ImageFormat;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
#[cfg(feature = "schema-export")]
use utoipa::ToSchema;

/// Role a codec plays. Mirrors `NamedEncoderName` / `NamedDecoderName`
/// as a single axis.
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Debug, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum CodecRole {
    Encode,
    Decode,
}

/// Kind of value a RIAPI key accepts. Present to help clients offer
/// inline validation.
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum RiapiValueKind {
    Integer,
    Float,
    Boolean,
    Enum,
    String,
}

/// Coarse grouping used to drive the server-side cache-key inclusion
/// rules (see [`ServerRecommendations`]).
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum RiapiCategory {
    /// Dimensions, crop, scale.
    Resize,
    /// Cropping-specific keys (c, c.gravity).
    Crop,
    /// Output format + encoder knobs.
    Output,
    /// Client-capability negotiation (`accept.*`).
    Negotiation,
    /// Color/tone/filter adjustments on the decoded pixels.
    Filter,
    /// Source-side handling (decoder hints, metadata).
    Source,
    /// Watermark / border / padding composition.
    Composition,
    /// Debug/tracing/dev knobs — not cache-relevant.
    Debug,
    /// Anything else not yet categorized.
    Other,
}

/// Build-time facts about this binary.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct BuildInfo {
    /// Feature flags active in this build (e.g. `"c-codecs"`, `"zen-codecs"`).
    pub features: Vec<String>,
    /// Formats the build-ceiling permanently denies for decode
    /// (`COMPILE_DENY_DECODE`). Snake-case format names.
    pub compile_deny_decode: Vec<String>,
    /// Formats the build-ceiling permanently denies for encode
    /// (`COMPILE_DENY_ENCODE`). Snake-case format names.
    pub compile_deny_encode: Vec<String>,
    /// Snake-case codec-priority preset this build defaults to
    /// (`CODEC_PRIORITY_DEFAULT`).
    pub codec_priority_default: String,
}

/// Union of the capability flags / ranges across every codec (for a
/// given role) that backs a format in the current build. Booleans are
/// OR-merged; ranges take the widest span.
///
/// Fields match the zencodec capability surface 1:1 where sensible, so
/// clients can reason about codec behaviour without calling into
/// codec-specific types.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct CapsSummary {
    pub icc: bool,
    pub exif: bool,
    pub xmp: bool,
    pub cicp: bool,
    pub lossy: bool,
    pub lossless: bool,
    pub hdr: bool,
    pub gain_map: bool,
    pub native_alpha: bool,
    pub native_gray: bool,
    pub native_16bit: bool,
    pub native_f32: bool,
    /// Encode-only: whether the codec supports animation emission.
    /// Decode-only: whether the codec supports animation parsing.
    pub animation: bool,
    /// Encode-only: whether the codec supports row-level push encoding.
    /// Decode-only: whether the codec supports row-level streaming decode.
    pub push_rows: bool,
    /// `None` if the codec has no effort knob.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub effort_range: Option<[i32; 2]>,
    /// `None` if the codec is lossless-only / has no quality knob.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub quality_range: Option<[f32; 2]>,
    pub threads_supported_range: [u16; 2],
}

impl CapsSummary {
    /// The identity element for union merging: all flags `false`, both
    /// ranges `None`, threads `(u16::MAX, 0)` so the first real merge
    /// fully replaces it.
    pub fn empty_for_union() -> Self {
        Self {
            threads_supported_range: [u16::MAX, 0],
            ..Self::default()
        }
    }

    /// OR-merge booleans, widen ranges. `self` is the accumulator.
    pub fn union_in_place(&mut self, other: &CapsSummary) {
        self.icc |= other.icc;
        self.exif |= other.exif;
        self.xmp |= other.xmp;
        self.cicp |= other.cicp;
        self.lossy |= other.lossy;
        self.lossless |= other.lossless;
        self.hdr |= other.hdr;
        self.gain_map |= other.gain_map;
        self.native_alpha |= other.native_alpha;
        self.native_gray |= other.native_gray;
        self.native_16bit |= other.native_16bit;
        self.native_f32 |= other.native_f32;
        self.animation |= other.animation;
        self.push_rows |= other.push_rows;
        self.effort_range = union_range_i32(self.effort_range, other.effort_range);
        self.quality_range = union_range_f32(self.quality_range, other.quality_range);
        self.threads_supported_range = [
            self.threads_supported_range[0].min(other.threads_supported_range[0]),
            self.threads_supported_range[1].max(other.threads_supported_range[1]),
        ];
    }
}

fn union_range_i32(a: Option<[i32; 2]>, b: Option<[i32; 2]>) -> Option<[i32; 2]> {
    match (a, b) {
        (Some(a), Some(b)) => Some([a[0].min(b[0]), a[1].max(b[1])]),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn union_range_f32(a: Option<[f32; 2]>, b: Option<[f32; 2]>) -> Option<[f32; 2]> {
    match (a, b) {
        (Some(a), Some(b)) => Some([a[0].min(b[0]), a[1].max(b[1])]),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Per-format availability + metadata + capability union across this
/// build's enabled encoders and decoders.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct FormatAvailability {
    pub decode: bool,
    pub encode: bool,
    pub display_name: String,
    pub preferred_mime_type: String,
    pub mime_types: Vec<String>,
    pub preferred_extension: String,
    pub extensions: Vec<String>,
    pub supports_alpha: bool,
    pub supports_animation: bool,
    pub supports_lossless: bool,
    pub supports_lossy: bool,
    pub magic_bytes_needed: u32,
    /// Union of all enabled-encoder capabilities for this format in
    /// this build. `None` when no encoder backs the format.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub encode_union: Option<CapsSummary>,
    /// Union of all enabled-decoder capabilities for this format.
    /// `None` when no decoder backs the format.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub decode_union: Option<CapsSummary>,
}

/// Per-codec row of the static info response.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct CodecAvailability {
    pub format: ImageFormat,
    pub role: CodecRole,
    pub caps: CapsSummary,
}

/// Annotated RIAPI key.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct RiapiKeyInfo {
    pub name: String,
    pub category: RiapiCategory,
    /// Whether the key participates in the cached representation the
    /// server should vary on. `false` for trace/debug knobs.
    pub cache_relevant: bool,
    pub accepts: RiapiValueKind,
    /// Populated when `accepts == Enum`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enum_values: Vec<String>,
    /// For `accept.*` keys, the HTTP `Accept` header media type that the
    /// server should translate into this knob at the edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accept_header_origin: Option<String>,
}

/// RIAPI schema bundle for this build.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct RiapiSchema {
    pub keys: Vec<RiapiKeyInfo>,
    /// Whether the parser silently drops unknown keys. Useful for clients
    /// that want to probe behaviour.
    pub ignores_unknown_keys: bool,
}

/// Opinionated deployment recommendations. These are suggestions only —
/// the server chooses how to apply them.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct ServerRecommendations {
    /// Map of `Accept` header media type → RIAPI key expression that the
    /// edge should synthesize. Example: `"image/webp"` → `"accept.webp=1"`.
    pub accept_header_translation: BTreeMap<String, String>,
    /// RIAPI keys the edge should strip before hashing the cache key.
    pub strip_from_cache_key: Vec<String>,
    /// Key patterns the edge should include in the cache key prefix
    /// (wildcards allowed, e.g. `"accept.*"`).
    pub include_in_cache_key_prefix: Vec<String>,
    /// Recommended HTTP `Vary` header value.
    pub vary_header: String,
    /// Freeform guidance clarifying subtle points (e.g. why we prefer
    /// `accept.*` in the cache key to `Vary: Accept`).
    pub notes: Vec<String>,
}

/// Top-level response for `v1/static/info`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct StaticInfoResponse {
    pub imageflow_version: String,
    pub build: BuildInfo,
    /// Keyed by snake-case `ImageFormat` name.
    pub formats_available: BTreeMap<String, FormatAvailability>,
    /// Keyed by snake-case codec name (matches
    /// `NamedEncoderName`/`NamedDecoderName`).
    pub codecs: BTreeMap<String, CodecAvailability>,
    pub riapi: RiapiSchema,
    pub server_recommendations: ServerRecommendations,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_union_merges_bools_and_ranges() {
        let mut acc = CapsSummary::empty_for_union();
        let a = CapsSummary {
            icc: true,
            lossy: true,
            quality_range: Some([0.0, 100.0]),
            effort_range: None,
            threads_supported_range: [1, 4],
            ..Default::default()
        };
        let b = CapsSummary {
            exif: true,
            lossless: true,
            quality_range: Some([10.0, 90.0]),
            effort_range: Some([1, 9]),
            threads_supported_range: [2, 16],
            ..Default::default()
        };
        acc.union_in_place(&a);
        acc.union_in_place(&b);
        assert!(acc.icc);
        assert!(acc.exif);
        assert!(acc.lossy);
        assert!(acc.lossless);
        assert_eq!(acc.quality_range, Some([0.0, 100.0]));
        assert_eq!(acc.effort_range, Some([1, 9]));
        assert_eq!(acc.threads_supported_range, [1, 16]);
    }

    #[test]
    fn caps_union_of_empty_is_empty() {
        let acc = CapsSummary::empty_for_union();
        // After no merges the accumulator still has no real data.
        assert!(!acc.icc);
        assert!(!acc.lossy);
        assert_eq!(acc.effort_range, None);
        assert_eq!(acc.quality_range, None);
        // Threads sentinel: caller must collapse to [1, 1] when nothing
        // was unioned in.
        assert_eq!(acc.threads_supported_range, [u16::MAX, 0]);
    }

    #[test]
    fn response_roundtrips_through_json() {
        let r = StaticInfoResponse {
            imageflow_version: "test".to_string(),
            build: BuildInfo {
                features: vec!["zen-codecs".to_string()],
                compile_deny_decode: Vec::new(),
                compile_deny_encode: Vec::new(),
                codec_priority_default: "v3_zen_first".to_string(),
            },
            formats_available: BTreeMap::new(),
            codecs: BTreeMap::new(),
            riapi: RiapiSchema {
                keys: Vec::new(),
                ignores_unknown_keys: false,
            },
            server_recommendations: ServerRecommendations {
                accept_header_translation: BTreeMap::new(),
                strip_from_cache_key: Vec::new(),
                include_in_cache_key_prefix: Vec::new(),
                vary_header: "Accept".to_string(),
                notes: Vec::new(),
            },
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: StaticInfoResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }
}
