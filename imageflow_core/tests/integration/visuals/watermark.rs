use crate::common::*;
use imageflow_types::{
    CommandStringKind, ConstraintMode, Node, ResampleHints,
};

const DEBUG_GRAPH: bool = false;
const POPULATE_CHECKSUMS: bool = true;

#[test]
fn test_watermark_image() {
    let matched = compare_multiple(
        Some(vec![
            IoTestEnum::Url(
                "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg"
                    .to_owned(),
            ),
            IoTestEnum::Url(
                "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/dice.png"
                    .to_owned(),
            ),
        ]),
        500,
        "test_watermark_image dice_fitcrop_90pct",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Constrain(imageflow_types::Constraint {
                w: Some(800),
                h: Some(800),
                hints: None,
                gravity: None,
                mode: ConstraintMode::Within,
                canvas_color: None,
            }),
            Node::Watermark(imageflow_types::Watermark {
                io_id: 1,
                gravity: Some(imageflow_types::ConstraintGravity::Percentage {
                    x: 100f32,
                    y: 100f32,
                }),
                fit_box: Some(imageflow_types::WatermarkConstraintBox::ImagePercentage {
                    x1: 30f32,
                    y1: 50f32,
                    x2: 90f32,
                    y2: 90f32,
                }),
                fit_mode: Some(imageflow_types::WatermarkConstraintMode::FitCrop),
                min_canvas_width: None,
                min_canvas_height: None,
                opacity: Some(0.9f32),
                hints: Some(ResampleHints {
                    sharpen_percent: Some(15f32),
                    down_filter: None,
                    up_filter: None,
                    scaling_colorspace: None,
                    background_color: None,
                    resample_when: None,
                    sharpen_when: None,
                }),
            }),
        ],
    );
    assert!(matched);
}

#[test]
fn test_watermark_image_command_string() {
    let matched = compare_multiple(
        Some(vec![
            IoTestEnum::Url(
                "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg"
                    .to_owned(),
            ),
            IoTestEnum::Url(
                "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/dice.png"
                    .to_owned(),
            ),
        ]),
        500,
        "test_watermark_image_command_string dice_fitcrop_90pct",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "width=800&height=800&mode=max".to_string(),
            decode: Some(0),
            encode: None,
            watermarks: Some(vec![imageflow_types::Watermark {
                io_id: 1,
                fit_box: Some(imageflow_types::WatermarkConstraintBox::ImagePercentage {
                    x1: 30f32,
                    y1: 50f32,
                    x2: 90f32,
                    y2: 90f32,
                }),
                fit_mode: Some(imageflow_types::WatermarkConstraintMode::FitCrop),
                gravity: Some(imageflow_types::ConstraintGravity::Percentage {
                    x: 100f32,
                    y: 100f32,
                }),
                min_canvas_width: None,
                min_canvas_height: None,
                opacity: Some(0.9f32),
                hints: Some(imageflow_types::ResampleHints {
                    sharpen_percent: Some(15f32),
                    down_filter: None,
                    up_filter: None,
                    scaling_colorspace: None,
                    background_color: None,
                    resample_when: None,
                    sharpen_when: None,
                }),
            }]),
        }],
    );
    assert!(matched);
}

#[test]
fn test_watermark_image_command_string_with_bgcolor() {
    let matched = compare_multiple(
        Some(vec![
            IoTestEnum::Url(
                "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg"
                    .to_owned(),
            ),
            IoTestEnum::Url(
                "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/dice.png"
                    .to_owned(),
            ),
        ]),
        500,
        "test_watermark_image_command_string_with_bgcolor dice_aaeeff",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "width=800&height=800&mode=max&bgcolor=aaeeff".to_string(),
            decode: Some(0),
            encode: None,
            watermarks: Some(vec![imageflow_types::Watermark {
                io_id: 1,
                fit_box: Some(imageflow_types::WatermarkConstraintBox::ImagePercentage {
                    x1: 30f32,
                    y1: 50f32,
                    x2: 90f32,
                    y2: 90f32,
                }),
                fit_mode: Some(imageflow_types::WatermarkConstraintMode::FitCrop),
                gravity: Some(imageflow_types::ConstraintGravity::Percentage {
                    x: 100f32,
                    y: 100f32,
                }),
                min_canvas_width: None,
                min_canvas_height: None,
                opacity: Some(0.9f32),
                hints: Some(imageflow_types::ResampleHints {
                    sharpen_percent: Some(15f32),
                    down_filter: None,
                    up_filter: None,
                    scaling_colorspace: None,
                    background_color: None,
                    resample_when: None,
                    sharpen_when: None,
                }),
            }]),
        }],
    );
    assert!(matched);
}

#[test]
fn test_watermark_image_small() {
    let matched = compare_multiple(Some(vec![
        IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned()),
        IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/1_webp_a.sm.png".to_owned())
    ]), 500,
                                   "test_watermark_image_small webp_within_90pct", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            Node::Decode {io_id: 0, commands: None},
            Node::Constrain(imageflow_types::Constraint{
                w: Some(800),
                h: Some(800),
                hints: None,
                gravity: None,
                mode: ConstraintMode::Within,
                canvas_color: None
            }),
            Node::Watermark(imageflow_types::Watermark{
                io_id: 1,
                gravity: Some(imageflow_types::ConstraintGravity::Percentage {x: 100f32, y: 100f32}),
                fit_box: Some(imageflow_types::WatermarkConstraintBox::ImagePercentage {x1: 0f32, y1: 0f32, x2: 90f32, y2: 90f32}),
                fit_mode: Some(imageflow_types::WatermarkConstraintMode::Within),
                min_canvas_width: None,
                min_canvas_height: None,
                opacity: Some(0.9f32),
                hints: Some(imageflow_types::ResampleHints{
                    sharpen_percent: Some(15f32),
                    down_filter: None,
                    up_filter: None,
                    scaling_colorspace: None,
                    background_color: None,
                    resample_when: None,
                    sharpen_when: None
                }),

            })
        ]
    );
    assert!(matched);
}

#[test]
fn test_watermark_image_pixel_margins() {
    let matched = compare_multiple(Some(vec![
        IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned()),
        IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/1_webp_a.sm.png".to_owned())
    ]), 500,
                                   "test_watermark_image_pixel_margins webp_700px_offset", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            Node::Decode {io_id: 0, commands: None},
            Node::Constrain(imageflow_types::Constraint{
                w: Some(800),
                h: Some(800),
                hints: None,
                gravity: None,
                mode: ConstraintMode::Within,
                canvas_color: None
            }),
            Node::Watermark(imageflow_types::Watermark{
                io_id: 1,
                gravity: Some(imageflow_types::ConstraintGravity::Percentage {x: 100f32, y: 100f32}),
                fit_box: Some(imageflow_types::WatermarkConstraintBox::ImageMargins {left: 700, top: 700, right: 0, bottom: 0}),
                fit_mode: Some(imageflow_types::WatermarkConstraintMode::Within),
                min_canvas_width: None,
                min_canvas_height: None,
                opacity: Some(0.9f32),
                hints: None,
            })
        ]
    );
    assert!(matched);
}

#[test]
fn test_watermark_image_on_png() {
    let matched = compare_multiple(Some(vec![
        IoTestEnum::Url("https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/shirt_transparent.png".to_owned()),
        IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/1_webp_a.sm.png".to_owned())
    ]), 500,
                                   "test_watermark_image_on_png shirt_with_webp", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            Node::Decode {io_id: 0, commands: None},
            Node::Watermark(imageflow_types::Watermark{
                io_id: 1,
                gravity: Some(imageflow_types::ConstraintGravity::Percentage {x: 100f32, y: 100f32}),
                fit_box: Some(imageflow_types::WatermarkConstraintBox::ImagePercentage {x1: 0f32, y1: 0f32, x2: 90f32, y2: 90f32}),
                fit_mode: Some(imageflow_types::WatermarkConstraintMode::Within),
                min_canvas_width: None,
                min_canvas_height: None,
                opacity: None,
                hints: Some(imageflow_types::ResampleHints{
                    sharpen_percent: None,
                    down_filter: None,
                    up_filter: None,
                    scaling_colorspace: None,
                    background_color: None,
                    resample_when: None,
                    sharpen_when: None
                }),

            })
        ]
    );
    assert!(matched);
}

#[test]
fn test_watermark_jpeg_over_pnga() {
    let matched = compare_multiple(Some(vec![
        IoTestEnum::Url("https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/shirt_transparent.png".to_owned()),
        IoTestEnum::Url("https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/gamma_test.jpg".to_owned())
    ]), 500,
                                   "test_watermark_jpeg_over_pnga gamma_test_30pct", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            Node::Decode {io_id: 0, commands: None},
            Node::Watermark(imageflow_types::Watermark{
                io_id: 1,
                gravity: Some(imageflow_types::ConstraintGravity::Percentage {x: 100f32, y: 100f32}),
                fit_box: Some(imageflow_types::WatermarkConstraintBox::ImagePercentage {x1: 0f32, y1: 0f32, x2: 90f32, y2: 90f32}),
                fit_mode: Some(imageflow_types::WatermarkConstraintMode::Within),
                min_canvas_width: None,
                min_canvas_height: None,
                opacity: Some(0.3f32),
                hints: Some(imageflow_types::ResampleHints{
                    sharpen_percent: None,
                    down_filter: None,
                    up_filter: None,
                    scaling_colorspace: None,
                    background_color: None,
                    resample_when: None,
                    sharpen_when: None
                }),

            })
        ]
    );
    assert!(matched);
}
