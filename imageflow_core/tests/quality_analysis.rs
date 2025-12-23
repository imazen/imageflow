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
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

//=============================================================================
// Configuration
//=============================================================================

/// Path to calibration measurements CSV
const CSV_PATH: &str = "/mnt/v/work/corpus_cache/v2/results/measurements_p2.csv";

/// Baseline condition for quality mapping
const BASELINE_PPD: u32 = 70;
const BASELINE_RATIO: f32 = 1.0;

/// Condition for diminishing returns analysis (most demanding)
const DEMANDING_PPD: u32 = 40;
const DEMANDING_RATIO: f32 = 1.0;

/// Forgiveness factor: allows target DSSIM to be this much worse than baseline
/// A 5% difference is typically imperceptible
const FORGIVENESS_FACTOR: f64 = 1.05;

/// Perceptibility thresholds (conservative, for desktop viewing at PPD=40)
const THRESH_IMPERCEPTIBLE: f64 = 0.0003;
const THRESH_MARGINAL: f64 = 0.0007;
const THRESH_SUBTLE: f64 = 0.0015;
const THRESH_NOTICEABLE: f64 = 0.0030;

/// Upper bounds from diminishing returns analysis
/// These are quality levels where each codec enters the imperceptible zone
fn quality_upper_bound(codec: &str) -> u32 {
    match codec {
        "mozjpeg" => 95, // Imperceptible at q96
        "webp" => 100,   // Still visible improvement at q100
        "avif_s6" => 95, // Imperceptible at q95
        _ => 100,
    }
}

/// Reference conditions for quality mapping (from most demanding to least)
/// Format: (name, ratio, ppd)
const REFERENCE_CONDITIONS: &[(&str, f32, u32)] = &[
    ("native-desktop", 1.0, 40), // Most demanding: desktop viewing
    ("native-laptop", 1.0, 70),  // Medium: laptop viewing
    ("native-phone", 1.0, 95),   // Least demanding: phone viewing
];

/// Quality tiers with MozJPEG reference values
const QUALITY_TIERS: &[(&str, u32)] = &[
    ("lossless", 100),
    ("highest", 95),
    ("high", 90),
    ("good", 85),
    ("medium", 76),
    ("mediumlow", 65),
    ("low", 50),
    ("lowest", 30),
];

/// Target viewing conditions for quality mapping analysis
///
/// Format: (name, ratio, ppd) where ratio = srcset_multiplier / device_DPPX
///
/// Srcset analysis table:
/// | Srcset | Phone (3x, 95ppd) | Laptop (1.5x, 70ppd) | Desktop (1x, 40ppd) |
/// |--------|-------------------|----------------------|---------------------|
/// | 1x     | 0.33, 95          | 0.67, 70             | 1.0, 40             |
/// | 1.5x   | 0.5, 95           | 1.0, 70              | 1.5, 40             |
/// | 2x     | 0.67, 95          | 1.33, 70             | 2.0, 40             |
/// | 3x     | 1.0, 95           | 2.0, 70              | 3.0, 40             |
const TARGET_CONDITIONS: &[(&str, f32, u32)] = &[
    // Demanding: undersized images (browser upscales, artifacts amplified)
    ("1x→phone", 0.33, 95),  // 1x srcset to 3x phone (worst case)
    ("1x→laptop", 0.67, 70), // 1x srcset to 1.5x laptop
    ("2x→phone", 0.67, 95),  // 2x srcset to 3x phone
    // Native: srcset matches device DPPX
    ("native-desktop", 1.0, 40), // 1x srcset to 1x desktop (baseline: most demanding PPD)
    ("native-laptop", 1.0, 70),  // 1.5x srcset to 1.5x laptop
    ("native-phone", 1.0, 95),   // 3x srcset to 3x phone
    // Forgiving: oversized images (browser downscales, artifacts hidden)
    ("2x→desktop", 2.0, 40), // 2x srcset to 1x desktop
];

const CODECS: &[&str] = &["mozjpeg", "webp", "avif_s6"];

//=============================================================================
// Data Structures
//=============================================================================

/// A single measurement from the calibration run
#[derive(Debug, Clone)]
pub struct Measurement {
    image: String,
    subdir: String, // Category/subfolder in corpus
    codec: String,
    quality: u32,
    ratio: f32,
    ppd: u32,
    dssim: f64,
    file_size: usize,
}

/// Perceptibility zone for a DSSIM value
#[derive(Debug, Clone, Copy, PartialEq)]
enum PerceptibilityZone {
    Imperceptible, // < 0.0003
    Marginal,      // < 0.0007
    Subtle,        // < 0.0015
    Noticeable,    // < 0.0030
    Degraded,      // >= 0.0030
}

impl PerceptibilityZone {
    fn from_dssim(dssim: f64) -> Self {
        if dssim < THRESH_IMPERCEPTIBLE {
            Self::Imperceptible
        } else if dssim < THRESH_MARGINAL {
            Self::Marginal
        } else if dssim < THRESH_SUBTLE {
            Self::Subtle
        } else if dssim < THRESH_NOTICEABLE {
            Self::Noticeable
        } else {
            Self::Degraded
        }
    }

    fn abbrev(&self) -> &'static str {
        match self {
            Self::Imperceptible => "IMP",
            Self::Marginal => "MAR",
            Self::Subtle => "SUB",
            Self::Noticeable => "NOT",
            Self::Degraded => "DEG",
        }
    }
}

/// Result of analyzing a quality step (q_from → q_to)
#[derive(Debug, Clone)]
pub struct QualityStepAnalysis {
    q_from: u32,
    q_to: u32,
    dssim_from: f64,
    dssim_to: f64,
    zone_from: PerceptibilityZone,
    zone_to: PerceptibilityZone,
    dssim_reduction_pct: f64,
    size_increase_pct: f64,
    efficiency: f64, // dssim_reduction_pct / size_increase_pct
}

/// Per-image quality mapping result
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used for debugging and potential future reporting
struct PerImageMapping {
    image: String,
    mapped_quality: u32,
    size_ratio: f64,
    dssim_ratio: f64,
}

/// Aggregated quality mapping with percentiles
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used for debugging and potential future reporting
struct AggregatedMapping {
    codec: String,
    ref_quality: u32,
    target_condition: String,
    n_images: usize,
    p50: u32,
    p75: u32,
    p90: u32,
    p95: u32,
    min: u32,
    max: u32,
    size_ratio_median: f64,
}

//=============================================================================
// Data Loading
//=============================================================================

fn load_measurements(path: &Path) -> Result<Vec<Measurement>, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open CSV: {}", e))?;
    let reader = BufReader::new(file);
    let mut measurements = Vec::new();
    let mut lines = reader.lines();

    // Parse header
    let header = lines.next().ok_or("Empty CSV")?.map_err(|e| e.to_string())?;
    let headers: Vec<&str> = header.split(',').collect();

    let col = |name: &str| -> Result<usize, String> {
        headers.iter().position(|&h| h == name).ok_or_else(|| format!("Missing column: {}", name))
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

//=============================================================================
// Statistical Helpers
//=============================================================================

fn percentile(values: &mut [f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((values.len() - 1) as f64 * p / 100.0) as usize;
    values[idx.min(values.len() - 1)]
}

fn percentile_u32(values: &mut [u32], p: f64) -> u32 {
    if values.is_empty() {
        return 0;
    }
    values.sort();
    let idx = ((values.len() - 1) as f64 * p / 100.0) as usize;
    values[idx.min(values.len() - 1)]
}

fn median(values: &mut [f64]) -> f64 {
    percentile(values, 50.0)
}

//=============================================================================
// Analysis: Diminishing Returns
//=============================================================================

/// Groups measurements by image for per-image analysis
fn group_by_image(measurements: &[Measurement]) -> HashMap<String, Vec<&Measurement>> {
    let mut by_image: HashMap<String, Vec<&Measurement>> = HashMap::new();
    for m in measurements {
        by_image.entry(m.image.clone()).or_default().push(m);
    }
    by_image
}

/// Gets measurements at a specific condition (codec, ratio, ppd)
fn get_at_condition<'a>(
    img_data: &[&'a Measurement],
    codec: &str,
    ratio: f32,
    ppd: u32,
) -> Vec<&'a Measurement> {
    img_data
        .iter()
        .filter(|m| m.codec == codec && (m.ratio - ratio).abs() < 0.02 && m.ppd == ppd)
        .copied()
        .collect()
}

/// Gets measurement at a specific quality from a condition's measurements
fn get_at_quality<'a>(measurements: &[&'a Measurement], quality: u32) -> Option<&'a Measurement> {
    measurements.iter().find(|m| m.quality == quality).copied()
}

/// Computes DSSIM reduction percentage, clamped at imperceptible threshold
///
/// # Algorithm
/// - Clamp both values to imperceptible threshold (improvements below that don't matter)
/// - Return (before - after) / before * 100
fn dssim_reduction_percent(dssim_before: f64, dssim_after: f64) -> f64 {
    let before = dssim_before.max(THRESH_IMPERCEPTIBLE);
    let after = dssim_after.max(THRESH_IMPERCEPTIBLE);
    if before <= after {
        return 0.0;
    }
    (before - after) / before * 100.0
}

/// Analyzes diminishing returns for a codec at the most demanding condition (1x-desktop)
///
/// # Algorithm
/// For each quality step q_low → q_high:
/// 1. For each image, measure DSSIM at q_low and q_high
/// 2. Compute DSSIM reduction % (clamped at imperceptible threshold)
/// 3. Compute file size increase %
/// 4. Compute efficiency = DSSIM reduction % / size increase %
/// 5. Return median across all images
pub fn analyze_diminishing_returns(
    by_image: &HashMap<String, Vec<&Measurement>>,
    codec: &str,
) -> Vec<QualityStepAnalysis> {
    // Get all quality values for this codec at the demanding condition
    let mut all_qualities: Vec<u32> = by_image
        .values()
        .flat_map(|img_data| {
            let measurements = get_at_condition(img_data, codec, DEMANDING_RATIO, DEMANDING_PPD);
            measurements.into_iter().map(|m| m.quality).collect::<Vec<_>>()
        })
        .collect();
    all_qualities.sort();
    all_qualities.dedup();

    let mut results = Vec::new();

    // Analyze each quality step
    for i in 0..all_qualities.len().saturating_sub(1) {
        let q_from = all_qualities[i];
        let q_to = all_qualities[i + 1];

        let mut dssim_froms = Vec::new();
        let mut dssim_tos = Vec::new();
        let mut reductions = Vec::new();
        let mut size_increases = Vec::new();

        for img_data in by_image.values() {
            let measurements = get_at_condition(img_data, codec, DEMANDING_RATIO, DEMANDING_PPD);

            let m_from = get_at_quality(&measurements, q_from);
            let m_to = get_at_quality(&measurements, q_to);

            if let (Some(from), Some(to)) = (m_from, m_to) {
                dssim_froms.push(from.dssim);
                dssim_tos.push(to.dssim);
                reductions.push(dssim_reduction_percent(from.dssim, to.dssim));
                if from.file_size > 0 {
                    let size_inc = (to.file_size as f64 - from.file_size as f64)
                        / from.file_size as f64
                        * 100.0;
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
        let efficiency = if med_size_inc > 0.0 { med_reduction / med_size_inc } else { 0.0 };

        results.push(QualityStepAnalysis {
            q_from,
            q_to,
            dssim_from: med_dssim_from,
            dssim_to: med_dssim_to,
            zone_from: PerceptibilityZone::from_dssim(med_dssim_from),
            zone_to: PerceptibilityZone::from_dssim(med_dssim_to),
            dssim_reduction_pct: med_reduction,
            size_increase_pct: med_size_inc,
            efficiency,
        });
    }

    results
}

/// Finds the quality level where a codec first enters the imperceptible zone
pub fn find_imperceptible_threshold(steps: &[QualityStepAnalysis]) -> Option<u32> {
    for step in steps {
        if step.zone_to == PerceptibilityZone::Imperceptible {
            return Some(step.q_to);
        }
    }
    None
}

//=============================================================================
// Analysis: Quality Mapping
//=============================================================================

/// Maps a reference quality to equivalent quality at a target condition
///
/// # Algorithm
/// For each image:
/// 1. Get DSSIM at baseline condition (1x-laptop) at ref_quality
/// 2. Compute threshold = base_dssim * forgiveness_factor
/// 3. Find LOWEST quality at target condition where DSSIM <= threshold
/// 4. Record this per-image mapping
///
/// Returns per-image mappings for aggregation
fn compute_per_image_mappings(
    by_image: &HashMap<String, Vec<&Measurement>>,
    codec: &str,
    ref_quality: u32,
    target_ratio: f32,
    target_ppd: u32,
) -> Vec<PerImageMapping> {
    let mut mappings = Vec::new();

    for (image, img_data) in by_image {
        let baseline = get_at_condition(img_data, codec, BASELINE_RATIO, BASELINE_PPD);
        let target = get_at_condition(img_data, codec, target_ratio, target_ppd);

        let base_m = match get_at_quality(&baseline, ref_quality) {
            Some(m) => m,
            None => continue,
        };

        let threshold = base_m.dssim * FORGIVENESS_FACTOR;

        // Find LOWEST quality that achieves threshold (smallest file)
        let mut target_sorted: Vec<_> = target.iter().collect();
        target_sorted.sort_by_key(|m| m.quality);

        let mut best_q = None;
        let mut best_m = None;

        for m in &target_sorted {
            if m.dssim <= threshold {
                best_q = Some(m.quality);
                best_m = Some(*m);
                break;
            }
        }

        // If none found, use highest available
        if best_q.is_none() {
            if let Some(max_m) = target_sorted.last() {
                best_q = Some(max_m.quality);
                best_m = Some(*max_m);
            }
        }

        if let (Some(q), Some(m)) = (best_q, best_m) {
            mappings.push(PerImageMapping {
                image: image.clone(),
                mapped_quality: q,
                size_ratio: if base_m.file_size > 0 {
                    m.file_size as f64 / base_m.file_size as f64
                } else {
                    1.0
                },
                dssim_ratio: if base_m.dssim > 0.0 { m.dssim / base_m.dssim } else { 1.0 },
            });
        }
    }

    mappings
}

/// Maps quality from explicit reference condition to target condition
/// Like compute_per_image_mappings but with configurable reference
fn compute_per_image_mappings_with_ref(
    by_image: &HashMap<String, Vec<&Measurement>>,
    codec: &str,
    ref_quality: u32,
    ref_ratio: f32,
    ref_ppd: u32,
    target_ratio: f32,
    target_ppd: u32,
) -> Vec<PerImageMapping> {
    let mut mappings = Vec::new();

    for (image, img_data) in by_image {
        let baseline = get_at_condition(img_data, codec, ref_ratio, ref_ppd);
        let target = get_at_condition(img_data, codec, target_ratio, target_ppd);

        let base_m = match get_at_quality(&baseline, ref_quality) {
            Some(m) => m,
            None => continue,
        };

        let threshold = base_m.dssim * FORGIVENESS_FACTOR;

        // Find LOWEST quality that achieves threshold (smallest file)
        let mut target_sorted: Vec<_> = target.iter().collect();
        target_sorted.sort_by_key(|m| m.quality);

        let mut best_q = None;
        let mut best_m = None;

        for m in &target_sorted {
            if m.dssim <= threshold {
                best_q = Some(m.quality);
                best_m = Some(*m);
                break;
            }
        }

        // If none found, use highest available
        if best_q.is_none() {
            if let Some(max_m) = target_sorted.last() {
                best_q = Some(max_m.quality);
                best_m = Some(*max_m);
            }
        }

        if let (Some(q), Some(m)) = (best_q, best_m) {
            mappings.push(PerImageMapping {
                image: image.clone(),
                mapped_quality: q,
                size_ratio: if base_m.file_size > 0 {
                    m.file_size as f64 / base_m.file_size as f64
                } else {
                    1.0
                },
                dssim_ratio: if base_m.dssim > 0.0 { m.dssim / base_m.dssim } else { 1.0 },
            });
        }
    }

    mappings
}

/// Aggregates per-image mappings into percentiles
fn aggregate_mappings(
    mappings: &[PerImageMapping],
    codec: &str,
    ref_quality: u32,
    target_name: &str,
) -> Option<AggregatedMapping> {
    if mappings.is_empty() {
        return None;
    }

    let qualities: Vec<u32> = mappings.iter().map(|m| m.mapped_quality).collect();
    let mut size_ratios: Vec<f64> = mappings.iter().map(|m| m.size_ratio).collect();

    Some(AggregatedMapping {
        codec: codec.to_string(),
        ref_quality,
        target_condition: target_name.to_string(),
        n_images: mappings.len(),
        p50: percentile_u32(&mut qualities.clone(), 50.0),
        p75: percentile_u32(&mut qualities.clone(), 75.0),
        p90: percentile_u32(&mut qualities.clone(), 90.0),
        p95: percentile_u32(&mut qualities.clone(), 95.0),
        min: *qualities.iter().min().unwrap_or(&0),
        max: *qualities.iter().max().unwrap_or(&0),
        size_ratio_median: median(&mut size_ratios),
    })
}

/// Applies constraints to a mapping
///
/// # Constraints
/// 1. Upper bound from diminishing returns (imperceptible zone)
/// 2. For demanding conditions: max 2x file size increase, fall back to P50 if exceeded
fn apply_constraints(agg: &AggregatedMapping, _ref_quality: u32, is_demanding: bool) -> u32 {
    let upper = quality_upper_bound(&agg.codec);
    let mut quality = agg.p75;

    // Apply upper bound
    quality = quality.min(upper);

    // For demanding conditions with large size increase, use P50
    if is_demanding && agg.size_ratio_median > 2.0 {
        quality = agg.p50.min(upper);
    }

    quality
}

//=============================================================================
// Cross-Codec Equivalence Analysis
//=============================================================================

/// Cross-codec quality equivalence result with multiple percentiles
#[derive(Debug, Clone)]
pub struct CodecEquivalence {
    pub ref_codec: String,
    pub ref_quality: u32,
    pub target_codec: String,
    /// Quality at various percentiles (P45, P50, P75, P90)
    pub p45: u32,
    pub p50: u32,
    pub p75: u32,
    pub p90: u32,
    pub dssim_ref: f64,
    pub dssim_target: f64,
    /// Size ratios at various percentiles
    pub size_ratio_p50: f64,
    pub size_ratio_p75: f64,
    pub n_images: usize,
}

/// Finds equivalent quality in target codec that matches reference codec's DSSIM
/// at native 1:1 conditions (no browser scaling, same PPD)
/// Returns multiple percentiles (P45, P50, P75, P90) for flexibility
fn find_codec_equivalence(
    by_image: &HashMap<String, Vec<&Measurement>>,
    ref_codec: &str,
    ref_quality: u32,
    target_codec: &str,
    ratio: f32,
    ppd: u32,
) -> Option<CodecEquivalence> {
    let mut equivalent_qualities: Vec<u32> = Vec::new();
    let mut size_ratios: Vec<f64> = Vec::new();
    let mut ref_dssims: Vec<f64> = Vec::new();
    let mut target_dssims: Vec<f64> = Vec::new();

    for img_data in by_image.values() {
        let ref_measurements = get_at_condition(img_data, ref_codec, ratio, ppd);
        let target_measurements = get_at_condition(img_data, target_codec, ratio, ppd);

        let ref_m = match get_at_quality(&ref_measurements, ref_quality) {
            Some(m) => m,
            None => continue,
        };

        // Find target quality that achieves same or better DSSIM
        let mut target_sorted: Vec<_> = target_measurements.iter().collect();
        target_sorted.sort_by_key(|m| m.quality);

        // Find lowest quality in target codec that achieves ref DSSIM (with small tolerance)
        let threshold = ref_m.dssim * 1.02; // 2% tolerance

        let mut best_q = None;
        let mut best_m = None;
        for m in &target_sorted {
            if m.dssim <= threshold {
                best_q = Some(m.quality);
                best_m = Some(*m);
                break;
            }
        }

        // If none found below threshold, use highest quality
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
        p45: percentile_u32(&mut equivalent_qualities.clone(), 45.0),
        p50: percentile_u32(&mut equivalent_qualities.clone(), 50.0),
        p75: percentile_u32(&mut equivalent_qualities.clone(), 75.0),
        p90: percentile_u32(&mut equivalent_qualities.clone(), 90.0),
        dssim_ref: median(&mut ref_dssims),
        dssim_target: median(&mut target_dssims),
        size_ratio_p50: percentile(&mut size_ratios.clone(), 50.0),
        size_ratio_p75: percentile(&mut size_ratios.clone(), 75.0),
        n_images: equivalent_qualities.len(),
    })
}

/// Formats cross-codec equivalence table with multiple percentile columns
pub fn format_codec_equivalence_table<W: Write>(
    out: &mut W,
    equivalences: &[CodecEquivalence],
    ref_codec: &str,
) -> std::io::Result<()> {
    // Group by ref_quality
    let mut by_quality: HashMap<u32, Vec<&CodecEquivalence>> = HashMap::new();
    for eq in equivalences {
        if eq.ref_codec == ref_codec {
            by_quality.entry(eq.ref_quality).or_default().push(eq);
        }
    }

    let mut qualities: Vec<u32> = by_quality.keys().copied().collect();
    qualities.sort();

    // Wide format with multiple percentiles
    writeln!(out, "\n┌{}┐", "─".repeat(130))?;
    writeln!(
        out,
        "│ {:^128} │",
        format!(
            "CODEC EQUIVALENCE (reference: {} at native 1:1, PPD=40)",
            ref_codec.to_uppercase()
        )
    )?;
    writeln!(
        out,
        "│ {:^128} │",
        "Lower percentiles = more aggressive (works for fewer images), Higher = conservative"
    )?;
    writeln!(out, "├────────┬────────────────────────────────────────────────────────────┬────────────────────────────────────────────────────────────┤")?;
    writeln!(out, "│ {:^6} │ {:^58} │ {:^58} │", ref_codec, "WebP", "AVIF (speed=6)")?;
    writeln!(
        out,
        "│        │ {:^13} {:^13} {:^13} {:^13} │ {:^13} {:^13} {:^13} {:^13} │",
        "P45", "P50", "P75", "P90", "P45", "P50", "P75", "P90"
    )?;
    writeln!(out, "├────────┼────────────────────────────────────────────────────────────┼────────────────────────────────────────────────────────────┤")?;

    for q in &qualities {
        if let Some(eqs) = by_quality.get(q) {
            let webp = eqs.iter().find(|e| e.target_codec == "webp");
            let avif = eqs.iter().find(|e| e.target_codec == "avif_s6");

            let format_eq = |eq: Option<&CodecEquivalence>| -> String {
                match eq {
                    Some(e) => {
                        let size_pct = (e.size_ratio_p75 * 100.0) as i32;
                        format!(
                            "{:>3} {:>3} {:>3} {:>3}  ({:>2}%)",
                            e.p45, e.p50, e.p75, e.p90, size_pct
                        )
                    }
                    None => "       N/A           ".to_string(),
                }
            };

            writeln!(
                out,
                "│ {:>6} │ {:^58} │ {:^58} │",
                format!("q{}", q),
                format_eq(webp.copied()),
                format_eq(avif.copied())
            )?;
        }
    }

    writeln!(out, "└────────┴────────────────────────────────────────────────────────────┴────────────────────────────────────────────────────────────┘")?;

    writeln!(out, "\nSize % shows P75 file size relative to MozJPEG reference at same quality.")?;
    writeln!(out, "Note: P45=aggressive (may show artifacts on 45% of images), P75=conservative default, P90=very safe.")?;

    Ok(())
}

//=============================================================================
// Reporting (separated from analysis)
//=============================================================================

/// Formats the diminishing returns analysis as a report
pub fn format_diminishing_returns_report<W: Write>(
    out: &mut W,
    codec: &str,
    steps: &[QualityStepAnalysis],
) -> std::io::Result<()> {
    writeln!(out, "\n{}", "━".repeat(120))?;
    writeln!(out, " {}", codec.to_uppercase())?;
    writeln!(out, "{}", "━".repeat(120))?;
    writeln!(
        out,
        "{:<10} │ {:>11} │ {:>4} │ {:>11} │ {:>4} │ {:>9} │ {:>8} │ {:>7} │ {:>12}",
        "Step",
        "DSSIM(from)",
        "Zone",
        "DSSIM(to)",
        "Zone",
        "Δ DSSIM%",
        "Size+%",
        "Effic",
        "Worth it?"
    )?;
    writeln!(out, "{}", "─".repeat(120))?;

    for step in steps {
        let verdict = if step.zone_to == PerceptibilityZone::Imperceptible {
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

        writeln!(out,
            "q{:<2}→{:<3} │ {:>11.5} │ {:>4} │ {:>11.5} │ {:>4} │ {:>8.1}% │ {:>7.1}% │ {:>7.2} │ {:>12}",
            step.q_from, step.q_to,
            step.dssim_from, step.zone_from.abbrev(),
            step.dssim_to, step.zone_to.abbrev(),
            step.dssim_reduction_pct, step.size_increase_pct,
            step.efficiency, verdict
        )?;
    }

    Ok(())
}

/// Multi-percentile aggregated mapping for a single condition
#[derive(Debug, Clone)]
pub struct ConditionMapping {
    pub condition_name: String,
    pub p50: u32,
    pub p75: u32,
    pub p90: u32,
    pub size_ratio_p75: f64,
}

/// Formats quality mappings with multiple percentile columns
pub fn format_quality_mappings_multi<W: Write>(
    out: &mut W,
    codec: &str,
    // ref_q -> [(condition_name, ConditionMapping)]
    mappings: &[(u32, Vec<ConditionMapping>)],
) -> std::io::Result<()> {
    // Calculate column width per condition: we show P50/P75/P90
    let col_width = 20;
    let total_width = 8 + 6 + TARGET_CONDITIONS.len() * (col_width + 1) + 3;

    writeln!(out, "\n┌{}┐", "─".repeat(total_width))?;
    writeln!(out, "│ {:^width$} │", codec.to_uppercase(), width = total_width - 2)?;
    writeln!(
        out,
        "│ {:^width$} │",
        "P50 / P75 / P90 (lower percentile = more aggressive)",
        width = total_width - 2
    )?;

    // Build header separator
    let mut sep = String::from("├────────┬──────");
    for _ in TARGET_CONDITIONS {
        sep.push_str(&format!("┬{}", "─".repeat(col_width)));
    }
    sep.push_str("┤");
    writeln!(out, "{}", sep)?;

    // Condition names header
    write!(out, "│ {:6} │ {:>4} ", "Tier", "Ref")?;
    for (name, _, _) in TARGET_CONDITIONS {
        write!(out, "│ {:^width$} ", name, width = col_width - 1)?;
    }
    writeln!(out, "│")?;

    // Separator before data rows
    let mut sep = String::from("├────────┼──────");
    for _ in TARGET_CONDITIONS {
        sep.push_str(&format!("┼{}", "─".repeat(col_width)));
    }
    sep.push_str("┤");
    writeln!(out, "{}", sep)?;

    for (tier_name, ref_q) in QUALITY_TIERS {
        write!(out, "│ {:6} │ {:>4} ", tier_name, ref_q)?;

        // Find mappings for this ref_q
        let row_mappings = mappings.iter().find(|(q, _)| q == ref_q).map(|(_, v)| v);

        for (cond_name, _, _) in TARGET_CONDITIONS {
            let cell = if *ref_q == 100 {
                "100".to_string()
            } else if let Some(m) = row_mappings {
                if let Some(cm) = m.iter().find(|c| c.condition_name == *cond_name) {
                    // Show P50/P75/P90 compactly
                    format!("{:>2}/{:>2}/{:>2}", cm.p50, cm.p75, cm.p90)
                } else {
                    "N/A".to_string()
                }
            } else {
                "N/A".to_string()
            };
            write!(out, "│ {:^width$} ", cell, width = col_width - 1)?;
        }
        writeln!(out, "│")?;
    }

    // Footer
    let mut sep = String::from("└────────┴──────");
    for _ in TARGET_CONDITIONS {
        sep.push_str(&format!("┴{}", "─".repeat(col_width)));
    }
    sep.push_str("┘");
    writeln!(out, "{}", sep)?;

    Ok(())
}

/// Formats quality mappings as a table (legacy single-value format)
pub fn format_quality_mappings<W: Write>(
    out: &mut W,
    codec: &str,
    mappings: &[(u32, Vec<(String, u32)>)], // ref_q -> [(condition_name, mapped_q)]
) -> std::io::Result<()> {
    writeln!(out, "\n┌{}┐", "─".repeat(95))?;
    writeln!(out, "│ {:^93} │", codec.to_uppercase())?;
    writeln!(
        out,
        "├──────────┬──────┬{}┤",
        "─────────────────┬".repeat(TARGET_CONDITIONS.len()).trim_end_matches('┬')
    )?;

    write!(out, "│ {:8} │ {:>4} │", "Tier", "Ref")?;
    for (name, _, _) in TARGET_CONDITIONS {
        write!(out, " {:^15} │", name)?;
    }
    writeln!(out)?;
    writeln!(
        out,
        "├──────────┼──────┼{}┤",
        "─────────────────┼".repeat(TARGET_CONDITIONS.len()).trim_end_matches('┼')
    )?;

    for (tier_name, ref_q) in QUALITY_TIERS {
        write!(out, "│ {:8} │ {:>4} │", tier_name, ref_q)?;

        // Find mappings for this ref_q
        let row_mappings = mappings.iter().find(|(q, _)| q == ref_q).map(|(_, v)| v);

        for (cond_name, _, _) in TARGET_CONDITIONS {
            let cell = if *ref_q == 100 {
                "100".to_string()
            } else if let Some(m) = row_mappings {
                if let Some((_, mapped_q)) = m.iter().find(|(n, _)| n == *cond_name) {
                    let delta = *mapped_q as i32 - *ref_q as i32;
                    if delta.abs() <= 3 {
                        format!("{}", mapped_q)
                    } else if delta > 0 {
                        format!("{} (+{})", mapped_q, delta)
                    } else {
                        format!("{} ({})", mapped_q, delta)
                    }
                } else {
                    "N/A".to_string()
                }
            } else {
                "N/A".to_string()
            };
            write!(out, " {:^15} │", cell)?;
        }
        writeln!(out)?;
    }

    writeln!(
        out,
        "└──────────┴──────┴{}┘",
        "─────────────────┴".repeat(TARGET_CONDITIONS.len()).trim_end_matches('┴')
    )?;

    Ok(())
}

//=============================================================================
// Main Analysis Entry Point
//=============================================================================

pub fn run_quality_analysis(csv_path: &Path) -> Result<String, String> {
    let mut output = String::new();
    use std::fmt::Write as FmtWrite;

    // Header with timestamp
    writeln!(output, "Generated: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"))
        .map_err(|e| e.to_string())?;
    writeln!(output).map_err(|e| e.to_string())?;

    // Load data
    let measurements = load_measurements(csv_path)?;
    writeln!(output, "Loaded {} measurements", measurements.len()).map_err(|e| e.to_string())?;

    let by_image = group_by_image(&measurements);
    writeln!(output, "Across {} images", by_image.len()).map_err(|e| e.to_string())?;

    // Count images by category
    let mut by_category: HashMap<String, usize> = HashMap::new();
    for (_image_name, image_measurements) in &by_image {
        if let Some(m) = image_measurements.first() {
            *by_category.entry(m.subdir.clone()).or_insert(0) += 1;
        }
    }
    let mut categories: Vec<_> = by_category.iter().collect();
    categories.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending

    writeln!(output, "\nImages by category:").map_err(|e| e.to_string())?;
    for (cat, count) in &categories {
        writeln!(output, "  {:20} {:>4} images", cat, count).map_err(|e| e.to_string())?;
    }
    writeln!(output).map_err(|e| e.to_string())?;

    // Methodology section
    writeln!(output, "{}", "=".repeat(120)).map_err(|e| e.to_string())?;
    writeln!(output, " METHODOLOGY").map_err(|e| e.to_string())?;
    writeln!(output, "{}", "=".repeat(120)).map_err(|e| e.to_string())?;
    writeln!(
        output,
        r#"
ALGORITHM:
  1. For each source image:
     a. Measure DSSIM at baseline (1x-laptop: PPD={}, ratio={}) at reference quality
     b. Compute threshold = base_dssim * {} (forgiveness factor)
     c. Find LOWEST quality at target condition where DSSIM <= threshold
     d. Record per-image quality mapping

  2. Aggregate per-image mappings:
     - P50 (median): works for 50% of images
     - P75: works for 75% of images (default, conservative)
     - P90/P95: more conservative options

  3. Apply constraints:
     - Upper bounds from diminishing returns (MozJPEG≤95, WebP≤100, AVIF≤95)
     - For demanding conditions: max 2x file size, fall back to P50 if exceeded

PERCEPTIBILITY THRESHOLDS (desktop, PPD={}):
  DSSIM < {:.4}: Imperceptible (visually lossless)
  DSSIM < {:.4}: Marginal (only A/B comparison)
  DSSIM < {:.4}: Subtle (barely noticeable)
  DSSIM < {:.4}: Noticeable (visible on inspection)
  DSSIM >= {:.4}: Degraded (clearly visible)
"#,
        BASELINE_PPD,
        BASELINE_RATIO,
        FORGIVENESS_FACTOR,
        DEMANDING_PPD,
        THRESH_IMPERCEPTIBLE,
        THRESH_MARGINAL,
        THRESH_SUBTLE,
        THRESH_NOTICEABLE,
        THRESH_NOTICEABLE
    )
    .map_err(|e| e.to_string())?;

    // Part 1: Diminishing Returns
    writeln!(output, "\n{}", "=".repeat(120)).map_err(|e| e.to_string())?;
    writeln!(output, " DIMINISHING RETURNS ANALYSIS (1x-desktop, PPD={})", DEMANDING_PPD)
        .map_err(|e| e.to_string())?;
    writeln!(output, "{}", "=".repeat(120)).map_err(|e| e.to_string())?;
    writeln!(output, "\nThis is the MOST DEMANDING condition - where artifacts are most visible.")
        .map_err(|e| e.to_string())?;
    writeln!(
        output,
        "Used to determine upper bounds (quality beyond which improvements are imperceptible)."
    )
    .map_err(|e| e.to_string())?;

    let mut out_bytes = output.as_bytes().to_vec();
    for codec in CODECS {
        let steps = analyze_diminishing_returns(&by_image, codec);
        format_diminishing_returns_report(&mut out_bytes, codec, &steps)
            .map_err(|e| e.to_string())?;

        // Summary
        let imperceptible_at = find_imperceptible_threshold(&steps);
        writeln!(
            out_bytes,
            "\n  {} reaches imperceptible zone at: q{}",
            codec.to_uppercase(),
            imperceptible_at.map(|q| q.to_string()).unwrap_or("never".to_string())
        )
        .map_err(|e| e.to_string())?;
    }
    output = String::from_utf8(out_bytes).map_err(|e| e.to_string())?;

    // Part 2: Cross-Codec Equivalence (native 1:1 at desktop PPD=40)
    writeln!(output, "\n\n{}", "=".repeat(120)).map_err(|e| e.to_string())?;
    writeln!(output, " CROSS-CODEC QUALITY EQUIVALENCE (native 1:1, PPD={})", DEMANDING_PPD)
        .map_err(|e| e.to_string())?;
    writeln!(output, "{}", "=".repeat(120)).map_err(|e| e.to_string())?;
    writeln!(
        output,
        "\nDirect quality translation between codecs at the most demanding viewing condition."
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        output,
        "For each MozJPEG quality, find WebP/AVIF quality that produces equivalent DSSIM."
    )
    .map_err(|e| e.to_string())?;
    writeln!(output, "Size % shows target file size relative to MozJPEG reference.")
        .map_err(|e| e.to_string())?;

    // Dynamically get all measured MozJPEG quality values at demanding condition
    let mut equivalences: Vec<CodecEquivalence> = Vec::new();
    let mut mozjpeg_qualities: Vec<u32> = by_image
        .values()
        .flat_map(|img_data| {
            get_at_condition(img_data, "mozjpeg", DEMANDING_RATIO, DEMANDING_PPD)
                .into_iter()
                .map(|m| m.quality)
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    mozjpeg_qualities.sort();
    writeln!(
        output,
        "Found {} MozJPEG quality levels with data at demanding condition",
        mozjpeg_qualities.len()
    )
    .map_err(|e| e.to_string())?;

    for &ref_q in &mozjpeg_qualities {
        for target_codec in &["webp", "avif_s6"] {
            if let Some(eq) = find_codec_equivalence(
                &by_image,
                "mozjpeg",
                ref_q,
                target_codec,
                DEMANDING_RATIO,
                DEMANDING_PPD,
            ) {
                equivalences.push(eq);
            }
        }
    }

    let mut out_bytes = output.as_bytes().to_vec();
    format_codec_equivalence_table(&mut out_bytes, &equivalences, "mozjpeg")
        .map_err(|e| e.to_string())?;
    output = String::from_utf8(out_bytes).map_err(|e| e.to_string())?;

    // Part 3: Quality Mappings from multiple reference baselines (with multi-percentile display)
    for (ref_name, ref_ratio, ref_ppd) in REFERENCE_CONDITIONS {
        writeln!(output, "\n\n{}", "=".repeat(160)).map_err(|e| e.to_string())?;
        writeln!(
            output,
            " QUALITY MAPPINGS (reference: {}, ratio={}, PPD={})",
            ref_name, ref_ratio, ref_ppd
        )
        .map_err(|e| e.to_string())?;
        writeln!(output, "{}", "=".repeat(160)).map_err(|e| e.to_string())?;
        writeln!(output, "\nShowing P50/P75/P90 with {}x forgiveness factor. Lower percentile = more aggressive quality reduction.", FORGIVENESS_FACTOR).map_err(|e| e.to_string())?;
        writeln!(output, "Maps quality FROM reference condition TO each target condition.")
            .map_err(|e| e.to_string())?;

        let mut out_bytes = output.as_bytes().to_vec();
        for codec in CODECS {
            let mut codec_mappings: Vec<(u32, Vec<ConditionMapping>)> = Vec::new();

            for (_tier_name, tier_q) in QUALITY_TIERS {
                let mut row: Vec<ConditionMapping> = Vec::new();

                for (cond_name, target_ratio, target_ppd) in TARGET_CONDITIONS {
                    let mapping = if *tier_q == 100 {
                        ConditionMapping {
                            condition_name: cond_name.to_string(),
                            p50: 100,
                            p75: 100,
                            p90: 100,
                            size_ratio_p75: 1.0,
                        }
                    } else {
                        // Use the current reference condition as baseline
                        let per_image = compute_per_image_mappings_with_ref(
                            &by_image,
                            codec,
                            *tier_q,
                            *ref_ratio,
                            *ref_ppd, // Reference condition
                            *target_ratio,
                            *target_ppd, // Target condition
                        );

                        if let Some(agg) = aggregate_mappings(&per_image, codec, *tier_q, cond_name)
                        {
                            ConditionMapping {
                                condition_name: cond_name.to_string(),
                                p50: agg.p50,
                                p75: agg.p75,
                                p90: agg.p90,
                                size_ratio_p75: agg.size_ratio_median,
                            }
                        } else {
                            ConditionMapping {
                                condition_name: cond_name.to_string(),
                                p50: *tier_q,
                                p75: *tier_q,
                                p90: *tier_q,
                                size_ratio_p75: 1.0,
                            }
                        }
                    };

                    row.push(mapping);
                }

                codec_mappings.push((*tier_q, row));
            }

            format_quality_mappings_multi(&mut out_bytes, codec, &codec_mappings)
                .map_err(|e| e.to_string())?;
        }
        output = String::from_utf8(out_bytes).map_err(|e| e.to_string())?;
    }

    writeln!(output, "\n{}", "=".repeat(120)).map_err(|e| e.to_string())?;
    writeln!(output, " ANALYSIS COMPLETE").map_err(|e| e.to_string())?;
    writeln!(output, "{}", "=".repeat(120)).map_err(|e| e.to_string())?;

    Ok(output)
}

//=============================================================================
// Tests
//=============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_perceptibility_zones() {
        assert_eq!(PerceptibilityZone::from_dssim(0.0001), PerceptibilityZone::Imperceptible);
        assert_eq!(PerceptibilityZone::from_dssim(0.0005), PerceptibilityZone::Marginal);
        assert_eq!(PerceptibilityZone::from_dssim(0.001), PerceptibilityZone::Subtle);
        assert_eq!(PerceptibilityZone::from_dssim(0.002), PerceptibilityZone::Noticeable);
        assert_eq!(PerceptibilityZone::from_dssim(0.01), PerceptibilityZone::Degraded);
    }

    #[test]
    fn test_dssim_reduction() {
        // Normal case
        assert!((dssim_reduction_percent(0.01, 0.005) - 50.0).abs() < 0.1);

        // Both below threshold - should be 0
        assert_eq!(dssim_reduction_percent(0.0001, 0.00005), 0.0);

        // One below threshold
        let result = dssim_reduction_percent(0.001, 0.0001);
        // 0.001 -> 0.0003 (clamped) = 70% reduction
        assert!(result > 60.0 && result < 80.0);
    }

    #[test]
    #[ignore]
    fn run_full_analysis() {
        let csv_path = PathBuf::from(CSV_PATH);
        match run_quality_analysis(&csv_path) {
            Ok(report) => {
                println!("{}", report);

                // Also write to file
                let output_path = csv_path.parent().unwrap().join("quality_analysis_rust.txt");
                std::fs::write(&output_path, &report).expect("Failed to write report");
                println!("\nWrote: {}", output_path.display());
            }
            Err(e) => eprintln!("Analysis failed: {}", e),
        }
    }

    #[test]
    #[ignore]
    fn generate_comparer_data() {
        let csv_path = PathBuf::from(CSV_PATH);
        let measurements = load_measurements(&csv_path).expect("Failed to load measurements");
        let by_image = group_by_image(&measurements);

        // Build JSON structure for comparer
        let mut output =
            String::from("// Generated by quality_analysis.rs\nconst imageArray = [\n");

        // Group images by category and name
        let mut images: Vec<(&str, &str, u32, u32, &Vec<&Measurement>)> = Vec::new();
        for (image_name, img_measurements) in &by_image {
            if let Some(first) = img_measurements.first() {
                // Get image dimensions from any measurement with ratio=1.0
                let dims = img_measurements
                    .iter()
                    .find(|m| (m.ratio - 1.0).abs() < 0.01)
                    .map(|_| (512, 512)) // Default dimensions
                    .unwrap_or((512, 512));

                images.push((&first.subdir, image_name, dims.0, dims.1, img_measurements));
            }
        }
        images.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        for (category, name, width, height, img_measurements) in &images {
            output.push_str(&format!(
                "  {{\n    \"category\": \"{}\",\n    \"name\": \"{}\",\n    \"width\": {},\n    \"height\": {},\n    \"measurements\": {{\n",
                category, name, width, height
            ));

            // Add measurements as a lookup map
            let mut first_m = true;
            for m in *img_measurements {
                if !first_m {
                    output.push_str(",\n");
                }
                first_m = false;

                let ratio_int = (m.ratio * 100.0).round() as u32;
                let key = format!("{}_q{}_r{}_ppd{}", m.codec, m.quality, ratio_int, m.ppd);

                output.push_str(&format!(
                    "      \"{}\": {{\"file_size\": {}, \"dssim\": {:.8}}}",
                    key, m.file_size, m.dssim
                ));
            }

            output.push_str("\n    }\n  },\n");
        }

        output.push_str("];\n");

        // Write to comparer directory
        let comparer_dir = PathBuf::from("/mnt/v/work/corpus_cache/v2/comparer");
        std::fs::create_dir_all(&comparer_dir).expect("Failed to create comparer directory");
        let data_path = comparer_dir.join("data.js");
        std::fs::write(&data_path, &output).expect("Failed to write data.js");

        println!("Generated {} images with measurements", images.len());
        println!("Wrote: {}", data_path.display());
    }
}
