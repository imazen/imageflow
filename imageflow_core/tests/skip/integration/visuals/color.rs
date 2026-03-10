#[allow(unused_imports)]
use crate::common::*;
use imageflow_types::Node;

#[test]
fn test_simple_filters() {
    let filters = vec![
        imageflow_types::ColorFilterSrgb::Contrast(1.0),
        imageflow_types::ColorFilterSrgb::Brightness(1.0),
        imageflow_types::ColorFilterSrgb::Saturation(1.0),
        imageflow_types::ColorFilterSrgb::Alpha(1.0),
        imageflow_types::ColorFilterSrgb::Contrast(0.3),
        imageflow_types::ColorFilterSrgb::Brightness(0.3),
        imageflow_types::ColorFilterSrgb::Saturation(0.3),
        imageflow_types::ColorFilterSrgb::Alpha(0.3),
        imageflow_types::ColorFilterSrgb::Contrast(-1.0),
        imageflow_types::ColorFilterSrgb::Brightness(-1.0),
        imageflow_types::ColorFilterSrgb::Saturation(-1.0),
        imageflow_types::ColorFilterSrgb::Alpha(-1.0),
        imageflow_types::ColorFilterSrgb::Contrast(-0.2),
        imageflow_types::ColorFilterSrgb::Brightness(-0.2),
        imageflow_types::ColorFilterSrgb::Saturation(-0.2),
        imageflow_types::ColorFilterSrgb::Alpha(-0.2),
        imageflow_types::ColorFilterSrgb::Sepia,
        imageflow_types::ColorFilterSrgb::GrayscaleNtsc,
        imageflow_types::ColorFilterSrgb::GrayscaleRy,
        imageflow_types::ColorFilterSrgb::GrayscaleFlat,
        imageflow_types::ColorFilterSrgb::GrayscaleBt709,
        imageflow_types::ColorFilterSrgb::Invert,
    ];

    for filter in filters {
        visual_check_bitmap! {
            source: "test_inputs/pngsuite/basn6a08.png",
            detail: &format!("{filter:?}"),
            steps: vec![
                Node::Decode { io_id: 0, commands: None },
                Node::ColorFilterSrgb(filter),
            ],
            tolerance: Tolerance::off_by_one(),
        }
    }
}

#[test]
fn test_white_balance_image() {
    visual_check_bitmap! {
        source: "test_inputs/red-night.png",
        detail: "red_night_auto",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::WhiteBalanceHistogramAreaThresholdSrgb { threshold: None },
        ],
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn test_white_balance_image_threshold_5() {
    visual_check_bitmap! {
        source: "test_inputs/red-night.png",
        detail: "t0.5",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::WhiteBalanceHistogramAreaThresholdSrgb { threshold: Some(0.5) },
        ],
        tolerance: Tolerance::off_by_one(),
    }
}
