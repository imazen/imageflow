use crate::common::*;
use imageflow_types::Node;

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
    let matched = test_with_callback("test_idct_linear roof_gamma_corrected", IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/roof_test_800x600.jpg".to_owned()),
    test_idct_callback);
    assert!(matched);
}

#[test]
fn test_idct_spatial_no_gamma() {
    let matched = test_with_callback("test_idct_spatial_no_gamma roof_approx", IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/roof_test_800x600.jpg".to_owned()),
                                     test_idct_no_gamma_callback);
    assert!(matched);
}
