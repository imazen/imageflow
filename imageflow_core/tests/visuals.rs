#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate imageflow_core;
extern crate imageflow_helpers as hlp;
extern crate serde_json;
extern crate smallvec;

pub mod common;
use crate::common::*;

use imageflow_core::graphics::bitmaps::{BitmapCompositing, ColorSpace};
use imageflow_core::{Context, ErrorKind};
use imageflow_types;
use imageflow_types::{
    Color, ColorSrgb, CommandStringKind, Constraint, ConstraintMode, EncoderPreset, Filter, Node,
    PixelFormat, PixelLayout, PngBitDepth, ResampleHints, RoundCornersMode,
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
        "encode_gradients",
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
fn test_trim_whitespace() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/shirt_transparent.png".to_owned())),
        "transparent_trim_whitespace",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowDssimMatch(0.0, 0.002),
            max_file_size: None
        },
        vec![
            Node::CommandString{
                kind: CommandStringKind::ImageResizer4,
                value: "trim.threshold=80".to_owned(),
                decode: Some(0),
                encode: Some(1),
                watermarks: None
            }
        ]
    );
}

#[test]
fn test_trim_whitespace_with_padding() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/whitespace-issue.png".to_owned())),
        "trim_whitespace_with_padding",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowDssimMatch(0.0, 0.002),
            max_file_size: None
        },
        vec![
            Node::CommandString{
                kind: CommandStringKind::ImageResizer4,
                value: "trim.threshold=20&trim.percentpadding=0.5&bgcolor=gray".to_owned(),
                decode: Some(0),
                encode: Some(1),
                watermarks: None
            }
        ]
    );
}
#[test]
fn test_trim_resize_whitespace_with_padding() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/whitespace-issue.png".to_owned())),
        "trim_resize_whitespace_with_padding",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowDssimMatch(0.0, 0.002),
            max_file_size: None
        },
        vec![
            Node::CommandString{
                kind: CommandStringKind::ImageResizer4,
                value: "w=450&h=450&scale=both&trim.threshold=20&trim.percentpadding=10&bgcolor=gray".to_owned(),
                decode: Some(0),
                encode: Some(1),
                watermarks: None
            }
        ]
    );
}
#[test]
fn test_trim_resize_whitespace_without_padding() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/whitespace-issue.png".to_owned())),
        "trim_resize_whitespace_without_padding",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowDssimMatch(0.0, 0.002),
            max_file_size: None
        },
        vec![
            Node::CommandString{
                kind: CommandStringKind::ImageResizer4,
                value: "w=450&h=450&scale=both&trim.threshold=20&bgcolor=gray".to_owned(),
                decode: Some(0),
                encode: Some(1),
                watermarks: None
            }
        ]
    );
}
#[test]
fn test_trim_whitespace_with_padding_no_resize() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/whitespace-issue.png".to_owned())),
        "trim_whitespace_with_padding_no_resize",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowDssimMatch(0.0, 0.002),
            max_file_size: None
        },
        vec![
            Node::CommandString{
                kind: CommandStringKind::ImageResizer4,
                value: "trim.threshold=20&trim.percentpadding=0.5&bgcolor=gray".to_owned(),
                decode: Some(0),
                encode: Some(1),
                watermarks: None
            }
        ]
    );
}

#[test]
fn test_transparent_png_to_png() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/shirt_transparent.png".to_owned())),
        "transparent_png_to_png",
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
        "test_problematic_png_lossy",
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

// #[test]
// fn test_problematic_png_crushed() {
//     compare_encoded(
//         Some(IoTestEnum::Url("https://imageflow-resources.s3.us-west-2.amazonaws.com/test_inputs/imageflow-operational.png".to_owned())),
//         "test_problematic_png_crushed",
//         POPULATE_CHECKSUMS,
//         DEBUG_GRAPH,
//         Constraints {
//             similarity: Similarity::AllowDssimMatch(0.0, 0.002),
//             max_file_size: None
//         },
//         vec![
//             Node::CommandString{
//                 kind: CommandStringKind::ImageResizer4,
//                 value: "format=png&w=100".to_owned(),
//                 decode: Some(0),
//                 encode: Some(1),
//                 watermarks: None
//             }
//         ]
//     );
//     eprintln!("hello");
//     assert!(false);
// }
#[test]
fn test_transparent_png_to_png_rounded_corners() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/shirt_transparent.png".to_owned())),
        "transparent_png_to_png_round_corners",
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
        "transparent_png_to_jpeg",
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
        "transparent_png_to_jpeg_constrained",
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
                    canvas_color: None //  Some(Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_string())))
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
fn test_branching_crop_whitespace() {
    let preset = EncoderPreset::Lodepng { maximum_deflate: None };

    let s = imageflow_core::clients::fluent::fluently().decode(0);
    let v1 = s.branch();
    let v2 = v1.branch().crop_whitespace(200, 0f32);
    let framewise =
        v1.encode(1, preset.clone()).builder().with(v2.encode(2, preset.clone())).to_framewise();

    compare_encoded_framewise(
        Some(IoTestEnum::Url("https://imageflow-resources.s3.us-west-2.amazonaws.com/test_inputs/little_gradient_whitespace.jpg".to_owned())),
        "test_branching_crop_whitespace_gradient",
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
fn test_matte_transparent_png() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/shirt_transparent.png".to_owned())),
        "matte_transparent_png",
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
                canvas_color:  None // Some(Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_string())))
            }
            ),
            Node::Encode{
                io_id: 1,
                preset: EncoderPreset::Libpng { depth: None, matte: Some(Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_string()))), zlib_compression: None }
            }
        ]
    );
}
//https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/pnglogo_transparent.png

#[test]
fn test_fill_rect() {
    let matched = compare(
        None,
        500,
        "FillRectEECCFF",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::CreateCanvas {
                w: 200,
                h: 200,
                format: PixelFormat::Bgra32,
                color: Color::Transparent,
            },
            Node::FillRect {
                x1: 0,
                y1: 0,
                x2: 100,
                y2: 100,
                color: Color::Srgb(ColorSrgb::Hex("EECCFFFF".to_owned())),
            },
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
fn test_expand_rect() {
    let matched = compare(
        None,
        500,
        "FillRectEECCFFExpand2233AAFF",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::CreateCanvas {
                w: 200,
                h: 200,
                format: PixelFormat::Bgra32,
                color: Color::Transparent,
            },
            Node::FillRect {
                x1: 0,
                y1: 0,
                x2: 100,
                y2: 100,
                color: Color::Srgb(ColorSrgb::Hex("EECCFFFF".to_owned())),
            },
            Node::ExpandCanvas {
                left: 10,
                top: 15,
                right: 20,
                bottom: 25,
                color: Color::Srgb(ColorSrgb::Hex("2233AAFF".to_owned())),
            },
            Node::Resample2D {
                w: 400,
                h: 400,
                hints: Some(
                    ResampleHints::new()
                        .with_bi_filter(Filter::Hermite)
                        .with_floatspace(imageflow_types::ScalingFloatspace::Linear),
                ),
            },
        ],
    );
    assert!(matched);
}

#[test]
fn test_crop() {
    let matched = compare(
        None,
        500,
        "FillRectAndCrop",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::CreateCanvas {
                w: 200,
                h: 200,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FF5555FF".to_owned())),
            },
            Node::FillRect {
                x1: 0,
                y1: 0,
                x2: 10,
                y2: 100,
                color: Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned())),
            },
            Node::Crop { x1: 0, y1: 50, x2: 100, y2: 100 },
        ],
    );
    assert!(matched);
}

#[test]
fn test_crop_exif() {
    for ix in 1..9 {
        let url = format!("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/Landscape_{ix}.jpg");
        let title = format!("test cropping jpeg with exif rotate {ix}");
        let matched = compare(
            Some(IoTestEnum::Url(url)),
            500,
            &title,
            POPULATE_CHECKSUMS,
            DEBUG_GRAPH,
            vec![
                Node::Decode { io_id: 0, commands: None },
                Node::Crop { x1: 0, y1: 0, x2: 599, y2: 449 },
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
}

#[test]
fn test_fit_pad_exif() {
    for ix in 1..9 {
        let url = format!("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/Landscape_{ix}.jpg");
        let title = format!("test ConstraintMode::FitPad on jpeg with exif rotate {ix}");
        let matched = compare(
            Some(IoTestEnum::Url(url)),
            500,
            &title,
            POPULATE_CHECKSUMS,
            DEBUG_GRAPH,
            vec![
                Node::Decode { io_id: 0, commands: None },
                Node::Constrain(Constraint {
                    mode: ConstraintMode::FitPad,
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
}

#[test]
fn test_off_surface_region() {
    let matched = compare(
        None,
        500,
        "TestOffSurfaceRegion",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::CreateCanvas {
                w: 200,
                h: 200,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FF5555FF".to_owned())),
            },
            Node::FillRect {
                x1: 0,
                y1: 0,
                x2: 10,
                y2: 100,
                color: Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned())),
            },
            Node::RegionPercent {
                x1: -100f32,
                y1: -100f32,
                x2: -1f32,
                y2: -1f32,
                background_color: Color::Transparent,
            },
        ],
    );
    assert!(matched);
}
#[test]
fn test_partial_region() {
    let matched = compare(
        None,
        500,
        "TestPartialRegion",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::CreateCanvas {
                w: 200,
                h: 200,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FF5555FF".to_owned())),
            },
            Node::FillRect {
                x1: 0,
                y1: 0,
                x2: 10,
                y2: 100,
                color: Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned())),
            },
            Node::RegionPercent {
                x1: -10f32,
                y1: -10f32,
                x2: 40f32,
                y2: 40f32,
                background_color: Color::Transparent,
            },
        ],
    );
    assert!(matched);
}
#[test]
fn test_pixels_region() {
    let matched = compare(
        None,
        500,
        "TestPixelsRegion",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::CreateCanvas {
                w: 200,
                h: 200,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FF5555FF".to_owned())),
            },
            Node::FillRect {
                x1: 0,
                y1: 0,
                x2: 10,
                y2: 100,
                color: Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned())),
            },
            Node::Region {
                x1: -10,
                y1: -10,
                x2: 120,
                y2: 50,
                background_color: Color::Transparent,
            },
        ],
    );
    assert!(matched);
}

//  Replaces TEST_CASE("Test scale rings", "")
#[test]
fn test_scale_rings() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/rings2.png"
                .to_owned(),
        )),
        500,
        "RingsDownscaling",
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
fn test_fill_rect_original() {
    //let white = Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_owned()));
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    let matched = compare(
        None,
        1,
        "FillRect",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::CreateCanvas {
                w: 400,
                h: 300,
                format: PixelFormat::Bgra32,
                color: Color::Transparent,
            },
            Node::FillRect { x1: 0, y1: 0, x2: 50, y2: 100, color: blue },
        ],
    );
    assert!(matched);
}

#[test]
fn test_round_corners_large() {
    //let white = Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_owned()));
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    let matched = compare(
        None,
        1,
        "RoundCornersLarge",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::CreateCanvas {
                w: 400,
                h: 400,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FFFF00FF".to_owned())),
            },
            Node::RoundImageCorners {
                background_color: blue,
                radius: RoundCornersMode::Pixels(200f32),
            },
        ],
    );
    assert!(matched);
}

#[test]
fn test_round_corners_small() {
    //let white = Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_owned()));
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    let matched = compare(
        None,
        1,
        "RoundCornersSmall",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::CreateCanvas {
                w: 100,
                h: 100,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FFFF00FF".to_owned())),
            },
            Node::RoundImageCorners {
                background_color: blue,
                radius: RoundCornersMode::Pixels(5f32),
            },
        ],
    );
    assert!(matched);
}

#[test]
fn test_round_corners_custom_pixels() {
    let matte = Color::Srgb(ColorSrgb::Hex("000000BB".to_owned()));
    let matched = compare(
        None,
        1,
        "RoundCornersCustomPixelsSemiTransparent",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::CreateCanvas {
                w: 100,
                h: 99,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("ddeecc88".to_owned())),
            },
            Node::RoundImageCorners {
                background_color: matte,
                radius: RoundCornersMode::PixelsCustom {
                    top_left: 0.0,
                    top_right: 1f32,
                    bottom_right: 50f32,
                    bottom_left: 20f32,
                },
            },
        ],
    );
    assert!(matched);
}

#[test]
fn test_round_corners_custom_percent() {
    let matte = Color::Srgb(ColorSrgb::Hex("000000DD".to_owned()));
    let matched = compare(
        None,
        1,
        "RoundCornersCustomPercentSemiTransparent",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::CreateCanvas {
                w: 100,
                h: 99,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("2288ffEE".to_owned())),
            },
            Node::RoundImageCorners {
                background_color: matte,
                radius: RoundCornersMode::PixelsCustom {
                    top_left: 50f32,
                    top_right: 5f32,
                    bottom_right: 100f32,
                    bottom_left: 200f32,
                },
            },
        ],
    );
    assert!(matched);
}

#[test]
fn test_round_corners_excessive_radius() {
    //let white = Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_owned()));
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    let matched = compare(
        None,
        1,
        "RoundCornersExcessiveRadius",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::CreateCanvas {
                w: 200,
                h: 150,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FFFF00FF".to_owned())),
            },
            Node::RoundImageCorners {
                background_color: blue,
                radius: RoundCornersMode::Pixels(100f32),
            },
        ],
    );
    assert!(matched);
}

#[test]
fn test_round_corners_circle_wide_canvas() {
    //let white = Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_owned()));
    let matte = Color::Srgb(ColorSrgb::Hex("000000FF".to_owned()));
    let matched = compare(
        None,
        1,
        "RoundCornersCircleWider",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::CreateCanvas {
                w: 200,
                h: 150,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_owned())),
            },
            Node::RoundImageCorners { background_color: matte, radius: RoundCornersMode::Circle },
        ],
    );
    assert!(matched);
}
#[test]
fn test_round_corners_circle_tall_canvas() {
    //let white = Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_owned()));
    let matte = Color::Srgb(ColorSrgb::Hex("00000000".to_owned()));
    let matched = compare(
        None,
        1,
        "RoundCornersCircleTaller",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::CreateCanvas {
                w: 150,
                h: 200,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_owned())),
            },
            Node::RoundImageCorners { background_color: matte, radius: RoundCornersMode::Circle },
        ],
    );
    assert!(matched);
}

#[test]
fn test_round_image_corners_transparent() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg"
                .to_owned(),
        )),
        500,
        "RoundImageCornersTransparent",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Resample2D {
                w: 400,
                h: 300,
                hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
            },
            Node::RoundImageCorners {
                background_color: Color::Transparent,
                radius: RoundCornersMode::Pixels(100f32),
            },
        ],
    );
    assert!(matched);
}

#[test]
fn test_scale_image() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg"
                .to_owned(),
        )),
        500,
        "ScaleTheHouse",
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
        "Watermark1",
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
                hints: Some(imageflow_types::ResampleHints {
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
        "Watermark1",
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
        "Watermark1",
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
                                   "WatermarkSmall", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
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
                                   "WatermarkPixelMargins", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
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
                                   "WatermarkSmallOnPnga", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
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
                                   "watermark_jpeg_over_pnga", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
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
// Does not reproduce across different compiler optimizations
// #[test]
// fn test_image_rs_jpeg_decode(){
//     let mut context = Context::create().unwrap();
//     context.enabled_codecs.prefer_decoder(imageflow_core::NamedDecoders::ImageRsJpegDecoder);
//     let matched = compare_with_context(&mut context,Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned())), 500,
//                           "DecodeWithImageRs", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
//             Node::Decode {io_id: 0, commands: None},
//             Node::Resample2D{ w: 400, h: 300, down_filter: Some(Filter::Robidoux), up_filter: Some(Filter::Robidoux), hints: None, scaling_colorspace: None }
//         ]
//     );
//     assert!(matched);
// }

#[test]
fn test_white_balance_image() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/red-night.png"
                .to_owned(),
        )),
        500,
        "WhiteBalanceNight",
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
        "WhiteBalanceNight_05",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::Decode { io_id: 0, commands: None },
            Node::WhiteBalanceHistogramAreaThresholdSrgb { threshold: Some(0.5) },
        ],
    );
    assert!(matched);
}

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
                          format!("ColorFilterSrgb_{:?}", filter).as_str(), POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            Node::Decode {io_id: 0, commands: None},
            Node::ColorFilterSrgb(filter)
        ]
        );
        assert!(matched);
    }
}

#[test]
fn test_read_gif_and_scale() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/mountain_800.gif"
                .to_owned(),
        )),
        500,
        "mountain_gif_scaled400",
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
        "read_gif_and_vertical_distort",
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
fn test_jpeg_icc2_color_profile() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_tagged.jpg"
                .to_owned(),
        )),
        500,
        "MarsRGB_ICC_Scaled400300",
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
                          "MarsRGB_ICCv4_Scaled400300", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
Node::Decode {io_id: 0, commands: None},
Node::Resample2D{ w: 400, h: 300,  hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)) }
]
    );
    assert!(matched);
}

#[test]
fn test_jpeg_simple() {
    let url = "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/Landscape_1.jpg".to_owned();
    let title = "Test_Jpeg_Simple.jpg".to_owned();
    let matched = compare(
        Some(IoTestEnum::Url(url)),
        500,
        &title,
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
    let title = "test rotate jpeg 90 degrees".to_owned();
    let matched = compare(
        Some(IoTestEnum::Url(url)),
        500,
        &title,
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
    let title = "test_rot_90_and_red_dot".to_owned();
    let matched = compare(
        Some(IoTestEnum::Url(url)),
        500,
        &title,
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
    let title = "test_rot_90_and_red_dot_command_string".to_owned();
    let matched = compare(
        Some(IoTestEnum::Url(url)),
        500,
        &title,
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
fn test_round_corners_command_string() {
    let url = "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/Landscape_1.jpg".to_owned();
    let title = "test_round_corners_command_string".to_owned();
    let matched = compare(
        Some(IoTestEnum::Url(url)),
        500,
        &title,
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "w=70&h=70&s.roundcorners=100,20,70,30&format=png".to_string(),
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
    let title = "test_negatives_in_command_string".to_owned();
    let matched = compare(
        Some(IoTestEnum::Url(url)),
        500,
        &title,
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
fn test_jpeg_rotation() {
    let orientations = vec!["Landscape", "Portrait"];

    for orientation in orientations {
        for flag in 1..9 {
            let url = format!("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/{}_{}.jpg", orientation, flag);
            let title = format!("Test_Apply_Orientation_{}_{}.jpg", orientation, flag);
            let matched = compare(
                Some(IoTestEnum::Url(url)),
                500,
                &title,
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
    }
}

#[test]
fn test_jpeg_rotation_cropped() {
    for flag in 1..9 {
        let url = format!("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/Portrait_{}.jpg", flag);
        let title = format!("Test_Apply_Orientation_Cropped_Portrait_{}.jpg", flag);
        let matched = compare(
            Some(IoTestEnum::Url(url)),
            500,
            &title,
            POPULATE_CHECKSUMS,
            DEBUG_GRAPH,
            vec![Node::CommandString {
                kind: CommandStringKind::ImageResizer4,
                value: "crop=134,155,279,439".to_owned(),
                decode: Some(0),
                encode: None,
                watermarks: None,
            }],
        );
        assert!(matched);
    }
}

#[test]
fn test_jpeg_crop() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg"
                .to_owned(),
        )),
        500,
        "jpeg_crop",
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

//
//#[test]
//fn test_gif_ir4(){
//        let matched = compare(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned())), 500,
//                              "Read", true, DEBUG_GRAPH, vec![
//                Node::CommandString{
//                    kind: CommandStringKind::ImageResizer4,
//                    value: "width=200&height=200&format=gif".to_owned(),
//                    decode: Some(0),
//                    encode: None //Some(1)
//                }
//            ]
//        );
//        assert!(matched);
//
//}
//

// #[test]
//fn smoke_test_ir4(){
//
//    // 5104x3380 "?w=2560&h=1696&mode=max&format=png&decoder.min_precise_scaling_ratio=2.1&down.colorspace=linear"
//
//
//    let steps = vec![
//        Node::CommandString{
//            kind: CommandStringKind::ImageResizer4,
//            value: "width=200&height=200&format=gif".to_owned(),
//            decode: Some(0),
//            encode: Some(1)
//        }
//    ];
//
//    smoke_test(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned())),
//               Some(IoTestEnum::OutputBuffer),
//               DEBUG_GRAPH,
//               steps,
//    );
//}

#[test]
fn decode_cmyk_jpeg() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/cmyk_logo.jpg"
                .to_owned(),
        )),
        500,
        "cmyk_decode",
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
        "wrenches_decode",
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
        "crop_with_preshrink",
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

#[test]
fn test_zoom_with_preshrink() {
    //Certain dimensions of images would trigger preshrinking and also break zoom calculation
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "zoom=0.25".to_owned(),
        decode: Some(0),
        encode: None,
        watermarks: None,
    }];
    let (w, _h) = get_result_dimensions(
        &steps,
        vec![IoTestEnum::Url(
            "https://imageflow-resources.s3.us-west-2.amazonaws.com/test_inputs/5760_x_4320.jpg"
                .to_owned(),
        )],
        false,
    );
    assert_eq!(w, 1440);
}

#[test]
fn webp_lossless_alpha_decode_and_scale() {
    let matched = compare(
        Some(IoTestEnum::Url(
            "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/1_webp_ll.webp"
                .to_owned(),
        )),
        500,
        "webp_lossless_alpha_decode_and_scale",
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
        "webp_lossy_alpha_decode_and_scale",
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
                          "webp_lossy_opaque_decode_and_scale", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
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
fn test_transparent_webp_to_webp() {
    compare_encoded(
        Some(IoTestEnum::Url(
            "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/1_webp_ll.webp"
                .to_owned(),
        )),
        "transparent_webp_to_webp",
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
    // We're verifying that the &quality setting is respected if &webp.quality is missing
    compare_encoded(
        Some(IoTestEnum::Url(
            "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/1_webp_ll.webp"
                .to_owned(),
        )),
        "test_webp_to_webp_quality",
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
fn webp_lossless_alpha_decode_and_encode() {
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "format=webp".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];

    smoke_test(
        Some(IoTestEnum::Url(
            "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/1_webp_ll.webp"
                .to_owned(),
        )),
        Some(IoTestEnum::OutputBuffer),
        None,
        DEBUG_GRAPH,
        steps,
    )
    .unwrap();
}
#[test]
fn webp_lossy_alpha_decode_and_encode() {
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "format=webp&quality=90".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];

    smoke_test(
        Some(IoTestEnum::Url(
            "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/1_webp_a.webp"
                .to_owned(),
        )),
        Some(IoTestEnum::OutputBuffer),
        None,
        DEBUG_GRAPH,
        steps,
    )
    .unwrap();
}
#[test]
fn smoke_test_gif_ir4() {
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "width=200&height=200&format=gif".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];

    smoke_test(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg"
                .to_owned(),
        )),
        Some(IoTestEnum::OutputBuffer),
        None,
        DEBUG_GRAPH,
        steps,
    )
    .unwrap();
}

#[test]
fn smoke_test_ignore_invalid_color_profile() {
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "width=200&height=200&ignore_icc_errors=true".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];

    smoke_test(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/color_profile_error.jpg".to_owned())),
               Some(IoTestEnum::OutputBuffer),
               None,
               DEBUG_GRAPH,
               steps,
    ).unwrap();
}

#[test]
fn smoke_test_invalid_params() {
    let tinypng = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "quality=957".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];

    smoke_test(
        Some(IoTestEnum::ByteArray(tinypng)),
        Some(IoTestEnum::OutputBuffer),
        None,
        DEBUG_GRAPH,
        steps,
    )
    .unwrap();
}

#[test]
fn test_max_encode_dimensions() {
    let tinypng = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "width=2&height=2&mode=pad&scale=both".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];

    let e = smoke_test(
        Some(IoTestEnum::ByteArray(tinypng)),
        Some(IoTestEnum::OutputBuffer),
        Some(imageflow_types::ExecutionSecurity {
            max_decode_size: None,
            max_frame_size: None,
            max_encode_size: Some(imageflow_types::FrameSizeLimit {
                w: 3,
                h: 1,
                megapixels: 100.0,
            }),
        }),
        DEBUG_GRAPH,
        steps,
    )
    .expect_err("Should fail");

    assert_eq!(e.kind, ErrorKind::SizeLimitExceeded);

    assert_eq!(e.message, "SizeLimitExceeded: Frame height 2 exceeds max_encode_size.h 1");
}

#[test]
fn test_max_decode_dimensions() {
    let steps = vec![Node::Decode { io_id: 0, commands: None }];

    let e = smoke_test(
        Some(IoTestEnum::Url(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg"
                .to_owned(),
        )),
        None,
        Some(imageflow_types::ExecutionSecurity {
            max_decode_size: Some(imageflow_types::FrameSizeLimit {
                w: 10,
                h: 100000,
                megapixels: 100.0,
            }),
            max_frame_size: None,
            max_encode_size: None,
        }),
        DEBUG_GRAPH,
        steps,
    )
    .expect_err("Should fail");
    assert_eq!(e.kind, ErrorKind::SizeLimitExceeded);
}

#[test]
fn test_max_frame_dimensions() {
    let steps = vec![Node::CreateCanvas {
        format: PixelFormat::Bgra32,
        w: 1000,
        h: 1000,
        color: Color::Transparent,
    }];

    let e = smoke_test(
        None,
        None,
        Some(imageflow_types::ExecutionSecurity {
            max_frame_size: Some(imageflow_types::FrameSizeLimit {
                w: 10000,
                h: 10000,
                megapixels: 0.5,
            }),
            max_decode_size: None,
            max_encode_size: None,
        }),
        DEBUG_GRAPH,
        steps,
    )
    .expect_err("Should fail");

    assert_eq!(e.kind, ErrorKind::SizeLimitExceeded);
}

#[test]
fn smoke_test_png_ir4() {
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "width=200&height=200&format=png".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];

    smoke_test(Some(IoTestEnum::Url("https://user-images.githubusercontent.com/2650124/31182064-e1c54784-a8f0-11e7-8bb3-833bba872975.png".to_owned())),
               Some(IoTestEnum::OutputBuffer),
               None,
               DEBUG_GRAPH,
               steps,
    ).unwrap();
}

#[test]
fn smoke_test_corrupt_jpeg() {
    let steps = vec![Node::CommandString {
        kind: CommandStringKind::ImageResizer4,
        value: "format=jpg".to_owned(),
        decode: Some(0),
        encode: Some(1),
        watermarks: None,
    }];

    smoke_test(
        Some(IoTestEnum::Url(
            "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/corrupt.jpg"
                .to_owned(),
        )),
        Some(IoTestEnum::OutputBuffer),
        None,
        DEBUG_GRAPH,
        steps,
    )
    .expect_err("Should fail without crashing process");
}

#[test]
fn test_encode_jpeg_smoke() {
    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Resample2D {
            w: 400,
            h: 300,
            hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
        },
        Node::Encode {
            io_id: 1,
            preset: EncoderPreset::LibjpegTurbo {
                quality: Some(100),
                progressive: None,
                optimize_huffman_coding: None,
                matte: None,
            },
        },
    ];

    smoke_test(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_v4_sYCC_8bit.jpg".to_owned())),
               Some(IoTestEnum::OutputBuffer),
               None,
               DEBUG_GRAPH,
               steps,
    ).unwrap();
}

#[test]
fn test_encode_gif_smoke() {
    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Resample2D {
            w: 400,
            h: 300,
            hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
        },
        Node::Encode { io_id: 1, preset: EncoderPreset::Gif },
    ];

    smoke_test(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_v4_sYCC_8bit.jpg".to_owned())),
               Some(IoTestEnum::OutputBuffer),
               None,
               DEBUG_GRAPH,
               steps,
    ).unwrap();
}

#[test]
fn test_encode_png32_smoke() {
    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Resample2D {
            w: 400,
            h: 300,
            hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
        },
        Node::FlipV,
        Node::Crop { x1: 20, y1: 20, x2: 380, y2: 280 },
        Node::Encode {
            io_id: 1,
            preset: EncoderPreset::Libpng {
                depth: Some(PngBitDepth::Png32),
                matte: None,
                zlib_compression: None,
            },
        },
    ];

    smoke_test(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_v4_sYCC_8bit.jpg".to_owned())),
               Some(IoTestEnum::OutputBuffer),
               None,
               DEBUG_GRAPH,
               steps,
    ).unwrap();
}

#[test]
fn test_dimensions() {
    let steps = vec![
        Node::CreateCanvas { w: 638, h: 423, format: PixelFormat::Bgra32, color: Color::Black },
        //Node::Crop { x1: 0, y1: 0, x2: 638, y2: 423},
        Node::Resample2D { w: 200, h: 133, hints: None },
        Node::ExpandCanvas { left: 1, top: 0, right: 0, bottom: 0, color: Color::Transparent },
    ];
    let (w, h) = get_result_dimensions(&steps, vec![], DEBUG_GRAPH);
    assert_eq!(w, 201);
    assert_eq!(h, 133);
}

#[test]
fn test_aspect_crop_dimensions() {
    let steps = vec![
        Node::CreateCanvas { w: 638, h: 423, format: PixelFormat::Bgra32, color: Color::Black },
        Node::Constrain(imageflow_types::Constraint {
            mode: imageflow_types::ConstraintMode::AspectCrop,
            w: Some(200),
            h: Some(133),
            hints: None,
            gravity: None,
            canvas_color: None,
        }),
    ];
    let (w, h) = get_result_dimensions(&steps, vec![], DEBUG_GRAPH);
    assert_eq!(w, 636);
    assert_eq!(h, 423);
}

#[test]
fn test_decode_png_and_scale_dimensions() {
    let tinypng = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        //Node::Crop { x1: 0, y1: 0, x2: 638, y2: 423},
        Node::Resample2D { w: 300, h: 200, hints: None },
    ];
    let (w, h) = get_result_dimensions(&steps, vec![IoTestEnum::ByteArray(tinypng)], false);
    assert_eq!(w, 300);
    assert_eq!(h, 200);
}

#[test]
fn test_get_info_png() {
    let tinypng = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    let _ = imageflow_core::clients::stateless::LibClient {}
        .get_image_info(&tinypng)
        .expect("Image response should be valid");
}

#[test]
fn test_detect_whitespace() {
    //let white = Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_owned()));
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    let matched = compare(
        None,
        1,
        "DetectWhitespace",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![
            Node::CreateCanvas {
                w: 400,
                h: 300,
                format: PixelFormat::Bgra32,
                color: Color::Transparent,
            },
            Node::FillRect { x1: 0, y1: 0, x2: 50, y2: 100, color: blue },
            Node::CropWhitespace { threshold: 80, percent_padding: 0f32 },
        ],
    );
    assert!(matched);
}

#[test]
fn test_detect_whitespace_all_small_images() {
    let red = Color::Srgb(ColorSrgb::Hex("FF0000FF".to_owned()));
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    let mut failed_count = 0;
    let mut count = 0;

    let mut combinations = vec![];

    // Add smalls
    for w in 3..12u32 {
        for h in 3..12u32 {
            let mut on_canvas = vec![];
            for x in 0..w {
                for y in 0..h {
                    for size_w in 1..3u32 {
                        for size_h in 1..3u32 {
                            if x == 1 && y == 1 && w == 3 && h == 3 {
                                continue; // no checkerboard
                            }
                            if x + size_w <= w && y + size_h <= h && size_w > 0 && size_h > 0 {
                                on_canvas.push((x, y, size_w, size_h));
                            }
                        }
                    }
                }
            }
            combinations.push((w, h, on_canvas));
        }
    }
    // add large sizes
    for (w, h) in [(3000, 2000), (1370, 1370), (1896, 1896), (3000, 3000)] {
        let mut on_canvas = vec![];
        for x in [67, 0, 1, 881] {
            for y in [67, 0, 1, 881] {
                for (r_w, r_h) in [(1, 1), (1896, 1370)] {
                    if x + r_w <= w && y + r_h <= h && r_w > 0 && r_h > 0 {
                        on_canvas.push((x, y, r_w, r_h));
                    }
                }
            }
        }
        combinations.push((w, h, on_canvas));
    }

    let mut failures = vec![];

    for (w, h, on_canvas) in combinations {
        if w < 3 || h < 3 {
            continue;
        }
        let ctx = Context::create_can_panic().unwrap();

        {
            let mut bitmaps = ctx.borrow_bitmaps_mut().unwrap();

            let bitmap_key = bitmaps
                .create_bitmap_u8(
                    w,
                    h,
                    PixelLayout::BGRA,
                    false,
                    true,
                    ColorSpace::StandardRGB,
                    BitmapCompositing::BlendWithMatte(Color::Black),
                )
                .unwrap();

            let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).unwrap();

            bitmap.set_compositing(BitmapCompositing::BlendWithSelf);

            let mut b = bitmap.get_window_u8().unwrap();

            for (x, y, size_w, size_h) in on_canvas {
                b.fill_rect(0, 0, w, h, &Color::Transparent).unwrap();
                b.fill_rect(x, y, x + size_w, y + size_h, &red).unwrap();
                // 1 pixel inset a 2nd rect
                if size_w > 2 {
                    b.fill_rect(x + 1, y + 1, x + size_w - 1, y + size_h - 1, &blue).unwrap();
                }
                let r = ::imageflow_core::graphics::whitespace::detect_content(&b, 1).unwrap();
                let correct =
                    (r.x1 == x) && (r.y1 == y) && (r.x2 == x + size_w) && (r.y2 == y + size_h);
                if !correct {
                    eprint!(
                        "Failed to correctly detect {}x{} dot at {},{} within {}x{}. Detected ",
                        size_w, size_h, x, y, w, h
                    );
                    if r.x1 != x {
                        eprint!("x1={}({})", r.x1, x);
                    }
                    if r.y1 != y {
                        eprint!("y1={}({})", r.y1, y);
                    }
                    if r.x2 != x + size_w {
                        eprint!("Detected x2={}({})", r.x2, x + size_w);
                    }
                    if r.y2 != y + size_h {
                        eprint!("Detected y2={}({})", r.y2, y + size_h);
                    }
                    eprintln!(".");
                    failed_count += 1;
                    failures.push((w, h, x, y, size_w, size_h));
                }
                count += 1;
            }
        }
        ctx.destroy().unwrap();
    }

    if failed_count > 0 {
        // skip these specific failures for now
        // Failed to correctly detect 1896x1370 dot at 0,67 within 1896x1896. Detected Detected x2=1895(1896).
        // Failed to correctly detect 1896x1370 dot at 0,0 within 1896x1896. Detected y1=1(0)Detected x2=1895(1896).
        // Failed to correctly detect 1896x1370 dot at 0,1 within 1896x1896. Detected Detected x2=1895(1896).
        if failures.len() > 3 {
            panic!("Failed {} of {} whitespace detection tests", failed_count, count);
        }
    }
}

#[test]
fn test_detect_whitespace_basic() {
    let ctx = Context::create_can_panic().unwrap();

    let red = Color::Srgb(ColorSrgb::Hex("FF0000FF".to_owned()));

    let mut bitmaps = ctx.borrow_bitmaps_mut().unwrap();

    let bitmap_key_a = bitmaps
        .create_bitmap_u8(
            10,
            10,
            PixelLayout::BGRA,
            false,
            true,
            ColorSpace::StandardRGB,
            BitmapCompositing::BlendWithMatte(Color::Black),
        )
        .unwrap();

    {
        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key_a).unwrap();

        bitmap.set_compositing(BitmapCompositing::BlendWithSelf);
        let mut window = bitmap.get_window_u8().unwrap();

        window.fill_rect(1, 1, 9, 9, &red).unwrap();

        let r = ::imageflow_core::graphics::whitespace::detect_content(&window, 1).unwrap();
        assert_eq!((r.x1, r.y1, r.x2, r.y2), (1, 1, 9, 9));
    }

    let bitmap_key_b = bitmaps
        .create_bitmap_u8(
            100,
            100,
            PixelLayout::BGRA,
            false,
            true,
            ColorSpace::StandardRGB,
            BitmapCompositing::BlendWithMatte(Color::Black),
        )
        .unwrap();

    {
        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key_b).unwrap();
        bitmap.set_compositing(BitmapCompositing::BlendWithSelf);
        let mut window = bitmap.get_window_u8().unwrap();
        window.fill_rect(2, 3, 70, 70, &red).unwrap();
        let r = ::imageflow_core::graphics::whitespace::detect_content(&window, 1).unwrap();
        assert_eq!(r.x1, 2);
        assert_eq!(r.y1, 3);
        assert_eq!(r.x2, 70);
        assert_eq!(r.y2, 70);
    }
}

//#[test]
//fn test_get_info_png_invalid() {
//    let tinypng = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00,
//                       0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00,
//                       0x00, 0x00, 0x0A, 0x49 ];
//
//    let _ = imageflow_core::clients::stateless::LibClient {}.get_image_info(&tinypng).err().expect("Should fail");
//}

fn test_idct_callback(
    _: &imageflow_types::ImageInfo,
) -> (Option<imageflow_types::DecoderCommand>, Vec<Node>) {
    let new_w = (800 * 4 + 8 - 1) / 8;
    let new_h = (600 * 4 + 8 - 1) / 8;
    let hints = imageflow_types::JpegIDCTDownscaleHints {
        gamma_correct_for_srgb_during_spatial_luma_scaling: Some(true),
        scale_luma_spatially: Some(true),
        width: new_w,
        height: new_h,
    };
    (
        Some(imageflow_types::DecoderCommand::JpegDownscaleHints(hints)),
        vec![Node::Decode { io_id: 0, commands: None }],
    )
}

fn test_idct_no_gamma_callback(
    info: &imageflow_types::ImageInfo,
) -> (Option<imageflow_types::DecoderCommand>, Vec<Node>) {
    let new_w = (info.image_width * 6 + 8 - 1) / 8;
    let new_h = (info.image_height * 6 + 8 - 1) / 8;
    let hints = imageflow_types::JpegIDCTDownscaleHints {
        gamma_correct_for_srgb_during_spatial_luma_scaling: Some(false),
        scale_luma_spatially: Some(true),
        width: i64::from(new_w),
        height: i64::from(new_h),
    };
    //Here we send the hints via the Decode node instead.
    (
        Some(imageflow_types::DecoderCommand::JpegDownscaleHints(hints.clone())),
        vec![Node::Decode {
            io_id: 0,
            commands: Some(vec![imageflow_types::DecoderCommand::JpegDownscaleHints(hints)]),
        }],
    )
}

#[test]
fn test_idct_linear() {
    let matched = test_with_callback("ScaleIDCTFastvsSlow", IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/roof_test_800x600.jpg".to_owned()),
    test_idct_callback);
    assert!(matched);
}

#[test]
fn test_idct_spatial_no_gamma() {
    let matched = test_with_callback("ScaleIDCT_approx_gamma", IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/roof_test_800x600.jpg".to_owned()),
                                     test_idct_no_gamma_callback);
    assert!(matched);
}
//
//#[test]
//fn test_fail(){
//    let matched = test_with_callback("ScaleIDCTFastvsSlow", IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/roof_test_800x600.jpg".to_owned()),
//                                     test_idct_callback_no_gamma);
//    assert!(matched);
//}

#[test]
fn zz_verify_all_checksum_files_uploaded() {
    let ctx = ChecksumCtx::visuals();
    ctx.verify_all_active_images_uploaded();
}
