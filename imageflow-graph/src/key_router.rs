//! Versioned RIAPI querystring key router.
//!
//! Partitions querystring key-value pairs into categories (layout, codec,
//! filter, decode, meta) based on a versioned vocabulary. The router is the
//! single source of truth for which keys are recognized and where they're
//! dispatched. Neither zenlayout nor the codec selector needs to know the
//! full key vocabulary — they receive pre-partitioned, validated inputs.
//!
//! # Usage
//!
//! ```
//! use imageflow_graph::key_router::{route_query, ApiVersion};
//!
//! let pairs = vec![
//!     ("w".into(), "800".into()),
//!     ("format".into(), "webp".into()),
//!     ("f.sharpen".into(), "15".into()),
//! ];
//! let routed = route_query(ApiVersion::V2, &pairs);
//! assert!(routed.layout.contains_key("w"));
//! assert!(routed.codec.contains_key("format"));
//! assert!(routed.filter.contains_key("f.sharpen"));
//! ```

use std::collections::BTreeMap;

use crate::srcset;

/// API version controls key recognition and deprecation behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiVersion {
    /// Imageflow v1 — all legacy keys recognized, deprecated keys warn.
    V1,
    /// Imageflow v2 API (Imageflow 3 product) — deprecated keys error.
    V2,
}

/// Which subsystem handles a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyTarget {
    /// Layout engine (zenlayout): sizing, cropping, rotation, orientation.
    Layout,
    /// Codec selector: format, quality, lossless, per-codec hints, accept.*.
    Codec,
    /// Filter pipeline: s.*, f.*, a.*, trim.*.
    Filter,
    /// Decoder config: ICC handling, frame selection, decoder hints.
    Decode,
    /// Composition: watermarks, overlays.
    Compose,
    /// Meta/no-op: cache, process, builder, preset, 404.
    Meta,
    /// Deprecated: recognized but discouraged.
    Deprecated,
}

/// A warning or error from key routing.
#[derive(Debug, Clone, PartialEq)]
pub enum QueryWarning {
    /// Key recognized but deprecated.
    Deprecated { key: String, suggestion: Option<&'static str> },
    /// Key not recognized at all.
    Unknown { key: String },
    /// Duplicate key (first value kept).
    Duplicate { key: String },
    /// Srcset expansion warning.
    Srcset(srcset::SrcsetWarning),
}

/// Partitioned query parameters, ready for dispatch to subsystems.
#[derive(Debug, Clone, Default)]
pub struct RoutedQuery {
    /// Keys for zenlayout (layout/sizing/orientation/crop).
    pub layout: BTreeMap<String, String>,
    /// Keys for the codec selector (format/quality/accept/per-codec hints).
    pub codec: BTreeMap<String, String>,
    /// Keys for the filter pipeline (s.*/f.*/a.*/trim.*).
    pub filter: BTreeMap<String, String>,
    /// Keys for decoder configuration (ICC, frame, decoder hints).
    pub decode: BTreeMap<String, String>,
    /// Keys for composition (watermarks, overlays).
    pub compose: BTreeMap<String, String>,
    /// Meta keys (no-op, informational).
    pub meta: BTreeMap<String, String>,
    /// Warnings accumulated during routing.
    pub warnings: Vec<QueryWarning>,
}

/// Parse a raw querystring and route keys through the versioned vocabulary.
///
/// Handles srcset/short expansion, key lowercasing, deduplication, alias
/// canonicalization, and deprecation.
pub fn route_querystring(version: ApiVersion, querystring: &str) -> RoutedQuery {
    // Parse querystring into key-value pairs
    let mut pairs: Vec<(String, String)> = Vec::new();
    for part in querystring.split('&') {
        if part.is_empty() {
            continue;
        }
        let (key, value) = match part.split_once('=') {
            Some((k, v)) => (k.to_ascii_lowercase(), v.to_string()),
            None => (part.to_ascii_lowercase(), String::new()),
        };
        pairs.push((key, value));
    }

    // Extract and expand srcset/short before routing
    let mut expanded = Vec::new();
    let mut srcset_warnings = Vec::new();
    let mut non_srcset = Vec::new();

    for (key, value) in &pairs {
        if key == "srcset" || key == "short" {
            let (srcset_pairs, warnings) = srcset::expand_srcset(value);
            expanded.extend(srcset_pairs);
            srcset_warnings.extend(warnings);
        } else {
            non_srcset.push((key.clone(), value.clone()));
        }
    }

    // Route srcset-expanded pairs first, then overlay explicit pairs.
    // Explicit keys override srcset-expanded ones without triggering duplicate warnings.
    let mut result = route_pairs(version, &expanded);
    overlay_pairs(version, &non_srcset, &mut result);
    result.warnings.extend(srcset_warnings.into_iter().map(QueryWarning::Srcset));
    result
}

/// Route pre-parsed key-value pairs (already lowercased, srcset already expanded).
pub fn route_query(version: ApiVersion, pairs: &[(String, String)]) -> RoutedQuery {
    route_pairs(version, pairs)
}

fn route_pairs(version: ApiVersion, pairs: &[(String, String)]) -> RoutedQuery {
    let mut result = RoutedQuery::default();
    let mut seen = std::collections::HashSet::new();
    insert_pairs(version, pairs, &mut result, &mut seen, true);
    result
}

/// Overlay additional pairs onto an existing result. Keys already present
/// are overwritten (explicit params override srcset-expanded ones) without
/// triggering duplicate warnings.
fn overlay_pairs(version: ApiVersion, pairs: &[(String, String)], result: &mut RoutedQuery) {
    let mut seen = std::collections::HashSet::new();
    insert_pairs(version, pairs, result, &mut seen, true);
}

fn insert_pairs(
    version: ApiVersion,
    pairs: &[(String, String)],
    result: &mut RoutedQuery,
    seen: &mut std::collections::HashSet<String>,
    warn_duplicates: bool,
) {
    for (key, value) in pairs {
        // Deduplicate within this batch: first value wins
        if !seen.insert(key.clone()) {
            if warn_duplicates {
                result.warnings.push(QueryWarning::Duplicate { key: key.clone() });
            }
            continue;
        }

        let (target, canonical) = classify_key(key);

        match target {
            KeyTarget::Deprecated => {
                result.warnings.push(QueryWarning::Deprecated {
                    key: key.clone(),
                    suggestion: deprecation_hint(key),
                });
                // In v1, still route deprecated keys for backward compat
                if version == ApiVersion::V1 {
                    if let Some(fallback) = deprecated_fallback(key) {
                        target_map(result, fallback).insert(canonical, value.clone());
                    }
                }
            }
            target => {
                target_map(result, target).insert(canonical, value.clone());
            }
        }
    }
}

fn target_map(result: &mut RoutedQuery, target: KeyTarget) -> &mut BTreeMap<String, String> {
    match target {
        KeyTarget::Layout => &mut result.layout,
        KeyTarget::Codec => &mut result.codec,
        KeyTarget::Filter => &mut result.filter,
        KeyTarget::Decode => &mut result.decode,
        KeyTarget::Compose => &mut result.compose,
        KeyTarget::Meta | KeyTarget::Deprecated => &mut result.meta,
    }
}

/// Classify a key and return (target, canonical_name).
///
/// Canonical name handles aliases (e.g., "width" → "w", "thumbnail" → "format").
fn classify_key(key: &str) -> (KeyTarget, String) {
    match key {
        // ── Layout ──
        "w" | "width" => (KeyTarget::Layout, "w".into()),
        "h" | "height" => (KeyTarget::Layout, "h".into()),
        "maxwidth" => (KeyTarget::Layout, "maxwidth".into()),
        "maxheight" => (KeyTarget::Layout, "maxheight".into()),
        "mode" => (KeyTarget::Layout, "mode".into()),
        "scale" => (KeyTarget::Layout, "scale".into()),
        "crop" => (KeyTarget::Layout, "crop".into()),
        "c" => (KeyTarget::Layout, "c".into()),
        "cropxunits" => (KeyTarget::Layout, "cropxunits".into()),
        "cropyunits" => (KeyTarget::Layout, "cropyunits".into()),
        "anchor" => (KeyTarget::Layout, "anchor".into()),
        "c.gravity" => (KeyTarget::Layout, "c.gravity".into()),
        "zoom" | "dpr" | "dppx" => (KeyTarget::Layout, "zoom".into()),
        "flip" => (KeyTarget::Layout, "flip".into()),
        "sflip" | "sourceflip" => (KeyTarget::Layout, "sflip".into()),
        "srotate" => (KeyTarget::Layout, "srotate".into()),
        "rotate" => (KeyTarget::Layout, "rotate".into()),
        "autorotate" => (KeyTarget::Layout, "autorotate".into()),
        "bgcolor" | "s.bgcolor" => (KeyTarget::Layout, "bgcolor".into()),

        // ── Codec selector ──
        "format" | "thumbnail" => (KeyTarget::Codec, "format".into()),
        "quality" => (KeyTarget::Codec, "quality".into()),
        "lossless" => (KeyTarget::Codec, "lossless".into()),
        "qp" => (KeyTarget::Codec, "qp".into()),
        "qp.dpr" | "qp.dppx" => (KeyTarget::Codec, "qp.dpr".into()),
        "accept.webp" => (KeyTarget::Codec, "accept.webp".into()),
        "accept.avif" => (KeyTarget::Codec, "accept.avif".into()),
        "accept.jxl" => (KeyTarget::Codec, "accept.jxl".into()),
        "accept.color_profiles" => (KeyTarget::Codec, "accept.color_profiles".into()),
        "subsampling" => (KeyTarget::Codec, "subsampling".into()),

        // Per-codec hints → codec selector
        "jpeg.quality" => (KeyTarget::Codec, key.into()),
        "jpeg.progressive" => (KeyTarget::Codec, key.into()),
        "jpeg.li" => (KeyTarget::Codec, key.into()),
        "png.quality" => (KeyTarget::Codec, key.into()),
        "png.lossless" => (KeyTarget::Codec, key.into()),
        "png.min_quality" => (KeyTarget::Codec, key.into()),
        "png.quantization_speed" => (KeyTarget::Codec, key.into()),
        "png.libpng" => (KeyTarget::Codec, key.into()),
        "png.max_deflate" => (KeyTarget::Codec, key.into()),
        "webp.quality" => (KeyTarget::Codec, key.into()),
        "webp.lossless" => (KeyTarget::Codec, key.into()),
        "avif.quality" => (KeyTarget::Codec, key.into()),
        "avif.speed" => (KeyTarget::Codec, key.into()),
        "jxl.quality" => (KeyTarget::Codec, key.into()),
        "jxl.lossless" => (KeyTarget::Codec, key.into()),
        "jxl.distance" => (KeyTarget::Codec, key.into()),
        "jxl.effort" => (KeyTarget::Codec, key.into()),

        // ── Filters ──
        "s.alpha" | "s.brightness" | "s.contrast" | "s.saturation" | "s.sepia" | "s.grayscale"
        | "s.invert" | "s.roundcorners" => (KeyTarget::Filter, key.into()),
        "f.sharpen" | "f.sharpen_when" => (KeyTarget::Filter, key.into()),
        "down.filter" | "up.filter" | "down.colorspace" | "up.colorspace" => {
            (KeyTarget::Filter, key.into())
        }
        "a.blur" | "a.sharpen" | "a.removenoise" | "a.balancewhite" => {
            (KeyTarget::Filter, key.into())
        }
        "trim.threshold" | "trim.percentpadding" => (KeyTarget::Filter, key.into()),

        // ── Decode ──
        "ignoreicc" | "ignore_icc_errors" => (KeyTarget::Decode, key.into()),
        "frame" | "page" => (KeyTarget::Decode, "frame".into()),
        "decoder.min_precise_scaling_ratio" => (KeyTarget::Decode, key.into()),

        // ── Composition ──
        "watermark_red_dot" => (KeyTarget::Compose, key.into()),
        "watermark" => (KeyTarget::Compose, key.into()),

        // ── Meta / no-op ──
        "cache" | "process" | "builder" | "preset" | "404" | "floatspace" | "dither" => {
            (KeyTarget::Meta, key.into())
        }
        "encoder" | "decoder" => (KeyTarget::Meta, key.into()),

        // ── Deprecated ──
        "stretch" => (KeyTarget::Deprecated, key.into()),
        "colors" => (KeyTarget::Deprecated, key.into()),
        "paddingcolor" | "bordercolor" | "paddingwidth" | "paddingheight" | "margin"
        | "borderwidth" => (KeyTarget::Deprecated, key.into()),
        "jpeg_idct_downscale_linear" | "fastscale" => (KeyTarget::Deprecated, key.into()),
        "jpeg.turbo" => (KeyTarget::Deprecated, key.into()),

        // ── Unknown ──
        _ => (KeyTarget::Meta, key.into()),
    }
}

/// Return a hint for what to use instead of a deprecated key.
fn deprecation_hint(key: &str) -> Option<&'static str> {
    match key {
        "stretch" => Some("use mode=stretch instead"),
        "paddingcolor" => Some("use bgcolor instead"),
        "colors" => Some("use png.quality instead"),
        "jpeg.turbo" => Some("encoder selection is automatic in v2"),
        "jpeg_idct_downscale_linear" | "fastscale" => {
            Some("decoder downscaling is automatic in v2")
        }
        "paddingwidth" | "paddingheight" | "margin" | "borderwidth" | "bordercolor" => {
            Some("use mode=pad with bgcolor instead")
        }
        _ => None,
    }
}

/// For deprecated keys in v1 mode, where should they be routed for backward compat?
fn deprecated_fallback(key: &str) -> Option<KeyTarget> {
    match key {
        "stretch" => Some(KeyTarget::Layout),
        "paddingcolor" => Some(KeyTarget::Layout),
        "colors" => Some(KeyTarget::Codec),
        "jpeg.turbo" => Some(KeyTarget::Codec),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_keys_routed() {
        let pairs = vec![
            ("w".into(), "800".into()),
            ("h".into(), "600".into()),
            ("mode".into(), "crop".into()),
        ];
        let r = route_query(ApiVersion::V2, &pairs);
        assert_eq!(r.layout.get("w"), Some(&"800".to_string()));
        assert_eq!(r.layout.get("h"), Some(&"600".to_string()));
        assert_eq!(r.layout.get("mode"), Some(&"crop".to_string()));
    }

    #[test]
    fn codec_keys_routed() {
        let pairs = vec![
            ("format".into(), "webp".into()),
            ("webp.quality".into(), "70".into()),
            ("qp".into(), "good".into()),
        ];
        let r = route_query(ApiVersion::V2, &pairs);
        assert_eq!(r.codec.get("format"), Some(&"webp".to_string()));
        assert_eq!(r.codec.get("webp.quality"), Some(&"70".to_string()));
        assert_eq!(r.codec.get("qp"), Some(&"good".to_string()));
    }

    #[test]
    fn filter_keys_routed() {
        let pairs = vec![
            ("s.sharpen".into(), "15".into()),
            ("f.sharpen".into(), "20".into()),
            ("s.contrast".into(), "0.5".into()),
        ];
        let r = route_query(ApiVersion::V2, &pairs);
        assert!(r.filter.contains_key("f.sharpen"));
        assert!(r.filter.contains_key("s.contrast"));
    }

    #[test]
    fn decode_keys_routed() {
        let pairs = vec![("frame".into(), "2".into()), ("ignoreicc".into(), "true".into())];
        let r = route_query(ApiVersion::V2, &pairs);
        assert!(r.decode.contains_key("frame"));
        assert!(r.decode.contains_key("ignoreicc"));
    }

    #[test]
    fn aliases_canonicalized() {
        let pairs = vec![
            ("width".into(), "800".into()),
            ("height".into(), "600".into()),
            ("thumbnail".into(), "webp".into()),
            ("sourceflip".into(), "h".into()),
            ("qp.dppx".into(), "2".into()),
            ("page".into(), "3".into()),
        ];
        let r = route_query(ApiVersion::V2, &pairs);
        assert!(r.layout.contains_key("w"));
        assert!(r.layout.contains_key("h"));
        assert!(r.codec.contains_key("format"));
        assert!(r.layout.contains_key("sflip"));
        assert!(r.codec.contains_key("qp.dpr"));
        assert!(r.decode.contains_key("frame"));
    }

    #[test]
    fn deprecated_warns_v2() {
        let pairs = vec![("stretch".into(), "fill".into())];
        let r = route_query(ApiVersion::V2, &pairs);
        assert!(r
            .warnings
            .iter()
            .any(|w| matches!(w, QueryWarning::Deprecated { key, .. } if key == "stretch")));
        // In v2, deprecated keys don't reach their target
        assert!(!r.layout.contains_key("stretch"));
    }

    #[test]
    fn deprecated_routes_v1() {
        let pairs = vec![("stretch".into(), "fill".into())];
        let r = route_query(ApiVersion::V1, &pairs);
        assert!(r.warnings.iter().any(|w| matches!(w, QueryWarning::Deprecated { .. })));
        // In v1, deprecated keys still route for backward compat
        assert!(r.layout.contains_key("stretch"));
    }

    #[test]
    fn duplicates_warn_first_wins() {
        let pairs = vec![("w".into(), "800".into()), ("w".into(), "400".into())];
        let r = route_query(ApiVersion::V2, &pairs);
        assert!(r.warnings.iter().any(|w| matches!(w, QueryWarning::Duplicate { .. })));
        assert_eq!(r.layout.get("w"), Some(&"800".to_string()));
    }

    #[test]
    fn srcset_expanded_in_querystring() {
        let r = route_querystring(ApiVersion::V2, "srcset=webp-70,100w&h=600");
        assert_eq!(r.codec.get("format"), Some(&"webp".to_string()));
        assert_eq!(r.codec.get("webp.quality"), Some(&"70".to_string()));
        assert_eq!(r.layout.get("w"), Some(&"100".to_string()));
        assert_eq!(r.layout.get("h"), Some(&"600".to_string()));
    }

    #[test]
    fn explicit_overrides_srcset() {
        // srcset says webp, explicit says jpeg — explicit wins (comes after)
        let r = route_querystring(ApiVersion::V2, "srcset=webp-70&format=jpeg");
        assert_eq!(r.codec.get("format"), Some(&"jpeg".to_string()));
    }

    #[test]
    fn full_realistic_query() {
        let r = route_querystring(
            ApiVersion::V2,
            "w=800&h=600&mode=crop&format=auto&qp=good&qp.dpr=2&accept.webp=true&accept.avif=true&f.sharpen=15&autorotate=true",
        );
        assert_eq!(r.layout.len(), 4); // w, h, mode, autorotate
        assert!(r.codec.len() >= 5); // format, qp, qp.dpr, accept.webp, accept.avif
        assert_eq!(r.filter.len(), 1); // f.sharpen
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn composition_keys_routed() {
        let pairs = vec![("watermark_red_dot".into(), "true".into())];
        let r = route_query(ApiVersion::V2, &pairs);
        assert!(r.compose.contains_key("watermark_red_dot"));
    }
}
