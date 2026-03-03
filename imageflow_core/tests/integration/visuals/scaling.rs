#[allow(unused_imports)]
use crate::common::*;
use imageflow_types::{
    CommandStringKind, Filter, Node, ResampleHints,
};

#[test]
fn test_scale_image() {
    visual_check_bitmap! {
        source: "test_inputs/waterhouse.jpg",
        detail: "waterhouse_robidoux_400x300",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Resample2D {
                w: 400,
                h: 300,
                hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
            },
        ],
    }
}

#[test]
fn test_scale_rings() {
    visual_check_bitmap! {
        source: "test_inputs/rings2.png",
        detail: "hermite_400x400",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Resample2D {
                w: 400,
                h: 400,
                hints: Some(ResampleHints::new().with_bi_filter(Filter::Hermite)),
            },
        ],
    }
}

#[test]
fn test_read_gif_and_scale() {
    visual_check_bitmap! {
        source: "test_inputs/mountain_800.gif",
        detail: "mountain_robidoux_400x300",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Resample2D {
                w: 400,
                h: 300,
                hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
            },
        ],
    }
}

#[test]
fn test_read_gif_and_vertical_distort() {
    visual_check_bitmap! {
        source: "test_inputs/mountain_800.gif",
        detail: "mountain_box_800x100",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Resample2D {
                w: 800,
                h: 100,
                hints: Some(ResampleHints::new().with_bi_filter(Filter::Box)),
            },
        ],
    }
}

#[test]
#[ignore] // gif crate doesn't support files without Trailer: https://github.com/image-rs/image-gif/issues/138
fn test_read_gif_eof() {
    visual_check_bitmap! {
        source: "https://user-images.githubusercontent.com/657201/139770827-18e25c4e-eb0a-4058-ba48-ddc3849090ee.gif",
        detail: "buggy_animated-gif",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Resample2D {
                w: 400,
                h: 300,
                hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
            },
        ],
    }
}

#[test]
fn webp_lossless_alpha_decode_and_scale() {
    visual_check_bitmap! {
        source: "test_inputs/1_webp_ll.webp",
        detail: "100x100",
        steps: vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "width=100&height=100".to_owned(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
    }
}

#[test]
fn webp_lossy_alpha_decode_and_scale() {
    visual_check_bitmap! {
        source: "test_inputs/1_webp_a.webp",
        detail: "100x100",
        steps: vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "width=100&height=100".to_owned(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
    }
}

#[test]
fn webp_lossy_noalpha_decode_and_scale() {
    visual_check_bitmap! {
        source: "test_inputs/lossy_mountain.webp",
        detail: "mountain_100x100",
        steps: vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "width=100&height=100".to_owned(),
            decode: Some(0),
            encode: None,
            watermarks: None,
        }],
    }
}

#[test]
fn test_jpeg_icc2_color_profile() {
    visual_check_bitmap! {
        source: "test_inputs/MarsRGB_tagged.jpg",
        detail: "mars_robidoux_400x300",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Resample2D {
                w: 400,
                h: 300,
                hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
            },
        ],
    }
}

#[test]
fn test_jpeg_icc4_color_profile() {
    visual_check_bitmap! {
        source: "test_inputs/MarsRGB_v4_sYCC_8bit.jpg",
        detail: "mars_v4_robidoux_400x300",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Resample2D {
                w: 400,
                h: 300,
                hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)),
            },
        ],
    }
}
