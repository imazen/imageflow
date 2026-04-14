#[allow(unused_imports)]
use crate::common::*;
use imageflow_types::{Color, ColorSrgb, Node, PixelFormat};

#[test]
fn test_trim_whitespace() {
    visual_check! {
        source: "test_inputs/shirt_transparent.png",
        detail: "transparent_shirt",
        command: "trim.threshold=80",
    }
}

#[test]
fn test_trim_whitespace_with_padding() {
    visual_check! {
        source: "test_inputs/whitespace-issue.png",
        detail: "gray_bg",
        command: "trim.threshold=20&trim.percentpadding=0.5&bgcolor=gray",
    }
}

#[test]
fn test_trim_resize_whitespace_with_padding() {
    visual_check! {
        source: "test_inputs/whitespace-issue.png",
        detail: "450x450_gray",
        command: "w=450&h=450&scale=both&trim.threshold=20&trim.percentpadding=10&bgcolor=gray",
    }
}

#[test]
fn test_trim_resize_whitespace_without_padding() {
    visual_check! {
        source: "test_inputs/whitespace-issue.png",
        detail: "450x450_gray",
        command: "w=450&h=450&scale=both&trim.threshold=20&bgcolor=gray",
    }
}

#[test]
fn test_trim_whitespace_with_padding_no_resize() {
    visual_check! {
        source: "test_inputs/whitespace-issue.png",
        detail: "gray_bg",
        command: "trim.threshold=20&trim.percentpadding=0.5&bgcolor=gray",
    }
}

// ============================================================================
// Whitespace trim via Node steps (CropWhitespace node)
// ============================================================================

#[test]
fn test_trim_node_on_generated_canvas() {
    // Create a canvas with a small colored rect, then trim the whitespace
    visual_check_bitmap! {
        detail: "blue_dot_trimmed",
        steps: vec![
            Node::CreateCanvas {
                w: 200,
                h: 200,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_owned())),
            },
            Node::FillRect {
                x1: 80,
                y1: 80,
                x2: 120,
                y2: 120,
                color: Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned())),
            },
            Node::CropWhitespace { threshold: 80, percent_padding: 0.0 },
        ],
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn test_trim_node_with_padding() {
    visual_check_bitmap! {
        detail: "blue_dot_padded_10pct",
        steps: vec![
            Node::CreateCanvas {
                w: 200,
                h: 200,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_owned())),
            },
            Node::FillRect {
                x1: 80,
                y1: 80,
                x2: 120,
                y2: 120,
                color: Color::Srgb(ColorSrgb::Hex("FF0000FF".to_owned())),
            },
            Node::CropWhitespace { threshold: 80, percent_padding: 10.0 },
        ],
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn test_trim_on_transparent_canvas() {
    // Transparent background with a colored region — trim should detect the non-transparent area
    visual_check_bitmap! {
        detail: "green_on_transparent",
        steps: vec![
            Node::CreateCanvas {
                w: 300,
                h: 300,
                format: PixelFormat::Bgra32,
                color: Color::Transparent,
            },
            Node::FillRect {
                x1: 100,
                y1: 100,
                x2: 200,
                y2: 200,
                color: Color::Srgb(ColorSrgb::Hex("00FF00FF".to_owned())),
            },
            Node::CropWhitespace { threshold: 1, percent_padding: 0.0 },
        ],
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn test_trim_then_resize() {
    // Trim whitespace, then resize — tests the eager materialization path
    visual_check_bitmap! {
        detail: "trimmed_then_300x300",
        steps: vec![
            Node::CreateCanvas {
                w: 400,
                h: 400,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_owned())),
            },
            Node::FillRect {
                x1: 50,
                y1: 50,
                x2: 150,
                y2: 150,
                color: Color::Srgb(ColorSrgb::Hex("FF5500FF".to_owned())),
            },
            Node::CropWhitespace { threshold: 80, percent_padding: 0.0 },
            Node::Resample2D {
                w: 300,
                h: 300,
                hints: Some(imageflow_types::ResampleHints::new()
                    .with_bi_filter(imageflow_types::Filter::Robidoux)),
            },
        ],
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn test_trim_on_photo() {
    // Trim real photo with gray background
    visual_check! {
        source: "test_inputs/whitespace-issue.png",
        detail: "low_threshold",
        command: "trim.threshold=10",
    }
}

#[test]
fn test_trim_with_high_threshold() {
    // High threshold = more aggressive trim (treats more colors as whitespace)
    visual_check! {
        source: "test_inputs/whitespace-issue.png",
        detail: "high_threshold",
        command: "trim.threshold=200",
    }
}
