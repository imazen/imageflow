#[allow(unused_imports)]
use crate::common::*;
use imageflow_types::{
    Color, ColorSrgb, Node, PixelFormat, Filter, ResampleHints, RoundCornersMode,
    CommandStringKind,
};

#[test]
fn test_fill_rect() {
    visual_check_bitmap! {
        detail: "eeccff_hermite_400x400",
        steps: vec![
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
    }
}

#[test]
fn test_fill_rect_original() {
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    visual_check_bitmap! {
        detail: "blue_on_transparent",
        steps: vec![
            Node::CreateCanvas {
                w: 400,
                h: 300,
                format: PixelFormat::Bgra32,
                color: Color::Transparent,
            },
            Node::FillRect { x1: 0, y1: 0, x2: 50, y2: 100, color: blue },
        ],
    }
}

#[test]
fn test_expand_rect() {
    visual_check_bitmap! {
        detail: "fill_expand_hermite_linear",
        steps: vec![
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
    }
}

#[test]
fn test_crop() {
    visual_check_bitmap! {
        detail: "red_canvas_blue_strip",
        steps: vec![
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
    }
}

#[test]
fn test_off_surface_region() {
    visual_check_bitmap! {
        detail: "all_negative",
        steps: vec![
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
    }
}

#[test]
fn test_transparent_canvas() {
    visual_check_bitmap! {
        detail: "200x200",
        steps: vec![Node::CreateCanvas {
            w: 200,
            h: 200,
            format: PixelFormat::Bgra32,
            color: Color::Srgb(ColorSrgb::Hex("00000000".to_owned())),
        }],
    }
}

#[test]
fn test_partial_region() {
    visual_check_bitmap! {
        detail: "overlap_40pct",
        steps: vec![
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
    }
}

#[test]
fn test_pixels_region() {
    visual_check_bitmap! {
        detail: "pixel_coords",
        steps: vec![
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
    }
}

#[test]
fn test_detect_whitespace() {
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    visual_check_bitmap! {
        detail: "blue_on_transparent",
        steps: vec![
            Node::CreateCanvas {
                w: 400,
                h: 300,
                format: PixelFormat::Bgra32,
                color: Color::Transparent,
            },
            Node::FillRect { x1: 0, y1: 0, x2: 50, y2: 100, color: blue },
            Node::CropWhitespace { threshold: 80, percent_padding: 0f32 },
        ],
    }
}

#[test]
fn test_round_corners_large() {
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    visual_check_bitmap! {
        detail: "400x400_r200",
        steps: vec![
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
    }
}

#[test]
fn test_round_corners_small() {
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    visual_check_bitmap! {
        detail: "100x100_r5",
        steps: vec![
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
    }
}

#[test]
fn test_round_corners_custom_pixels() {
    let matte = Color::Srgb(ColorSrgb::Hex("000000BB".to_owned()));
    visual_check_bitmap! {
        detail: "semitransparent_mixed_radii",
        steps: vec![
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
    }
}

#[test]
fn test_round_corners_custom_percent() {
    let matte = Color::Srgb(ColorSrgb::Hex("000000DD".to_owned()));
    visual_check_bitmap! {
        detail: "semitransparent_mixed_radii",
        steps: vec![
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
    }
}

#[test]
fn test_round_corners_excessive_radius() {
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    visual_check_bitmap! {
        detail: "200x150_r100",
        steps: vec![
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
    }
}

#[test]
fn test_round_corners_circle_wide_canvas() {
    let matte = Color::Srgb(ColorSrgb::Hex("000000FF".to_owned()));
    visual_check_bitmap! {
        detail: "200x150_black",
        steps: vec![
            Node::CreateCanvas {
                w: 200,
                h: 150,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_owned())),
            },
            Node::RoundImageCorners { background_color: matte, radius: RoundCornersMode::Circle },
        ],
    }
}

#[test]
fn test_round_corners_circle_tall_canvas() {
    let matte = Color::Srgb(ColorSrgb::Hex("00000000".to_owned()));
    visual_check_bitmap! {
        detail: "150x200_transparent",
        steps: vec![
            Node::CreateCanvas {
                w: 150,
                h: 200,
                format: PixelFormat::Bgra32,
                color: Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_owned())),
            },
            Node::RoundImageCorners { background_color: matte, radius: RoundCornersMode::Circle },
        ],
    }
}

#[test]
fn test_round_image_corners_transparent() {
    visual_check_bitmap! {
        source: "test_inputs/waterhouse.jpg",
        detail: "waterhouse_400x300_r100",
        steps: vec![
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
    }
}

#[test]
fn test_round_corners_command_string() {
    visual_check_bitmap! {
        source: "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/Landscape_1.jpg",
        detail: "landscape_mixed_radii_png",
        steps: vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "w=70&h=70&s.roundcorners=100,20,70,30&format=png".to_string(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
    }
}
