#![forbid(unsafe_code)]
//! Composite saliency detection for smart cropping.
//!
//! Combines three simple pixel-level signals:
//! 1. Edge detection (Laplacian on luminance)
//! 2. Skin tone detection (YCbCr chrominance range, inclusive of Fitzpatrick I-VI)
//! 3. Saturation detection (HSL saturation above threshold)
//!
//! Optimized for cache efficiency: two passes over pixels (fused signal
//! computation + edge detection) and O(1)-per-pixel prefix-sum blur.

use crate::AnalysisConfig;
use imageflow_types::{FocusKind, FocusRect};
use multiversion::multiversion;

/// Pre-allocated work buffers to avoid per-call allocations.
struct WorkBuffers {
    luminance: Vec<f32>,
    score: Vec<f32>,
    blur_temp: Vec<f32>,
    prefix: Vec<f64>,
}

impl WorkBuffers {
    fn new(pixel_count: usize, max_dim: usize) -> Self {
        Self {
            luminance: vec![0.0f32; pixel_count],
            score: vec![0.0f32; pixel_count],
            blur_temp: vec![0.0f32; pixel_count],
            // Prefix buffer needs max_dim+1 entries for the row/column prefix sums
            prefix: vec![0.0f64; max_dim + 1],
        }
    }
}

/// Analyze a BGRA32 image for salient regions.
pub(crate) fn analyze(
    pixels: &[u8],
    width: u32,
    height: u32,
    config: &AnalysisConfig,
) -> Vec<FocusRect> {
    if width == 0 || height == 0 || pixels.len() < (width * height * 4) as usize {
        return vec![];
    }

    // Downsample to working resolution
    let (work_w, work_h, work_pixels) =
        downsample_bgra(pixels, width, height, config.max_working_size);
    if work_w < 3 || work_h < 3 {
        return vec![];
    }

    let w = work_w as usize;
    let h = work_h as usize;
    let pixel_count = w * h;

    let mut bufs = WorkBuffers::new(pixel_count, w.max(h));

    // Compute white balance compensation parameters
    let (cb_center, cr_center) = if config.white_balance_compensate {
        compute_wb_adjusted_centers(&work_pixels, pixel_count)
    } else {
        (102.0f32, 153.0f32)
    };

    // Pass 1: Fused signal computation — single pass over BGRA pixels.
    // Produces luminance buffer (for edge detection) and initial score buffer
    // (skin*weight + saturation*weight).
    compute_all_signals(
        &work_pixels,
        &mut bufs.luminance,
        &mut bufs.score,
        cb_center,
        cr_center,
        config.skin_weight,
        config.saturation_weight,
    );

    // Pass 2: Edge detection (Laplacian on luminance), added to score buffer.
    compute_edge_map(&bufs.luminance, &mut bufs.score, w, h, config.edge_weight);

    // Prefix-sum box blur: O(1) per pixel regardless of radius
    let blur_radius = (work_w.min(work_h) / 8).max(2) as usize;
    prefix_sum_blur(&bufs.score, &mut bufs.blur_temp, &mut bufs.prefix, w, h, blur_radius);

    // Find peak and derive bounding box from thresholded region
    extract_focus_rects(&bufs.blur_temp, work_w, work_h, config.min_rect_area_percent)
}

/// Downsample BGRA pixels using bilinear interpolation.
fn downsample_bgra(pixels: &[u8], width: u32, height: u32, max_dim: u32) -> (u32, u32, Vec<u8>) {
    if width <= max_dim && height <= max_dim {
        return (width, height, pixels.to_vec());
    }

    let scale = max_dim as f32 / width.max(height) as f32;
    let new_w = ((width as f32 * scale).round() as u32).max(1);
    let new_h = ((height as f32 * scale).round() as u32).max(1);

    let mut out = vec![0u8; (new_w * new_h * 4) as usize];
    let x_ratio = width as f32 / new_w as f32;
    let y_ratio = height as f32 / new_h as f32;

    for dy in 0..new_h {
        for dx in 0..new_w {
            let sx = ((dx as f32 + 0.5) * x_ratio - 0.5).max(0.0).min((width - 1) as f32);
            let sy = ((dy as f32 + 0.5) * y_ratio - 0.5).max(0.0).min((height - 1) as f32);

            let x0 = sx.floor() as u32;
            let y0 = sy.floor() as u32;
            let x1 = (x0 + 1).min(width - 1);
            let y1 = (y0 + 1).min(height - 1);
            let fx = sx - x0 as f32;
            let fy = sy - y0 as f32;

            let idx00 = ((y0 * width + x0) * 4) as usize;
            let idx10 = ((y0 * width + x1) * 4) as usize;
            let idx01 = ((y1 * width + x0) * 4) as usize;
            let idx11 = ((y1 * width + x1) * 4) as usize;

            let out_idx = ((dy * new_w + dx) * 4) as usize;
            for c in 0..4 {
                let v00 = pixels[idx00 + c] as f32;
                let v10 = pixels[idx10 + c] as f32;
                let v01 = pixels[idx01 + c] as f32;
                let v11 = pixels[idx11 + c] as f32;
                let v = v00 * (1.0 - fx) * (1.0 - fy)
                    + v10 * fx * (1.0 - fy)
                    + v01 * (1.0 - fx) * fy
                    + v11 * fx * fy;
                out[out_idx + c] = v.round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    (new_w, new_h, out)
}

/// Compute white-balance-adjusted detection range centers.
fn compute_wb_adjusted_centers(pixels: &[u8], count: usize) -> (f32, f32) {
    let mut cb_center = 102.0f32;
    let mut cr_center = 153.0f32;

    let (mean_cb, mean_cr) = compute_mean_chroma(pixels, count);

    let cb_shift = mean_cb - 128.0;
    let cr_shift = mean_cr - 128.0;

    if cb_shift.abs() > 3.0 {
        cb_center += cb_shift * 0.5;
    }
    if cr_shift.abs() > 3.0 {
        cr_center += cr_shift * 0.5;
    }

    (cb_center, cr_center)
}

/// Compute image-wide mean Cb and Cr values for white balance estimation.
fn compute_mean_chroma(pixels: &[u8], count: usize) -> (f32, f32) {
    let mut sum_cb = 0.0f64;
    let mut sum_cr = 0.0f64;

    for i in 0..count {
        let idx = i * 4;
        let b_val = pixels[idx] as f64;
        let g_val = pixels[idx + 1] as f64;
        let r_val = pixels[idx + 2] as f64;

        sum_cb += -0.169 * r_val - 0.331 * g_val + 0.500 * b_val + 128.0;
        sum_cr += 0.500 * r_val - 0.419 * g_val - 0.081 * b_val + 128.0;
    }

    let n = count as f64;
    ((sum_cb / n) as f32, (sum_cr / n) as f32)
}

/// Fused Pass 1: Single pass over BGRA pixels computing luminance, skin score,
/// and saturation score. Writes luminance to `lum_out` and weighted
/// (skin + saturation) score to `score_out`.
///
/// This replaces 3 separate passes (compute_luminance + compute_skin_map +
/// compute_saturation_map + score combination), halving memory traffic.
#[multiversion(targets("x86_64+avx2+fma", "x86_64+avx2", "aarch64+neon", "x86_64+sse4.1"))]
fn compute_all_signals(
    pixels: &[u8],
    lum_out: &mut [f32],
    score_out: &mut [f32],
    cb_center: f32,
    cr_center: f32,
    skin_weight: f32,
    sat_weight: f32,
) {
    let count = lum_out.len();
    let cb_half = 25.0f32;
    let cr_half = 20.0f32;
    let cb_lo = cb_center - cb_half;
    let cb_hi = cb_center + cb_half;
    let cr_lo = cr_center - cr_half;
    let cr_hi = cr_center + cr_half;
    let sat_threshold = 0.4f32;

    for i in 0..count {
        let idx = i * 4;
        let b_u8 = pixels[idx];
        let g_u8 = pixels[idx + 1];
        let r_u8 = pixels[idx + 2];

        let b_f = b_u8 as f32;
        let g_f = g_u8 as f32;
        let r_f = r_u8 as f32;

        // BT.709 luminance (0.0-1.0)
        let lum = (0.2126 * r_f + 0.7152 * g_f + 0.0722 * b_f) / 255.0;
        lum_out[i] = lum;

        let mut pixel_score = 0.0f32;

        // --- Skin detection (YCbCr chrominance) ---
        if skin_weight > 0.0 {
            let y = 0.299 * r_f + 0.587 * g_f + 0.114 * b_f;
            if y > 40.0 {
                let cb = -0.169 * r_f - 0.331 * g_f + 0.500 * b_f + 128.0;
                let cr = 0.500 * r_f - 0.419 * g_f - 0.081 * b_f + 128.0;

                if cb >= cb_lo && cb <= cb_hi && cr >= cr_lo && cr <= cr_hi {
                    let cb_norm = ((cb - cb_center) / cb_half).abs();
                    let cr_norm = ((cr - cr_center) / cr_half).abs();
                    let dist = cb_norm.max(cr_norm);
                    pixel_score += (1.0 - dist).max(0.0) * skin_weight;
                }
            }
        }

        // --- Saturation detection (HSL) ---
        if sat_weight > 0.0 {
            let r_n = r_f / 255.0;
            let g_n = g_f / 255.0;
            let b_n = b_f / 255.0;

            let max_c = r_n.max(g_n).max(b_n);
            let min_c = r_n.min(g_n).min(b_n);
            let lightness = (max_c + min_c) / 2.0;

            if (0.05..=0.9).contains(&lightness) {
                let delta = max_c - min_c;
                if delta >= f32::EPSILON {
                    let saturation = if lightness <= 0.5 {
                        delta / (max_c + min_c)
                    } else {
                        delta / (2.0 - max_c - min_c)
                    };

                    if saturation > sat_threshold {
                        pixel_score +=
                            (saturation - sat_threshold) / (1.0 - sat_threshold) * sat_weight;
                    }
                }
            }
        }

        score_out[i] = pixel_score;
    }
}

/// Pass 2: Edge detection using 3x3 Laplacian kernel on luminance.
/// Adds edge_weight * edge_strength to the existing score buffer.
#[multiversion(targets("x86_64+avx2+fma", "x86_64+avx2", "aarch64+neon", "x86_64+sse4.1"))]
fn compute_edge_map(luminance: &[f32], score: &mut [f32], w: usize, h: usize, edge_weight: f32) {
    if edge_weight <= 0.0 {
        return;
    }
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let idx = y * w + x;
            let center = luminance[idx];
            let top = luminance[(y - 1) * w + x];
            let bottom = luminance[(y + 1) * w + x];
            let left = luminance[y * w + (x - 1)];
            let right = luminance[y * w + (x + 1)];
            let laplacian = (4.0 * center - top - bottom - left - right).abs();
            score[idx] += laplacian.min(1.0) * edge_weight;
        }
    }
}

/// O(1)-per-pixel separable box blur using prefix sums.
///
/// Two passes: horizontal then vertical. Each uses a running prefix sum
/// so the blur cost is independent of radius.
fn prefix_sum_blur(
    input: &[f32],
    output: &mut [f32],
    prefix: &mut [f64],
    width: usize,
    height: usize,
    radius: usize,
) {
    if radius == 0 {
        output.copy_from_slice(input);
        return;
    }

    // Horizontal pass: input → output
    prefix_sum_blur_h(input, output, prefix, width, height, radius);

    // Vertical pass: output → output (in-place via prefix buffer)
    prefix_sum_blur_v(output, prefix, width, height, radius);
}

/// Horizontal prefix-sum blur pass.
#[multiversion(targets("x86_64+avx2+fma", "x86_64+avx2", "aarch64+neon", "x86_64+sse4.1"))]
fn prefix_sum_blur_h(
    input: &[f32],
    output: &mut [f32],
    prefix: &mut [f64],
    width: usize,
    height: usize,
    radius: usize,
) {
    for y in 0..height {
        let row_off = y * width;

        // Build prefix sum for this row (1-indexed: prefix[0]=0, prefix[k]=sum of first k elements)
        prefix[0] = 0.0;
        for x in 0..width {
            prefix[x + 1] = prefix[x] + input[row_off + x] as f64;
        }

        // Compute blurred values using prefix sum lookups
        for x in 0..width {
            let lo = (x + 1).saturating_sub(radius + 1); // inclusive left
            let hi = (x + radius + 1).min(width); // exclusive right
            let count = (hi - lo) as f64;
            output[row_off + x] = ((prefix[hi] - prefix[lo]) / count) as f32;
        }
    }
}

/// Vertical prefix-sum blur pass (in-place).
#[multiversion(targets("x86_64+avx2+fma", "x86_64+avx2", "aarch64+neon", "x86_64+sse4.1"))]
fn prefix_sum_blur_v(
    data: &mut [f32],
    prefix: &mut [f64],
    width: usize,
    height: usize,
    radius: usize,
) {
    for x in 0..width {
        // Build prefix sum for this column
        prefix[0] = 0.0;
        for y in 0..height {
            prefix[y + 1] = prefix[y] + data[y * width + x] as f64;
        }

        // Compute blurred values
        for y in 0..height {
            let lo = (y + 1).saturating_sub(radius + 1);
            let hi = (y + radius + 1).min(height);
            let count = (hi - lo) as f64;
            data[y * width + x] = ((prefix[hi] - prefix[lo]) / count) as f32;
        }
    }
}

/// Extract focus rects from a blurred score map by thresholding at
/// a fraction of the peak value and finding the bounding box.
fn extract_focus_rects(
    score_map: &[f32],
    width: u32,
    height: u32,
    min_area_percent: f32,
) -> Vec<FocusRect> {
    let w = width as usize;
    let h = height as usize;

    let max_score = score_map.iter().copied().fold(0.0f32, f32::max);
    if max_score <= f32::EPSILON {
        return vec![];
    }

    let threshold = max_score * 0.50;

    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0usize;
    let mut max_y = 0usize;

    for y in 0..h {
        for x in 0..w {
            if score_map[y * w + x] >= threshold {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    if max_x <= min_x || max_y <= min_y {
        return vec![];
    }

    let x1 = (min_x as f32 / w as f32) * 100.0;
    let y1 = (min_y as f32 / h as f32) * 100.0;
    let x2 = ((max_x + 1) as f32 / w as f32) * 100.0;
    let y2 = ((max_y + 1) as f32 / h as f32) * 100.0;

    let area = (x2 - x1) * (y2 - y1);
    if area < min_area_percent * min_area_percent {
        return vec![];
    }

    vec![FocusRect { x1, y1, x2, y2, weight: 1.0, kind: FocusKind::Saliency }]
}

/// Score a single pixel for skin tone using YCbCr chrominance.
#[cfg(test)]
fn score_skin_pixel(r: u8, g: u8, b: u8) -> f32 {
    let r_val = r as f32;
    let g_val = g as f32;
    let b_val = b as f32;

    let y = 0.299 * r_val + 0.587 * g_val + 0.114 * b_val;
    let cb = -0.169 * r_val - 0.331 * g_val + 0.500 * b_val + 128.0;
    let cr = 0.500 * r_val - 0.419 * g_val - 0.081 * b_val + 128.0;

    if y <= 40.0 {
        return 0.0;
    }

    let cb_center = 102.0f32;
    let cr_center = 153.0f32;
    let cb_half = 25.0f32;
    let cr_half = 20.0f32;

    if cb < cb_center - cb_half
        || cb > cb_center + cb_half
        || cr < cr_center - cr_half
        || cr > cr_center + cr_half
    {
        return 0.0;
    }

    let cb_norm = ((cb - cb_center) / cb_half).abs();
    let cr_norm = ((cr - cr_center) / cr_half).abs();
    let dist = cb_norm.max(cr_norm);
    (1.0 - dist).max(0.0)
}

/// Compute skin map for a BGRA image (used by tests and validation).
#[cfg(test)]
fn compute_skin_map(pixels: &[u8], width: u32, height: u32, wb_compensate: bool) -> Vec<f32> {
    let count = (width * height) as usize;
    let mut lum = vec![0.0f32; count];
    let mut score = vec![0.0f32; count];

    let (cb_center, cr_center) = if wb_compensate {
        compute_wb_adjusted_centers(pixels, count)
    } else {
        (102.0f32, 153.0f32)
    };

    // Use compute_all_signals with skin_weight=1.0, sat_weight=0.0
    // to isolate the skin signal
    compute_all_signals(pixels, &mut lum, &mut score, cb_center, cr_center, 1.0, 0.0);
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_luminance() {
        let pixels = [255u8, 255, 255, 255]; // white BGRA
        let mut lum = vec![0.0f32; 1];
        let mut score = vec![0.0f32; 1];
        compute_all_signals(&pixels, &mut lum, &mut score, 102.0, 153.0, 0.0, 0.0);
        assert!((lum[0] - 1.0).abs() < 0.01);

        let pixels = [0u8, 0, 0, 255]; // black BGRA
        compute_all_signals(&pixels, &mut lum, &mut score, 102.0, 153.0, 0.0, 0.0);
        assert!(lum[0].abs() < 0.01);
    }

    #[test]
    fn test_skin_detection_on_skin_color() {
        let r = 200u8;
        let g = 145u8;
        let b = 112u8;
        let pixels = [b, g, r, 255];
        let skin = compute_skin_map(&pixels, 1, 1, false);
        assert!(skin[0] > 0.3, "Skin color should score high: {}", skin[0]);
    }

    #[test]
    fn test_skin_fitzpatrick_type_i() {
        let score = score_skin_pixel(255, 224, 196);
        assert!(score > 0.3, "Fitzpatrick I should detect as skin: {score}");
    }

    #[test]
    fn test_skin_fitzpatrick_type_ii() {
        let score = score_skin_pixel(234, 192, 159);
        assert!(score > 0.3, "Fitzpatrick II should detect as skin: {score}");
    }

    #[test]
    fn test_skin_fitzpatrick_type_iii() {
        let score = score_skin_pixel(198, 155, 119);
        assert!(score > 0.3, "Fitzpatrick III should detect as skin: {score}");
    }

    #[test]
    fn test_skin_fitzpatrick_type_iv() {
        let score = score_skin_pixel(160, 114, 78);
        assert!(score > 0.3, "Fitzpatrick IV should detect as skin: {score}");
    }

    #[test]
    fn test_skin_fitzpatrick_type_v() {
        let score = score_skin_pixel(112, 73, 46);
        assert!(score > 0.3, "Fitzpatrick V should detect as skin: {score}");
    }

    #[test]
    fn test_skin_fitzpatrick_type_vi() {
        let score = score_skin_pixel(62, 39, 25);
        assert!(score > 0.3, "Fitzpatrick VI should detect as skin: {score}");
    }

    #[test]
    fn test_non_skin_blue_sky() {
        let score = score_skin_pixel(135, 206, 235);
        assert!(score < 0.1, "Blue sky should not detect as skin: {score}");
    }

    #[test]
    fn test_non_skin_green_grass() {
        let score = score_skin_pixel(76, 153, 0);
        assert!(score < 0.1, "Green grass should not detect as skin: {score}");
    }

    #[test]
    fn test_non_skin_red_car() {
        let score = score_skin_pixel(220, 20, 20);
        assert!(score < 0.1, "Red car paint should not detect as skin: {score}");
    }

    #[test]
    fn test_non_skin_pure_white() {
        let score = score_skin_pixel(255, 255, 255);
        assert!(score < 0.1, "Pure white should not detect as skin: {score}");
    }

    #[test]
    fn test_prefix_sum_blur_identity() {
        let data = vec![1.0f32; 9];
        let mut output = vec![0.0f32; 9];
        let mut prefix = vec![0.0f64; 4]; // max(3,3)+1
        prefix_sum_blur(&data, &mut output, &mut prefix, 3, 3, 1);
        for v in &output {
            assert!((v - 1.0).abs() < 0.01, "Expected ~1.0, got {v}");
        }
    }

    #[test]
    fn test_prefix_sum_blur_matches_naive() {
        // Compare prefix-sum blur against a simple reference implementation
        let w = 16;
        let h = 12;
        let radius = 3;
        let data: Vec<f32> = (0..w * h).map(|i| (i as f32 * 0.01).sin().abs()).collect();

        // Naive blur
        let naive = naive_box_blur(&data, w, h, radius);

        // Prefix-sum blur
        let mut output = vec![0.0f32; w * h];
        let mut prefix = vec![0.0f64; w.max(h) + 1];
        prefix_sum_blur(&data, &mut output, &mut prefix, w, h, radius);

        for i in 0..w * h {
            assert!(
                (output[i] - naive[i]).abs() < 1e-4,
                "Mismatch at {i}: prefix={} naive={}",
                output[i],
                naive[i]
            );
        }
    }

    /// Naive O(radius) box blur for comparison testing.
    fn naive_box_blur(data: &[f32], width: usize, height: usize, radius: usize) -> Vec<f32> {
        let mut temp = vec![0.0f32; width * height];
        for y in 0..height {
            for x in 0..width {
                let x0 = x.saturating_sub(radius);
                let x1 = (x + radius).min(width - 1);
                let count = (x1 - x0 + 1) as f32;
                let mut sum = 0.0f32;
                for xx in x0..=x1 {
                    sum += data[y * width + xx];
                }
                temp[y * width + x] = sum / count;
            }
        }

        let mut result = vec![0.0f32; width * height];
        for y in 0..height {
            for x in 0..width {
                let y0 = y.saturating_sub(radius);
                let y1 = (y + radius).min(height - 1);
                let count = (y1 - y0 + 1) as f32;
                let mut sum = 0.0f32;
                for yy in y0..=y1 {
                    sum += temp[yy * width + x];
                }
                result[y * width + x] = sum / count;
            }
        }
        result
    }

    #[test]
    fn test_extract_focus_rects_empty_for_zero_map() {
        let map = vec![0.0f32; 100];
        let rects = extract_focus_rects(&map, 10, 10, 1.0);
        assert!(rects.is_empty());
    }

    #[test]
    fn test_white_balance_compensation() {
        let skin_r = 198u8;
        let skin_g = 155u8;
        let skin_b = 119u8;
        let bg_r = 128u8;
        let bg_g = 128u8;
        let bg_b = 128u8;

        let warm = |r: u8, g: u8, b: u8| -> (u8, u8, u8) {
            (r.saturating_add(20), g, b.saturating_sub(10))
        };
        let (wr, wg, wb) = warm(skin_r, skin_g, skin_b);
        let (br, bg_val, bb) = warm(bg_r, bg_g, bg_b);

        let mut pixels = Vec::with_capacity(9 * 4);
        for i in 0..9 {
            if i == 4 {
                pixels.extend_from_slice(&[wb, wg, wr, 255]);
            } else {
                pixels.extend_from_slice(&[bb, bg_val, br, 255]);
            }
        }

        let skin_wb = compute_skin_map(&pixels, 3, 3, true);
        let skin_no_wb = compute_skin_map(&pixels, 3, 3, false);

        assert!(skin_wb[4] > 0.1, "WB-compensated should detect warm-cast skin: {}", skin_wb[4]);
        assert!(
            skin_wb[4] >= skin_no_wb[4],
            "WB compensation should help, not hurt: wb={} no_wb={}",
            skin_wb[4],
            skin_no_wb[4]
        );
    }
}
