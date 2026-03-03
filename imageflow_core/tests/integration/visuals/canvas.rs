use crate::common::*;
use imageflow_types::{
    Color, ColorSrgb, Node, PixelFormat, Filter, ResampleHints, RoundCornersMode,
    CommandStringKind,
};

const DEBUG_GRAPH: bool = false;
const POPULATE_CHECKSUMS: bool = true;

#[test]
fn test_fill_rect() {
    let matched = compare(
        None,
        500,
        "test_fill_rect eeccff_hermite_400x400",
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
fn test_fill_rect_original() {
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    let matched = compare(
        None,
        1,
        "test_fill_rect_original blue_on_transparent",
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
fn test_expand_rect() {
    let matched = compare(
        None,
        500,
        "test_expand_rect fill_expand_hermite_linear",
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
        "test_crop red_canvas_blue_strip",
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
fn test_off_surface_region() {
    let matched = compare(
        None,
        500,
        "test_off_surface_region all_negative",
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
fn test_transparent_canvas() {
    let matched = compare(
        None,
        500,
        "test_transparent_canvas 200x200",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        vec![Node::CreateCanvas {
            w: 200,
            h: 200,
            format: PixelFormat::Bgra32,
            color: Color::Srgb(ColorSrgb::Hex("00000000".to_owned())),
        }],
    );
    assert!(matched);
}

#[test]
fn test_partial_region() {
    let matched = compare(
        None,
        500,
        "test_partial_region overlap_40pct",
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
        "test_pixels_region pixel_coords",
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

#[test]
fn test_detect_whitespace() {
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    let matched = compare(
        None,
        1,
        "test_detect_whitespace blue_on_transparent",
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
fn test_round_corners_large() {
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    let matched = compare(
        None,
        1,
        "test_round_corners_large 400x400_r200",
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
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    let matched = compare(
        None,
        1,
        "test_round_corners_small 100x100_r5",
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
        "test_round_corners_custom_pixels semitransparent_mixed_radii",
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
        "test_round_corners_custom_percent semitransparent_mixed_radii",
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
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    let matched = compare(
        None,
        1,
        "test_round_corners_excessive_radius 200x150_r100",
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
    let matte = Color::Srgb(ColorSrgb::Hex("000000FF".to_owned()));
    let matched = compare(
        None,
        1,
        "test_round_corners_circle_wide_canvas 200x150_black",
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
    let matte = Color::Srgb(ColorSrgb::Hex("00000000".to_owned()));
    let matched = compare(
        None,
        1,
        "test_round_corners_circle_tall_canvas 150x200_transparent",
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
        "test_round_image_corners_transparent waterhouse_400x300_r100",
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
fn test_round_corners_command_string() {
    let url = "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/Landscape_1.jpg".to_owned();
    let matched = compare(
        Some(IoTestEnum::Url(url)),
        500,
        "test_round_corners_command_string landscape_mixed_radii_png",
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
