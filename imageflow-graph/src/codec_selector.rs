//! Codec selection: format auto-selection and quality resolution.
//!
//! Takes the codec partition from [`super::key_router`] and image facts,
//! produces a [`FormatDecision`] (format + quality intent + per-codec hints).
//!
//! This is the authoritative implementation for `format=auto`, `qp`, and
//! per-codec quality. Both the v1 compatibility path and the v3 engine
//! delegate here.

use std::collections::BTreeMap;

use crate::quality::{QualityIntent, QualityProfile};

// ── Input types ─────────────────────────────────────────────────────────

/// Observable facts about the image being processed.
#[derive(Debug, Clone, Default)]
pub struct ImageFacts {
    pub has_alpha: bool,
    pub has_animation: bool,
    pub pixel_count: u64,
    pub source_format: Option<ImageFormat>,
    pub source_lossless: Option<bool>,
}

/// Which output formats are allowed.
#[derive(Debug, Clone)]
pub struct AllowedFormats {
    pub jpeg: bool,
    pub png: bool,
    pub gif: bool,
    pub webp: bool,
    pub avif: bool,
    pub jxl: bool,
}

impl Default for AllowedFormats {
    fn default() -> Self {
        Self::web_safe()
    }
}

impl AllowedFormats {
    /// Conservative default: JPEG, PNG, GIF only.
    pub fn web_safe() -> Self {
        Self { jpeg: true, png: true, gif: true, webp: false, avif: false, jxl: false }
    }

    /// Modern web: includes WebP.
    pub fn modern_web() -> Self {
        Self { jpeg: true, png: true, gif: true, webp: true, avif: false, jxl: false }
    }

    /// Everything enabled.
    pub fn all() -> Self {
        Self { jpeg: true, png: true, gif: true, webp: true, avif: true, jxl: true }
    }

    pub fn allows(&self, format: ImageFormat) -> bool {
        match format {
            ImageFormat::Jpeg => self.jpeg,
            ImageFormat::Png => self.png,
            ImageFormat::Gif => self.gif,
            ImageFormat::Webp => self.webp,
            ImageFormat::Avif => self.avif,
            ImageFormat::Jxl => self.jxl,
        }
    }
}

/// Image formats for codec selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Gif,
    Webp,
    Avif,
    Jxl,
}

// ── Codec intent (parsed from query keys) ───────────────────────────────

/// Parsed codec-related user intent from querystring parameters.
///
/// Constructed from the `codec` partition of [`super::key_router::RoutedQuery`].
#[derive(Debug, Clone, Default)]
pub struct CodecIntent {
    /// Explicit format choice (`None` = auto).
    pub format: Option<FormatChoice>,
    /// Quality profile from `qp=`.
    pub quality_profile: Option<QualityProfile>,
    /// Fallback quality from `quality=` (0–100).
    pub quality_fallback: Option<f32>,
    /// DPR adjustment for quality from `qp.dpr=`.
    pub quality_dpr: Option<f32>,
    /// Global lossless preference from `lossless=`.
    pub lossless: Option<BoolKeep>,
    /// Allowed formats from `accept.*` keys.
    pub allowed: AllowedFormats,
    /// Per-codec hints (raw key-value pairs for downstream config builders).
    pub hints: PerCodecHints,
}

/// Explicit format choice from `format=` or `thumbnail=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatChoice {
    /// A specific format.
    Specific(ImageFormat),
    /// `format=auto` — let the selector decide.
    Auto,
    /// `format=keep` — match source format.
    Keep,
}

/// Tri-state for lossless parameters: true, false, or keep (match source).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolKeep {
    True,
    False,
    Keep,
}

/// Per-codec encoder hints (raw key-value pairs).
#[derive(Debug, Clone, Default)]
pub struct PerCodecHints {
    pub jpeg: BTreeMap<String, String>,
    pub png: BTreeMap<String, String>,
    pub webp: BTreeMap<String, String>,
    pub avif: BTreeMap<String, String>,
    pub jxl: BTreeMap<String, String>,
}

// ── Decision output ─────────────────────────────────────────────────────

/// The result of codec selection: what format, what quality, why.
#[derive(Debug, Clone)]
pub struct FormatDecision {
    /// The selected output format.
    pub format: ImageFormat,
    /// Resolved quality intent with per-codec calibration.
    pub quality: QualityIntent,
    /// Per-codec hints for the selected format.
    pub hints: BTreeMap<String, String>,
    /// Explanation trace for debugging.
    pub trace: Vec<SelectionStep>,
}

/// A step in the selection decision, for debugging/auditing.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectionStep {
    FormatChosen { format: ImageFormat, reason: &'static str },
    FormatSkipped { format: ImageFormat, reason: &'static str },
    Info { message: &'static str },
}

// ── Parsing codec intent from routed keys ───────────────────────────────

impl CodecIntent {
    /// Parse a `CodecIntent` from the codec partition of a routed query.
    pub fn from_routed_codec_keys(keys: &BTreeMap<String, String>) -> Self {
        let mut intent = Self::default();

        // format=
        if let Some(v) = keys.get("format") {
            intent.format = parse_format_choice(v);
        }

        // qp= (quality profile)
        if let Some(v) = keys.get("qp") {
            intent.quality_profile = QualityProfile::parse(v);
        }

        // quality= (fallback numeric quality)
        if let Some(v) = keys.get("quality") {
            intent.quality_fallback = v.parse().ok();
        }

        // qp.dpr= (DPR adjustment)
        if let Some(v) = keys.get("qp.dpr") {
            let number = v.strip_suffix('x').unwrap_or(v);
            intent.quality_dpr = number.parse().ok();
        }

        // lossless=
        if let Some(v) = keys.get("lossless") {
            intent.lossless = parse_bool_keep(v);
        }

        // accept.* keys
        if let Some(v) = keys.get("accept.webp") {
            if parse_bool(v) == Some(true) {
                intent.allowed.webp = true;
            }
        }
        if let Some(v) = keys.get("accept.avif") {
            if parse_bool(v) == Some(true) {
                intent.allowed.avif = true;
            }
        }
        if let Some(v) = keys.get("accept.jxl") {
            if parse_bool(v) == Some(true) {
                intent.allowed.jxl = true;
            }
        }

        // Per-codec hints: partition by prefix
        for (k, v) in keys {
            if let Some(rest) = k.strip_prefix("jpeg.") {
                intent.hints.jpeg.insert(rest.to_string(), v.clone());
            } else if let Some(rest) = k.strip_prefix("png.") {
                intent.hints.png.insert(rest.to_string(), v.clone());
            } else if let Some(rest) = k.strip_prefix("webp.") {
                intent.hints.webp.insert(rest.to_string(), v.clone());
            } else if let Some(rest) = k.strip_prefix("avif.") {
                intent.hints.avif.insert(rest.to_string(), v.clone());
            } else if let Some(rest) = k.strip_prefix("jxl.") {
                intent.hints.jxl.insert(rest.to_string(), v.clone());
            }
        }

        intent
    }

    /// Resolve the quality intent from profile, fallback, and DPR.
    pub fn resolve_quality(&self) -> QualityIntent {
        if let Some(profile) = self.quality_profile {
            QualityIntent::from_profile(profile, self.quality_dpr)
        } else if let Some(fallback) = self.quality_fallback {
            QualityIntent::from_value(fallback)
        } else {
            // No quality specified — default depends on context
            QualityIntent::default()
        }
    }
}

// ── Format selection ────────────────────────────────────────────────────

/// Select the output format and resolve quality.
///
/// This is the main entry point for codec decisions. It handles:
/// - Explicit format selection (`format=jpeg`)
/// - Format auto-selection (`format=auto` or with `qp=`)
/// - Keep-source passthrough (`format=keep`)
/// - Quality resolution from `qp`, `quality`, per-codec hints, and DPR
pub fn select_format(intent: &CodecIntent, facts: &ImageFacts) -> FormatDecision {
    let mut trace = Vec::new();
    let quality = intent.resolve_quality();

    // Determine the target format
    let format_choice = intent.format.unwrap_or_else(|| {
        // No format specified: auto if qp is set, keep otherwise
        if intent.quality_profile.is_some() {
            FormatChoice::Auto
        } else {
            FormatChoice::Keep
        }
    });

    let format = match format_choice {
        FormatChoice::Specific(f) => {
            trace.push(SelectionStep::FormatChosen { format: f, reason: "explicitly specified" });
            f
        }
        FormatChoice::Keep => {
            let f = facts.source_format.unwrap_or(ImageFormat::Jpeg);
            trace.push(SelectionStep::FormatChosen { format: f, reason: "keeping source format" });
            f
        }
        FormatChoice::Auto => select_auto_format(intent, facts, &quality, &mut trace),
    };

    // Get hints for the selected format
    let hints = match format {
        ImageFormat::Jpeg => intent.hints.jpeg.clone(),
        ImageFormat::Png => intent.hints.png.clone(),
        ImageFormat::Webp => intent.hints.webp.clone(),
        ImageFormat::Avif => intent.hints.avif.clone(),
        ImageFormat::Jxl => intent.hints.jxl.clone(),
        ImageFormat::Gif => BTreeMap::new(),
    };

    FormatDecision { format, quality, hints, trace }
}

/// Auto-select the best format based on image facts and allowed formats.
fn select_auto_format(
    intent: &CodecIntent,
    facts: &ImageFacts,
    quality: &QualityIntent,
    trace: &mut Vec<SelectionStep>,
) -> ImageFormat {
    let allowed = &intent.allowed;
    let want_lossless = intent.lossless == Some(BoolKeep::True) || quality.is_lossless();

    // Animated images → GIF or WebP (if allowed)
    if facts.has_animation {
        trace.push(SelectionStep::Info { message: "source is animated" });
        if allowed.allows(ImageFormat::Webp) {
            trace.push(SelectionStep::FormatChosen {
                format: ImageFormat::Webp,
                reason: "animated + WebP allowed",
            });
            return ImageFormat::Webp;
        }
        if allowed.allows(ImageFormat::Gif) {
            trace.push(SelectionStep::FormatChosen {
                format: ImageFormat::Gif,
                reason: "animated + GIF fallback",
            });
            return ImageFormat::Gif;
        }
    }

    // Lossless preference → PNG, WebP lossless, JXL lossless
    if want_lossless {
        trace.push(SelectionStep::Info { message: "lossless requested" });
        if allowed.allows(ImageFormat::Jxl) {
            trace.push(SelectionStep::FormatChosen {
                format: ImageFormat::Jxl,
                reason: "lossless + JXL allowed (best lossless compression)",
            });
            return ImageFormat::Jxl;
        }
        if allowed.allows(ImageFormat::Webp) {
            trace.push(SelectionStep::FormatChosen {
                format: ImageFormat::Webp,
                reason: "lossless + WebP allowed",
            });
            return ImageFormat::Webp;
        }
        if allowed.allows(ImageFormat::Png) {
            trace.push(SelectionStep::FormatChosen {
                format: ImageFormat::Png,
                reason: "lossless + PNG fallback",
            });
            return ImageFormat::Png;
        }
    }

    // Alpha channel → prefer formats that support alpha
    if facts.has_alpha {
        trace.push(SelectionStep::Info { message: "source has alpha" });
        // Prefer modern formats for alpha
        for &fmt in &[ImageFormat::Jxl, ImageFormat::Avif, ImageFormat::Webp, ImageFormat::Png] {
            if allowed.allows(fmt) {
                trace.push(SelectionStep::FormatChosen {
                    format: fmt,
                    reason: "alpha + format allowed",
                });
                return fmt;
            }
        }
    }

    // Lossy, no alpha → prefer best quality-per-byte
    // JXL > AVIF > WebP > JPEG (roughly)
    for &(fmt, reason) in &[
        (ImageFormat::Jxl, "best lossy compression ratio"),
        (ImageFormat::Avif, "excellent lossy compression"),
        (ImageFormat::Webp, "good lossy compression, wide support"),
        (ImageFormat::Jpeg, "universal lossy format"),
    ] {
        if allowed.allows(fmt) {
            // Skip JPEG for alpha images (handled above, but defensive)
            if fmt == ImageFormat::Jpeg && facts.has_alpha {
                trace.push(SelectionStep::FormatSkipped {
                    format: fmt,
                    reason: "JPEG doesn't support alpha",
                });
                continue;
            }
            trace.push(SelectionStep::FormatChosen { format: fmt, reason });
            return fmt;
        }
        trace.push(SelectionStep::FormatSkipped { format: fmt, reason: "not allowed" });
    }

    // Nothing allowed? Fall back to JPEG regardless.
    trace.push(SelectionStep::Info { message: "no allowed format found, falling back to JPEG" });
    ImageFormat::Jpeg
}

// ── Parse helpers ───────────────────────────────────────────────────────

fn parse_format_choice(value: &str) -> Option<FormatChoice> {
    match value.to_ascii_lowercase().as_str() {
        "jpeg" | "jpg" => Some(FormatChoice::Specific(ImageFormat::Jpeg)),
        "png" => Some(FormatChoice::Specific(ImageFormat::Png)),
        "gif" => Some(FormatChoice::Specific(ImageFormat::Gif)),
        "webp" => Some(FormatChoice::Specific(ImageFormat::Webp)),
        "avif" => Some(FormatChoice::Specific(ImageFormat::Avif)),
        "jxl" => Some(FormatChoice::Specific(ImageFormat::Jxl)),
        "auto" => Some(FormatChoice::Auto),
        "keep" => Some(FormatChoice::Keep),
        _ => None,
    }
}

fn parse_bool_keep(value: &str) -> Option<BoolKeep> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(BoolKeep::True),
        "false" | "0" | "no" => Some(BoolKeep::False),
        "keep" => Some(BoolKeep::Keep),
        _ => None,
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auto_intent() -> CodecIntent {
        CodecIntent {
            format: Some(FormatChoice::Auto),
            quality_profile: Some(QualityProfile::Good),
            allowed: AllowedFormats::all(),
            ..Default::default()
        }
    }

    fn facts() -> ImageFacts {
        ImageFacts { source_format: Some(ImageFormat::Jpeg), ..Default::default() }
    }

    #[test]
    fn explicit_format_honored() {
        let intent = CodecIntent {
            format: Some(FormatChoice::Specific(ImageFormat::Webp)),
            ..Default::default()
        };
        let decision = select_format(&intent, &facts());
        assert_eq!(decision.format, ImageFormat::Webp);
    }

    #[test]
    fn keep_uses_source() {
        let intent = CodecIntent { format: Some(FormatChoice::Keep), ..Default::default() };
        let mut f = facts();
        f.source_format = Some(ImageFormat::Png);
        let decision = select_format(&intent, &f);
        assert_eq!(decision.format, ImageFormat::Png);
    }

    #[test]
    fn auto_prefers_jxl_when_allowed() {
        let decision = select_format(&auto_intent(), &facts());
        assert_eq!(decision.format, ImageFormat::Jxl);
    }

    #[test]
    fn auto_falls_back_to_webp_then_jpeg() {
        let mut intent = auto_intent();
        intent.allowed.jxl = false;
        intent.allowed.avif = false;
        let decision = select_format(&intent, &facts());
        assert_eq!(decision.format, ImageFormat::Webp);

        intent.allowed.webp = false;
        let decision = select_format(&intent, &facts());
        assert_eq!(decision.format, ImageFormat::Jpeg);
    }

    #[test]
    fn alpha_avoids_jpeg() {
        let mut intent = auto_intent();
        intent.allowed = AllowedFormats { jpeg: true, png: true, ..AllowedFormats::web_safe() };
        let mut f = facts();
        f.has_alpha = true;
        let decision = select_format(&intent, &f);
        assert_eq!(decision.format, ImageFormat::Png);
    }

    #[test]
    fn animated_prefers_webp() {
        let mut f = facts();
        f.has_animation = true;
        let mut intent = auto_intent();
        intent.allowed = AllowedFormats::modern_web();
        let decision = select_format(&intent, &f);
        assert_eq!(decision.format, ImageFormat::Webp);
    }

    #[test]
    fn lossless_prefers_jxl() {
        let mut intent = auto_intent();
        intent.lossless = Some(BoolKeep::True);
        let decision = select_format(&intent, &facts());
        assert_eq!(decision.format, ImageFormat::Jxl);
    }

    #[test]
    fn lossless_falls_back_to_png() {
        let mut intent = auto_intent();
        intent.lossless = Some(BoolKeep::True);
        intent.allowed = AllowedFormats::web_safe();
        let decision = select_format(&intent, &facts());
        assert_eq!(decision.format, ImageFormat::Png);
    }

    #[test]
    fn qp_triggers_auto_format() {
        let intent = CodecIntent {
            quality_profile: Some(QualityProfile::Good),
            allowed: AllowedFormats::web_safe(),
            ..Default::default()
        };
        let decision = select_format(&intent, &facts());
        // qp without explicit format → auto → JPEG (only lossy in web_safe)
        assert_eq!(decision.format, ImageFormat::Jpeg);
    }

    #[test]
    fn quality_resolved_from_qp() {
        let intent = CodecIntent {
            quality_profile: Some(QualityProfile::High),
            quality_dpr: Some(2.0),
            ..Default::default()
        };
        let q = intent.resolve_quality();
        assert!(q.generic_quality > 91.0); // High = 91, DPR 2 < 3 baseline → quality increases
    }

    #[test]
    fn quality_falls_back_to_numeric() {
        let intent = CodecIntent { quality_fallback: Some(75.0), ..Default::default() };
        let q = intent.resolve_quality();
        assert!((q.generic_quality - 75.0).abs() < 0.01);
    }

    #[test]
    fn parse_from_routed_keys() {
        let mut keys = BTreeMap::new();
        keys.insert("format".into(), "auto".into());
        keys.insert("qp".into(), "good".into());
        keys.insert("qp.dpr".into(), "2".into());
        keys.insert("accept.webp".into(), "true".into());
        keys.insert("jpeg.progressive".into(), "true".into());
        keys.insert("webp.quality".into(), "70".into());

        let intent = CodecIntent::from_routed_codec_keys(&keys);
        assert_eq!(intent.format, Some(FormatChoice::Auto));
        assert_eq!(intent.quality_profile, Some(QualityProfile::Good));
        assert_eq!(intent.quality_dpr, Some(2.0));
        assert!(intent.allowed.webp);
        assert_eq!(intent.hints.jpeg.get("progressive"), Some(&"true".to_string()));
        assert_eq!(intent.hints.webp.get("quality"), Some(&"70".to_string()));
    }

    #[test]
    fn web_safe_default() {
        let allowed = AllowedFormats::web_safe();
        assert!(allowed.allows(ImageFormat::Jpeg));
        assert!(allowed.allows(ImageFormat::Png));
        assert!(allowed.allows(ImageFormat::Gif));
        assert!(!allowed.allows(ImageFormat::Webp));
        assert!(!allowed.allows(ImageFormat::Avif));
        assert!(!allowed.allows(ImageFormat::Jxl));
    }

    #[test]
    fn trace_is_populated() {
        let decision = select_format(&auto_intent(), &facts());
        assert!(!decision.trace.is_empty());
    }
}
