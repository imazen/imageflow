//! Pure-function codec selection logic.
//!
//! `CodecDecision` selects output formats and encoder implementations based on
//! image characteristics, user intent, and available codecs — with no `Context`
//! dependency. Every decision is traced via structured [`SelectionStep`] entries
//! for auditability and debugging.
//!
//! Design principles:
//! - **Format selection**: preference-ordered candidate list; every candidate
//!   checked for both `AllowedFormats` permission AND encoder availability.
//! - **Encoder selection**: first capable encoder in priority order wins.
//!   Rank is recorded in the trace but does not override priority.
//! - **Security**: [`CodecDecision::is_encoder_enabled`] gates instantiation.
//! - **Typed config**: [`EncoderConfig`] is a per-format enum, not a god-struct.
//!
//! Wire in by replacing `CodecSelector` (mod.rs) and
//! `format_select_with_specified` (auto.rs).

use super::{EnabledCodecs, EncoderCaps, NamedEncoders};
use imageflow_types::{AllowedFormats, Color, ImageFormat, PngBitDepth, QualityProfile};
use std::fmt;

// ── Structured trace ─────────────────────────────────────────────────────

/// A single step in the codec selection decision process.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectionStep {
    /// A format was selected.
    FormatChosen { format: ImageFormat, reason: &'static str },
    /// A format was considered but rejected.
    FormatSkipped { format: ImageFormat, reason: &'static str },
    /// An encoder was selected.
    EncoderChosen { encoder: NamedEncoders, rank: Option<u8>, reason: &'static str },
    /// An encoder was considered but rejected.
    EncoderSkipped { encoder: NamedEncoders, reason: &'static str },
    /// Informational (e.g. "entering animation path").
    Info { message: &'static str },
    /// Terminal: no viable option found.
    NoResult { reason: &'static str },
}

impl fmt::Display for SelectionStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FormatChosen { format, reason } => {
                write!(f, "[chosen] {:?}: {}", format, reason)
            }
            Self::FormatSkipped { format, reason } => {
                write!(f, "[skip]   {:?}: {}", format, reason)
            }
            Self::EncoderChosen { encoder, rank, reason } => {
                write!(f, "[chosen] {:?} (rank {:?}): {}", encoder, rank, reason)
            }
            Self::EncoderSkipped { encoder, reason } => {
                write!(f, "[skip]   {:?}: {}", encoder, reason)
            }
            Self::Info { message } => write!(f, "[info]   {}", message),
            Self::NoResult { reason } => write!(f, "[none]   {}", reason),
        }
    }
}

// ── Decision result ──────────────────────────────────────────────────────

/// The outcome of a selection decision, with structured trace.
#[derive(Debug, Clone)]
pub struct Decision<T: fmt::Debug + Clone> {
    pub chosen: T,
    pub trace: Vec<SelectionStep>,
}

impl<T: fmt::Debug + Clone> Decision<T> {
    /// Human-readable trace summary, one line per step.
    pub fn trace_summary(&self) -> String {
        self.trace.iter().map(|s| s.to_string()).collect::<Vec<_>>().join("\n")
    }

    /// True if the trace contains a step matching the predicate.
    pub fn trace_contains(&self, pred: impl Fn(&SelectionStep) -> bool) -> bool {
        self.trace.iter().any(pred)
    }
}

/// Combined format + encoder decision.
#[derive(Debug, Clone)]
pub struct FormatAndEncoder {
    pub format: ImageFormat,
    pub encoder: NamedEncoders,
    pub trace: Vec<SelectionStep>,
}

impl FormatAndEncoder {
    pub fn trace_summary(&self) -> String {
        self.trace.iter().map(|s| s.to_string()).collect::<Vec<_>>().join("\n")
    }
}

// ── Input types ──────────────────────────────────────────────────────────

/// Observable facts about the image being encoded.
/// Trivially constructible in tests — no Context dependency.
#[derive(Debug, Clone)]
pub struct ImageFacts {
    pub has_alpha: bool,
    pub has_animation: bool,
    pub pixel_count: u64,
    pub source_format: Option<ImageFormat>,
    pub source_lossless: Option<bool>,
}

impl Default for ImageFacts {
    fn default() -> Self {
        Self {
            has_alpha: false,
            has_animation: false,
            pixel_count: 0,
            source_format: None,
            source_lossless: None,
        }
    }
}

/// What the caller wants. Separate from image facts.
#[derive(Debug, Clone)]
pub struct EncodingIntent {
    /// Which formats the request permits (already expanded from set shorthands).
    pub allowed: AllowedFormats,
    /// Explicit lossless/lossy preference.  `None` = auto-decide.
    pub lossless: Option<bool>,
    /// Quality profile for per-codec quality mapping.
    pub quality_profile: Option<QualityProfile>,
    /// Pre-resolved format choice (`Keep` already resolved to source format).
    /// If `Some`, used when an encoder exists; otherwise falls back to auto.
    pub specified_format: Option<ImageFormat>,
}

impl Default for EncodingIntent {
    fn default() -> Self {
        Self {
            allowed: AllowedFormats::web_safe(),
            lossless: None,
            quality_profile: None,
            specified_format: None,
        }
    }
}

// ── Typed encoder config ─────────────────────────────────────────────────

/// Per-format encoder configuration.  Each variant carries only the fields
/// relevant to that format — mismatched fields are a type error, not a
/// silently-ignored `Option`.
#[derive(Debug, Clone)]
pub enum EncoderConfig {
    Jpeg {
        quality: u8,
        progressive: bool,
        /// Use libjpeg-turbo-compatible mode.  When false, use advanced mozjpeg.
        classic: bool,
        optimize_huffman: bool,
        matte: Option<Color>,
    },
    Png(PngConfig),
    WebP {
        /// Lossy quality 0–100.  `None` for lossless.
        quality: Option<f32>,
        lossless: bool,
        matte: Option<Color>,
    },
    Gif,
    Jxl {
        /// Butteraugli distance.  `None` for lossless.
        distance: Option<f32>,
        lossless: bool,
    },
    Avif {
        quality: f32,
        speed: u8,
        lossless: bool,
        matte: Option<Color>,
    },
}

/// PNG has three distinct encoding strategies.
#[derive(Debug, Clone)]
pub enum PngConfig {
    Lossless {
        max_deflate: Option<bool>,
        matte: Option<Color>,
    },
    Quantized {
        speed: u8,
        target_quality: u8,
        min_quality: u8,
        max_deflate: Option<bool>,
        matte: Option<Color>,
    },
    LibPng {
        depth: Option<PngBitDepth>,
        matte: Option<Color>,
        zlib_compression: Option<u8>,
    },
}

impl EncoderConfig {
    /// The format this config is for.
    pub fn format(&self) -> ImageFormat {
        match self {
            Self::Jpeg { .. } => ImageFormat::Jpeg,
            Self::Png(_) => ImageFormat::Png,
            Self::WebP { .. } => ImageFormat::Webp,
            Self::Gif => ImageFormat::Gif,
            Self::Jxl { .. } => ImageFormat::Jxl,
            Self::Avif { .. } => ImageFormat::Avif,
        }
    }

    /// True if this config's format matches the encoder's format.
    pub fn matches_encoder(&self, encoder: NamedEncoders) -> bool {
        self.format() == encoder.caps().format
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Rank for the requested mode, or `None` if the encoder can't do it.
fn rank_for_mode(caps: &EncoderCaps, lossless: bool) -> Option<u8> {
    if lossless {
        if caps.lossless {
            Some(caps.lossless_rank)
        } else {
            None
        }
    } else if caps.lossy {
        Some(caps.lossy_rank)
    } else {
        None
    }
}

/// Check if a format is enabled in `AllowedFormats`.
fn is_format_allowed(fmt: ImageFormat, allowed: &AllowedFormats) -> bool {
    match fmt {
        ImageFormat::Jpeg => allowed.jpeg == Some(true),
        ImageFormat::Png => allowed.png == Some(true),
        ImageFormat::Gif => allowed.gif == Some(true),
        ImageFormat::Webp => allowed.webp == Some(true),
        ImageFormat::Jxl => allowed.jxl == Some(true),
        ImageFormat::Avif => allowed.avif == Some(true),
        _ => false,
    }
}

/// Format preference order for static (non-animated) images.
/// Each entry: `(format, reason)` in descending preference.
fn format_preference_order(
    facts: &ImageFacts,
    want_lossless: bool,
) -> &'static [(ImageFormat, &'static str)] {
    use ImageFormat::*;

    if want_lossless {
        &[
            (Jxl, "best lossless compression"),
            (Webp, "good lossless compression"),
            (Png, "universal lossless"),
            (Avif, "lossless available"),
        ]
    } else if facts.has_alpha {
        &[
            (Jxl, "best alpha compression"),
            (Avif, "excellent lossy alpha"),
            (Webp, "good lossy alpha"),
            (Png, "alpha fallback"),
        ]
    } else if facts.pixel_count < 3_000_000 {
        &[
            (Jxl, "best compression"),
            (Avif, "excellent for small images"),
            (Jpeg, "universal lossy"),
            (Webp, "lossy fallback"),
            (Png, "last resort"),
        ]
    } else {
        &[
            (Jxl, "best compression"),
            (Jpeg, "fast universal lossy"),
            (Avif, "good but slow for large images"),
            (Webp, "lossy fallback"),
            (Png, "last resort"),
        ]
    }
}

/// Format candidates for animated images.
fn animated_format_candidates(want_lossless: bool) -> &'static [(ImageFormat, &'static str)] {
    use ImageFormat::*;

    if want_lossless {
        &[(Webp, "animated lossless"), (Gif, "animated lossless fallback")]
    } else {
        &[
            (Avif, "best animated compression"),
            (Webp, "animated encoding"),
            (Gif, "animated fallback"),
        ]
    }
}

// ── The selector ─────────────────────────────────────────────────────────

/// Pure-function codec decision engine.  No `Context` dependency.
pub struct CodecDecision<'a> {
    codecs: &'a EnabledCodecs,
}

impl<'a> CodecDecision<'a> {
    pub fn new(codecs: &'a EnabledCodecs) -> Self {
        Self { codecs }
    }

    // ── Format selection ─────────────────────────────────────────────

    /// Select output format based on image facts and user intent.
    pub fn select_format(
        &self,
        facts: &ImageFacts,
        intent: &EncodingIntent,
    ) -> Option<Decision<ImageFormat>> {
        let mut trace = Vec::new();

        // Honor explicit format if any encoder exists for it
        if let Some(fmt) = intent.specified_format {
            if self.has_any_encoder(fmt) {
                trace.push(SelectionStep::FormatChosen {
                    format: fmt,
                    reason: "explicitly specified",
                });
                return Some(Decision { chosen: fmt, trace });
            }
            trace.push(SelectionStep::FormatSkipped {
                format: fmt,
                reason: "specified but no encoder available",
            });
        }

        if !intent.allowed.any_formats_enabled() {
            trace.push(SelectionStep::NoResult { reason: "no formats enabled in allowed set" });
            return None;
        }

        let want_lossless = intent.lossless == Some(true) || facts.source_lossless == Some(true);

        if facts.has_animation {
            trace.push(SelectionStep::Info { message: "animation path" });
            return self.select_animated_format(&intent.allowed, want_lossless, trace);
        }

        // Static image: iterate preference list
        let candidates = format_preference_order(facts, want_lossless);
        for &(fmt, reason) in candidates {
            if !is_format_allowed(fmt, &intent.allowed) {
                trace.push(SelectionStep::FormatSkipped { format: fmt, reason: "not allowed" });
                continue;
            }
            if !self.has_any_encoder(fmt) {
                trace.push(SelectionStep::FormatSkipped {
                    format: fmt,
                    reason: "no encoder available",
                });
                continue;
            }
            trace.push(SelectionStep::FormatChosen { format: fmt, reason });
            return Some(Decision { chosen: fmt, trace });
        }

        trace.push(SelectionStep::NoResult { reason: "no suitable format found" });
        None
    }

    // ── Encoder selection ────────────────────────────────────────────

    /// Select the best encoder for a format+mode.
    /// **First capable encoder in priority order wins.**
    /// Rank is recorded in the trace but does not override priority.
    pub fn select_encoder(
        &self,
        format: ImageFormat,
        lossless: bool,
    ) -> Option<Decision<NamedEncoders>> {
        let mut trace = Vec::new();

        for &enc in &self.codecs.encoders {
            let caps = enc.caps();
            if caps.format != format {
                continue;
            }
            match rank_for_mode(&caps, lossless) {
                Some(rank) => {
                    trace.push(SelectionStep::EncoderChosen {
                        encoder: enc,
                        rank: Some(rank),
                        reason: "first capable in priority order",
                    });
                    return Some(Decision { chosen: enc, trace });
                }
                None => {
                    trace.push(SelectionStep::EncoderSkipped {
                        encoder: enc,
                        reason: if lossless { "no lossless support" } else { "no lossy support" },
                    });
                }
            }
        }

        trace.push(SelectionStep::NoResult {
            reason: if lossless {
                "no lossless encoder available"
            } else {
                "no lossy encoder available"
            },
        });
        None
    }

    // ── Combined selection ───────────────────────────────────────────

    /// Select format, then select encoder.  If the preferred mode has no
    /// encoder, gracefully falls back to the opposite mode.
    pub fn select(&self, facts: &ImageFacts, intent: &EncodingIntent) -> Option<FormatAndEncoder> {
        let fmt_decision = self.select_format(facts, intent)?;
        let want_lossless = intent.lossless == Some(true) || facts.source_lossless == Some(true);

        // Try preferred mode first
        if let Some(enc_decision) = self.select_encoder(fmt_decision.chosen, want_lossless) {
            let mut trace = fmt_decision.trace;
            trace.extend(enc_decision.trace);
            return Some(FormatAndEncoder {
                format: fmt_decision.chosen,
                encoder: enc_decision.chosen,
                trace,
            });
        }

        // Graceful mode fallback
        let mut trace = fmt_decision.trace;
        trace.push(SelectionStep::Info {
            message: if want_lossless {
                "no lossless encoder, trying lossy"
            } else {
                "no lossy encoder, trying lossless"
            },
        });
        let enc_decision = self.select_encoder(fmt_decision.chosen, !want_lossless)?;
        trace.extend(enc_decision.trace);
        Some(FormatAndEncoder { format: fmt_decision.chosen, encoder: enc_decision.chosen, trace })
    }

    // ── Security ─────────────────────────────────────────────────────

    /// Check that an encoder is in the enabled list.
    /// Call this before instantiation to enforce the security policy.
    pub fn is_encoder_enabled(&self, encoder: NamedEncoders) -> bool {
        self.codecs.has_encoder(encoder)
    }

    // ── Internal ─────────────────────────────────────────────────────

    fn select_animated_format(
        &self,
        allowed: &AllowedFormats,
        want_lossless: bool,
        mut trace: Vec<SelectionStep>,
    ) -> Option<Decision<ImageFormat>> {
        let candidates = animated_format_candidates(want_lossless);

        for &(fmt, reason) in candidates {
            if !is_format_allowed(fmt, allowed) {
                trace.push(SelectionStep::FormatSkipped { format: fmt, reason: "not allowed" });
                continue;
            }
            if !self.codecs.format_supports_animation(fmt) {
                trace.push(SelectionStep::FormatSkipped {
                    format: fmt,
                    reason: "encoder lacks animation support",
                });
                continue;
            }
            if !self.has_any_encoder(fmt) {
                trace.push(SelectionStep::FormatSkipped {
                    format: fmt,
                    reason: "no encoder available",
                });
                continue;
            }
            trace.push(SelectionStep::FormatChosen { format: fmt, reason });
            return Some(Decision { chosen: fmt, trace });
        }

        trace.push(SelectionStep::NoResult { reason: "no animated format available" });
        None
    }

    fn has_any_encoder(&self, format: ImageFormat) -> bool {
        self.codecs.has_encoder_for_format(format)
    }
}

// ══════════════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use imageflow_types::AllowedFormats;

    // ── Helpers ──────────────────────────────────────────────────────

    fn codecs(encoders: &[NamedEncoders]) -> EnabledCodecs {
        EnabledCodecs {
            decoders: smallvec::SmallVec::new(),
            encoders: smallvec::SmallVec::from_slice(encoders),
        }
    }

    /// All zen encoders — covers every format.
    fn all_zen() -> EnabledCodecs {
        codecs(&[
            NamedEncoders::ZenJpegEncoder,
            NamedEncoders::ZenWebPEncoder,
            NamedEncoders::ZenGifEncoder,
            NamedEncoders::ZenJxlEncoder,
            NamedEncoders::ZenAvifEncoder,
            NamedEncoders::PngQuantEncoder,
            NamedEncoders::LodePngEncoder,
        ])
    }

    /// C-only encoders (no JXL, AVIF).
    fn c_only() -> EnabledCodecs {
        codecs(&[
            NamedEncoders::MozJpegEncoder,
            NamedEncoders::LibPngRsEncoder,
            NamedEncoders::WebPEncoder,
            NamedEncoders::PngQuantEncoder,
            NamedEncoders::LodePngEncoder,
            NamedEncoders::GifEncoder,
        ])
    }

    /// Bare minimum: just JPEG + PNG.
    fn jpeg_png() -> EnabledCodecs {
        codecs(&[NamedEncoders::MozJpegEncoder, NamedEncoders::LodePngEncoder])
    }

    fn decide(c: &EnabledCodecs) -> CodecDecision<'_> {
        CodecDecision::new(c)
    }

    fn web_safe() -> AllowedFormats {
        AllowedFormats::web_safe()
    }

    fn all_allowed() -> AllowedFormats {
        AllowedFormats::all()
    }

    // ── Format selection: basic paths ────────────────────────────────

    #[test]
    fn no_formats_enabled_returns_none() {
        let c = all_zen();
        let result = decide(&c).select_format(
            &ImageFacts::default(),
            &EncodingIntent { allowed: AllowedFormats::none(), ..Default::default() },
        );
        assert!(result.is_none());
    }

    #[test]
    fn specified_format_honored_when_encoder_exists() {
        let c = all_zen();
        let result = decide(&c)
            .select_format(
                &ImageFacts::default(),
                &EncodingIntent { specified_format: Some(ImageFormat::Avif), ..Default::default() },
            )
            .unwrap();
        assert_eq!(result.chosen, ImageFormat::Avif);
        assert!(result.trace_contains(|s| matches!(
            s,
            SelectionStep::FormatChosen {
                format: ImageFormat::Avif,
                reason: "explicitly specified",
            }
        )));
    }

    #[test]
    fn specified_format_falls_back_when_no_encoder() {
        let c = jpeg_png(); // no AVIF encoder
        let result = decide(&c)
            .select_format(
                &ImageFacts::default(),
                &EncodingIntent {
                    allowed: web_safe(),
                    specified_format: Some(ImageFormat::Avif),
                    ..Default::default()
                },
            )
            .unwrap();
        // Falls back to auto — JPEG for lossy opaque
        assert_eq!(result.chosen, ImageFormat::Jpeg);
        assert!(result.trace_contains(|s| matches!(
            s,
            SelectionStep::FormatSkipped {
                format: ImageFormat::Avif,
                reason: "specified but no encoder available",
            }
        )));
    }

    // ── Format selection: JXL priority ───────────────────────────────

    #[test]
    fn jxl_wins_when_available_and_allowed() {
        let c = all_zen();
        let result = decide(&c)
            .select_format(
                &ImageFacts::default(),
                &EncodingIntent { allowed: all_allowed(), ..Default::default() },
            )
            .unwrap();
        assert_eq!(result.chosen, ImageFormat::Jxl);
    }

    #[test]
    fn jxl_skipped_when_not_allowed() {
        let c = all_zen();
        let mut allowed = all_allowed();
        allowed.jxl = None;
        let result = decide(&c)
            .select_format(
                &ImageFacts::default(),
                &EncodingIntent { allowed, ..Default::default() },
            )
            .unwrap();
        assert_ne!(result.chosen, ImageFormat::Jxl);
    }

    #[test]
    fn jxl_skipped_when_no_encoder() {
        let c = c_only(); // no JXL encoder
        let result = decide(&c)
            .select_format(
                &ImageFacts::default(),
                &EncodingIntent { allowed: all_allowed(), ..Default::default() },
            )
            .unwrap();
        assert_ne!(result.chosen, ImageFormat::Jxl);
    }

    // ── Format selection: lossless ───────────────────────────────────

    #[test]
    fn lossless_preference_order_without_jxl() {
        let c = c_only(); // has WebP + PNG, no JXL
        let mut allowed = all_allowed();
        allowed.jxl = None;
        let result = decide(&c)
            .select_format(
                &ImageFacts::default(),
                &EncodingIntent { allowed, lossless: Some(true), ..Default::default() },
            )
            .unwrap();
        assert_eq!(result.chosen, ImageFormat::Webp);
    }

    #[test]
    fn lossless_falls_to_png_without_webp() {
        let c = jpeg_png();
        let result = decide(&c)
            .select_format(
                &ImageFacts::default(),
                &EncodingIntent { allowed: web_safe(), lossless: Some(true), ..Default::default() },
            )
            .unwrap();
        assert_eq!(result.chosen, ImageFormat::Png);
    }

    #[test]
    fn source_lossless_triggers_lossless_path() {
        let c = c_only();
        let mut allowed = all_allowed();
        allowed.jxl = None;
        let result = decide(&c)
            .select_format(
                &ImageFacts { source_lossless: Some(true), ..Default::default() },
                &EncodingIntent { allowed, ..Default::default() },
            )
            .unwrap();
        // Lossless path: WebP preferred over PNG
        assert_eq!(result.chosen, ImageFormat::Webp);
    }

    // ── Format selection: alpha ──────────────────────────────────────

    #[test]
    fn alpha_prefers_avif_when_available() {
        let c = all_zen();
        let mut allowed = all_allowed();
        allowed.jxl = None; // skip JXL to test alpha path
        let result = decide(&c)
            .select_format(
                &ImageFacts { has_alpha: true, ..Default::default() },
                &EncodingIntent { allowed, ..Default::default() },
            )
            .unwrap();
        assert_eq!(result.chosen, ImageFormat::Avif);
    }

    #[test]
    fn alpha_falls_to_webp_without_avif() {
        let c = c_only();
        let mut allowed = all_allowed();
        allowed.jxl = None;
        allowed.avif = None;
        let result = decide(&c)
            .select_format(
                &ImageFacts { has_alpha: true, ..Default::default() },
                &EncodingIntent { allowed, ..Default::default() },
            )
            .unwrap();
        assert_eq!(result.chosen, ImageFormat::Webp);
    }

    #[test]
    fn alpha_falls_to_png_as_last_alpha_format() {
        let c = jpeg_png();
        let result = decide(&c)
            .select_format(
                &ImageFacts { has_alpha: true, ..Default::default() },
                &EncodingIntent { allowed: web_safe(), ..Default::default() },
            )
            .unwrap();
        assert_eq!(result.chosen, ImageFormat::Png);
    }

    // ── Format selection: lossy opaque pixel-count threshold ─────────

    #[test]
    fn small_lossy_opaque_prefers_avif() {
        let c = all_zen();
        let mut allowed = all_allowed();
        allowed.jxl = None;
        let result = decide(&c)
            .select_format(
                &ImageFacts { pixel_count: 1_000_000, ..Default::default() },
                &EncodingIntent { allowed, ..Default::default() },
            )
            .unwrap();
        assert_eq!(result.chosen, ImageFormat::Avif);
    }

    #[test]
    fn large_lossy_opaque_prefers_jpeg() {
        let c = all_zen();
        let mut allowed = all_allowed();
        allowed.jxl = None;
        let result = decide(&c)
            .select_format(
                &ImageFacts { pixel_count: 5_000_000, ..Default::default() },
                &EncodingIntent { allowed, ..Default::default() },
            )
            .unwrap();
        assert_eq!(result.chosen, ImageFormat::Jpeg);
    }

    #[test]
    fn web_safe_opaque_selects_jpeg() {
        let c = c_only();
        let result = decide(&c)
            .select_format(
                &ImageFacts::default(),
                &EncodingIntent { allowed: web_safe(), ..Default::default() },
            )
            .unwrap();
        assert_eq!(result.chosen, ImageFormat::Jpeg);
    }

    // ── Format selection: animation ──────────────────────────────────

    #[test]
    fn animated_lossy_gif_fallback_without_animated_encoders() {
        let c = c_only(); // C WebP/GIF — GIF has animation
        let result = decide(&c)
            .select_format(
                &ImageFacts { has_animation: true, ..Default::default() },
                &EncodingIntent { allowed: all_allowed(), ..Default::default() },
            )
            .unwrap();
        // C WebP doesn't have animation caps, so GIF wins
        assert_eq!(result.chosen, ImageFormat::Gif);
    }

    #[test]
    fn animated_returns_none_when_gif_not_allowed_and_no_animated_encoder() {
        let c = jpeg_png(); // no GIF, no animated encoder
        let mut allowed = web_safe();
        allowed.gif = None;
        let result = decide(&c).select_format(
            &ImageFacts { has_animation: true, ..Default::default() },
            &EncodingIntent { allowed, ..Default::default() },
        );
        assert!(result.is_none());
    }

    // ── Format selection: encoder availability checked for ALL formats ─

    #[test]
    fn format_allowed_but_no_encoder_is_skipped() {
        let c = codecs(&[NamedEncoders::MozJpegEncoder]); // only JPEG
        let result = decide(&c)
            .select_format(
                &ImageFacts::default(),
                &EncodingIntent { allowed: all_allowed(), ..Default::default() },
            )
            .unwrap();
        // JXL, AVIF — allowed but no encoder. Should land on JPEG.
        assert_eq!(result.chosen, ImageFormat::Jpeg);
        assert!(result.trace_contains(|s| matches!(
            s,
            SelectionStep::FormatSkipped {
                format: ImageFormat::Jxl,
                reason: "no encoder available",
            }
        )));
    }

    // ── Encoder selection ────────────────────────────────────────────

    #[test]
    fn first_capable_encoder_wins() {
        let c = codecs(&[NamedEncoders::MozJpegEncoder, NamedEncoders::ZenJpegEncoder]);
        let result = decide(&c).select_encoder(ImageFormat::Jpeg, false).unwrap();
        assert_eq!(result.chosen, NamedEncoders::MozJpegEncoder);
    }

    #[test]
    fn priority_order_determines_winner() {
        // Reverse order → ZenJpeg wins
        let c = codecs(&[NamedEncoders::ZenJpegEncoder, NamedEncoders::MozJpegEncoder]);
        let result = decide(&c).select_encoder(ImageFormat::Jpeg, false).unwrap();
        assert_eq!(result.chosen, NamedEncoders::ZenJpegEncoder);
    }

    #[test]
    fn encoder_wrong_format_skipped() {
        let c = codecs(&[NamedEncoders::MozJpegEncoder, NamedEncoders::LodePngEncoder]);
        let result = decide(&c).select_encoder(ImageFormat::Png, true).unwrap();
        assert_eq!(result.chosen, NamedEncoders::LodePngEncoder);
    }

    #[test]
    fn encoder_wrong_mode_skipped() {
        // MozJpeg is lossy-only
        let c = codecs(&[NamedEncoders::MozJpegEncoder]);
        let result = decide(&c).select_encoder(ImageFormat::Jpeg, true);
        assert!(result.is_none());
    }

    #[test]
    fn encoder_none_for_missing_format() {
        let c = codecs(&[NamedEncoders::MozJpegEncoder]);
        let result = decide(&c).select_encoder(ImageFormat::Png, false);
        assert!(result.is_none());
    }

    #[test]
    fn encoder_empty_codecs_returns_none() {
        let c = codecs(&[]);
        let result = decide(&c).select_encoder(ImageFormat::Jpeg, false);
        assert!(result.is_none());
    }

    #[test]
    fn lossy_png_selects_pngquant_over_lodepng() {
        let c = codecs(&[
            NamedEncoders::LodePngEncoder,  // lossless-only
            NamedEncoders::PngQuantEncoder, // lossy
        ]);
        let result = decide(&c).select_encoder(ImageFormat::Png, false).unwrap();
        assert_eq!(result.chosen, NamedEncoders::PngQuantEncoder);
        // LodePng should be in the trace as skipped
        assert!(result.trace_contains(|s| matches!(
            s,
            SelectionStep::EncoderSkipped { encoder: NamedEncoders::LodePngEncoder, .. }
        )));
    }

    // ── Combined selection ───────────────────────────────────────────

    #[test]
    fn combined_select_format_and_encoder() {
        let c = c_only();
        let result = decide(&c)
            .select(
                &ImageFacts::default(),
                &EncodingIntent { allowed: web_safe(), ..Default::default() },
            )
            .unwrap();
        assert_eq!(result.format, ImageFormat::Jpeg);
        assert_eq!(result.encoder, NamedEncoders::MozJpegEncoder);
    }

    #[test]
    fn combined_select_none_when_no_encoder_for_any_format() {
        let c = codecs(&[]);
        let result = decide(&c).select(
            &ImageFacts::default(),
            &EncodingIntent { allowed: web_safe(), ..Default::default() },
        );
        assert!(result.is_none());
    }

    #[test]
    fn combined_graceful_mode_fallback() {
        // Only lossless PNG encoder, but user asks for lossy
        let c = codecs(&[NamedEncoders::LodePngEncoder]);
        let mut allowed = AllowedFormats::none();
        allowed.png = Some(true);
        let result = decide(&c)
            .select(
                &ImageFacts::default(),
                &EncodingIntent { allowed, lossless: Some(false), ..Default::default() },
            )
            .unwrap();
        // Should gracefully fall back from lossy→lossless
        assert_eq!(result.format, ImageFormat::Png);
        assert_eq!(result.encoder, NamedEncoders::LodePngEncoder);
        // Trace should record the fallback
        assert!(result.trace.iter().any(|s| matches!(
            s,
            SelectionStep::Info { message: "no lossy encoder, trying lossless" }
        )));
    }

    // ── Trace ────────────────────────────────────────────────────────

    #[test]
    fn trace_summary_is_readable() {
        let c = all_zen();
        let result = decide(&c)
            .select_format(
                &ImageFacts::default(),
                &EncodingIntent { allowed: all_allowed(), ..Default::default() },
            )
            .unwrap();
        let summary = result.trace_summary();
        assert!(summary.contains("[chosen]"));
        assert!(summary.contains("Jxl"));
    }

    #[test]
    fn trace_records_all_skipped_formats() {
        // Only JPEG encoder — all higher-preference formats should be skipped
        let c = codecs(&[NamedEncoders::MozJpegEncoder]);
        let result = decide(&c)
            .select_format(
                &ImageFacts::default(),
                &EncodingIntent { allowed: all_allowed(), ..Default::default() },
            )
            .unwrap();
        assert_eq!(result.chosen, ImageFormat::Jpeg);
        // JXL and AVIF (at minimum) should be skipped before JPEG
        let skipped_count = result
            .trace
            .iter()
            .filter(|s| matches!(s, SelectionStep::FormatSkipped { .. }))
            .count();
        assert!(
            skipped_count >= 2,
            "expected at least 2 skipped formats, got {}: {:?}",
            skipped_count,
            result.trace
        );
    }

    #[test]
    fn encoder_trace_records_rank() {
        let c = codecs(&[NamedEncoders::ZenJpegEncoder]);
        let result = decide(&c).select_encoder(ImageFormat::Jpeg, false).unwrap();
        assert!(result.trace_contains(|s| matches!(
            s,
            SelectionStep::EncoderChosen {
                encoder: NamedEncoders::ZenJpegEncoder,
                rank: Some(3),
                ..
            }
        )));
    }

    // ── EncoderConfig ────────────────────────────────────────────────

    #[test]
    fn encoder_config_format_matches_variant() {
        assert_eq!(
            EncoderConfig::Jpeg {
                quality: 90,
                progressive: true,
                classic: false,
                optimize_huffman: false,
                matte: None,
            }
            .format(),
            ImageFormat::Jpeg
        );
        assert_eq!(
            EncoderConfig::Png(PngConfig::Lossless { max_deflate: None, matte: None }).format(),
            ImageFormat::Png
        );
        assert_eq!(
            EncoderConfig::WebP { quality: Some(80.0), lossless: false, matte: None }.format(),
            ImageFormat::Webp
        );
        assert_eq!(EncoderConfig::Gif.format(), ImageFormat::Gif);
        assert_eq!(
            EncoderConfig::Jxl { distance: Some(1.0), lossless: false }.format(),
            ImageFormat::Jxl
        );
        assert_eq!(
            EncoderConfig::Avif { quality: 60.0, speed: 6, lossless: false, matte: None }.format(),
            ImageFormat::Avif
        );
    }

    #[test]
    fn encoder_config_matches_correct_encoder() {
        let cfg = EncoderConfig::Jpeg {
            quality: 90,
            progressive: true,
            classic: false,
            optimize_huffman: false,
            matte: None,
        };
        assert!(cfg.matches_encoder(NamedEncoders::MozJpegEncoder));
        assert!(cfg.matches_encoder(NamedEncoders::ZenJpegEncoder));
        assert!(!cfg.matches_encoder(NamedEncoders::LodePngEncoder));
        assert!(!cfg.matches_encoder(NamedEncoders::ZenWebPEncoder));
    }

    // ── Security ─────────────────────────────────────────────────────

    #[test]
    fn is_encoder_enabled_respects_codecs_list() {
        let c = codecs(&[NamedEncoders::MozJpegEncoder, NamedEncoders::LodePngEncoder]);
        let d = decide(&c);
        assert!(d.is_encoder_enabled(NamedEncoders::MozJpegEncoder));
        assert!(d.is_encoder_enabled(NamedEncoders::LodePngEncoder));
        assert!(!d.is_encoder_enabled(NamedEncoders::ZenJpegEncoder));
        assert!(!d.is_encoder_enabled(NamedEncoders::ZenAvifEncoder));
        assert!(!d.is_encoder_enabled(NamedEncoders::WebPEncoder));
    }

    #[test]
    fn empty_codecs_nothing_enabled() {
        let c = codecs(&[]);
        let d = decide(&c);
        assert!(!d.is_encoder_enabled(NamedEncoders::MozJpegEncoder));
        assert!(!d.is_encoder_enabled(NamedEncoders::GifEncoder));
    }

    // ── Capability consistency ───────────────────────────────────────

    #[test]
    fn rank_for_mode_returns_none_when_unsupported() {
        let caps = NamedEncoders::MozJpegEncoder.caps();
        assert!(rank_for_mode(&caps, false).is_some()); // lossy: yes
        assert!(rank_for_mode(&caps, true).is_none()); // lossless: no

        let caps = NamedEncoders::LodePngEncoder.caps();
        assert!(rank_for_mode(&caps, false).is_none()); // lossy: no
        assert!(rank_for_mode(&caps, true).is_some()); // lossless: yes
    }

    #[test]
    fn all_encoder_caps_have_positive_rank_for_supported_modes() {
        let all_encoders = [
            NamedEncoders::MozJpegEncoder,
            NamedEncoders::ZenJpegEncoder,
            NamedEncoders::WebPEncoder,
            NamedEncoders::ZenWebPEncoder,
            NamedEncoders::GifEncoder,
            NamedEncoders::ZenGifEncoder,
            NamedEncoders::ZenJxlEncoder,
            NamedEncoders::ZenAvifEncoder,
            NamedEncoders::PngQuantEncoder,
            NamedEncoders::LodePngEncoder,
            NamedEncoders::LibPngRsEncoder,
        ];
        for enc in &all_encoders {
            let caps = enc.caps();
            if caps.lossy {
                assert!(caps.lossy_rank > 0, "{:?} has lossy=true but lossy_rank=0", enc);
            }
            if caps.lossless {
                assert!(caps.lossless_rank > 0, "{:?} has lossless=true but lossless_rank=0", enc);
            }
            // At least one mode must be supported
            assert!(caps.lossy || caps.lossless, "{:?} supports neither lossy nor lossless", enc);
        }
    }

    // ── is_format_allowed ────────────────────────────────────────────

    #[test]
    fn is_format_allowed_respects_option_bool() {
        let allowed = web_safe(); // jpeg, png, gif = Some(true); rest = None
        assert!(is_format_allowed(ImageFormat::Jpeg, &allowed));
        assert!(is_format_allowed(ImageFormat::Png, &allowed));
        assert!(is_format_allowed(ImageFormat::Gif, &allowed));
        assert!(!is_format_allowed(ImageFormat::Webp, &allowed));
        assert!(!is_format_allowed(ImageFormat::Jxl, &allowed));
        assert!(!is_format_allowed(ImageFormat::Avif, &allowed));
    }

    #[test]
    fn none_is_not_allowed() {
        let mut allowed = AllowedFormats::none();
        allowed.jpeg = None; // explicitly None, not Some(false)
        assert!(!is_format_allowed(ImageFormat::Jpeg, &allowed));
    }
}
