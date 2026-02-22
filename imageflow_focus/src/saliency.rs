#![forbid(unsafe_code)]
//! Composite saliency detection for smart cropping.
//!
//! Combines three simple pixel-level signals:
//! 1. Edge detection (Laplacian on luminance)
//! 2. Skin tone detection (YCbCr chrominance range, inclusive of Fitzpatrick I-VI)
//! 3. Saturation detection (HSL saturation above threshold)
//!
//! The combined score map is blurred to find the peak region, which is
//! returned as a FocusRect.

use crate::AnalysisConfig;
use imageflow_types::{FocusKind, FocusRect};

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

    let pixel_count = (work_w * work_h) as usize;

    // Compute individual signal maps
    let luminance = compute_luminance(&work_pixels, work_w, work_h);
    let edge_map = compute_edge_map(&luminance, work_w, work_h);
    let skin_map = compute_skin_map(&work_pixels, work_w, work_h, config.white_balance_compensate);
    let sat_map = compute_saturation_map(&work_pixels, work_w, work_h);

    // Combine into composite score
    let mut score_map = vec![0.0f32; pixel_count];
    for i in 0..pixel_count {
        score_map[i] = edge_map[i] * config.edge_weight
            + skin_map[i] * config.skin_weight
            + sat_map[i] * config.saturation_weight;
    }

    // Box-blur the score map to find the general region (sigma ~ 1/8 of image)
    let blur_radius = (work_w.min(work_h) / 8).max(2) as usize;
    let blurred = box_blur(&score_map, work_w as usize, work_h as usize, blur_radius);

    // Find peak and derive bounding box from thresholded region
    extract_focus_rects(&blurred, work_w, work_h, config.min_rect_area_percent)
}

/// Downsample BGRA pixels using simple area averaging.
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

            // Bilinear interpolation
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

/// Compute luminance (0.0-1.0) from BGRA pixels using BT.709 weights.
fn compute_luminance(pixels: &[u8], width: u32, height: u32) -> Vec<f32> {
    let count = (width * height) as usize;
    let mut lum = Vec::with_capacity(count);
    for i in 0..count {
        let idx = i * 4;
        let b = pixels[idx] as f32 / 255.0;
        let g = pixels[idx + 1] as f32 / 255.0;
        let r = pixels[idx + 2] as f32 / 255.0;
        // BT.709 luminance
        lum.push(0.2126 * r + 0.7152 * g + 0.0722 * b);
    }
    lum
}

/// Edge detection using 3x3 Laplacian kernel on luminance.
/// Returns edge strength 0.0-1.0.
fn compute_edge_map(luminance: &[f32], width: u32, height: u32) -> Vec<f32> {
    let w = width as usize;
    let h = height as usize;
    let mut edges = vec![0.0f32; w * h];

    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let center = luminance[y * w + x];
            let top = luminance[(y - 1) * w + x];
            let bottom = luminance[(y + 1) * w + x];
            let left = luminance[y * w + (x - 1)];
            let right = luminance[y * w + (x + 1)];
            // Laplacian: 4*center - neighbors
            let laplacian = (4.0 * center - top - bottom - left - right).abs();
            edges[y * w + x] = laplacian.min(1.0);
        }
    }
    edges
}

/// Skin tone detection using YCbCr chrominance range.
///
/// Uses Chai & Ngan (1999) / Kovac et al. (2003) ranges validated across
/// Fitzpatrick skin types I-VI. YCbCr separates luminance from chrominance,
/// making detection robust to illumination changes and inclusive of all skin tones.
///
/// Returns 0.0-1.0 score (higher = more skin-like).
fn compute_skin_map(
    pixels: &[u8],
    width: u32,
    height: u32,
    white_balance_compensate: bool,
) -> Vec<f32> {
    let count = (width * height) as usize;
    let mut skin = vec![0.0f32; count];

    // Nominal detection range centers (neutral white balance)
    let mut cb_center = 102.0f32;
    let mut cr_center = 153.0f32;

    if white_balance_compensate {
        // Compute image-wide mean Cb/Cr for white balance compensation
        let (mean_cb, mean_cr) = compute_mean_chroma(pixels, count);

        // Shift detection range if mean deviates significantly from neutral (128)
        let cb_shift = mean_cb - 128.0;
        let cr_shift = mean_cr - 128.0;

        // Apply partial correction (50%) to avoid over-compensating
        if cb_shift.abs() > 3.0 {
            cb_center += cb_shift * 0.5;
        }
        if cr_shift.abs() > 3.0 {
            cr_center += cr_shift * 0.5;
        }
    }

    // Detection range half-widths
    let cb_half = 25.0f32; // 77..127 centered at 102
    let cr_half = 20.0f32; // 133..173 centered at 153

    for i in 0..count {
        let idx = i * 4;
        let b_val = pixels[idx] as f32;
        let g_val = pixels[idx + 1] as f32;
        let r_val = pixels[idx + 2] as f32;

        // BGRA → YCbCr (ITU-R BT.601)
        let y = 0.299 * r_val + 0.587 * g_val + 0.114 * b_val;
        let cb = -0.169 * r_val - 0.331 * g_val + 0.500 * b_val + 128.0;
        let cr = 0.500 * r_val - 0.419 * g_val - 0.081 * b_val + 128.0;

        // Luminance floor: avoid false positives on very dark pixels
        if y <= 40.0 {
            continue;
        }

        // Check chrominance ranges
        let cb_lo = cb_center - cb_half;
        let cb_hi = cb_center + cb_half;
        let cr_lo = cr_center - cr_half;
        let cr_hi = cr_center + cr_half;

        if cb < cb_lo || cb > cb_hi || cr < cr_lo || cr > cr_hi {
            continue;
        }

        // Score: smooth falloff from range center using Chebyshev distance.
        // Chebyshev (max of per-axis distances) matches the rectangular detection
        // range shape, avoiding the Euclidean penalty at corners that would
        // unfairly reduce scores for skin tones near the range edges.
        let cb_norm = ((cb - cb_center) / cb_half).abs();
        let cr_norm = ((cr - cr_center) / cr_half).abs();
        let dist = cb_norm.max(cr_norm);
        skin[i] = (1.0 - dist).max(0.0);
    }
    skin
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

/// Saturation detection: pixels with HSL saturation > 0.4 and
/// brightness in [0.05, 0.9].
/// Returns 0.0-1.0 score.
fn compute_saturation_map(pixels: &[u8], width: u32, height: u32) -> Vec<f32> {
    let count = (width * height) as usize;
    let mut sat_map = vec![0.0f32; count];
    let sat_threshold = 0.4f32;

    for i in 0..count {
        let idx = i * 4;
        let b = pixels[idx] as f32 / 255.0;
        let g = pixels[idx + 1] as f32 / 255.0;
        let r = pixels[idx + 2] as f32 / 255.0;

        let max_c = r.max(g).max(b);
        let min_c = r.min(g).min(b);
        let lightness = (max_c + min_c) / 2.0;

        // Brightness bounds
        if !(0.05..=0.9).contains(&lightness) {
            continue;
        }

        // HSL saturation
        let delta = max_c - min_c;
        if delta < f32::EPSILON {
            continue;
        }
        let saturation =
            if lightness <= 0.5 { delta / (max_c + min_c) } else { delta / (2.0 - max_c - min_c) };

        if saturation > sat_threshold {
            sat_map[i] = (saturation - sat_threshold) / (1.0 - sat_threshold);
        }
    }
    sat_map
}

/// Simple box blur (separable, two-pass).
fn box_blur(data: &[f32], width: usize, height: usize, radius: usize) -> Vec<f32> {
    if radius == 0 {
        return data.to_vec();
    }

    // Horizontal pass
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

    // Vertical pass
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

/// Extract focus rects from a blurred score map by thresholding at
/// a fraction of the peak value and finding the bounding box of the
/// thresholded region.
fn extract_focus_rects(
    score_map: &[f32],
    width: u32,
    height: u32,
    min_area_percent: f32,
) -> Vec<FocusRect> {
    let w = width as usize;
    let h = height as usize;

    // Find peak value
    let max_score = score_map.iter().copied().fold(0.0f32, f32::max);
    if max_score <= f32::EPSILON {
        return vec![];
    }

    // Threshold at 50% of peak
    let threshold = max_score * 0.50;

    // Find bounding box of thresholded region
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

    // Convert to percentage coordinates
    let x1 = (min_x as f32 / w as f32) * 100.0;
    let y1 = (min_y as f32 / h as f32) * 100.0;
    let x2 = ((max_x + 1) as f32 / w as f32) * 100.0;
    let y2 = ((max_y + 1) as f32 / h as f32) * 100.0;

    // Check minimum area
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

    let cb_lo = cb_center - cb_half;
    let cb_hi = cb_center + cb_half;
    let cr_lo = cr_center - cr_half;
    let cr_hi = cr_center + cr_half;

    if cb < cb_lo || cb > cb_hi || cr < cr_lo || cr > cr_hi {
        return 0.0;
    }

    let cb_norm = ((cb - cb_center) / cb_half).abs();
    let cr_norm = ((cr - cr_center) / cr_half).abs();
    let dist = cb_norm.max(cr_norm);
    (1.0 - dist).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_luminance() {
        // Pure white BGRA pixel
        let pixels = [255, 255, 255, 255];
        let lum = compute_luminance(&pixels, 1, 1);
        assert!((lum[0] - 1.0).abs() < 0.01);

        // Pure black
        let pixels = [0, 0, 0, 255];
        let lum = compute_luminance(&pixels, 1, 1);
        assert!(lum[0].abs() < 0.01);
    }

    #[test]
    fn test_skin_detection_on_skin_color() {
        // Approximate skin color in BGRA: R=200, G=145, B=112
        let r = 200u8;
        let g = 145u8;
        let b = 112u8;
        let pixels = [b, g, r, 255];
        let skin = compute_skin_map(&pixels, 1, 1, false);
        assert!(skin[0] > 0.3, "Skin color should score high: {}", skin[0]);
    }

    // Test YCbCr skin detection across all Fitzpatrick skin types
    #[test]
    fn test_skin_fitzpatrick_type_i() {
        // Type I: very light (RGB ~255, 224, 196)
        let score = score_skin_pixel(255, 224, 196);
        assert!(score > 0.3, "Fitzpatrick I should detect as skin: {score}");
    }

    #[test]
    fn test_skin_fitzpatrick_type_ii() {
        // Type II: light (RGB ~234, 192, 159)
        let score = score_skin_pixel(234, 192, 159);
        assert!(score > 0.3, "Fitzpatrick II should detect as skin: {score}");
    }

    #[test]
    fn test_skin_fitzpatrick_type_iii() {
        // Type III: medium (RGB ~198, 155, 119)
        let score = score_skin_pixel(198, 155, 119);
        assert!(score > 0.3, "Fitzpatrick III should detect as skin: {score}");
    }

    #[test]
    fn test_skin_fitzpatrick_type_iv() {
        // Type IV: olive (RGB ~160, 114, 78)
        let score = score_skin_pixel(160, 114, 78);
        assert!(score > 0.3, "Fitzpatrick IV should detect as skin: {score}");
    }

    #[test]
    fn test_skin_fitzpatrick_type_v() {
        // Type V: brown (RGB ~112, 73, 46)
        let score = score_skin_pixel(112, 73, 46);
        assert!(score > 0.3, "Fitzpatrick V should detect as skin: {score}");
    }

    #[test]
    fn test_skin_fitzpatrick_type_vi() {
        // Type VI: dark brown (RGB ~62, 39, 25)
        let score = score_skin_pixel(62, 39, 25);
        assert!(score > 0.3, "Fitzpatrick VI should detect as skin: {score}");
    }

    // Test non-skin color rejection
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
    fn test_box_blur_identity() {
        let data = vec![1.0; 9];
        let result = box_blur(&data, 3, 3, 1);
        for v in &result {
            assert!((v - 1.0).abs() < 0.01);
        }
    }

    #[test]
    fn test_extract_focus_rects_empty_for_zero_map() {
        let map = vec![0.0f32; 100];
        let rects = extract_focus_rects(&map, 10, 10, 1.0);
        assert!(rects.is_empty());
    }

    #[test]
    fn test_white_balance_compensation() {
        // Create a small image: mostly neutral gray background with one skin pixel.
        // Apply a warm color cast (+20R, -10B) to the whole image.
        // WB compensation should still detect the skin pixel.
        let skin_r = 198u8;
        let skin_g = 155u8;
        let skin_b = 119u8;
        let bg_r = 128u8;
        let bg_g = 128u8;
        let bg_b = 128u8;

        // Apply warm cast
        let warm = |r: u8, g: u8, b: u8| -> (u8, u8, u8) {
            (r.saturating_add(20), g, b.saturating_sub(10))
        };
        let (wr, wg, wb) = warm(skin_r, skin_g, skin_b);
        let (br, bg_val, bb) = warm(bg_r, bg_g, bg_b);

        // 3x3 image: 8 background + 1 skin pixel (center)
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

        // With WB compensation, the warm-cast skin pixel should still be detected
        assert!(skin_wb[4] > 0.1, "WB-compensated should detect warm-cast skin: {}", skin_wb[4]);
        // Without WB compensation, the shifted skin may or may not be detected,
        // but WB version should be >= the non-WB version
        assert!(
            skin_wb[4] >= skin_no_wb[4],
            "WB compensation should help, not hurt: wb={} no_wb={}",
            skin_wb[4],
            skin_no_wb[4]
        );
    }
}
