use crate::common::*;
use imageflow_types::{
    Color, ColorSrgb, CommandStringKind, Constraint, ConstraintMode,
    EncoderPreset, Node,
};

const DEBUG_GRAPH: bool = false;
const POPULATE_CHECKSUMS: bool = true;

#[test]
fn test_encode_gradients() {
    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Encode { io_id: 1, preset: EncoderPreset::libpng32() },
    ];

    compare_encoded(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/gradients.png"
                .to_owned(),
        )),
        "test_encode_gradients png32_passthrough",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            max_file_size: Some(100000),
            similarity: Similarity::AllowOffByOneBytesRatio(0.01),
        },
        steps,
    );
}

#[test]
fn test_transparent_png_to_png() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/shirt_transparent.png".to_owned())),
        "test_transparent_png_to_png shirt",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowOffByOneBytesCount(100),
            max_file_size: None
        },
        vec![
            Node::CommandString{
                kind: CommandStringKind::ImageResizer4,
                value: "format=png".to_owned(),
                decode: Some(0),
                encode: Some(1),
                watermarks: None
            }
        ]
    );
}

#[test]
fn test_problematic_png_lossy() {
    compare_encoded(
        Some(IoTestEnum::Url("https://imageflow-resources.s3.us-west-2.amazonaws.com/test_inputs/png_turns_empty_2.png".to_owned())),
        "test_problematic_png_lossy crop_1230x760",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowDssimMatch(0.0, 0.002),
            max_file_size: None
        },
        vec![
            Node::CommandString{
                kind: CommandStringKind::ImageResizer4,
                value: "w=1230&h=760&png.quality=75&mode=crop&scale=both".to_owned(),
                decode: Some(0),
                encode: Some(1),
                watermarks: None
            }
        ]
    );
}

#[test]
fn test_transparent_png_to_png_rounded_corners() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/shirt_transparent.png".to_owned())),
        "test_transparent_png_to_png_rounded_corners shirt_cropped",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowOffByOneBytesCount(100),
            max_file_size: None
        },
        vec![
            Node::CommandString{
                kind: CommandStringKind::ImageResizer4,
                value: "format=png&crop=10,10,70,70&cropxunits=100&cropyunits=100&s.roundcorners=100".to_owned(),
                decode: Some(0),
                encode: Some(1),
                watermarks: None
            }
        ]
    );
}

#[test]
fn test_transparent_png_to_jpeg() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/shirt_transparent.png".to_owned())),
        "test_transparent_png_to_jpeg shirt",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowDssimMatch(0.0, 0.002),
            max_file_size: None
        },
        vec![
            Node::CommandString{
                kind: CommandStringKind::ImageResizer4,
                value: "format=jpg".to_owned(),
                decode: Some(0),
                encode: Some(1),
                watermarks: None
            }
        ]
    );
}

#[test]
fn test_transparent_png_to_jpeg_constrain() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/shirt_transparent.png".to_owned())),
        "test_transparent_png_to_jpeg_constrain 300x300_mozjpeg",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowDssimMatch(0.0, 0.002),
            max_file_size: None
        },
        vec![
            Node::Decode{
                io_id: 0,
                commands: None
            },
            Node::Constrain(Constraint {
                    mode: ConstraintMode::Within,
                    w: Some(300),
                    h: Some(300),
                    hints: None,
                    gravity: None,
                    canvas_color: None
                }
            ),
            Node::Encode{
                io_id: 1,
                preset: EncoderPreset::Mozjpeg { quality: Some(100), progressive: None, matte: None }
            }
        ]
    );
}

#[test]
fn test_matte_transparent_png() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/shirt_transparent.png".to_owned())),
        "test_matte_transparent_png shirt_300x300_white_matte",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowDssimMatch(0.0, 0.002),
            max_file_size: None
        },
        vec![
            Node::Decode{
                io_id: 0,
                commands: None
            },
            Node::Constrain(Constraint {
                mode: ConstraintMode::Within,
                w: Some(300),
                h: Some(300),
                hints: None,
                gravity: None,
                canvas_color:  None
            }
            ),
            Node::Encode{
                io_id: 1,
                preset: EncoderPreset::Libpng { depth: None, matte: Some(Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_string()))), zlib_compression: None }
            }
        ]
    );
}

#[test]
fn test_branching_crop_whitespace() {
    let preset = EncoderPreset::Lodepng { maximum_deflate: None };

    let s = imageflow_core::clients::fluent::fluently().decode(0);
    let v1 = s.branch();
    let v2 = v1.branch().crop_whitespace(200, 0f32);
    let framewise =
        v1.encode(1, preset.clone()).builder().with(v2.encode(2, preset.clone())).to_framewise();

    compare_encoded_framewise(
        Some(IoTestEnum::Url("https://imageflow-resources.s3.us-west-2.amazonaws.com/test_inputs/little_gradient_whitespace.jpg".to_owned())),
        "test_branching_crop_whitespace gradient",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowDssimMatch(0.0, 0.002),
            max_file_size: None
        },
        framewise,
        2
    );
}

#[test]
fn test_transparent_webp_to_webp() {
    compare_encoded(
        Some(IoTestEnum::Url(
            "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/1_webp_ll.webp"
                .to_owned(),
        )),
        "test_transparent_webp_to_webp lossless_100x100",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints { similarity: Similarity::AllowOffByOneBytesCount(500), max_file_size: None },
        vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "format=webp&width=100&height=100&webp.lossless=true".to_owned(),
            decode: Some(0),
            encode: Some(1),
            watermarks: None,
        }],
    );
}

#[test]
fn test_webp_to_webp_quality() {
    compare_encoded(
        Some(IoTestEnum::Url(
            "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/1_webp_ll.webp"
                .to_owned(),
        )),
        "test_webp_to_webp_quality q5_100x100",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowDssimMatch(0.0, 1.0),
            max_file_size: Some(2000),
        },
        vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "format=webp&width=100&height=100&quality=5".to_owned(),
            decode: Some(0),
            encode: Some(1),
            watermarks: None,
        }],
    );
}

#[test]
fn test_jpeg_simple() {
    let url = "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/Landscape_1.jpg".to_owned();
    let matched = compare(
        Some(IoTestEnum::Url(url)),
        500,
        "test_jpeg_simple landscape_within_70x70",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Constrain(Constraint {
                mode: ConstraintMode::Within,
                w: Some(70),
                h: Some(70),
                hints: None,
                gravity: None,
                canvas_color: None,
            }),
        ],
    );
    assert!(matched);
}

#[test]
fn test_jpeg_simple_rot_90() {
    let url = "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/Landscape_1.jpg".to_owned();
    let matched = compare(
        Some(IoTestEnum::Url(url)),
        500,
        "test_jpeg_simple_rot_90 landscape_70x70",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Constrain(Constraint {
                mode: ConstraintMode::Within,
                w: Some(70),
                h: Some(70),
                hints: None,
                gravity: None,
                canvas_color: None,
            }),
            Node::Rotate90,
        ],
    );
    assert!(matched);
}

#[test]
fn test_rot_90_and_red_dot() {
    let url = "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/Landscape_1.jpg".to_owned();
    let matched = compare(
        Some(IoTestEnum::Url(url)),
        500,
        "test_rot_90_and_red_dot landscape_70x70",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Constrain(Constraint {
                mode: ConstraintMode::Within,
                w: Some(70),
                h: Some(70),
                hints: None,
                gravity: None,
                canvas_color: None,
            }),
            Node::Rotate90,
            Node::WatermarkRedDot,
        ],
    );
    assert!(matched);
}

#[test]
fn test_rot_90_and_red_dot_command_string() {
    let url = "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/Landscape_1.jpg".to_owned();
    let matched = compare(
        Some(IoTestEnum::Url(url)),
        500,
        "test_rot_90_and_red_dot_command_string landscape_70x70",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "w=70&h=70&mode=max&rotate=90&watermark_red_dot=true".to_string(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
    );
    assert!(matched);
}

#[test]
fn test_negatives_in_command_string() {
    let url = "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/red-leaf.jpg"
        .to_owned();
    let matched = compare(
        Some(IoTestEnum::Url(url)),
        500,
        "test_negatives_in_command_string red_leaf_negative_height",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "h=-100&maxwidth=2&mode=crop".to_string(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
    );
    assert!(matched);
}

#[test]
fn test_jpeg_crop() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg"
                .to_owned(),
        )),
        500,
        "test_jpeg_crop waterhouse_100x200",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "width=100&height=200&mode=crop".to_owned(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
    );
    assert!(matched);
}

#[test]
fn decode_cmyk_jpeg() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/cmyk_logo.jpg"
                .to_owned(),
        )),
        500,
        "decode_cmyk_jpeg logo_passthrough",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "".to_owned(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
    );
    assert!(matched);
}

#[test]
fn decode_rgb_with_cmyk_profile_jpeg() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/wrenches.jpg"
                .to_owned(),
        )),
        500,
        "decode_rgb_with_cmyk_profile_jpeg wrenches_ignore_icc",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "ignore_icc_errors=true".to_owned(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
    );
    assert!(matched);
}

#[test]
fn test_crop_with_preshrink() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://resizer-images.s3.amazonaws.com/private/cropissue.jpg".to_owned(),
        )),
        500,
        "test_crop_with_preshrink 170x220_crop",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "w=170&h=220&mode=crop&scale=both&crop=449,0,-472,0".to_owned(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
    );
    assert!(matched);
}
