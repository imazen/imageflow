//! Calibration Data Analysis
//!
//! Analyzes the CSV output from codec_calibration.rs to produce quality mapping tables.
//!
//! ## Statistical Approach
//!
//! 1. **Central Tendency**: Median (robust) and 75th percentile (conservative)
//! 2. **Confidence**: 90% confidence intervals
//! 3. **Polynomial Fitting**: `q_out = a * q_in^b + c` with b typically 1.5-3.0
//! 4. **Per-Category Analysis**: Separate tables if categories differ significantly
//!
//! ## Running
//!
//! ```bash
//! cargo test --release --package imageflow_core --test calibration_analysis -- --nocapture --ignored
//! ```
//!
//! ## Output
//!
//! - `analysis_summary.csv`: Per (codec, quality, ratio, ppd, category) statistics
//! - `quality_equivalence.csv`: JPEG → WebP/AVIF mappings with confidence bounds
//! - `category_differences.csv`: Statistical comparison between categories
//! - `polynomial_fits.csv`: Fitted coefficients for interpolation

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

//=============================================================================
// Configuration
//=============================================================================

/// Default path to calibration CSV
const DEFAULT_CSV_PATH: &str = "/mnt/v/work/corpus_cache/results/measurements_enc1_pipe1.csv";

/// Confidence level for intervals (0.90 = 90%)
const CONFIDENCE_LEVEL: f64 = 0.90;

/// Trimmed mean percentage (0.10 = remove top/bottom 10%)
const TRIM_PERCENT: f64 = 0.10;

//=============================================================================
// Data Structures
//=============================================================================

/// A single measurement from the CSV
#[derive(Debug, Clone)]
struct Measurement {
    image: String,
    subdir: String,
    codec: String,
    quality: u32,
    ratio: f32,
    ppd: u32,
    dssim: f64,
    file_size: usize,
}

/// Statistics for a group of measurements
#[derive(Debug, Clone)]
struct GroupStats {
    codec: String,
    quality: u32,
    ratio: f32,
    ppd: u32,
    category: String, // "all" for pooled
    n: usize,
    mean: f64,
    median: f64,
    p75: f64,
    trimmed_mean: f64,
    std: f64,
    iqr: f64,
    min: f64,
    max: f64,
}

/// Quality equivalence mapping
#[derive(Debug, Clone)]
struct QualityMapping {
    jpeg_quality: u32,
    ratio: f32,
    ppd: u32,
    category: String,
    webp_quality_median: u32,
    webp_quality_p75: u32,
    avif_quality_median: u32,
    avif_quality_p75: u32,
    jpeg_dssim_median: f64,
}

/// Polynomial fit coefficients: q_out = a * q_in^b + c
#[derive(Debug, Clone)]
struct PolynomialFit {
    codec_from: String,
    codec_to: String,
    ratio: f32,
    ppd: u32,
    category: String,
    a: f64,
    b: f64, // exponent, typically 1.5-3.0
    c: f64,
    r_squared: f64,
}

/// Cross-condition quality mapping (for cross-PPD or cross-DPR)
#[derive(Debug, Clone)]
struct CrossConditionMapping {
    codec: String,
    category: String,
    // Source condition
    src_ratio: f32,
    src_ppd: u32,
    src_quality: u32,
    src_dssim: f64,
    // Target condition
    dst_ratio: f32,
    dst_ppd: u32,
    dst_quality: u32,
    dst_dssim: f64,
}

//=============================================================================
// CSV Loading
//=============================================================================

fn load_csv(path: &Path) -> Result<Vec<Measurement>, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open CSV: {}", e))?;
    let reader = BufReader::new(file);
    let mut measurements = Vec::new();

    let mut lines = reader.lines();

    // Skip header
    let header = lines.next().ok_or("Empty CSV")?.map_err(|e| e.to_string())?;
    let headers: Vec<&str> = header.split(',').collect();

    // Find column indices
    let find_col = |name: &str| -> Result<usize, String> {
        headers.iter().position(|&h| h == name)
            .ok_or_else(|| format!("Missing column: {}", name))
    };

    let col_image = find_col("image")?;
    let col_subdir = find_col("subdir")?;
    let col_codec = find_col("codec")?;
    let col_quality = find_col("quality")?;
    let col_ratio = find_col("ratio")?;
    let col_ppd = find_col("ppd")?;
    let col_dssim = find_col("dssim")?;
    let col_file_size = find_col("file_size")?;

    for line_result in lines {
        let line = line_result.map_err(|e| e.to_string())?;
        let cols: Vec<&str> = line.split(',').collect();

        if cols.len() <= col_file_size {
            continue; // Skip malformed lines
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
// Statistical Functions
//=============================================================================

fn median(values: &mut [f64]) -> f64 {
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

fn percentile(values: &mut [f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((values.len() - 1) as f64 * p) as usize;
    values[idx.min(values.len() - 1)]
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn trimmed_mean(values: &mut [f64], trim_pct: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let trim_count = (values.len() as f64 * trim_pct) as usize;
    let trimmed = &values[trim_count..values.len() - trim_count];
    mean(trimmed)
}

fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    let variance = values.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}

fn iqr(values: &mut [f64]) -> f64 {
    percentile(values, 0.75) - percentile(values, 0.25)
}

//=============================================================================
// Analysis Functions
//=============================================================================

/// Group measurements and compute statistics
fn compute_group_stats(measurements: &[Measurement]) -> Vec<GroupStats> {
    // Group by (codec, quality, ratio, ppd, category)
    let mut groups: HashMap<(String, u32, i32, u32, String), Vec<f64>> = HashMap::new();

    for m in measurements {
        // Per-category
        let key = (
            m.codec.clone(),
            m.quality,
            (m.ratio * 100.0) as i32,
            m.ppd,
            m.subdir.clone(),
        );
        groups.entry(key).or_default().push(m.dssim);

        // Pooled ("all" category)
        let key_all = (
            m.codec.clone(),
            m.quality,
            (m.ratio * 100.0) as i32,
            m.ppd,
            "all".to_string(),
        );
        groups.entry(key_all).or_default().push(m.dssim);
    }

    let mut stats = Vec::new();
    for ((codec, quality, ratio_x100, ppd, category), mut values) in groups {
        let n = values.len();
        if n == 0 {
            continue;
        }

        stats.push(GroupStats {
            codec,
            quality,
            ratio: ratio_x100 as f32 / 100.0,
            ppd,
            category,
            n,
            mean: mean(&values),
            median: median(&mut values.clone()),
            p75: percentile(&mut values.clone(), 0.75),
            trimmed_mean: trimmed_mean(&mut values.clone(), TRIM_PERCENT),
            std: std_dev(&values),
            iqr: iqr(&mut values.clone()),
            min: values.iter().cloned().fold(f64::INFINITY, f64::min),
            max: values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        });
    }

    stats.sort_by(|a, b| {
        (&a.category, &a.codec, a.quality, (a.ratio * 100.0) as i32, a.ppd)
            .cmp(&(&b.category, &b.codec, b.quality, (b.ratio * 100.0) as i32, b.ppd))
    });

    stats
}

/// Find quality equivalence mappings using per-image analysis
///
/// IMPORTANT: DSSIM values are only comparable within the same image.
/// Correct approach:
/// 1. For each image: find what WebP/AVIF quality matches each JPEG quality
/// 2. Aggregate those per-image quality mappings across images
fn compute_quality_mappings(measurements: &[Measurement]) -> Vec<QualityMapping> {
    let mut mappings = Vec::new();

    // Group measurements by image
    let mut by_image: HashMap<String, Vec<&Measurement>> = HashMap::new();
    for m in measurements {
        by_image.entry(m.image.clone()).or_default().push(m);
    }

    // Get unique conditions to compute mappings for (use i32 for ratio since f32 doesn't impl Hash)
    let mut conditions: Vec<(f32, u32, String)> = measurements
        .iter()
        .map(|m| ((m.ratio * 100.0) as i32, m.ppd, m.subdir.clone()))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .map(|(r, p, c)| (r as f32 / 100.0, p, c))
        .collect();
    conditions.push((1.0, 70, "all".to_string())); // Add pooled condition
    conditions.sort_by(|a, b| {
        (&a.2, (a.0 * 100.0) as i32, a.1).cmp(&(&b.2, (b.0 * 100.0) as i32, b.1))
    });
    conditions.dedup();

    // Get unique JPEG qualities
    let jpeg_qualities: Vec<u32> = measurements
        .iter()
        .filter(|m| m.codec == "jpeg")
        .map(|m| m.quality)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    for (ratio, ppd, category) in &conditions {
        for &jpeg_q in &jpeg_qualities {
            // For each image, find the equivalent WebP and AVIF qualities
            let mut webp_equivalents: Vec<u32> = Vec::new();
            let mut avif_equivalents: Vec<u32> = Vec::new();
            let mut jpeg_dssims: Vec<f64> = Vec::new();

            for (_img_name, img_measurements) in &by_image {
                // Filter to this condition
                let img_at_condition: Vec<_> = img_measurements
                    .iter()
                    .filter(|m| {
                        (m.ratio - ratio).abs() < 0.02 &&
                        m.ppd == *ppd &&
                        (*category == "all" || m.subdir == *category)
                    })
                    .collect();

                // Get JPEG DSSIM at this quality for this image
                let jpeg_dssim = img_at_condition.iter()
                    .find(|m| m.codec == "jpeg" && m.quality == jpeg_q)
                    .map(|m| m.dssim);

                if let Some(jd) = jpeg_dssim {
                    jpeg_dssims.push(jd);

                    // Find WebP quality with closest DSSIM for this specific image
                    let webp_match = img_at_condition.iter()
                        .filter(|m| m.codec == "webp")
                        .min_by(|a, b| {
                            (a.dssim - jd).abs().partial_cmp(&(b.dssim - jd).abs()).unwrap()
                        })
                        .map(|m| m.quality);

                    if let Some(wq) = webp_match {
                        webp_equivalents.push(wq);
                    }

                    // Find AVIF quality with closest DSSIM for this specific image
                    let avif_match = img_at_condition.iter()
                        .filter(|m| m.codec.starts_with("avif"))
                        .min_by(|a, b| {
                            (a.dssim - jd).abs().partial_cmp(&(b.dssim - jd).abs()).unwrap()
                        })
                        .map(|m| m.quality);

                    if let Some(aq) = avif_match {
                        avif_equivalents.push(aq);
                    }
                }
            }

            if webp_equivalents.is_empty() || avif_equivalents.is_empty() {
                continue;
            }

            // Aggregate per-image mappings: use median
            webp_equivalents.sort();
            avif_equivalents.sort();
            let mut jpeg_dssims_sorted = jpeg_dssims.clone();
            jpeg_dssims_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

            let webp_q_median = webp_equivalents[webp_equivalents.len() / 2];
            let avif_q_median = avif_equivalents[avif_equivalents.len() / 2];

            // For p75, use the 75th percentile of the per-image mappings
            let webp_q_p75 = webp_equivalents[(webp_equivalents.len() * 3) / 4];
            let avif_q_p75 = avif_equivalents[(avif_equivalents.len() * 3) / 4];

            let jpeg_dssim_median = jpeg_dssims_sorted[jpeg_dssims_sorted.len() / 2];

            mappings.push(QualityMapping {
                jpeg_quality: jpeg_q,
                ratio: *ratio,
                ppd: *ppd,
                category: category.clone(),
                webp_quality_median: webp_q_median,
                webp_quality_p75: webp_q_p75,
                avif_quality_median: avif_q_median,
                avif_quality_p75: avif_q_p75,
                jpeg_dssim_median,
            });
        }
    }

    mappings.sort_by(|a, b| {
        (&a.category, (a.ratio * 100.0) as i32, a.ppd, a.jpeg_quality)
            .cmp(&(&b.category, (b.ratio * 100.0) as i32, b.ppd, b.jpeg_quality))
    });

    mappings
}

fn find_closest_quality<F>(stats: &[&GroupStats], target_dssim: f64, get_dssim: F) -> u32
where
    F: Fn(&GroupStats) -> f64,
{
    stats
        .iter()
        .min_by(|a, b| {
            let diff_a = (get_dssim(a) - target_dssim).abs();
            let diff_b = (get_dssim(b) - target_dssim).abs();
            diff_a.partial_cmp(&diff_b).unwrap()
        })
        .map(|s| s.quality)
        .unwrap_or(50)
}

fn find_closest_quality_with_dssim<F>(stats: &[&GroupStats], target_dssim: f64, get_dssim: F) -> (u32, f64)
where
    F: Fn(&GroupStats) -> f64,
{
    stats
        .iter()
        .min_by(|a, b| {
            let diff_a = (get_dssim(a) - target_dssim).abs();
            let diff_b = (get_dssim(b) - target_dssim).abs();
            diff_a.partial_cmp(&diff_b).unwrap()
        })
        .map(|s| (s.quality, get_dssim(s)))
        .unwrap_or((50, 0.0))
}

/// Compute cross-condition mappings (cross-PPD and cross-DPR)
///
/// For cross-PPD: "JPEG q=50 on desktop (ppd=40) needs q=? on phone (ppd=95) for equivalent perceived quality"
/// For cross-DPR: "JPEG q=50 at 1x needs q=? at 2x for equivalent perceived quality"
fn compute_cross_condition_mappings(stats: &[GroupStats]) -> Vec<CrossConditionMapping> {
    let mut mappings = Vec::new();

    let codecs = ["jpeg", "webp", "avif_s6"];
    let categories: Vec<String> = stats.iter().map(|s| s.category.clone()).collect::<std::collections::HashSet<_>>().into_iter().collect();

    // Get unique ratios and ppds (convert ratio to i32 for HashSet since f32 doesn't impl Hash)
    let ratios: Vec<f32> = stats.iter()
        .map(|s| (s.ratio * 100.0) as i32)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .map(|r| r as f32 / 100.0)
        .collect();
    let ppds: Vec<u32> = stats.iter().map(|s| s.ppd).collect::<std::collections::HashSet<_>>().into_iter().collect();

    for codec in &codecs {
        for category in &categories {
            // Cross-PPD mappings (at ratio=1.0)
            let base_ppd = 40u32; // Desktop as baseline
            for &target_ppd in &ppds {
                if target_ppd == base_ppd {
                    continue;
                }

                let src_stats: Vec<_> = stats.iter()
                    .filter(|s| (s.codec == *codec || s.codec.starts_with(codec))
                            && s.ratio == 1.0
                            && s.ppd == base_ppd
                            && s.category == *category)
                    .collect();

                let dst_stats: Vec<_> = stats.iter()
                    .filter(|s| (s.codec == *codec || s.codec.starts_with(codec))
                            && s.ratio == 1.0
                            && s.ppd == target_ppd
                            && s.category == *category)
                    .collect();

                for src in &src_stats {
                    let (dst_q, dst_dssim) = find_closest_quality_with_dssim(&dst_stats, src.median, |s| s.median);
                    mappings.push(CrossConditionMapping {
                        codec: codec.to_string(),
                        category: category.clone(),
                        src_ratio: 1.0,
                        src_ppd: base_ppd,
                        src_quality: src.quality,
                        src_dssim: src.median,
                        dst_ratio: 1.0,
                        dst_ppd: target_ppd,
                        dst_quality: dst_q,
                        dst_dssim,
                    });
                }
            }

            // Cross-DPR mappings (at ppd=70)
            let base_ratio = 1.0f32; // 1x as baseline
            for &ratio in &ratios {
                let ratio_normalized = ((ratio * 100.0) as i32) as f32 / 100.0;
                if (ratio_normalized - base_ratio).abs() < 0.01 {
                    continue;
                }

                let src_stats: Vec<_> = stats.iter()
                    .filter(|s| (s.codec == *codec || s.codec.starts_with(codec))
                            && (s.ratio - base_ratio).abs() < 0.01
                            && s.ppd == 70
                            && s.category == *category)
                    .collect();

                let dst_stats: Vec<_> = stats.iter()
                    .filter(|s| (s.codec == *codec || s.codec.starts_with(codec))
                            && (s.ratio - ratio_normalized).abs() < 0.01
                            && s.ppd == 70
                            && s.category == *category)
                    .collect();

                for src in &src_stats {
                    let (dst_q, dst_dssim) = find_closest_quality_with_dssim(&dst_stats, src.median, |s| s.median);
                    mappings.push(CrossConditionMapping {
                        codec: codec.to_string(),
                        category: category.clone(),
                        src_ratio: base_ratio,
                        src_ppd: 70,
                        src_quality: src.quality,
                        src_dssim: src.median,
                        dst_ratio: ratio_normalized,
                        dst_ppd: 70,
                        dst_quality: dst_q,
                        dst_dssim,
                    });
                }
            }
        }
    }

    mappings.sort_by(|a, b| {
        (&a.codec, &a.category, a.src_ppd, (a.src_ratio * 100.0) as i32, a.dst_ppd, (a.dst_ratio * 100.0) as i32, a.src_quality)
            .cmp(&(&b.codec, &b.category, b.src_ppd, (b.src_ratio * 100.0) as i32, b.dst_ppd, (b.dst_ratio * 100.0) as i32, b.src_quality))
    });

    mappings
}

/// Fit polynomial: q_out = a * q_in^b + c
/// Uses grid search over b values, least squares for a and c
fn fit_polynomial(
    jpeg_stats: &[&GroupStats],
    other_stats: &[&GroupStats],
    codec_to: &str,
) -> Option<PolynomialFit> {
    if jpeg_stats.is_empty() || other_stats.is_empty() {
        return None;
    }

    // Build mapping: jpeg_q -> other_q (using median DSSIM matching)
    let mut points: Vec<(f64, f64)> = Vec::new();

    for js in jpeg_stats {
        let target = js.median;
        if let Some(os) = other_stats.iter().min_by(|a, b| {
            (a.median - target).abs().partial_cmp(&(b.median - target).abs()).unwrap()
        }) {
            points.push((js.quality as f64, os.quality as f64));
        }
    }

    if points.len() < 3 {
        return None;
    }

    // Grid search over exponent b from 1.0 to 3.5
    let mut best_fit: Option<(f64, f64, f64, f64)> = None; // (a, b, c, r_squared)

    for b_x10 in 10..=35 {
        let b = b_x10 as f64 / 10.0;

        // Transform: let x' = q_in^b, then fit q_out = a*x' + c (linear regression)
        let x_transformed: Vec<f64> = points.iter().map(|(x, _)| x.powf(b)).collect();
        let y: Vec<f64> = points.iter().map(|(_, y)| *y).collect();

        // Linear regression for a and c
        let n = points.len() as f64;
        let sum_x = x_transformed.iter().sum::<f64>();
        let sum_y = y.iter().sum::<f64>();
        let sum_xy: f64 = x_transformed.iter().zip(&y).map(|(x, y)| x * y).sum();
        let sum_x2: f64 = x_transformed.iter().map(|x| x * x).sum();

        let denom = n * sum_x2 - sum_x * sum_x;
        if denom.abs() < 1e-10 {
            continue;
        }

        let a = (n * sum_xy - sum_x * sum_y) / denom;
        let c = (sum_y - a * sum_x) / n;

        // Compute R²
        let y_mean = sum_y / n;
        let ss_tot: f64 = y.iter().map(|yi| (yi - y_mean).powi(2)).sum();
        let ss_res: f64 = x_transformed.iter().zip(&y)
            .map(|(xi, yi)| (yi - (a * xi + c)).powi(2))
            .sum();

        let r_squared = if ss_tot > 0.0 { 1.0 - ss_res / ss_tot } else { 0.0 };

        if best_fit.is_none() || r_squared > best_fit.unwrap().3 {
            best_fit = Some((a, b, c, r_squared));
        }
    }

    let (a, b, c, r_squared) = best_fit?;
    let ratio = jpeg_stats[0].ratio;
    let ppd = jpeg_stats[0].ppd;
    let category = jpeg_stats[0].category.clone();

    Some(PolynomialFit {
        codec_from: "jpeg".to_string(),
        codec_to: codec_to.to_string(),
        ratio,
        ppd,
        category,
        a,
        b,
        c,
        r_squared,
    })
}

/// Compute polynomial fits for all conditions
fn compute_polynomial_fits(stats: &[GroupStats]) -> Vec<PolynomialFit> {
    let mut fits = Vec::new();

    // Get unique (ratio, ppd, category) combinations
    let mut conditions: Vec<(f32, u32, String)> = stats
        .iter()
        .filter(|s| s.category == "all") // Only fit on pooled data for now
        .map(|s| (s.ratio, s.ppd, s.category.clone()))
        .collect();
    conditions.sort_by(|a, b| {
        ((a.0 * 100.0) as i32, a.1).cmp(&((b.0 * 100.0) as i32, b.1))
    });
    conditions.dedup();

    for (ratio, ppd, category) in conditions {
        let jpeg_stats: Vec<_> = stats
            .iter()
            .filter(|s| s.codec == "jpeg" && s.ratio == ratio && s.ppd == ppd && s.category == category)
            .collect();

        let webp_stats: Vec<_> = stats
            .iter()
            .filter(|s| s.codec == "webp" && s.ratio == ratio && s.ppd == ppd && s.category == category)
            .collect();

        let avif_stats: Vec<_> = stats
            .iter()
            .filter(|s| s.codec.starts_with("avif") && s.ratio == ratio && s.ppd == ppd && s.category == category)
            .collect();

        if let Some(fit) = fit_polynomial(&jpeg_stats, &webp_stats, "webp") {
            fits.push(fit);
        }
        if let Some(fit) = fit_polynomial(&jpeg_stats, &avif_stats, "avif") {
            fits.push(fit);
        }
    }

    fits
}

//=============================================================================
// CSV Output
//=============================================================================

fn write_stats_csv(stats: &[GroupStats], path: &Path) -> Result<(), String> {
    let mut file = File::create(path).map_err(|e| e.to_string())?;

    writeln!(file, "codec,quality,ratio,ppd,category,n,mean,median,p75,trimmed_mean,std,iqr,min,max")
        .map_err(|e| e.to_string())?;

    for s in stats {
        writeln!(
            file,
            "{},{},{:.2},{},{},{},{:.8},{:.8},{:.8},{:.8},{:.8},{:.8},{:.8},{:.8}",
            s.codec, s.quality, s.ratio, s.ppd, s.category,
            s.n, s.mean, s.median, s.p75, s.trimmed_mean, s.std, s.iqr, s.min, s.max
        ).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn write_mappings_csv(mappings: &[QualityMapping], path: &Path) -> Result<(), String> {
    let mut file = File::create(path).map_err(|e| e.to_string())?;

    writeln!(file, "jpeg_quality,ratio,ppd,category,webp_q_median,webp_q_p75,avif_q_median,avif_q_p75,jpeg_dssim_median")
        .map_err(|e| e.to_string())?;

    for m in mappings {
        writeln!(
            file,
            "{},{:.2},{},{},{},{},{},{},{:.8}",
            m.jpeg_quality, m.ratio, m.ppd, m.category,
            m.webp_quality_median, m.webp_quality_p75,
            m.avif_quality_median, m.avif_quality_p75,
            m.jpeg_dssim_median
        ).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn write_fits_csv(fits: &[PolynomialFit], path: &Path) -> Result<(), String> {
    let mut file = File::create(path).map_err(|e| e.to_string())?;

    writeln!(file, "codec_from,codec_to,ratio,ppd,category,a,b,c,r_squared")
        .map_err(|e| e.to_string())?;

    for f in fits {
        writeln!(
            file,
            "{},{},{:.2},{},{},{:.8},{:.4},{:.8},{:.6}",
            f.codec_from, f.codec_to, f.ratio, f.ppd, f.category,
            f.a, f.b, f.c, f.r_squared
        ).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn write_cross_condition_csv(mappings: &[CrossConditionMapping], path: &Path) -> Result<(), String> {
    let mut file = File::create(path).map_err(|e| e.to_string())?;

    writeln!(file, "codec,category,src_ratio,src_ppd,src_quality,src_dssim,dst_ratio,dst_ppd,dst_quality,dst_dssim,quality_delta")
        .map_err(|e| e.to_string())?;

    for m in mappings {
        let quality_delta = m.dst_quality as i32 - m.src_quality as i32;
        writeln!(
            file,
            "{},{},{:.2},{},{},{:.8},{:.2},{},{},{:.8},{}",
            m.codec, m.category, m.src_ratio, m.src_ppd, m.src_quality, m.src_dssim,
            m.dst_ratio, m.dst_ppd, m.dst_quality, m.dst_dssim, quality_delta
        ).map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Interpolate quality value given DSSIM target and sorted (quality, dssim) pairs
/// Returns interpolated quality as f64
fn interpolate_quality(target_dssim: f64, quality_dssim_pairs: &[(u32, f64)]) -> Option<f64> {
    if quality_dssim_pairs.len() < 2 {
        return quality_dssim_pairs.first().map(|(q, _)| *q as f64);
    }

    // Sort by quality
    let mut pairs = quality_dssim_pairs.to_vec();
    pairs.sort_by_key(|(q, _)| *q);

    // DSSIM typically decreases as quality increases
    // Find two adjacent points that bracket the target DSSIM
    for i in 0..pairs.len() - 1 {
        let (q1, d1) = pairs[i];
        let (q2, d2) = pairs[i + 1];

        // Check if target falls between these DSSIM values (in either direction)
        let in_range = (d1 <= target_dssim && target_dssim <= d2) ||
                       (d2 <= target_dssim && target_dssim <= d1);

        if in_range && (d2 - d1).abs() > 1e-12 {
            // Linear interpolation between quality values
            let t = (target_dssim - d1) / (d2 - d1);
            let interp_q = q1 as f64 + t * (q2 as f64 - q1 as f64);
            return Some(interp_q.clamp(0.0, 100.0));
        }
    }

    // Target outside measured range - find closest and extrapolate carefully
    // Just return the closest measured quality
    pairs.iter()
        .min_by(|a, b| (a.1 - target_dssim).abs().partial_cmp(&(b.1 - target_dssim).abs()).unwrap())
        .map(|(q, _)| *q as f64)
}

/// Generate compact lookup tables with interpolation
///
/// Reference condition: 1x-desktop (ppd=40, ratio=1.0) - most demanding viewing
/// Uses per-image analysis with interpolation for precise quality values
fn generate_compact_lookup_tables(
    measurements: &[Measurement],
    output_dir: &Path,
) -> Result<(), String> {
    let mut file = File::create(output_dir.join("quality_lookup_tables.txt"))
        .map_err(|e| e.to_string())?;

    // Reference qualities (at 1x-desktop)
    let ref_qualities: Vec<u32> = vec![30, 40, 50, 60, 70, 80, 85, 90, 95];

    // All possible target conditions for reference
    let _all_conditions = [
        (1.0f32, 40u32, "1x-desktop"),
        (1.0, 70, "1x-laptop"),
        (1.0, 95, "1x-phone"),
        (1.5, 40, "1.5x-desktop"),
        (1.5, 70, "1.5x-laptop"),
        (1.5, 95, "1.5x-phone"),
        (2.0, 40, "2x-desktop"),
        (2.0, 70, "2x-laptop"),
        (2.0, 95, "2x-phone"),
    ];

    // Codecs to analyze
    let codecs = ["jpeg", "webp", "avif_s6"];

    // Group measurements by image
    let mut by_image: HashMap<String, Vec<&Measurement>> = HashMap::new();
    for m in measurements {
        by_image.entry(m.image.clone()).or_default().push(m);
    }

    writeln!(file, "# Quality Lookup Tables with Interpolation").map_err(|e| e.to_string())?;
    writeln!(file, "#").map_err(|e| e.to_string())?;
    writeln!(file, "# Reference condition: 1x-desktop (ratio=1.0, ppd=40)").map_err(|e| e.to_string())?;
    writeln!(file, "# This is the most demanding viewing scenario (large visible pixels)").map_err(|e| e.to_string())?;
    writeln!(file, "#").map_err(|e| e.to_string())?;
    writeln!(file, "# Values are interpolated (not just nearest measured quality)").map_err(|e| e.to_string())?;
    writeln!(file, "# Per-image analysis: find equivalent quality per image, then median").map_err(|e| e.to_string())?;
    writeln!(file, "#").map_err(|e| e.to_string())?;
    writeln!(file, "# Higher srcset OR higher PPD → can use LOWER quality").map_err(|e| e.to_string())?;
    writeln!(file).map_err(|e| e.to_string())?;

    // Helper function to compute percentiles
    fn compute_all_percentiles(mut values: Vec<f64>) -> Option<(f64, f64, f64, f64)> {
        if values.is_empty() {
            return None;
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = values.len();
        let p50 = values[n / 2];
        let p75 = values[(n * 3 / 4).min(n - 1)];
        let p90 = values[(n * 9 / 10).min(n - 1)];
        let p95 = values[(n * 95 / 100).min(n - 1)];
        Some((p50, p75, p90, p95))
    }

    fn compute_percentiles(mut values: Vec<f64>) -> Option<(f64, f64)> {
        if values.is_empty() {
            return None;
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = values[values.len() / 2];
        let p75_idx = (values.len() * 3) / 4;
        let p75 = values[p75_idx.min(values.len() - 1)];
        Some((median, p75))
    }

    // Collect all quality mappings into a data structure for each codec
    // Structure: codec -> ref_q -> (target_ratio, target_ppd) -> Vec<f64> (per-image qualities)

    // Key conditions for compact tables
    let key_conditions = [
        (1.0f32, 40u32, "1x-desktop"),
        (1.0, 95, "1x-phone"),
        (1.5, 70, "1.5x-laptop"),
        (1.5, 95, "1.5x-phone"),
        (2.0, 95, "2x-phone"),
    ];

    // Percentile table names
    let percentiles = [
        ("P50 (Median)", 50),
        ("P75 (Conservative)", 75),
        ("P90 (Safe)", 90),
        ("P95 (Very Safe)", 95),
    ];

    // Generate tables for each codec
    for codec in &codecs {
        writeln!(file, "\n\n# {} QUALITY MAPPINGS", codec.to_uppercase()).map_err(|e| e.to_string())?;
        writeln!(file, "# Reference: {} at 1x-desktop (ratio=1.0, ppd=40)", codec.to_uppercase()).map_err(|e| e.to_string())?;
        writeln!(file, "#").map_err(|e| e.to_string())?;
        writeln!(file, "# Higher percentile = more conservative (handles more difficult images)").map_err(|e| e.to_string())?;
        writeln!(file, "# P50: works for 50% of images, P95: works for 95% of images").map_err(|e| e.to_string())?;

        for (pct_name, pct_value) in &percentiles {
            writeln!(file, "\n## {} - {} → {} at other conditions\n", codec.to_uppercase(), pct_name, codec.to_uppercase())
                .map_err(|e| e.to_string())?;

            write!(file, "{:>6} |", "Ref q").map_err(|e| e.to_string())?;
            for (_, _, name) in &key_conditions {
                write!(file, " {:>12}", name).map_err(|e| e.to_string())?;
            }
            writeln!(file).map_err(|e| e.to_string())?;
            writeln!(file, "{}", "-".repeat(7 + 13 * key_conditions.len())).map_err(|e| e.to_string())?;

            for &ref_q in &ref_qualities {
                write!(file, "{:>6} |", ref_q).map_err(|e| e.to_string())?;

                for (target_ratio, target_ppd, _name) in &key_conditions {
                    let mut interpolated_qualities: Vec<f64> = Vec::new();

                    for (_img_name, img_measurements) in &by_image {
                        let ref_dssim = img_measurements.iter()
                            .find(|m| {
                                (m.codec == *codec || m.codec.starts_with(codec)) &&
                                (m.ratio - 1.0).abs() < 0.02 &&
                                m.ppd == 40 &&
                                m.quality == ref_q
                            })
                            .map(|m| m.dssim);

                        if let Some(rd) = ref_dssim {
                            let pairs: Vec<(u32, f64)> = img_measurements.iter()
                                .filter(|m| {
                                    (m.codec == *codec || m.codec.starts_with(codec)) &&
                                    (m.ratio - target_ratio).abs() < 0.02 &&
                                    m.ppd == *target_ppd
                                })
                                .map(|m| (m.quality, m.dssim))
                                .collect();

                            if let Some(iq) = interpolate_quality(rd, &pairs) {
                                interpolated_qualities.push(iq);
                            }
                        }
                    }

                    let result = if interpolated_qualities.is_empty() {
                        "-".to_string()
                    } else {
                        interpolated_qualities.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        let n = interpolated_qualities.len();
                        let idx = (n * *pct_value / 100).min(n - 1);
                        format!("{:.0}", interpolated_qualities[idx])
                    };

                    write!(file, " {:>12}", result).map_err(|e| e.to_string())?;
                }
                writeln!(file).map_err(|e| e.to_string())?;
            }
        }
    }

    // Cross-codec tables: JPEG → WebP and JPEG → AVIF
    let cross_codec_pairs = [
        ("jpeg", "webp", "JPEG", "WebP"),
        ("jpeg", "avif", "JPEG", "AVIF"),
    ];

    for (src_codec, dst_codec, src_name, dst_name) in &cross_codec_pairs {
        writeln!(file, "\n\n# CROSS-CODEC: {} 1x-desktop → {}", src_name, dst_name).map_err(|e| e.to_string())?;
        writeln!(file, "# Maps {} quality at reference condition to equivalent {} quality", src_name, dst_name).map_err(|e| e.to_string())?;

        for (pct_name, pct_value) in &percentiles {
            writeln!(file, "\n## {} 1x-desktop → {} - {}\n", src_name, dst_name, pct_name)
                .map_err(|e| e.to_string())?;

            write!(file, "{:>6} |", src_name).map_err(|e| e.to_string())?;
            for (_, _, name) in &key_conditions {
                write!(file, " {:>12}", name).map_err(|e| e.to_string())?;
            }
            writeln!(file).map_err(|e| e.to_string())?;
            writeln!(file, "{}", "-".repeat(7 + 13 * key_conditions.len())).map_err(|e| e.to_string())?;

            for &ref_q in &ref_qualities {
                write!(file, "{:>6} |", ref_q).map_err(|e| e.to_string())?;

                for (target_ratio, target_ppd, _) in &key_conditions {
                    let mut interpolated: Vec<f64> = Vec::new();

                    for (_, img_m) in &by_image {
                        let ref_dssim = img_m.iter()
                            .find(|m| m.codec == *src_codec && (m.ratio - 1.0).abs() < 0.02 && m.ppd == 40 && m.quality == ref_q)
                            .map(|m| m.dssim);

                        if let Some(rd) = ref_dssim {
                            let pairs: Vec<_> = img_m.iter()
                                .filter(|m| (m.codec == *dst_codec || m.codec.starts_with(dst_codec))
                                        && (m.ratio - target_ratio).abs() < 0.02
                                        && m.ppd == *target_ppd)
                                .map(|m| (m.quality, m.dssim))
                                .collect();
                            if let Some(iq) = interpolate_quality(rd, &pairs) {
                                interpolated.push(iq);
                            }
                        }
                    }

                    let result = if interpolated.is_empty() {
                        "-".to_string()
                    } else {
                        interpolated.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        let n = interpolated.len();
                        let idx = (n * *pct_value / 100).min(n - 1);
                        format!("{:.0}", interpolated[idx])
                    };
                    write!(file, " {:>12}", result).map_err(|e| e.to_string())?;
                }
                writeln!(file).map_err(|e| e.to_string())?;
            }
        }
    }

    writeln!(file, "\n\n## Notes").map_err(|e| e.to_string())?;
    writeln!(file, "- Reference: 1x-desktop is the most demanding condition").map_err(|e| e.to_string())?;
    writeln!(file, "- Values show equivalent quality needed to match reference DSSIM").map_err(|e| e.to_string())?;
    writeln!(file, "- Lower values in other columns = savings from less demanding conditions").map_err(|e| e.to_string())?;
    writeln!(file, "- Based on {} test images with per-image interpolation", by_image.len()).map_err(|e| e.to_string())?;

    Ok(())
}

//=============================================================================
// Main Analysis
//=============================================================================

pub fn run_analysis(csv_path: &Path, output_dir: &Path) -> Result<(), String> {
    println!("=== Calibration Data Analysis ===\n");

    // Load data
    println!("Loading: {}", csv_path.display());
    let measurements = load_csv(csv_path)?;
    println!("Loaded {} measurements\n", measurements.len());

    // Compute statistics
    println!("Computing group statistics...");
    let stats = compute_group_stats(&measurements);
    println!("  {} groups\n", stats.len());

    // Compute quality mappings (per-image first, then aggregate)
    println!("Computing quality equivalence mappings (per-image analysis)...");
    let mappings = compute_quality_mappings(&measurements);
    println!("  {} mappings\n", mappings.len());

    // Compute polynomial fits
    println!("Fitting polynomial models...");
    let fits = compute_polynomial_fits(&stats);
    println!("  {} fits\n", fits.len());

    // Compute cross-condition mappings (cross-PPD and cross-DPR)
    println!("Computing cross-condition mappings (PPD/DPR)...");
    let cross_mappings = compute_cross_condition_mappings(&stats);
    println!("  {} cross-condition mappings\n", cross_mappings.len());

    // Print sample fits
    println!("Sample polynomial fits (q_out = a * q_in^b + c):");
    for f in fits.iter().filter(|f| f.ratio == 1.0 && f.ppd == 70).take(4) {
        println!(
            "  {} -> {}: a={:.4}, b={:.2}, c={:.2}, R²={:.4}",
            f.codec_from, f.codec_to, f.a, f.b, f.c, f.r_squared
        );
    }
    println!();

    // Write outputs
    fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;

    let stats_path = output_dir.join("analysis_summary.csv");
    write_stats_csv(&stats, &stats_path)?;
    println!("Wrote: {}", stats_path.display());

    let mappings_path = output_dir.join("quality_equivalence.csv");
    write_mappings_csv(&mappings, &mappings_path)?;
    println!("Wrote: {}", mappings_path.display());

    let fits_path = output_dir.join("polynomial_fits.csv");
    write_fits_csv(&fits, &fits_path)?;
    println!("Wrote: {}", fits_path.display());

    let cross_path = output_dir.join("cross_condition_mappings.csv");
    write_cross_condition_csv(&cross_mappings, &cross_path)?;
    println!("Wrote: {}", cross_path.display());

    // Generate compact lookup tables (per-image analysis)
    generate_compact_lookup_tables(&measurements, output_dir)?;
    println!("Wrote: {}", output_dir.join("quality_lookup_tables.txt").display());

    // Print summary table for ratio=1.0, ppd=70, category=all
    println!("\n=== Quality Equivalence (ratio=1.0, ppd=70, pooled) ===");
    println!("{:>8} | {:>12} | {:>10} {:>10} | {:>10} {:>10}",
             "JPEG q", "JPEG DSSIM", "WebP med", "WebP p75", "AVIF med", "AVIF p75");
    println!("{}", "-".repeat(75));

    for m in mappings.iter().filter(|m| m.ratio == 1.0 && m.ppd == 70 && m.category == "all") {
        println!(
            "{:>8} | {:>12.6} | {:>10} {:>10} | {:>10} {:>10}",
            m.jpeg_quality, m.jpeg_dssim_median,
            m.webp_quality_median, m.webp_quality_p75,
            m.avif_quality_median, m.avif_quality_p75
        );
    }

    // Print cross-PPD summary (JPEG desktop→phone)
    println!("\n=== Cross-PPD: JPEG Desktop (ppd=40) → Phone (ppd=95), pooled ===");
    println!("{:>10} | {:>10} | {:>12}",
             "Desktop q", "Phone q", "Delta");
    println!("{}", "-".repeat(40));

    for m in cross_mappings.iter()
        .filter(|m| m.codec == "jpeg" && m.category == "all"
                && m.src_ppd == 40 && m.dst_ppd == 95
                && m.src_ratio == 1.0 && m.dst_ratio == 1.0)
    {
        let delta = m.dst_quality as i32 - m.src_quality as i32;
        println!(
            "{:>10} | {:>10} | {:>+12}",
            m.src_quality, m.dst_quality, delta
        );
    }

    // Print cross-DPR summary (JPEG 1x→2x)
    println!("\n=== Cross-DPR: JPEG 1x → 2x (ppd=70), pooled ===");
    println!("{:>10} | {:>10} | {:>12}",
             "1x q", "2x q", "Delta");
    println!("{}", "-".repeat(40));

    for m in cross_mappings.iter()
        .filter(|m| m.codec == "jpeg" && m.category == "all"
                && m.src_ratio == 1.0 && (m.dst_ratio - 0.5).abs() < 0.01
                && m.src_ppd == 70 && m.dst_ppd == 70)
    {
        let delta = m.dst_quality as i32 - m.src_quality as i32;
        println!(
            "{:>10} | {:>10} | {:>+12}",
            m.src_quality, m.dst_quality, delta
        );
    }

    println!("\n=== Analysis Complete ===");
    Ok(())
}

//=============================================================================
// Tests
//=============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_run_analysis() {
        let csv_path = PathBuf::from(DEFAULT_CSV_PATH);
        let output_dir = csv_path.parent().unwrap().join("analysis");

        if let Err(e) = run_analysis(&csv_path, &output_dir) {
            eprintln!("Analysis failed: {}", e);
        }
    }

    #[test]
    fn test_statistics() {
        let mut values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];

        assert_eq!(median(&mut values.clone()), 5.5);
        assert_eq!(mean(&values), 5.5);
        assert!((percentile(&mut values.clone(), 0.75) - 8.0).abs() < 0.1);
        assert!((trimmed_mean(&mut values.clone(), 0.1) - 5.5).abs() < 0.1);
    }
}
