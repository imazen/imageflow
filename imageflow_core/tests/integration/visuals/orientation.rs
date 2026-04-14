#[allow(unused_imports)]
use crate::common::*;
use imageflow_types::{CommandStringKind, Constraint, ConstraintMode, Node};

#[test]
fn test_jpeg_rotation() {
    let orientations = vec!["Landscape", "Portrait"];

    for orientation in orientations {
        for flag in 1..9 {
            let url = format!(
                "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/{}_{}.jpg",
                orientation, flag
            );
            visual_check_bitmap! {
                source: &url,
                detail: &format!("{orientation}_{flag}"),
                steps: vec![
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
                tolerance: Tolerance::off_by_one(),
            }
        }
    }
}

#[test]
fn test_jpeg_rotation_cropped() {
    for flag in 1..9 {
        let url = format!(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/Portrait_{}.jpg",
            flag
        );
        visual_check_bitmap! {
            source: &url,
            detail: &format!("portrait_{flag}"),
            steps: vec![Node::CommandString {
                kind: CommandStringKind::ImageResizer4,
                value: "crop=134,155,279,439".to_owned(),
                decode: Some(0),
                encode: None,
                watermarks: None,
            }],
            tolerance: Similarity::MaxZdsim(0.02).to_tolerance_spec(),
        }
    }
}

#[test]
fn test_crop_exif() {
    for ix in 1..9 {
        let url = format!(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/Landscape_{ix}.jpg"
        );
        visual_check_bitmap! {
            source: &url,
            detail: &format!("landscape_{ix}"),
            steps: vec![
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
            tolerance: Tolerance::off_by_one(),
        }
    }
}

#[test]
fn test_fit_pad_exif() {
    for ix in 1..9 {
        let url = format!(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/Landscape_{ix}.jpg"
        );
        visual_check_bitmap! {
            source: &url,
            detail: &format!("landscape_{ix}"),
            steps: vec![
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
            tolerance: Tolerance::off_by_one(),
        }
    }
}
