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

/// Run IDCT test inline: create context, add input, get image info,
/// apply decoder hints via callback, execute, then evaluate.
fn run_idct_test(
    identity: &TestIdentity,
    detail: &str,
    source_url: &str,
    callback: fn(
        &imageflow_types::ImageInfo,
    ) -> (Option<imageflow_types::DecoderCommand>, Vec<Node>),
) {
    test_init();
    let mut context = imageflow_core::Context::create().unwrap();
    IoTestTranslator {}.add(&mut context, 0, IoTestEnum::Url(source_url.to_owned())).unwrap();

    let image_info = context.get_unscaled_rotated_image_info(0).unwrap();
    let (tell_decoder, mut steps) = callback(&image_info);

    if let Some(what) = tell_decoder {
        let send_hints = imageflow_types::TellDecoder001 { io_id: 0, command: what };
        let send_hints_str = serde_json::to_string_pretty(&send_hints).unwrap();
        context.message("v1/tell_decoder", send_hints_str.as_bytes()).1.unwrap();
    }

    let capture_id = 0;
    steps.push(imageflow_types::Node::CaptureBitmapKey { capture_id });

    let send_execute = imageflow_types::Execute001 {
        framewise: imageflow_types::Framewise::Steps(steps),
        security: None,
        graph_recording: None,
        job_options: None,
    };
    context.execute_1(send_execute).unwrap();

    let bitmap_key = context.get_captured_bitmap_key(capture_id).unwrap();
    let tolerance = Tolerance::off_by_one();
    let matched = check_visual_bitmap(identity, detail, &context, bitmap_key, &tolerance);
    context.destroy().unwrap();
    assert!(matched);
}

#[test]
fn test_idct_linear() {
    let identity = test_identity!();
    let source_url =
        "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/roof_test_800x600.jpg";
    run_idct_test(&identity, "roof_gamma_corrected", source_url, test_idct_callback);
}

#[test]
fn test_idct_spatial_no_gamma() {
    let identity = test_identity!();
    let source_url =
        "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/roof_test_800x600.jpg";
    run_idct_test(&identity, "roof_approx", source_url, test_idct_no_gamma_callback);
}
