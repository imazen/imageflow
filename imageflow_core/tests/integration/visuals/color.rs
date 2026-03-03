use crate::common::*;
use imageflow_types::Node;

const DEBUG_GRAPH: bool = false;
const POPULATE_CHECKSUMS: bool = true;

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
        let matched = compare(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/pngsuite/basn6a08.png".to_owned())), 500,
                          format!("test_simple_filters/{:?}", filter).as_str(), POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            Node::Decode {io_id: 0, commands: None},
            Node::ColorFilterSrgb(filter)
        ]
        );
        assert!(matched);
    }
}

#[test]
fn test_white_balance_image() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/red-night.png"
                .to_owned(),
        )),
        500,
        "test_white_balance_image red_night_auto",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::Decode { io_id: 0, commands: None },
            Node::WhiteBalanceHistogramAreaThresholdSrgb { threshold: None },
        ],
    );
    assert!(matched);
}

#[test]
fn test_white_balance_image_threshold_5() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/red-night.png"
                .to_owned(),
        )),
        500,
        "test_white_balance_image_threshold_5 t0.5",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::Decode { io_id: 0, commands: None },
            Node::WhiteBalanceHistogramAreaThresholdSrgb { threshold: Some(0.5) },
        ],
    );
    assert!(matched);
}
