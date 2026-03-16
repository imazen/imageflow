//! Quality profile resolution and per-codec calibration.
//!
//! The quality system has two tiers:
//!
//! 1. **QualityProfile** — user-facing named presets (`good`, `high`) or numeric
//!    values (`qp=75`) from RIAPI querystrings. Codec-agnostic.
//!
//! 2. **QualityIntent** — resolved quality with DPR adjustment applied.
//!    Provides per-codec quality lookups via calibration tables derived from
//!    SSIM-equivalence benchmarking.
//!
//! Zen codecs consume `generic_quality` directly via their own calibration
//! tables (`with_generic_quality()`). C codecs (mozjpeg, libwebp) receive
//! pre-calibrated native values from the tables here.

use core::fmt;

/// Named quality presets or a direct percentage.
///
/// Parsed from the RIAPI `qp=` parameter. Named presets map to fixed points
/// on a perceptual 0–100 scale; `Percent` passes through directly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QualityProfile {
    Lowest,
    Low,
    MediumLow,
    Medium,
    Good,
    High,
    Highest,
    Lossless,
    Percent(f32),
}

impl QualityProfile {
    /// Parse from a string value (case-insensitive).
    /// Returns `None` if the string is not a recognized profile name or valid number.
    pub fn parse(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "lowest" => Some(Self::Lowest),
            "low" => Some(Self::Low),
            "medium-low" | "mediumlow" => Some(Self::MediumLow),
            "medium" | "med" => Some(Self::Medium),
            "good" | "medium-high" | "mediumhigh" => Some(Self::Good),
            "high" => Some(Self::High),
            "highest" => Some(Self::Highest),
            "lossless" => Some(Self::Lossless),
            v => {
                if let Ok(n) = v.parse::<f32>() {
                    if n.is_finite() {
                        return Some(Self::Percent(n.clamp(0.0, 100.0)));
                    }
                }
                None
            }
        }
    }

    /// Map to the generic 0–100 quality scale.
    pub fn to_generic_quality(self) -> f32 {
        match self {
            Self::Lowest => 15.0,
            Self::Low => 20.0,
            Self::MediumLow => 34.0,
            Self::Medium => 55.0,
            Self::Good => 73.0,
            Self::High => 91.0,
            Self::Highest => 96.0,
            Self::Lossless => 100.0,
            Self::Percent(v) => v.clamp(0.0, 100.0),
        }
    }
}

impl fmt::Display for QualityProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lowest => write!(f, "lowest"),
            Self::Low => write!(f, "low"),
            Self::MediumLow => write!(f, "medium-low"),
            Self::Medium => write!(f, "medium"),
            Self::Good => write!(f, "good"),
            Self::High => write!(f, "high"),
            Self::Highest => write!(f, "highest"),
            Self::Lossless => write!(f, "lossless"),
            Self::Percent(v) => write!(f, "{v:.0}"),
        }
    }
}

// ── Quality intent ──────────────────────────────────────────────────────

/// Resolved quality intent. Computed once, consumed by codec config builders.
///
/// Combines a quality profile with DPR adjustment into a single `generic_quality`
/// float on a codec-agnostic 0–100 scale. Per-codec methods apply calibration
/// tables to convert to native quality values.
#[derive(Debug, Clone, Copy)]
pub struct QualityIntent {
    /// Codec-agnostic quality 0–100, already DPR-adjusted.
    pub generic_quality: f32,
    /// The original profile before DPR adjustment, if any.
    pub profile: Option<QualityProfile>,
    /// The DPR value used for adjustment (`None` or 3.0 = no adjustment).
    pub dpr: Option<f32>,
}

impl Default for QualityIntent {
    fn default() -> Self {
        Self::from_profile(QualityProfile::Good, None)
    }
}

impl QualityIntent {
    /// Resolve a quality profile + optional DPR into a concrete quality.
    pub fn from_profile(profile: QualityProfile, dpr: Option<f32>) -> Self {
        let base_q = profile.to_generic_quality();
        let adjusted = apply_dpr_adjustment(base_q, dpr);
        Self { generic_quality: adjusted, profile: Some(profile), dpr }
    }

    /// Direct numeric quality (no profile, no DPR adjustment).
    pub fn from_value(quality: f32) -> Self {
        Self { generic_quality: quality.clamp(0.0, 100.0), profile: None, dpr: None }
    }

    /// Is this requesting lossless encoding?
    pub fn is_lossless(&self) -> bool {
        self.profile == Some(QualityProfile::Lossless) || self.generic_quality >= 100.0
    }

    // ── Per-codec quality lookups ────────────────────────────────────

    /// Quality value for mozjpeg (0–100).
    pub fn mozjpeg_quality(&self) -> u8 {
        interpolate(self.generic_quality, &MOZJPEG_TABLE).clamp(0.0, 100.0) as u8
    }

    /// Quality value for libwebp (0–100).
    pub fn libwebp_quality(&self) -> f32 {
        interpolate(self.generic_quality, &LIBWEBP_TABLE)
    }

    /// JXL butteraugli distance (lower = better quality).
    pub fn jxl_distance(&self) -> f32 {
        if self.is_lossless() {
            0.0
        } else {
            interpolate(self.generic_quality, &JXL_DISTANCE_TABLE)
        }
    }

    /// AVIF quality for zenavif/rav1e (0–100).
    pub fn avif_quality(&self) -> f32 {
        interpolate(self.generic_quality, &AVIF_QUALITY_TABLE)
    }

    /// PNG quantization quality range (min, max).
    pub fn png_quality_range(&self) -> (u8, u8) {
        let min = interpolate(self.generic_quality, &PNG_MIN_QUALITY_TABLE).clamp(0.0, 100.0) as u8;
        let max = interpolate(self.generic_quality, &PNG_MAX_QUALITY_TABLE).clamp(0.0, 100.0) as u8;
        (min, max.max(min))
    }

    /// AVIF encoder speed (0–10, higher = faster/lower quality).
    pub fn avif_speed(&self) -> u8 {
        interpolate(self.generic_quality, &AVIF_SPEED_TABLE).clamp(0.0, 10.0) as u8
    }

    /// JXL effort level (0–10, higher = slower/better compression).
    pub fn jxl_effort(&self) -> u8 {
        interpolate(self.generic_quality, &JXL_EFFORT_TABLE).clamp(0.0, 10.0) as u8
    }
}

// ── DPR adjustment ──────────────────────────────────────────────────────

/// Apply DPR-based quality adjustment.
///
/// At DPR 3x (baseline), no adjustment. At DPR 1x, each source pixel covers
/// more screen pixels — artifacts are magnified, so we increase quality.
/// At DPR 6x, artifacts are hidden, so we decrease quality.
///
/// Operates in perceptual space (scales distance-from-lossless) rather than
/// linearly in quality units, preventing overshooting at the extremes.
fn apply_dpr_adjustment(base_quality: f32, dpr: Option<f32>) -> f32 {
    match dpr {
        None => base_quality,
        Some(d) if (d - 3.0).abs() < 0.01 => base_quality,
        Some(d) => {
            let d = d.clamp(0.1, 12.0);
            // At DPR 3, factor = 1.0 (no change).
            // At DPR 1, factor = 3.0 (need higher quality).
            // At DPR 6, factor = 0.5 (can lower quality).
            let factor = 3.0 / d;
            let perceptual_distance = 100.0 - base_quality;
            let adjusted_distance = perceptual_distance / factor;
            (100.0 - adjusted_distance).clamp(5.0, 99.0)
        }
    }
}

// ── Calibration tables ──────────────────────────────────────────────────
//
// Each table maps generic_quality → codec-native quality value.
// Interpolation between anchor points is linear.
// Derived from SSIM-equivalence benchmarking.

type CalibrationTable = [(f32, f32)];

fn interpolate(generic_q: f32, table: &CalibrationTable) -> f32 {
    let q = generic_q.clamp(0.0, 100.0);
    if q <= table[0].0 {
        return table[0].1;
    }
    if q >= table[table.len() - 1].0 {
        return table[table.len() - 1].1;
    }
    for window in table.windows(2) {
        let (lo_q, lo_v) = window[0];
        let (hi_q, hi_v) = window[1];
        if q >= lo_q && q <= hi_q {
            let t = (q - lo_q) / (hi_q - lo_q);
            return lo_v + t * (hi_v - lo_v);
        }
    }
    table[table.len() - 1].1
}

// Mozjpeg: generic → mozjpeg quality (0–100).
// Mozjpeg compresses better than libjpeg at low quality, so the mapping
// is slightly concave.
#[rustfmt::skip]
const MOZJPEG_TABLE: [(f32, f32); 12] = [
    (5.0, 5.0), (15.0, 15.0), (20.0, 20.0), (34.0, 34.0),
    (55.0, 57.0), (73.0, 73.0), (80.0, 80.0), (85.0, 85.0),
    (91.0, 91.0), (96.0, 96.0), (99.0, 99.0), (100.0, 100.0),
];

// libwebp: generic → libwebp quality (0–100).
// WebP's quality scale is nonlinear — quality 80 is closer to JPEG 90
// than to JPEG 80.
#[rustfmt::skip]
const LIBWEBP_TABLE: [(f32, f32); 12] = [
    (5.0, 5.0), (15.0, 15.0), (20.0, 20.0), (34.0, 34.0),
    (55.0, 53.0), (73.0, 76.0), (80.0, 82.0), (85.0, 88.0),
    (91.0, 93.0), (96.0, 96.0), (99.0, 99.0), (100.0, 100.0),
];

// JXL: generic → butteraugli distance (INVERSE — lower distance = higher quality).
#[rustfmt::skip]
const JXL_DISTANCE_TABLE: [(f32, f32); 10] = [
    (5.0, 25.0), (15.0, 13.0), (20.0, 7.4), (34.0, 4.3),
    (55.0, 3.92), (73.0, 2.58), (91.0, 1.0), (96.0, 0.5),
    (99.0, 0.1), (100.0, 0.0),
];

// AVIF: generic → native quality (0–100).
// AVIF's quality scale is compressed — quality 55 AVIF ≈ quality 73 JPEG.
#[rustfmt::skip]
const AVIF_QUALITY_TABLE: [(f32, f32); 10] = [
    (5.0, 5.0), (15.0, 23.0), (20.0, 34.0), (34.0, 44.0),
    (55.0, 45.0), (73.0, 55.0), (91.0, 66.0), (96.0, 100.0),
    (99.0, 100.0), (100.0, 100.0),
];

// AVIF speed: generic quality → encoder speed (higher quality → more effort).
#[rustfmt::skip]
const AVIF_SPEED_TABLE: [(f32, f32); 4] = [
    (0.0, 6.0), (73.0, 6.0), (96.0, 6.0), (100.0, 5.0),
];

// JXL effort: generic quality → encoder effort (higher quality → more effort).
#[rustfmt::skip]
const JXL_EFFORT_TABLE: [(f32, f32); 4] = [
    (0.0, 5.0), (73.0, 5.0), (96.0, 7.0), (100.0, 8.0),
];

// PNG min quality: generic → imagequant min_quality.
#[rustfmt::skip]
const PNG_MIN_QUALITY_TABLE: [(f32, f32); 6] = [
    (0.0, 0.0), (34.0, 15.0), (55.0, 30.0),
    (73.0, 50.0), (91.0, 70.0), (100.0, 90.0),
];

// PNG max quality: generic → imagequant target_quality.
#[rustfmt::skip]
const PNG_MAX_QUALITY_TABLE: [(f32, f32); 6] = [
    (0.0, 20.0), (34.0, 50.0), (55.0, 70.0),
    (73.0, 85.0), (91.0, 95.0), (100.0, 100.0),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_profiles_parse() {
        assert_eq!(QualityProfile::parse("good"), Some(QualityProfile::Good));
        assert_eq!(QualityProfile::parse("HIGHEST"), Some(QualityProfile::Highest));
        assert_eq!(QualityProfile::parse("medium-low"), Some(QualityProfile::MediumLow));
        assert_eq!(QualityProfile::parse("med"), Some(QualityProfile::Medium));
        assert_eq!(QualityProfile::parse("75"), Some(QualityProfile::Percent(75.0)));
        assert_eq!(QualityProfile::parse("garbage"), None);
    }

    #[test]
    fn profile_quality_monotonic() {
        let profiles = [
            QualityProfile::Lowest,
            QualityProfile::Low,
            QualityProfile::MediumLow,
            QualityProfile::Medium,
            QualityProfile::Good,
            QualityProfile::High,
            QualityProfile::Highest,
            QualityProfile::Lossless,
        ];
        let qualities: Vec<f32> = profiles.iter().map(|p| p.to_generic_quality()).collect();
        for w in qualities.windows(2) {
            assert!(w[1] > w[0], "quality should be monotonically increasing");
        }
    }

    #[test]
    fn dpr_adjustment_baseline() {
        let intent = QualityIntent::from_profile(QualityProfile::Good, Some(3.0));
        assert!((intent.generic_quality - 73.0).abs() < 0.01);
    }

    #[test]
    fn dpr_adjustment_low_dpr_increases_quality() {
        let base = QualityIntent::from_profile(QualityProfile::Good, None);
        let low_dpr = QualityIntent::from_profile(QualityProfile::Good, Some(1.0));
        assert!(low_dpr.generic_quality > base.generic_quality);
    }

    #[test]
    fn dpr_adjustment_high_dpr_decreases_quality() {
        let base = QualityIntent::from_profile(QualityProfile::Good, None);
        let high_dpr = QualityIntent::from_profile(QualityProfile::Good, Some(6.0));
        assert!(high_dpr.generic_quality < base.generic_quality);
    }

    #[test]
    fn per_codec_quality_sanity() {
        let intent = QualityIntent::from_profile(QualityProfile::Good, None);
        assert!((50..95).contains(&intent.mozjpeg_quality()));
        assert!((50.0..95.0).contains(&intent.libwebp_quality()));
        assert!((0.5..5.0).contains(&intent.jxl_distance()));
        assert!((30.0..80.0).contains(&intent.avif_quality()));
    }

    #[test]
    fn lossless_detection() {
        assert!(QualityIntent::from_profile(QualityProfile::Lossless, None).is_lossless());
        assert!(QualityIntent::from_value(100.0).is_lossless());
        assert!(!QualityIntent::from_profile(QualityProfile::High, None).is_lossless());
    }

    #[test]
    fn interpolation_boundaries() {
        // Below first point
        assert!((interpolate(0.0, &MOZJPEG_TABLE) - 5.0).abs() < 0.01);
        // Above last point
        assert!((interpolate(105.0, &MOZJPEG_TABLE) - 100.0).abs() < 0.01);
        // Exact anchor
        assert!((interpolate(73.0, &MOZJPEG_TABLE) - 73.0).abs() < 0.01);
    }
}
