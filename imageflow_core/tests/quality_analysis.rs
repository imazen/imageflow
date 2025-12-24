//! Quality Calibration Analysis
//!
//! Analyzes calibration measurements to produce quality mapping recommendations.
//!
//! ## Methodology
//!
//! ### Core Principle
//! DSSIM values are only comparable within the same source image. We cannot compare
//! "image A at q80" vs "image B at q80". Instead, we:
//! 1. For each image: find the quality at target condition that achieves baseline DSSIM
//! 2. Aggregate those per-image quality mappings using percentiles
//!
//! ### Analysis Types
//!
//! 1. **Diminishing Returns** (uses 1x-desktop, the most demanding condition)
//!    - For each quality step: measure DSSIM reduction vs file size increase
//!    - Determines upper bounds (quality beyond which improvements are imperceptible)
//!
//! 2. **Quality Mapping** (baseline: 1x-laptop at reference quality)
//!    - Maps reference quality to equivalent quality at other conditions
//!    - Uses forgiveness factor (1.05x) to allow slightly worse DSSIM for smaller files
//!
//! 3. **Gap-Based Polynomial Interpolation**
//!    - For each gap between measured quality values, fit a polynomial
//!    - Cross-validate by skipping points and checking prediction accuracy
//!    - Average adjacent polynomials for smooth transitions
//!
//! ### Key Parameters
//!
//! - **Baseline**: 1x-laptop (PPD=70, ratio=1.0) - typical premium laptop viewing
//! - **Forgiveness**: 1.05 - allows 5% worse DSSIM (imperceptible, enables smaller files)
//! - **Perceptibility thresholds** (for desktop, PPD=40):
//!   - < 0.0003: Imperceptible (visually lossless)
//!   - < 0.0007: Marginal (only A/B comparison reveals)
//!   - < 0.0015: Subtle (barely noticeable)
//!   - < 0.0030: Noticeable (visible on inspection)
//!   - >= 0.0030: Degraded (clearly visible artifacts)
//!
//! ## Running
//!
//! ```bash
//! cargo test --release --package imageflow_core --test quality_analysis -- --nocapture --ignored
//! ```

use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

//=============================================================================
// Configuration Types
//=============================================================================

/// A viewing condition defined by pixel density and display ratio
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewingCondition {
    pub name: &'static str,
    pub ratio: f32, // srcset_multiplier / device_DPPX
    pub ppd: u32,   // pixels per degree (viewing distance dependent)
}

impl ViewingCondition {
    pub const fn new(name: &'static str, ratio: f32, ppd: u32) -> Self {
        Self { name, ratio, ppd }
    }

    /// Check if this condition matches the given ratio and ppd
    pub fn matches(&self, ratio: f32, ppd: u32) -> bool {
        (self.ratio - ratio).abs() < 0.02 && self.ppd == ppd
    }
}

/// Standard viewing conditions for analysis
pub mod conditions {
    use super::ViewingCondition;

    // Native conditions (srcset matches device DPPX)
    pub const NATIVE_DESKTOP: ViewingCondition = ViewingCondition::new("native-desktop", 1.0, 40);
    pub const NATIVE_LAPTOP: ViewingCondition = ViewingCondition::new("native-laptop", 1.0, 70);
    pub const NATIVE_PHONE: ViewingCondition = ViewingCondition::new("native-phone", 1.0, 95);

    // Undersized (browser upscales, artifacts amplified)
    pub const SRCSET_1X_PHONE: ViewingCondition = ViewingCondition::new("1x→phone", 0.33, 95);
    pub const SRCSET_1X_LAPTOP: ViewingCondition = ViewingCondition::new("1x→laptop", 0.67, 70);
    pub const SRCSET_2X_PHONE: ViewingCondition = ViewingCondition::new("2x→phone", 0.67, 95);

    // Oversized (browser downscales, artifacts hidden)
    pub const SRCSET_2X_DESKTOP: ViewingCondition = ViewingCondition::new("2x→desktop", 2.0, 40);
    pub const SRCSET_2X_LAPTOP: ViewingCondition = ViewingCondition::new("2x→laptop", 1.33, 70);
    pub const SRCSET_3X_PHONE: ViewingCondition = ViewingCondition::new("3x→phone", 1.0, 95);

    /// All standard conditions for comprehensive analysis
    pub const ALL: &[ViewingCondition] = &[
        SRCSET_1X_PHONE,
        SRCSET_1X_LAPTOP,
        SRCSET_2X_PHONE,
        NATIVE_DESKTOP,
        NATIVE_LAPTOP,
        NATIVE_PHONE,
        SRCSET_2X_DESKTOP,
    ];

    /// Key conditions for compact tables
    pub const KEY: &[ViewingCondition] = &[
        NATIVE_DESKTOP,
        NATIVE_LAPTOP,
        NATIVE_PHONE,
        SRCSET_2X_DESKTOP,
    ];

    /// Baseline condition for quality mapping
    pub const BASELINE: ViewingCondition = NATIVE_LAPTOP;

    /// Most demanding condition (for diminishing returns analysis)
    pub const DEMANDING: ViewingCondition = NATIVE_DESKTOP;
}

/// DSSIM bucket definition for perceptibility analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DssimBucket {
    Imperceptible, // < 0.0003
    Marginal,      // < 0.0007
    Subtle,        // < 0.0015
    Noticeable,    // < 0.0030
    Degraded,      // >= 0.0030
}

impl DssimBucket {
    pub const THRESHOLDS: &'static [(DssimBucket, f64, &'static str)] = &[
        (DssimBucket::Imperceptible, 0.0003, "IMP"),
        (DssimBucket::Marginal, 0.0007, "MAR"),
        (DssimBucket::Subtle, 0.0015, "SUB"),
        (DssimBucket::Noticeable, 0.0030, "NOT"),
        (DssimBucket::Degraded, f64::INFINITY, "DEG"),
    ];

    pub fn from_dssim(dssim: f64) -> Self {
        for (bucket, threshold, _) in Self::THRESHOLDS {
            if dssim < *threshold {
                return *bucket;
            }
        }
        DssimBucket::Degraded
    }

    pub fn abbrev(&self) -> &'static str {
        for (bucket, _, abbrev) in Self::THRESHOLDS {
            if bucket == self {
                return abbrev;
            }
        }
        "DEG"
    }

    pub fn threshold(&self) -> f64 {
        for (bucket, threshold, _) in Self::THRESHOLDS {
            if bucket == self {
                return *threshold;
            }
        }
        f64::INFINITY
    }
}

/// Configuration for report generation
#[derive(Debug, Clone)]
pub struct ReportConfig {
    /// Include raw data (measured points only)
    pub include_data_only: bool,
    /// Include interpolated values
    pub include_interpolated: bool,
    /// Forgiveness factor for DSSIM matching (1.05 = allow 5% worse)
    pub forgiveness_factor: f64,
    /// Percentiles to report
    pub percentiles: Vec<u32>,
    /// Viewing conditions to include
    pub conditions: Vec<ViewingCondition>,
    /// Quality tiers to report
    pub quality_tiers: Vec<(&'static str, u32)>,
    /// Codecs to analyze
    pub codecs: Vec<&'static str>,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            include_data_only: true,
            include_interpolated: true,
            forgiveness_factor: 1.05,
            percentiles: vec![50, 75, 90, 95],
            conditions: conditions::KEY.to_vec(),
            quality_tiers: vec![
                ("lossless", 100),
                ("highest", 95),
                ("high", 90),
                ("good", 85),
                ("medium", 76),
                ("mediumlow", 65),
                ("low", 50),
                ("lowest", 30),
            ],
            codecs: vec!["mozjpeg", "webp", "avif_s6"],
        }
    }
}

/// Configuration for polynomial interpolation
#[derive(Debug, Clone)]
pub struct InterpolationConfig {
    /// Minimum exponent for power law fitting
    pub min_exponent: f64,
    /// Maximum exponent for power law fitting
    pub max_exponent: f64,
    /// Exponent search step size
    pub exponent_step: f64,
    /// Minimum R² for valid fit
    pub min_r_squared: f64,
}

impl Default for InterpolationConfig {
    fn default() -> Self {
        Self {
            min_exponent: 0.5,
            max_exponent: 3.0,
            exponent_step: 0.1,
            min_r_squared: 0.90,
        }
    }
}

//=============================================================================
// Data Structures
//=============================================================================

/// A single measurement from the calibration run
#[derive(Debug, Clone)]
pub struct Measurement {
    pub image: String,
    pub subdir: String, // Category/subfolder in corpus
    pub codec: String,
    pub quality: u32,
    pub ratio: f32,
    pub ppd: u32,
    pub dssim: f64,
    pub file_size: usize,
}

/// Polynomial coefficients for quality interpolation: q_out = a * q_in^b + c
#[derive(Debug, Clone, Copy)]
pub struct GapPolynomial {
    pub q_low: u32,
    pub q_high: u32,
    pub a: f64,
    pub b: f64, // exponent
    pub c: f64,
    pub r_squared: f64,
    pub validation_error: f64, // Error when predicting skipped point
}

impl GapPolynomial {
    /// Interpolate quality for a given input quality
    pub fn interpolate(&self, q_in: f64) -> f64 {
        (self.a * q_in.powf(self.b) + self.c).clamp(0.0, 100.0)
    }

    /// Check if this polynomial covers the given quality value
    pub fn covers(&self, q: u32) -> bool {
        q >= self.q_low && q <= self.q_high
    }
}

/// Collection of gap polynomials for a specific (codec, condition) pair
#[derive(Debug, Clone)]
pub struct InterpolationTable {
    pub codec: String,
    pub condition: ViewingCondition,
    pub polynomials: Vec<GapPolynomial>,
}

impl InterpolationTable {
    /// Find the polynomial that covers the given quality
    pub fn find_polynomial(&self, q: u32) -> Option<&GapPolynomial> {
        self.polynomials.iter().find(|p| p.covers(q))
    }

    /// Interpolate quality, falling back to linear if no polynomial found
    pub fn interpolate(&self, q_in: f64) -> f64 {
        let q_u32 = q_in.round() as u32;
        if let Some(poly) = self.find_polynomial(q_u32) {
            poly.interpolate(q_in)
        } else {
            q_in // fallback
        }
    }
}

//=============================================================================
// Statistical Helpers (with unit tests)
//=============================================================================

/// Compute median of a slice (sorts in place)
pub fn median(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

/// Compute percentile using linear interpolation (R-7 method)
pub fn percentile(values: &mut [f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pos = p * (values.len() - 1) as f64;
    let lower = pos.floor() as usize;
    let upper = (lower + 1).min(values.len() - 1);
    let frac = pos - lower as f64;
    values[lower] * (1.0 - frac) + values[upper] * frac
}

/// Compute percentile for u32 values
pub fn percentile_u32(values: &mut [u32], p: f64) -> u32 {
    if values.is_empty() {
        return 0;
    }
    values.sort();
    let pos = p * (values.len() - 1) as f64;
    let lower = pos.floor() as usize;
    let upper = (lower + 1).min(values.len() - 1);
    let frac = pos - lower as f64;
    let result = values[lower] as f64 * (1.0 - frac) + values[upper] as f64 * frac;
    result.round() as u32
}

/// Compute arithmetic mean
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Compute trimmed mean (removes top and bottom trim_pct)
pub fn trimmed_mean(values: &mut [f64], trim_pct: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let trim_count = (values.len() as f64 * trim_pct) as usize;
    if trim_count * 2 >= values.len() {
        return median(values);
    }
    let trimmed = &values[trim_count..values.len() - trim_count];
    mean(trimmed)
}

/// Compute sample standard deviation
pub fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    let variance = values.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}

/// Compute interquartile range
pub fn iqr(values: &mut [f64]) -> f64 {
    percentile(values, 0.75) - percentile(values, 0.25)
}

//=============================================================================
// Data Loading
//=============================================================================

/// Load measurements from CSV file
pub fn load_measurements(path: &Path) -> Result<Vec<Measurement>, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open CSV: {}", e))?;
    let reader = BufReader::new(file);
    let mut measurements = Vec::new();
    let mut lines = reader.lines();

    // Parse header
    let header = lines.next().ok_or("Empty CSV")?.map_err(|e| e.to_string())?;
    let headers: Vec<&str> = header.split(',').collect();

    let col = |name: &str| -> Result<usize, String> {
        headers
            .iter()
            .position(|&h| h == name)
            .ok_or_else(|| format!("Missing column: {}", name))
    };

    let col_image = col("image")?;
    let col_subdir = col("subdir")?;
    let col_codec = col("codec")?;
    let col_quality = col("quality")?;
    let col_ratio = col("ratio")?;
    let col_ppd = col("ppd")?;
    let col_dssim = col("dssim")?;
    let col_file_size = col("file_size")?;

    for line_result in lines {
        let line = line_result.map_err(|e| e.to_string())?;
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() <= col_file_size {
            continue;
        }

        measurements.push(Measurement {
            image: cols[col_image].to_string(),
            subdir: cols[col_subdir].to_string(),
            codec: cols[col_codec].to_string(),
            quality: cols[col_quality].parse().unwrap_or(0),
            ratio: cols[col_ratio].parse().unwrap_or(1.0),
            ppd: cols[col_ppd].parse().unwrap_or(70),
            dssim: cols[col_dssim].parse().unwrap_or(0.0),
            file_size: cols[col_file_size].parse().unwrap_or(0),
        });
    }

    Ok(measurements)
}

/// Group measurements by image name
pub fn group_by_image(measurements: &[Measurement]) -> HashMap<String, Vec<&Measurement>> {
    let mut by_image: HashMap<String, Vec<&Measurement>> = HashMap::new();
    for m in measurements {
        by_image.entry(m.image.clone()).or_default().push(m);
    }
    by_image
}

/// Get measurements at a specific condition
pub fn get_at_condition<'a>(
    img_data: &[&'a Measurement],
    codec: &str,
    condition: ViewingCondition,
) -> Vec<&'a Measurement> {
    img_data
        .iter()
        .filter(|m| {
            (m.codec == codec || m.codec.starts_with(codec))
                && condition.matches(m.ratio, m.ppd)
        })
        .copied()
        .collect()
}

//=============================================================================
// Gap-Based Polynomial Interpolation
//=============================================================================

/// Fit a polynomial q_out = a * q_in^b + c using grid search
/// Returns (a, b, c, r_squared)
fn fit_power_law(points: &[(f64, f64)], config: &InterpolationConfig) -> Option<(f64, f64, f64, f64)> {
    if points.len() < 3 {
        return None;
    }

    let mut best_fit: Option<(f64, f64, f64, f64)> = None;
    let mut b = config.min_exponent;

    while b <= config.max_exponent {
        // Transform: let x' = q_in^b, then fit q_out = a*x' + c (linear regression)
        let x_transformed: Vec<f64> = points.iter().map(|(x, _)| x.powf(b)).collect();
        let y: Vec<f64> = points.iter().map(|(_, y)| *y).collect();

        // Linear regression for a and c
        let n = points.len() as f64;
        let sum_x: f64 = x_transformed.iter().sum();
        let sum_y: f64 = y.iter().sum();
        let sum_xy: f64 = x_transformed.iter().zip(&y).map(|(x, y)| x * y).sum();
        let sum_x2: f64 = x_transformed.iter().map(|x| x * x).sum();

        let denom = n * sum_x2 - sum_x * sum_x;
        if denom.abs() < 1e-10 {
            b += config.exponent_step;
            continue;
        }

        let a = (n * sum_xy - sum_x * sum_y) / denom;
        let c = (sum_y - a * sum_x) / n;

        // Compute R²
        let y_mean = sum_y / n;
        let ss_tot: f64 = y.iter().map(|yi| (yi - y_mean).powi(2)).sum();
        let ss_res: f64 = x_transformed
            .iter()
            .zip(&y)
            .map(|(xi, yi)| (yi - (a * xi + c)).powi(2))
            .sum();

        let r_squared = if ss_tot > 0.0 {
            1.0 - ss_res / ss_tot
        } else {
            0.0
        };

        if best_fit.is_none() || r_squared > best_fit.unwrap().3 {
            best_fit = Some((a, b, c, r_squared));
        }

        b += config.exponent_step;
    }

    best_fit
}

/// Fit a gap polynomial by skipping a point and validating prediction
/// Returns the polynomial with validation error
pub fn fit_gap_polynomial(
    points: &[(u32, f64)], // (quality, dssim) pairs sorted by quality
    skip_idx: usize,
    config: &InterpolationConfig,
) -> Option<GapPolynomial> {
    if points.len() < 4 || skip_idx >= points.len() {
        return None;
    }

    let skipped = points[skip_idx];
    let training: Vec<(f64, f64)> = points
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != skip_idx)
        .map(|(_, (q, d))| (*q as f64, *d))
        .collect();

    let (a, b, c, r_squared) = fit_power_law(&training, config)?;

    // Validate by predicting the skipped point
    let predicted = a * (skipped.0 as f64).powf(b) + c;
    let validation_error = (predicted - skipped.1).abs();

    let q_low = points.first()?.0;
    let q_high = points.last()?.0;

    Some(GapPolynomial {
        q_low,
        q_high,
        a,
        b,
        c,
        r_squared,
        validation_error,
    })
}

/// Compute gap polynomials for all quality gaps, averaging adjacent fits
pub fn compute_gap_polynomials(
    points: &[(u32, f64)], // (quality, dssim) pairs sorted by quality
    config: &InterpolationConfig,
) -> Vec<GapPolynomial> {
    if points.len() < 4 {
        return Vec::new();
    }

    let mut gap_polys = Vec::new();

    // For each internal point (not first or last), fit by skipping it
    for skip_idx in 1..points.len() - 1 {
        let q_low = points[skip_idx - 1].0;
        let q_high = points[skip_idx + 1].0;

        // Skip if gap is too small (consecutive quality values)
        if q_high - q_low <= 2 {
            continue;
        }

        // Fit polynomial by skipping this point
        if let Some(poly) = fit_gap_polynomial(points, skip_idx, config) {
            gap_polys.push((skip_idx, poly));
        }
    }

    // Average adjacent polynomials for each gap
    let mut result = Vec::new();
    for i in 0..gap_polys.len() {
        let (idx, poly) = &gap_polys[i];

        // Find adjacent polynomials to average with
        let mut a_sum = poly.a;
        let mut b_sum = poly.b;
        let mut c_sum = poly.c;
        let mut count = 1.0;

        // Average with previous if exists and overlaps
        if i > 0 {
            let (prev_idx, prev_poly) = &gap_polys[i - 1];
            if idx - prev_idx <= 2 {
                a_sum += prev_poly.a;
                b_sum += prev_poly.b;
                c_sum += prev_poly.c;
                count += 1.0;
            }
        }

        // Average with next if exists and overlaps
        if i + 1 < gap_polys.len() {
            let (next_idx, next_poly) = &gap_polys[i + 1];
            if next_idx - idx <= 2 {
                a_sum += next_poly.a;
                b_sum += next_poly.b;
                c_sum += next_poly.c;
                count += 1.0;
            }
        }

        result.push(GapPolynomial {
            q_low: poly.q_low,
            q_high: poly.q_high,
            a: a_sum / count,
            b: b_sum / count,
            c: c_sum / count,
            r_squared: poly.r_squared,
            validation_error: poly.validation_error,
        });
    }

    result
}

/// Linear interpolation between two points for quality
pub fn linear_interpolate_quality(
    target_dssim: f64,
    points: &[(u32, f64)], // (quality, dssim) sorted by quality
) -> Option<f64> {
    if points.len() < 2 {
        return points.first().map(|(q, _)| *q as f64);
    }

    // DSSIM typically decreases as quality increases
    // Find two adjacent points that bracket the target
    for i in 0..points.len() - 1 {
        let (q1, d1) = points[i];
        let (q2, d2) = points[i + 1];

        // Check if target falls between these DSSIM values
        let in_range =
            (d1 <= target_dssim && target_dssim <= d2) || (d2 <= target_dssim && target_dssim <= d1);

        if in_range && (d2 - d1).abs() > 1e-12 {
            let t = (target_dssim - d1) / (d2 - d1);
            let interp_q = q1 as f64 + t * (q2 as f64 - q1 as f64);
            return Some(interp_q.clamp(0.0, 100.0));
        }
    }

    // Target outside range - return closest
    points
        .iter()
        .min_by(|a, b| (a.1 - target_dssim).abs().partial_cmp(&(b.1 - target_dssim).abs()).unwrap())
        .map(|(q, _)| *q as f64)
}

//=============================================================================
// Analysis Functions
//=============================================================================

/// Result of analyzing a quality step (q_from → q_to)
#[derive(Debug, Clone)]
pub struct QualityStepAnalysis {
    pub q_from: u32,
    pub q_to: u32,
    pub dssim_from: f64,
    pub dssim_to: f64,
    pub bucket_from: DssimBucket,
    pub bucket_to: DssimBucket,
    pub dssim_reduction_pct: f64,
    pub size_increase_pct: f64,
    pub efficiency: f64,
}

/// Analyze diminishing returns for a codec at a given condition
pub fn analyze_diminishing_returns(
    by_image: &HashMap<String, Vec<&Measurement>>,
    codec: &str,
    condition: ViewingCondition,
) -> Vec<QualityStepAnalysis> {
    // Get all quality values for this codec at condition
    let mut all_qualities: Vec<u32> = by_image
        .values()
        .flat_map(|img_data| {
            get_at_condition(img_data, codec, condition)
                .into_iter()
                .map(|m| m.quality)
        })
        .collect();
    all_qualities.sort();
    all_qualities.dedup();

    let mut results = Vec::new();

    for i in 0..all_qualities.len().saturating_sub(1) {
        let q_from = all_qualities[i];
        let q_to = all_qualities[i + 1];

        let mut dssim_froms = Vec::new();
        let mut dssim_tos = Vec::new();
        let mut reductions = Vec::new();
        let mut size_increases = Vec::new();

        for img_data in by_image.values() {
            let measurements = get_at_condition(img_data, codec, condition);

            let m_from = measurements.iter().find(|m| m.quality == q_from);
            let m_to = measurements.iter().find(|m| m.quality == q_to);

            if let (Some(from), Some(to)) = (m_from, m_to) {
                dssim_froms.push(from.dssim);
                dssim_tos.push(to.dssim);

                // Compute DSSIM reduction clamped at imperceptible threshold
                let thresh = DssimBucket::Imperceptible.threshold();
                let before = from.dssim.max(thresh);
                let after = to.dssim.max(thresh);
                let reduction = if before > after {
                    (before - after) / before * 100.0
                } else {
                    0.0
                };
                reductions.push(reduction);

                if from.file_size > 0 {
                    let size_inc =
                        (to.file_size as f64 - from.file_size as f64) / from.file_size as f64 * 100.0;
                    size_increases.push(size_inc);
                }
            }
        }

        if dssim_froms.is_empty() || size_increases.is_empty() {
            continue;
        }

        let med_dssim_from = median(&mut dssim_froms);
        let med_dssim_to = median(&mut dssim_tos);
        let med_reduction = median(&mut reductions);
        let med_size_inc = median(&mut size_increases);
        let efficiency = if med_size_inc > 0.0 {
            med_reduction / med_size_inc
        } else {
            0.0
        };

        results.push(QualityStepAnalysis {
            q_from,
            q_to,
            dssim_from: med_dssim_from,
            dssim_to: med_dssim_to,
            bucket_from: DssimBucket::from_dssim(med_dssim_from),
            bucket_to: DssimBucket::from_dssim(med_dssim_to),
            dssim_reduction_pct: med_reduction,
            size_increase_pct: med_size_inc,
            efficiency,
        });
    }

    results
}

/// Cross-codec quality equivalence result
#[derive(Debug, Clone)]
pub struct CodecEquivalence {
    pub ref_codec: String,
    pub ref_quality: u32,
    pub target_codec: String,
    pub condition: ViewingCondition,
    pub p50: u32,
    pub p75: u32,
    pub p90: u32,
    pub dssim_ref: f64,
    pub dssim_target: f64,
    pub size_ratio_p50: f64,
    pub size_ratio_p75: f64,
    pub n_images: usize,
}

/// Find equivalent quality between codecs at a given condition
pub fn find_codec_equivalence(
    by_image: &HashMap<String, Vec<&Measurement>>,
    ref_codec: &str,
    ref_quality: u32,
    target_codec: &str,
    condition: ViewingCondition,
    forgiveness: f64,
) -> Option<CodecEquivalence> {
    let mut equivalent_qualities: Vec<u32> = Vec::new();
    let mut size_ratios: Vec<f64> = Vec::new();
    let mut ref_dssims: Vec<f64> = Vec::new();
    let mut target_dssims: Vec<f64> = Vec::new();

    for img_data in by_image.values() {
        let ref_measurements = get_at_condition(img_data, ref_codec, condition);
        let target_measurements = get_at_condition(img_data, target_codec, condition);

        let ref_m = ref_measurements.iter().find(|m| m.quality == ref_quality);
        let ref_m = match ref_m {
            Some(m) => m,
            None => continue,
        };

        // Find target quality that achieves same or better DSSIM (with forgiveness)
        let threshold = ref_m.dssim * forgiveness;

        let mut target_sorted: Vec<_> = target_measurements.iter().collect();
        target_sorted.sort_by_key(|m| m.quality);

        // Find lowest quality that achieves threshold
        let mut best_q = None;
        let mut best_m = None;
        for m in &target_sorted {
            if m.dssim <= threshold {
                best_q = Some(m.quality);
                best_m = Some(*m);
                break;
            }
        }

        // If none found, use highest quality
        if best_q.is_none() {
            if let Some(max_m) = target_sorted.last() {
                best_q = Some(max_m.quality);
                best_m = Some(*max_m);
            }
        }

        if let (Some(q), Some(m)) = (best_q, best_m) {
            equivalent_qualities.push(q);
            ref_dssims.push(ref_m.dssim);
            target_dssims.push(m.dssim);
            if ref_m.file_size > 0 {
                size_ratios.push(m.file_size as f64 / ref_m.file_size as f64);
            }
        }
    }

    if equivalent_qualities.is_empty() {
        return None;
    }

    Some(CodecEquivalence {
        ref_codec: ref_codec.to_string(),
        ref_quality,
        target_codec: target_codec.to_string(),
        condition,
        p50: percentile_u32(&mut equivalent_qualities.clone(), 0.50),
        p75: percentile_u32(&mut equivalent_qualities.clone(), 0.75),
        p90: percentile_u32(&mut equivalent_qualities.clone(), 0.90),
        dssim_ref: median(&mut ref_dssims),
        dssim_target: median(&mut target_dssims),
        size_ratio_p50: percentile(&mut size_ratios.clone(), 0.50),
        size_ratio_p75: percentile(&mut size_ratios.clone(), 0.75),
        n_images: equivalent_qualities.len(),
    })
}

/// DSSIM bucket analysis result
#[derive(Debug, Clone)]
pub struct DssimBucketAnalysis {
    pub codec: String,
    pub bucket: DssimBucket,
    pub condition_qualities: Vec<(ViewingCondition, u32, u32, u32)>, // (condition, p50, p75, p90)
}

/// Analyze quality required to achieve each DSSIM bucket at various conditions
pub fn analyze_dssim_buckets(
    by_image: &HashMap<String, Vec<&Measurement>>,
    codec: &str,
    conditions: &[ViewingCondition],
) -> Vec<DssimBucketAnalysis> {
    let mut results = Vec::new();

    for (bucket, threshold, _) in DssimBucket::THRESHOLDS {
        if *bucket == DssimBucket::Degraded {
            continue; // Skip degraded - it's the catchall
        }

        let mut condition_quals = Vec::new();

        for condition in conditions {
            let mut qualities_achieving_bucket: Vec<u32> = Vec::new();

            for img_data in by_image.values() {
                let measurements = get_at_condition(img_data, codec, *condition);
                let mut sorted: Vec<_> = measurements.iter().collect();
                sorted.sort_by_key(|m| m.quality);

                // Find lowest quality that achieves this bucket (dssim < threshold)
                for m in &sorted {
                    if m.dssim < *threshold {
                        qualities_achieving_bucket.push(m.quality);
                        break;
                    }
                }
            }

            if !qualities_achieving_bucket.is_empty() {
                let p50 = percentile_u32(&mut qualities_achieving_bucket.clone(), 0.50);
                let p75 = percentile_u32(&mut qualities_achieving_bucket.clone(), 0.75);
                let p90 = percentile_u32(&mut qualities_achieving_bucket.clone(), 0.90);
                condition_quals.push((*condition, p50, p75, p90));
            }
        }

        results.push(DssimBucketAnalysis {
            codec: codec.to_string(),
            bucket: *bucket,
            condition_qualities: condition_quals,
        });
    }

    results
}

//=============================================================================
// Reporting Helpers
//=============================================================================

/// Format a table header row
pub fn format_table_header(columns: &[&str], widths: &[usize]) -> String {
    let mut row = String::from("│");
    for (col, width) in columns.iter().zip(widths.iter()) {
        write!(row, " {:^width$} │", col, width = width - 2).unwrap();
    }
    row
}

/// Format a table separator
pub fn format_table_separator(widths: &[usize], style: char) -> String {
    let mut sep = String::new();
    let (left, mid, right) = match style {
        '┌' => ('┌', '┬', '┐'),
        '├' => ('├', '┼', '┤'),
        '└' => ('└', '┴', '┘'),
        _ => ('├', '┼', '┤'),
    };
    sep.push(left);
    for (i, width) in widths.iter().enumerate() {
        sep.push_str(&"─".repeat(*width));
        if i < widths.len() - 1 {
            sep.push(mid);
        }
    }
    sep.push(right);
    sep
}

/// Format diminishing returns report
pub fn format_diminishing_returns_report(
    codec: &str,
    steps: &[QualityStepAnalysis],
    condition: ViewingCondition,
) -> String {
    let mut out = String::new();

    writeln!(out, "\n{}", "━".repeat(120)).unwrap();
    writeln!(out, " {} at {} (PPD={})", codec.to_uppercase(), condition.name, condition.ppd).unwrap();
    writeln!(out, "{}", "━".repeat(120)).unwrap();
    writeln!(
        out,
        "{:<10} │ {:>11} │ {:>4} │ {:>11} │ {:>4} │ {:>9} │ {:>8} │ {:>7} │ {:>12}",
        "Step", "DSSIM(from)", "Zone", "DSSIM(to)", "Zone", "Δ DSSIM%", "Size+%", "Effic", "Worth it?"
    ).unwrap();
    writeln!(out, "{}", "─".repeat(120)).unwrap();

    for step in steps {
        let verdict = if step.bucket_to == DssimBucket::Imperceptible {
            "NO (lossless)"
        } else if step.dssim_reduction_pct < 0.5 {
            "NO (tiny)"
        } else if step.efficiency > 1.0 {
            "YES"
        } else if step.efficiency > 0.5 {
            "MAYBE"
        } else if step.efficiency > 0.2 {
            "MARGINAL"
        } else {
            "NO (costly)"
        };

        writeln!(
            out,
            "q{:<2}→{:<3} │ {:>11.5} │ {:>4} │ {:>11.5} │ {:>4} │ {:>8.1}% │ {:>7.1}% │ {:>7.2} │ {:>12}",
            step.q_from, step.q_to,
            step.dssim_from, step.bucket_from.abbrev(),
            step.dssim_to, step.bucket_to.abbrev(),
            step.dssim_reduction_pct, step.size_increase_pct,
            step.efficiency, verdict
        ).unwrap();
    }

    out
}

/// Format DSSIM bucket table
pub fn format_dssim_bucket_table(analyses: &[DssimBucketAnalysis], config: &ReportConfig) -> String {
    let mut out = String::new();

    // Group by codec
    let mut by_codec: HashMap<&str, Vec<&DssimBucketAnalysis>> = HashMap::new();
    for a in analyses {
        by_codec.entry(&a.codec).or_default().push(a);
    }

    for (codec, codec_analyses) in &by_codec {
        writeln!(out, "\n{}", "═".repeat(100)).unwrap();
        writeln!(out, " {} - Quality required for each DSSIM bucket", codec.to_uppercase()).unwrap();
        writeln!(out, " P50 / P75 / P90 (lower percentile = more aggressive)").unwrap();
        writeln!(out, "{}", "═".repeat(100)).unwrap();

        // Header
        write!(out, "{:>15} │", "Bucket").unwrap();
        for cond in &config.conditions {
            write!(out, " {:^18} │", cond.name).unwrap();
        }
        writeln!(out).unwrap();
        writeln!(out, "{}", "─".repeat(100)).unwrap();

        for analysis in codec_analyses.iter() {
            write!(out, "{:>15} │", format!("{} (<{:.4})", analysis.bucket.abbrev(), analysis.bucket.threshold())).unwrap();

            for cond in &config.conditions {
                let cell = analysis
                    .condition_qualities
                    .iter()
                    .find(|(c, _, _, _)| c.name == cond.name)
                    .map(|(_, p50, p75, p90)| format!("{:>2}/{:>2}/{:>2}", p50, p75, p90))
                    .unwrap_or_else(|| "N/A".to_string());
                write!(out, " {:^18} │", cell).unwrap();
            }
            writeln!(out).unwrap();
        }
    }

    out
}

/// Format codec equivalence table
pub fn format_codec_equivalence_table(
    equivalences: &[CodecEquivalence],
    ref_codec: &str,
    config: &ReportConfig,
) -> String {
    let mut out = String::new();

    // Group by ref_quality
    let mut by_quality: HashMap<u32, Vec<&CodecEquivalence>> = HashMap::new();
    for eq in equivalences {
        if eq.ref_codec == ref_codec {
            by_quality.entry(eq.ref_quality).or_default().push(eq);
        }
    }

    let mut qualities: Vec<u32> = by_quality.keys().copied().collect();
    qualities.sort();

    writeln!(out, "\n┌{}┐", "─".repeat(100)).unwrap();
    writeln!(
        out,
        "│ {:^98} │",
        format!("CODEC EQUIVALENCE (reference: {} at {:?})", ref_codec.to_uppercase(), conditions::DEMANDING.name)
    ).unwrap();
    writeln!(out, "│ {:^98} │", "P50 / P75 / P90 (size%)").unwrap();
    writeln!(out, "├────────┬──────────────────────────────────────────┬──────────────────────────────────────────┤").unwrap();

    write!(out, "│ {:^6} │", ref_codec).unwrap();
    for codec in &config.codecs {
        if *codec != ref_codec {
            write!(out, " {:^40} │", codec).unwrap();
        }
    }
    writeln!(out).unwrap();
    writeln!(out, "├────────┼──────────────────────────────────────────┼──────────────────────────────────────────┤").unwrap();

    for q in &qualities {
        write!(out, "│ {:>6} │", format!("q{}", q)).unwrap();

        if let Some(eqs) = by_quality.get(q) {
            for codec in &config.codecs {
                if *codec == ref_codec {
                    continue;
                }
                let eq = eqs.iter().find(|e| e.target_codec.starts_with(codec));
                let cell = match eq {
                    Some(e) => format!(
                        "{:>2}/{:>2}/{:>2} ({:>3.0}%)",
                        e.p50, e.p75, e.p90, e.size_ratio_p75 * 100.0
                    ),
                    None => "N/A".to_string(),
                };
                write!(out, " {:^40} │", cell).unwrap();
            }
        }
        writeln!(out).unwrap();
    }

    writeln!(out, "└────────┴──────────────────────────────────────────┴──────────────────────────────────────────┘").unwrap();

    out
}

//=============================================================================
// Main Analysis Entry Points
//=============================================================================

/// Full analysis with configurable output
pub fn run_analysis(
    measurements: &[Measurement],
    config: &ReportConfig,
) -> String {
    let mut output = String::new();

    writeln!(output, "Generated: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")).unwrap();

    let by_image = group_by_image(measurements);
    writeln!(output, "Loaded {} measurements across {} images", measurements.len(), by_image.len()).unwrap();

    // Category breakdown
    let mut by_category: HashMap<String, usize> = HashMap::new();
    for (_, img_measurements) in &by_image {
        if let Some(m) = img_measurements.first() {
            *by_category.entry(m.subdir.clone()).or_insert(0) += 1;
        }
    }
    writeln!(output, "\nImages by category:").unwrap();
    let mut categories: Vec<_> = by_category.iter().collect();
    categories.sort_by(|a, b| b.1.cmp(a.1));
    for (cat, count) in &categories {
        writeln!(output, "  {:20} {:>4} images", cat, count).unwrap();
    }

    // Diminishing returns analysis
    writeln!(output, "\n{}", "=".repeat(120)).unwrap();
    writeln!(output, " DIMINISHING RETURNS ANALYSIS").unwrap();
    writeln!(output, "{}", "=".repeat(120)).unwrap();

    for codec in &config.codecs {
        let steps = analyze_diminishing_returns(&by_image, codec, conditions::DEMANDING);
        output.push_str(&format_diminishing_returns_report(codec, &steps, conditions::DEMANDING));
    }

    // Cross-codec equivalence
    writeln!(output, "\n{}", "=".repeat(120)).unwrap();
    writeln!(output, " CROSS-CODEC EQUIVALENCE").unwrap();
    writeln!(output, "{}", "=".repeat(120)).unwrap();

    let ref_codec = "mozjpeg";
    let mut equivalences = Vec::new();

    // Get measured quality values
    let mut ref_qualities: Vec<u32> = by_image
        .values()
        .flat_map(|img_data| {
            get_at_condition(img_data, ref_codec, conditions::DEMANDING)
                .into_iter()
                .map(|m| m.quality)
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    ref_qualities.sort();

    for &ref_q in &ref_qualities {
        for target_codec in &config.codecs {
            if *target_codec == ref_codec {
                continue;
            }
            if let Some(eq) = find_codec_equivalence(
                &by_image,
                ref_codec,
                ref_q,
                target_codec,
                conditions::DEMANDING,
                config.forgiveness_factor,
            ) {
                equivalences.push(eq);
            }
        }
    }

    output.push_str(&format_codec_equivalence_table(&equivalences, ref_codec, config));

    // DSSIM bucket analysis
    writeln!(output, "\n{}", "=".repeat(120)).unwrap();
    writeln!(output, " DSSIM BUCKET ANALYSIS").unwrap();
    writeln!(output, "{}", "=".repeat(120)).unwrap();
    writeln!(output, " Quality required to achieve each perceptibility level").unwrap();

    let mut bucket_analyses = Vec::new();
    for codec in &config.codecs {
        let analyses = analyze_dssim_buckets(&by_image, codec, &config.conditions);
        bucket_analyses.extend(analyses);
    }

    output.push_str(&format_dssim_bucket_table(&bucket_analyses, config));

    output
}

/// Generate lookup tables with polynomial coefficients
pub fn generate_interpolation_tables(
    measurements: &[Measurement],
    config: &InterpolationConfig,
) -> Vec<InterpolationTable> {
    let by_image = group_by_image(measurements);
    let mut tables = Vec::new();

    let codecs = ["mozjpeg", "webp", "avif_s6"];

    for codec in &codecs {
        for condition in conditions::KEY {
            // Collect (quality, dssim) pairs aggregated across images
            let mut quality_dssims: HashMap<u32, Vec<f64>> = HashMap::new();

            for img_data in by_image.values() {
                let measurements = get_at_condition(img_data, codec, *condition);
                for m in measurements {
                    quality_dssims.entry(m.quality).or_default().push(m.dssim);
                }
            }

            // Use median DSSIM for each quality
            let mut points: Vec<(u32, f64)> = quality_dssims
                .into_iter()
                .map(|(q, mut dssims)| (q, median(&mut dssims)))
                .collect();
            points.sort_by_key(|(q, _)| *q);

            let polynomials = compute_gap_polynomials(&points, config);

            tables.push(InterpolationTable {
                codec: codec.to_string(),
                condition: *condition,
                polynomials,
            });
        }
    }

    tables
}

//=============================================================================
// Tests
//=============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Statistical function tests
    mod stats {
        use super::*;

        #[test]
        fn test_median_odd() {
            let mut values = vec![1.0, 3.0, 2.0];
            assert_eq!(median(&mut values), 2.0);
        }

        #[test]
        fn test_median_even() {
            let mut values = vec![1.0, 2.0, 3.0, 4.0];
            assert_eq!(median(&mut values), 2.5);
        }

        #[test]
        fn test_median_empty() {
            let mut values: Vec<f64> = vec![];
            assert_eq!(median(&mut values), 0.0);
        }

        #[test]
        fn test_percentile_interpolation() {
            let mut values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
            // p=0.75, pos=6.75, interpolate between 7 and 8
            let p75 = percentile(&mut values, 0.75);
            assert!((p75 - 7.75).abs() < 0.01, "Expected 7.75, got {}", p75);
        }

        #[test]
        fn test_percentile_u32() {
            let mut values = vec![10u32, 20, 30, 40, 50, 60, 70, 80, 90, 100];
            let p75 = percentile_u32(&mut values, 0.75);
            assert!((p75 as i32 - 78).abs() <= 1, "Expected ~78, got {}", p75);
        }

        #[test]
        fn test_mean() {
            let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
            assert_eq!(mean(&values), 3.0);
        }

        #[test]
        fn test_trimmed_mean() {
            // With 20% trim on 10 values, removes 2 from each end
            let mut values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 100.0];
            let tm = trimmed_mean(&mut values, 0.2);
            // Trimmed: [3, 4, 5, 6, 7, 8], mean = 5.5
            assert!((tm - 5.5).abs() < 0.01, "Expected 5.5, got {}", tm);
        }

        #[test]
        fn test_std_dev() {
            let values = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
            let sd = std_dev(&values);
            // Sample std dev of [2,4,4,4,5,5,7,9] is ~2.14
            assert!((sd - 2.14).abs() < 0.1, "Expected ~2.14, got {}", sd);
        }

        #[test]
        fn test_iqr() {
            let mut values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
            let iqr_val = iqr(&mut values);
            // Q3 = 7.75, Q1 = 3.25, IQR = 4.5
            assert!((iqr_val - 4.5).abs() < 0.1, "Expected ~4.5, got {}", iqr_val);
        }
    }

    // Interpolation tests
    mod interpolation {
        use super::*;

        #[test]
        fn test_linear_interpolate_basic() {
            let points = vec![(50u32, 0.01), (60, 0.008), (70, 0.005), (80, 0.003)];

            // Target DSSIM between 70 and 80
            let q = linear_interpolate_quality(0.004, &points);
            assert!(q.is_some());
            let q = q.unwrap();
            assert!(q > 70.0 && q < 80.0, "Expected between 70-80, got {}", q);
        }

        #[test]
        fn test_linear_interpolate_exact() {
            let points = vec![(50u32, 0.01), (60, 0.008), (70, 0.005)];
            let q = linear_interpolate_quality(0.008, &points);
            assert!(q.is_some());
            assert!((q.unwrap() - 60.0).abs() < 0.1);
        }

        #[test]
        fn test_fit_power_law() {
            // Create points that follow q_out = 0.5 * q_in^1.5 + 10
            let points: Vec<(f64, f64)> = vec![
                (30.0, 0.5 * 30.0_f64.powf(1.5) + 10.0),
                (50.0, 0.5 * 50.0_f64.powf(1.5) + 10.0),
                (70.0, 0.5 * 70.0_f64.powf(1.5) + 10.0),
                (90.0, 0.5 * 90.0_f64.powf(1.5) + 10.0),
            ];

            let config = InterpolationConfig::default();
            let fit = fit_power_law(&points, &config);
            assert!(fit.is_some());

            let (_a, b, _c, r_squared) = fit.unwrap();
            assert!(r_squared > 0.99, "R² should be very high, got {}", r_squared);
            assert!((b - 1.5).abs() < 0.2, "Exponent should be ~1.5, got {}", b);
        }

        #[test]
        fn test_gap_polynomial_covers() {
            let poly = GapPolynomial {
                q_low: 50,
                q_high: 80,
                a: 1.0,
                b: 1.0,
                c: 0.0,
                r_squared: 0.99,
                validation_error: 0.001,
            };

            assert!(poly.covers(50));
            assert!(poly.covers(65));
            assert!(poly.covers(80));
            assert!(!poly.covers(49));
            assert!(!poly.covers(81));
        }
    }

    // DSSIM bucket tests
    mod dssim_buckets {
        use super::*;

        #[test]
        fn test_bucket_classification() {
            assert_eq!(DssimBucket::from_dssim(0.0001), DssimBucket::Imperceptible);
            assert_eq!(DssimBucket::from_dssim(0.0005), DssimBucket::Marginal);
            assert_eq!(DssimBucket::from_dssim(0.001), DssimBucket::Subtle);
            assert_eq!(DssimBucket::from_dssim(0.002), DssimBucket::Noticeable);
            assert_eq!(DssimBucket::from_dssim(0.01), DssimBucket::Degraded);
        }

        #[test]
        fn test_bucket_abbrev() {
            assert_eq!(DssimBucket::Imperceptible.abbrev(), "IMP");
            assert_eq!(DssimBucket::Marginal.abbrev(), "MAR");
            assert_eq!(DssimBucket::Subtle.abbrev(), "SUB");
            assert_eq!(DssimBucket::Noticeable.abbrev(), "NOT");
            assert_eq!(DssimBucket::Degraded.abbrev(), "DEG");
        }
    }

    // Viewing condition tests
    mod viewing_conditions {
        use super::*;

        #[test]
        fn test_condition_matching() {
            let cond = ViewingCondition::new("test", 1.0, 70);
            assert!(cond.matches(1.0, 70));
            assert!(cond.matches(1.01, 70)); // Within tolerance
            assert!(!cond.matches(1.0, 40));
            assert!(!cond.matches(0.5, 70));
        }
    }

    // Integration tests (require CSV data)
    #[test]
    #[ignore]
    fn run_full_analysis() {
        let csv_path = std::path::PathBuf::from("/mnt/v/work/corpus_cache/v2/results/measurements_p2.csv");
        let measurements = load_measurements(&csv_path).expect("Failed to load CSV");

        let config = ReportConfig::default();
        let report = run_analysis(&measurements, &config);
        println!("{}", report);

        // Write to file
        let output_path = csv_path.parent().unwrap().join("quality_analysis_report.txt");
        std::fs::write(&output_path, &report).expect("Failed to write report");
        println!("\nWrote: {}", output_path.display());
    }

    #[test]
    #[ignore]
    fn test_interpolation_tables() {
        let csv_path = std::path::PathBuf::from("/mnt/v/work/corpus_cache/v2/results/measurements_p2.csv");
        let measurements = load_measurements(&csv_path).expect("Failed to load CSV");

        let config = InterpolationConfig::default();
        let tables = generate_interpolation_tables(&measurements, &config);

        for table in &tables {
            println!(
                "\n{} at {} ({} polynomials)",
                table.codec, table.condition.name, table.polynomials.len()
            );
            for poly in &table.polynomials {
                println!(
                    "  q{}-{}: y = {:.4} * x^{:.2} + {:.4} (R²={:.3}, err={:.5})",
                    poly.q_low, poly.q_high, poly.a, poly.b, poly.c, poly.r_squared, poly.validation_error
                );
            }
        }
    }
}
