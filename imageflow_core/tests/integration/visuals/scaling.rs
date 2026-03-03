use crate::common::*;
use imageflow_types::{
    CommandStringKind, Filter, Node, ResampleHints,
};

const DEBUG_GRAPH: bool = false;
const POPULATE_CHECKSUMS: bool = true;

#[test]
fn test_scale_image() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg"
                .to_owned(),
        )),
        500,
        "test_scale_image waterhouse_robidoux_400x300",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Resample2D {
                w: 400,
                h: 300,
                hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
            },
        ],
    );
    assert!(matched);
}

#[test]
fn test_scale_rings() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/rings2.png"
                .to_owned(),
        )),
        500,
        "test_scale_rings hermite_400x400",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Resample2D {
                w: 400,
                h: 400,
                hints: Some(ResampleHints::new().with_bi_filter(Filter::Hermite)),
            },
        ],
    );
    assert!(matched);
}

#[test]
fn test_read_gif_and_scale() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/mountain_800.gif"
                .to_owned(),
        )),
        500,
        "test_read_gif_and_scale mountain_robidoux_400x300",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Resample2D {
                w: 400,
                h: 300,
                hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
            },
        ],
    );
    assert!(matched);
}

#[test]
fn test_read_gif_and_vertical_distort() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/mountain_800.gif"
                .to_owned(),
        )),
        500,
        "test_read_gif_and_vertical_distort mountain_box_800x100",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Resample2D {
                w: 800,
                h: 100,
                hints: Some(ResampleHints::new().with_bi_filter(Filter::Box)),
            },
        ],
    );
    assert!(matched);
}

#[test]
#[ignore] // gif crate doesn't support files without Trailer: https://github.com/image-rs/image-gif/issues/138
fn test_read_gif_eof() {
    let matched = compare(Some(IoTestEnum::Url("https://user-images.githubusercontent.com/657201/139770827-18e25c4e-eb0a-4058-ba48-ddc3849090ee.gif".to_owned())), 500,
                          "buggy_animated-gif", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            Node::Decode {io_id: 0, commands: None},
            Node::Resample2D{ w: 400, h: 300,  hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)) }
        ]
    );
    assert!(matched);
}

#[test]
fn webp_lossless_alpha_decode_and_scale() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/1_webp_ll.webp"
                .to_owned(),
        )),
        500,
        "webp_lossless_alpha_decode_and_scale 100x100",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "width=100&height=100".to_owned(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
    );
    assert!(matched);
}

#[test]
fn webp_lossy_alpha_decode_and_scale() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/1_webp_a.webp"
                .to_owned(),
        )),
        500,
        "webp_lossy_alpha_decode_and_scale 100x100",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "width=100&height=100".to_owned(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
    );
    assert!(matched);
}

#[test]
fn webp_lossy_noalpha_decode_and_scale() {
    let matched = compare(Some(IoTestEnum::Url("https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/lossy_mountain.webp".to_owned())), 500,
                          "webp_lossy_noalpha_decode_and_scale mountain_100x100", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            Node::CommandString{
                kind: CommandStringKind::ImageResizer4,
                value: "width=100&height=100".to_owned(),
                decode: Some(0),
                encode: None,
                watermarks: None
            }
        ]
    );
    assert!(matched);
}

#[test]
fn test_jpeg_icc2_color_profile() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_tagged.jpg"
                .to_owned(),
        )),
        500,
        "test_jpeg_icc2_color_profile mars_robidoux_400x300",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Resample2D {
                w: 400,
                h: 300,
                hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
            },
        ],
    );
    assert!(matched);
}

#[test]
fn test_jpeg_icc4_color_profile() {
    let matched = compare(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_v4_sYCC_8bit.jpg".to_owned())), 500,
                          "test_jpeg_icc4_color_profile mars_v4_robidoux_400x300", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
Node::Decode {io_id: 0, commands: None},
Node::Resample2D{ w: 400, h: 300,  hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)) }
]
    );
    assert!(matched);
}
