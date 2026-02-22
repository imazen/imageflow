//! Face detection using rustface (SeetaFace cascade classifier).
//!
//! This module is only compiled when the `faces` feature is enabled.
//! Rustface contains unsafe code, so the `faces` feature should be
//! opt-in only.

use imageflow_types::{FocusKind, FocusRect};

/// Detect faces in a BGRA32 image.
///
/// Converts to grayscale internally, runs the cascade classifier,
/// and returns percentage-coordinate focus rects with `kind: Face`.
pub(crate) fn detect(
    pixels: &[u8],
    width: u32,
    height: u32,
    model: &rustface::Model,
) -> Vec<FocusRect> {
    if width == 0 || height == 0 || pixels.len() < (width * height * 4) as usize {
        return vec![];
    }

    // Convert BGRA to grayscale
    let gray = bgra_to_gray(pixels, width, height);

    // Create detector from model
    let mut detector = rustface::create_detector_with_model(model.clone());

    // Configure detector
    let min_face_size = (width.min(height) as f32 * 0.03).max(20.0) as u32;
    detector.set_min_face_size(min_face_size);
    detector.set_score_thresh(2.0);
    detector.set_pyramid_scale_factor(0.8);
    detector.set_slide_window_step(4, 4);

    // Run detection
    let image_data = rustface::ImageData::new(&gray, width, height);
    let faces = detector.detect(&image_data);

    // Convert pixel bounding boxes to percentage coordinates
    faces
        .iter()
        .filter_map(|face| {
            let bbox = face.bbox();
            let x = bbox.x().max(0) as f32;
            let y = bbox.y().max(0) as f32;
            let w = bbox.width() as f32;
            let h = bbox.height() as f32;

            // Convert to percentages
            let x1 = (x / width as f32) * 100.0;
            let y1 = (y / height as f32) * 100.0;
            let x2 = ((x + w) / width as f32).min(1.0) * 100.0;
            let y2 = ((y + h) / height as f32).min(1.0) * 100.0;

            if x2 <= x1 || y2 <= y1 {
                return None;
            }

            Some(FocusRect {
                x1,
                y1,
                x2,
                y2,
                weight: 10.0, // Faces dominate other signals
                kind: FocusKind::Face,
            })
        })
        .collect()
}

/// Convert BGRA pixels to grayscale using BT.709 weights.
fn bgra_to_gray(pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
    let count = (width * height) as usize;
    let mut gray = Vec::with_capacity(count);
    for i in 0..count {
        let idx = i * 4;
        let b = pixels[idx] as f32;
        let g = pixels[idx + 1] as f32;
        let r = pixels[idx + 2] as f32;
        // BT.709
        let lum = (0.2126 * r + 0.7152 * g + 0.0722 * b).round().clamp(0.0, 255.0);
        gray.push(lum as u8);
    }
    gray
}
