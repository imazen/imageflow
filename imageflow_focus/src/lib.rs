#![forbid(unsafe_code)]
//! Composite saliency and focus detection for smart cropping.
//!
//! Provides a smartcrop.js/libvips-style composite scoring engine that
//! identifies visually important regions in an image without machine learning
//! models. Operates at reduced resolution for speed.
//!
//! ## Features
//! - `saliency` (default): Edge, skin tone, and saturation detection
//! - `faces`: Face detection via rustface (adds ~1.2MB model dependency)

mod saliency;

#[cfg(feature = "faces")]
mod faces;

use imageflow_types::FocusRect;

/// Configuration for the analysis engine
#[derive(Clone, Debug)]
pub struct AnalysisConfig {
    /// Maximum working dimension (image is downscaled to this for analysis)
    pub max_working_size: u32,
    /// Minimum focus rect area as percentage of image (rects smaller than this are dropped)
    pub min_rect_area_percent: f32,
    /// Weight for skin-tone signal
    pub skin_weight: f32,
    /// Weight for edge/detail signal
    pub edge_weight: f32,
    /// Weight for saturation signal
    pub saturation_weight: f32,
    /// Compensate for white balance shifts in skin detection.
    /// When true, shifts the YCbCr detection range center based on image-wide
    /// mean chrominance to handle warm/cool color casts.
    pub white_balance_compensate: bool,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            max_working_size: 256,
            min_rect_area_percent: 1.0,
            skin_weight: 1.8,
            edge_weight: 0.2,
            saturation_weight: 0.1,
            white_balance_compensate: true,
        }
    }
}

/// Analyze a BGRA image and return focus regions.
///
/// `pixels` must be a packed BGRA32 buffer of `width * height * 4` bytes.
/// Returns focus rects in percentage coordinates (0-100).
#[cfg(feature = "saliency")]
pub fn analyze_saliency(
    pixels: &[u8],
    width: u32,
    height: u32,
    config: &AnalysisConfig,
) -> Vec<FocusRect> {
    saliency::analyze(pixels, width, height, config)
}

/// Detect faces in a BGRA image and return focus regions.
///
/// `pixels` must be a packed BGRA32 buffer of `width * height * 4` bytes.
/// Returns focus rects in percentage coordinates (0-100) with `kind: Face`.
#[cfg(feature = "faces")]
pub fn detect_faces(
    pixels: &[u8],
    width: u32,
    height: u32,
    model: &rustface::Model,
) -> Vec<FocusRect> {
    faces::detect(pixels, width, height, model)
}

/// Run all available detectors and fuse results.
///
/// Face rects get weight 10.0, saliency rects get weight 1.0.
/// If faces are detected, they dominate the combined focus.
pub fn analyze_all(
    pixels: &[u8],
    width: u32,
    height: u32,
    config: &AnalysisConfig,
    #[cfg(feature = "faces")] face_model: Option<&rustface::Model>,
) -> Vec<FocusRect> {
    let mut results = Vec::new();

    #[cfg(feature = "saliency")]
    {
        results.extend(analyze_saliency(pixels, width, height, config));
    }

    #[cfg(feature = "faces")]
    if let Some(model) = face_model {
        let face_rects = detect_faces(pixels, width, height, model);
        results.extend(face_rects);
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use imageflow_types::FocusKind;

    fn make_test_image(width: u32, height: u32) -> Vec<u8> {
        // Create a simple test image: gray background with a bright red square in the upper-left
        let mut pixels = vec![128u8; (width * height * 4) as usize];
        let rect_w = width / 4;
        let rect_h = height / 4;
        for y in 0..rect_h {
            for x in 0..rect_w {
                let idx = ((y * width + x) * 4) as usize;
                pixels[idx] = 30; // B
                pixels[idx + 1] = 30; // G
                pixels[idx + 2] = 220; // R
                pixels[idx + 3] = 255; // A
            }
        }
        pixels
    }

    #[test]
    #[cfg(feature = "saliency")]
    fn test_saliency_detects_bright_region() {
        let w = 128;
        let h = 128;
        let pixels = make_test_image(w, h);
        let config = AnalysisConfig::default();

        let rects = analyze_saliency(&pixels, w, h, &config);
        assert!(!rects.is_empty(), "Should detect at least one salient region");

        // The bright red square is in the upper-left quadrant (0-25%, 0-25%)
        // The centroid of detected rects should be in that general area
        let first = &rects[0];
        assert_eq!(first.kind, FocusKind::Saliency);
        assert!(first.x1 < 50.0, "Salient region should be in the left half");
        assert!(first.y1 < 50.0, "Salient region should be in the top half");
    }

    #[test]
    #[cfg(feature = "saliency")]
    fn test_uniform_image_returns_center() {
        let w = 64;
        let h = 64;
        // Completely uniform gray image
        let pixels = vec![128u8; (w * h * 4) as usize];
        let config = AnalysisConfig::default();

        let rects = analyze_saliency(&pixels, w, h, &config);
        // For a uniform image, either no rects or a centered rect
        if !rects.is_empty() {
            let (cx, cy) = rects[0].center();
            assert!(
                (cx - 50.0).abs() < 25.0 && (cy - 50.0).abs() < 25.0,
                "Uniform image should return near-center rect"
            );
        }
    }
}
