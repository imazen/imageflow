#[allow(unused_imports)]
use crate::common::*;
use imageflow_types::{
    CommandStringKind, ConstraintMode, Node, ResampleHints,
};

#[test]
fn test_watermark_image() {
    visual_check_bitmap! {
        sources: [
            "test_inputs/waterhouse.jpg",
            "test_inputs/dice.png",
        ],
        detail: "dice_fitcrop_90pct",
        steps: vec![
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
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn test_watermark_image_command_string() {
    visual_check_bitmap! {
        sources: [
            "test_inputs/waterhouse.jpg",
            "test_inputs/dice.png",
        ],
        detail: "dice_fitcrop_90pct",
        steps: vec![Node::CommandString {
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
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn test_watermark_image_command_string_with_bgcolor() {
    visual_check_bitmap! {
        sources: [
            "test_inputs/waterhouse.jpg",
            "test_inputs/dice.png",
        ],
        detail: "dice_aaeeff",
        steps: vec![Node::CommandString {
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
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn test_watermark_image_small() {
    visual_check_bitmap! {
        sources: [
            "test_inputs/waterhouse.jpg",
            "test_inputs/1_webp_a.sm.png",
        ],
        detail: "webp_within_90pct",
        steps: vec![
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
                gravity: Some(imageflow_types::ConstraintGravity::Percentage { x: 100f32, y: 100f32 }),
                fit_box: Some(imageflow_types::WatermarkConstraintBox::ImagePercentage { x1: 0f32, y1: 0f32, x2: 90f32, y2: 90f32 }),
                fit_mode: Some(imageflow_types::WatermarkConstraintMode::Within),
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
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn test_watermark_image_pixel_margins() {
    visual_check_bitmap! {
        sources: [
            "test_inputs/waterhouse.jpg",
            "test_inputs/1_webp_a.sm.png",
        ],
        detail: "webp_700px_offset",
        steps: vec![
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
                gravity: Some(imageflow_types::ConstraintGravity::Percentage { x: 100f32, y: 100f32 }),
                fit_box: Some(imageflow_types::WatermarkConstraintBox::ImageMargins { left: 700, top: 700, right: 0, bottom: 0 }),
                fit_mode: Some(imageflow_types::WatermarkConstraintMode::Within),
                min_canvas_width: None,
                min_canvas_height: None,
                opacity: Some(0.9f32),
                hints: None,
            }),
        ],
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn test_watermark_image_on_png() {
    visual_check_bitmap! {
        sources: [
            "test_inputs/shirt_transparent.png",
            "test_inputs/1_webp_a.sm.png",
        ],
        detail: "shirt_with_webp",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Watermark(imageflow_types::Watermark {
                io_id: 1,
                gravity: Some(imageflow_types::ConstraintGravity::Percentage { x: 100f32, y: 100f32 }),
                fit_box: Some(imageflow_types::WatermarkConstraintBox::ImagePercentage { x1: 0f32, y1: 0f32, x2: 90f32, y2: 90f32 }),
                fit_mode: Some(imageflow_types::WatermarkConstraintMode::Within),
                min_canvas_width: None,
                min_canvas_height: None,
                opacity: None,
                hints: Some(imageflow_types::ResampleHints {
                    sharpen_percent: None,
                    down_filter: None,
                    up_filter: None,
                    scaling_colorspace: None,
                    background_color: None,
                    resample_when: None,
                    sharpen_when: None,
                }),
            }),
        ],
        tolerance: Tolerance::off_by_one(),
    }
}

#[test]
fn test_watermark_jpeg_over_pnga() {
    visual_check_bitmap! {
        sources: [
            "test_inputs/shirt_transparent.png",
            "test_inputs/gamma_test.jpg",
        ],
        detail: "gamma_test_30pct",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Watermark(imageflow_types::Watermark {
                io_id: 1,
                gravity: Some(imageflow_types::ConstraintGravity::Percentage { x: 100f32, y: 100f32 }),
                fit_box: Some(imageflow_types::WatermarkConstraintBox::ImagePercentage { x1: 0f32, y1: 0f32, x2: 90f32, y2: 90f32 }),
                fit_mode: Some(imageflow_types::WatermarkConstraintMode::Within),
                min_canvas_width: None,
                min_canvas_height: None,
                opacity: Some(0.3f32),
                hints: Some(imageflow_types::ResampleHints {
                    sharpen_percent: None,
                    down_filter: None,
                    up_filter: None,
                    scaling_colorspace: None,
                    background_color: None,
                    resample_when: None,
                    sharpen_when: None,
                }),
            }),
        ],
        tolerance: Tolerance::off_by_one(),
    }
}
