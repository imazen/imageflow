#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate imageflow_core;
extern crate imageflow_helpers as hlp;
extern crate serde_json;
extern crate smallvec;

pub mod common;
use crate::common::*;

use imageflow_types;
use imageflow_core::{Context, ErrorKind, FlowError, CodeLocation};
use imageflow_core::ffi::BitmapBgra;
use imageflow_types::{PixelFormat, Color, Node, ColorSrgb,
                      EncoderPreset, ResampleHints, Filter, CommandStringKind,
                        ConstraintMode, Constraint, PngBitDepth};


const DEBUG_GRAPH: bool = false;
const POPULATE_CHECKSUMS: bool = true;



#[test]
fn test_encode_gradients() {
    let steps = vec![
        Node::Decode { io_id: 0, commands: None },
        Node::Encode {
            io_id: 1,
            preset: EncoderPreset::libpng32()
        }
    ];

    compare_encoded(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/gradients.png".to_owned())),
                    "encode_gradients",
                    POPULATE_CHECKSUMS,
                    DEBUG_GRAPH,
                    Constraints {
                        max_file_size: Some(100000),
                        similarity: Similarity::AllowOffByOneBytesRatio(0.01)
                    },
                    steps
    );
}



#[test]
fn test_fill_rect(){
    let matched = compare(None, 500,
                          "FillRectEECCFF", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
        Node::CreateCanvas {w: 200, h: 200, format: PixelFormat::Bgra32, color: Color::Transparent},
        Node::FillRect{x1:0, y1:0, x2:100, y2:100, color: Color::Srgb(ColorSrgb::Hex("EECCFFFF".to_owned()))},
        Node::Resample2D{ w: 400, h: 400, hints: Some(ResampleHints::new().with_bi_filter(Filter::Hermite)) }
        ]
    );
    assert!(matched);
}

#[test]
fn test_expand_rect(){
    let matched = compare(None, 500,
                          "FillRectEECCFFExpand2233AAFF", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
        Node::CreateCanvas {w: 200, h: 200, format: PixelFormat::Bgra32, color: Color::Transparent},
        Node::FillRect{x1:0, y1:0, x2:100, y2:100, color: Color::Srgb(ColorSrgb::Hex("EECCFFFF".to_owned()))},
        Node::ExpandCanvas{left: 10, top: 15, right: 20, bottom: 25, color: Color::Srgb(ColorSrgb::Hex("2233AAFF".to_owned()))},
        Node::Resample2D{ w: 400, h: 400,
            hints: Some(ResampleHints::new().with_bi_filter(Filter::Hermite).with_floatspace(imageflow_types::ScalingFloatspace::Linear))
             }
        ]
    );
    assert!(matched);
}

#[test]
fn test_crop(){
    for _ in 1..100 { //WTF are we looping 100 times for?
        let matched = compare(None, 500,
                              "FillRectAndCrop", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            Node::CreateCanvas { w: 200, h: 200, format: PixelFormat::Bgra32, color: Color::Srgb(ColorSrgb::Hex("FF5555FF".to_owned())) },
            Node::FillRect { x1: 0, y1: 0, x2: 10, y2: 100, color: Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned())) },
            Node::Crop { x1: 0, y1: 50, x2: 100, y2: 100 }
            ]
        );
        assert!(matched);
    }
}

#[test]
fn test_off_surface_region(){

        let matched = compare(None, 500,
                              "TestOffSurfaceRegion", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
                Node::CreateCanvas { w: 200, h: 200, format: PixelFormat::Bgra32, color: Color::Srgb(ColorSrgb::Hex("FF5555FF".to_owned())) },
                Node::FillRect { x1: 0, y1: 0, x2: 10, y2: 100, color: Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned())) },
                Node::RegionPercent { x1: -100f32, y1: -100f32, x2: -1f32, y2: -1f32, background_color: Color::Transparent}
            ]
        );
        assert!(matched);

}
#[test]
fn test_partial_region(){

    let matched = compare(None, 500,
                          "TestPartialRegion", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            Node::CreateCanvas { w: 200, h: 200, format: PixelFormat::Bgra32, color: Color::Srgb(ColorSrgb::Hex("FF5555FF".to_owned())) },
            Node::FillRect { x1: 0, y1: 0, x2: 10, y2: 100, color: Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned())) },
            Node::RegionPercent { x1: -10f32, y1: -10f32, x2: 40f32, y2: 40f32, background_color: Color::Transparent}
        ]
    );
    assert!(matched);

}
#[test]
fn test_pixels_region(){

    let matched = compare(None, 500,
                          "TestPixelsRegion", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            Node::CreateCanvas { w: 200, h: 200, format: PixelFormat::Bgra32, color: Color::Srgb(ColorSrgb::Hex("FF5555FF".to_owned())) },
            Node::FillRect { x1: 0, y1: 0, x2: 10, y2: 100, color: Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned())) },
            Node::Region { x1: -10, y1: -10, x2: 120, y2: 50, background_color: Color::Transparent}
        ]
    );
    assert!(matched);

}


//  Replaces TEST_CASE("Test scale rings", "")
#[test]
fn test_scale_rings(){
    let matched = compare(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/rings2.png".to_owned())), 500,
        "RingsDownscaling", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
        Node::Decode {io_id: 0, commands: None},
        Node::Resample2D{ w: 400, h: 400,  hints: Some(ResampleHints::new().with_bi_filter(Filter::Hermite)) }
        ]
    );
    assert!(matched);
}


#[test]
fn test_fill_rect_original(){
    //let white = Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_owned()));
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    let matched = compare(None, 1, "FillRect", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
        Node::CreateCanvas {w: 400, h: 300, format: PixelFormat::Bgra32, color: Color::Transparent},
        Node::FillRect{x1:0, y1:0, x2:50, y2:100, color: blue},
        ]
    );
    assert!(matched);
}

#[test]
fn test_scale_image() {
    let matched = compare(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned())), 500,
                          "ScaleTheHouse", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
        Node::Decode {io_id: 0, commands: None},
        Node::Resample2D{ w: 400, h: 300,  hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)) }
        ]
    );
    assert!(matched);
}

#[test]
fn test_watermark_image() {
    let matched = compare_multiple(Some(vec![
        IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned()),
        IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/dice.png".to_owned())
    ]), 500,
                          "Watermark1", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
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
                fit_box: Some(imageflow_types::WatermarkConstraintBox::ImagePercentage {x1: 30f32, y1: 50f32, x2: 90f32, y2: 90f32}),
                fit_mode: Some(imageflow_types::WatermarkConstraintMode::FitCrop),
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
fn test_watermark_image_command_string() {
    let matched = compare_multiple(Some(vec![
        IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned()),
        IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/dice.png".to_owned())
    ]), 500,
                                   "Watermark1", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            Node::CommandString{
                kind: CommandStringKind::ImageResizer4,
                value: "width=800&height=800&mode=max".to_string(),
                decode: Some(0),
                encode: None,
                watermarks: Some(vec![imageflow_types::Watermark{
                    io_id: 1,
                    fit_box: Some(imageflow_types::WatermarkConstraintBox::ImagePercentage {x1: 30f32, y1: 50f32, x2: 90f32, y2: 90f32}),
                    fit_mode: Some(imageflow_types::WatermarkConstraintMode::FitCrop),
                    gravity: Some(imageflow_types::ConstraintGravity::Percentage {x: 100f32, y: 100f32}),
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

                }
                ])
            }
        ]
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
    let matched = compare(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/red-night.png".to_owned())), 500,
                          "WhiteBalanceNight", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            Node::Decode {io_id: 0, commands: None},
            Node::WhiteBalanceHistogramAreaThresholdSrgb { threshold: None}
        ]
    );
    assert!(matched);
}
#[test]
fn test_read_gif() {
    let matched = compare(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/mountain_800.gif".to_owned())), 500,
                          "mountain_gif_scaled400", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            Node::Decode {io_id: 0, commands: None},
            Node::Resample2D{ w: 400, h: 300,  hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)) }
        ]
    );
    assert!(matched);
}



#[test]
fn test_jpeg_icc2_color_profile() {
    let matched = compare(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_tagged.jpg".to_owned())), 500,
                          "MarsRGB_ICC_Scaled400300", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
Node::Decode {io_id: 0, commands: None},
Node::Resample2D{ w: 400, h: 300,  hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)) }
]
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
    let matched = compare(Some(IoTestEnum::Url(url)), 500, &title, POPULATE_CHECKSUMS, DEBUG_GRAPH,
                          vec![Node::Decode { io_id: 0, commands: None },
                               Node::Constrain(Constraint { mode: ConstraintMode::Within, w: Some(70), h: Some(70), hints: None, gravity: None, canvas_color: None })]);
    assert!(matched);
}



#[test]
fn test_jpeg_rotation() {
    let orientations = vec!["Landscape", "Portrait"];

    for orientation in orientations {
        for flag in 1..9 {
            let url = format!("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation/{}_{}.jpg", orientation, flag);
            let title = format!("Test_Apply_Orientation_{}_{}.jpg", orientation, flag);
            let matched = compare(Some(IoTestEnum::Url(url)), 500, &title, POPULATE_CHECKSUMS, DEBUG_GRAPH,
                                  vec![Node::Decode {io_id: 0, commands: None},
                                       Node::Constrain(Constraint{mode: ConstraintMode::Within, w: Some(70), h: Some(70), hints: None, gravity: None, canvas_color: None })]);
            assert!(matched);
        }
    }

}


#[test]
fn test_jpeg_crop() {
    let matched = compare(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned())), 500,
                          "jpeg_crop", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            Node::CommandString{
                kind: CommandStringKind::ImageResizer4,
                value: "width=100&height=200&mode=crop".to_owned(),
                decode: Some(0),
                encode: None,
                watermarks: None
            }
        ]
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
    let steps = vec![
        Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: "width=200&height=200&format=gif".to_owned(),
            decode: Some(0),
            encode: Some(1),
            watermarks: None
        }
    ];

    let result = smoke_test(Some(IoTestEnum::Url("https://upload.wikimedia.org/wikipedia/commons/0/0e/Youngstown_State_Athletics.jpg".to_owned())),
                            Some(IoTestEnum::OutputBuffer),
                            None,
                            DEBUG_GRAPH,
                            steps,
    );
    let err = result.expect_err("CMYK jpeg decodes should fail");
    assert_eq!(err.category(), crate::imageflow_core::ErrorCategory::ImageMalformed);
    assert_eq!(err.message,"JpegDecodingError: CMYK JPEG support not implemented");

}


#[test]
fn webp_lossless_alpha_decode_and_scale() {
    let matched = compare(Some(IoTestEnum::Url("https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/1_webp_ll.webp".to_owned())), 500,
                          "webp_lossless_alpha_decode_and_scale", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
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
fn webp_lossy_alpha_decode_and_scale() {
    let matched = compare(Some(IoTestEnum::Url("https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/1_webp_a.webp".to_owned())), 500,
                          "webp_lossy_alpha_decode_and_scale", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
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
fn webp_lossless_alpha_roundtrip(){

    let steps = vec![
        Node::CommandString{
            kind: CommandStringKind::ImageResizer4,
            value: "format=webp".to_owned(),
            decode: Some(0),
            encode: Some(1),
            watermarks: None
        }
    ];

    smoke_test(Some(IoTestEnum::Url("https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/1_webp_ll.webp".to_owned())),
               Some(IoTestEnum::OutputBuffer),
               None,
               DEBUG_GRAPH,
               steps,
    ).unwrap();
}
#[test]
fn webp_lossy_alpha_roundtrip(){

    let steps = vec![
        Node::CommandString{
            kind: CommandStringKind::ImageResizer4,
            value: "format=webp&quality=90".to_owned(),
            decode: Some(0),
            encode: Some(1),
            watermarks: None
        }
    ];

    smoke_test(Some(IoTestEnum::Url("https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/1_webp_a.webp".to_owned())),
               Some(IoTestEnum::OutputBuffer),
               None,
               DEBUG_GRAPH,
               steps,
    ).unwrap();
}
#[test]
fn smoke_test_gif_ir4(){

    let steps = vec![
        Node::CommandString{
            kind: CommandStringKind::ImageResizer4,
            value: "width=200&height=200&format=gif".to_owned(),
            decode: Some(0),
            encode: Some(1),
            watermarks: None
        }
    ];

    smoke_test(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned())),
               Some(IoTestEnum::OutputBuffer),
               None,
               DEBUG_GRAPH,
               steps,
    ).unwrap();
}

#[test]
fn smoke_test_ignore_invalid_color_profile(){

    let steps = vec![
        Node::CommandString{
            kind: CommandStringKind::ImageResizer4,
            value: "width=200&height=200&ignore_icc_errors=true".to_owned(),
            decode: Some(0),
            encode: Some(1),
            watermarks: None
        }
    ];

    smoke_test(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/color_profile_error.jpg".to_owned())),
               Some(IoTestEnum::OutputBuffer),
               None,
               DEBUG_GRAPH,
               steps,
    ).unwrap();
}

#[test]
fn test_max_encode_dimensions(){

    let tinypng = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00,
                       0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00,
                       0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
                       0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82 ];

    let steps = vec![
        Node::CommandString{
            kind: CommandStringKind::ImageResizer4,
            value: "width=2&height=2&mode=pad&scale=both".to_owned(),
            decode: Some(0),
            encode: Some(1),
            watermarks: None
        }
    ];

    let e = smoke_test(Some(IoTestEnum::ByteArray(tinypng)),
               Some(IoTestEnum::OutputBuffer),
               Some(imageflow_types::ExecutionSecurity{
                   max_decode_size: None,
                   max_frame_size: None,
                   max_encode_size: Some(imageflow_types::FrameSizeLimit{
                       w: 3,
                       h: 1,
                       megapixels: 100.0
                   })
               }),
               DEBUG_GRAPH,
               steps,
    ).expect_err("Should fail");

    assert_eq!(e.kind, ErrorKind::SizeLimitExceeded);

    assert_eq!(e.message, "SizeLimitExceeded: Frame height 2 exceeds max_encode_size.h 1");

}

#[test]
fn test_max_decode_dimensions(){

    let steps = vec![
        Node::Decode {io_id: 0, commands: None},
    ];

    let e = smoke_test(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned())),
               None,
               Some(imageflow_types::ExecutionSecurity{
                   max_decode_size: Some(imageflow_types::FrameSizeLimit{
                       w: 10,
                       h: 100000,
                       megapixels: 100.0
                   }),
                   max_frame_size: None,
                   max_encode_size: None
               }),
               DEBUG_GRAPH,
               steps,
    ).expect_err("Should fail");
    assert_eq!(e.kind, ErrorKind::SizeLimitExceeded);

}

#[test]
fn test_max_frame_dimensions(){

    let steps = vec![
        Node::CreateCanvas {
            format: PixelFormat::Bgra32,
            w: 1000,
            h: 1000,
            color: Color::Transparent
        }
    ];

    let e = smoke_test(None,
               None,
               Some(imageflow_types::ExecutionSecurity{
                   max_frame_size: Some(imageflow_types::FrameSizeLimit{
                       w: 10000,
                       h: 10000,
                       megapixels: 0.5
                   }),
                   max_decode_size: None,
                   max_encode_size: None
               }),
               DEBUG_GRAPH,
               steps,
    ).expect_err("Should fail");

    assert_eq!(e.kind, ErrorKind::SizeLimitExceeded);

}

#[test]
fn smoke_test_png_ir4(){

    let steps = vec![
        Node::CommandString{
            kind: CommandStringKind::ImageResizer4,
            value: "width=200&height=200&format=png".to_owned(),
            decode: Some(0),
            encode: Some(1),
            watermarks: None
        }
    ];

    smoke_test(Some(IoTestEnum::Url("https://user-images.githubusercontent.com/2650124/31182064-e1c54784-a8f0-11e7-8bb3-833bba872975.png".to_owned())),
               Some(IoTestEnum::OutputBuffer),
               None,
               DEBUG_GRAPH,
               steps,
    ).unwrap();
}




#[test]
fn test_encode_jpeg_smoke() {
    let steps = vec![
        Node::Decode {io_id: 0, commands: None},
        Node::Resample2D{ w: 400, h: 300,  hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux)) },
        Node::Encode{ io_id: 1, preset: EncoderPreset::LibjpegTurbo {quality: Some(100), progressive: None, optimize_huffman_coding: None}}
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
        Node::Decode {io_id: 0, commands: None},
        Node::Resample2D{ w: 400, h: 300, hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux))},
        Node::Encode{ io_id: 1, preset: EncoderPreset::Gif}
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
        Node::Decode {io_id: 0, commands: None},
        Node::Resample2D{ w: 400, h: 300, hints: Some(ResampleHints::new().with_bi_filter(Filter::Robidoux))},
        Node::FlipV,
        Node::Crop{ x1: 20, y1: 20, x2: 380, y2: 280},
        Node::Encode{ io_id: 1, preset: EncoderPreset::Libpng {depth: Some(PngBitDepth::Png32), matte: None,  zlib_compression: None}}
    ];

    smoke_test(Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_v4_sYCC_8bit.jpg".to_owned())),
               Some(IoTestEnum::OutputBuffer),
               None,
               DEBUG_GRAPH,
               steps,
    ).unwrap();
}


#[test]
fn test_dimensions(){
    let steps = vec![
    Node::CreateCanvas{w: 638, h: 423, format: PixelFormat::Bgra32, color: Color::Black},
    //Node::Crop { x1: 0, y1: 0, x2: 638, y2: 423},
    Node::Resample2D{w:200,h:133, hints: None},
    Node::ExpandCanvas{left:1, top: 0, right:0, bottom: 0, color: Color::Transparent},
    ];
    let (w, h) = get_result_dimensions(&steps, vec![], DEBUG_GRAPH);
    assert_eq!(w,201);
    assert_eq!(h,133);

}

#[test]
fn test_aspect_crop_dimensions(){
    let steps = vec![
        Node::CreateCanvas{w: 638, h: 423, format: PixelFormat::Bgra32, color: Color::Black},
        Node::Constrain(imageflow_types::Constraint{ mode: imageflow_types::ConstraintMode::AspectCrop, w: Some(200),h: Some(133), hints: None, gravity: None, canvas_color: None })
    ];
    let (w, h) = get_result_dimensions(&steps, vec![], DEBUG_GRAPH);
    assert_eq!(w,636);
    assert_eq!(h,423);

}


#[test]
fn test_decode_png_and_scale_dimensions(){

    let tinypng = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00,
    0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00,
    0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
        0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82 ];


    let steps = vec![
    Node::Decode{io_id: 0, commands: None},
    //Node::Crop { x1: 0, y1: 0, x2: 638, y2: 423},
    Node::Resample2D{w:300,h:200,  hints: None},
    ];
    let (w, h) = get_result_dimensions(&steps, vec![ IoTestEnum::ByteArray(tinypng)], false);
    assert_eq!(w,300);
    assert_eq!(h,200);

}

#[test]
fn test_get_info_png() {
    let tinypng = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00,
                       0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00,
                       0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
                       0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82 ];

    let _ = imageflow_core::clients::stateless::LibClient {}.get_image_info(&tinypng).expect("Image response should be valid");
}

#[test]
fn test_detect_whitespace(){
    //let white = Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_owned()));
    let blue = Color::Srgb(ColorSrgb::Hex("0000FFFF".to_owned()));
    let matched = compare(None, 1,
                          "DetectWhitespace", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            Node::CreateCanvas {w: 400, h: 300, format: PixelFormat::Bgra32, color: Color::Transparent},
            Node::FillRect{x1:0, y1:0, x2:50, y2:100, color: blue},
            Node::CropWhitespace {threshold: 80, percent_padding: 0f32}
        ]
    );
    assert!(matched);
}

#[test]
fn test_detect_whitespace_all_small_images(){
    let ctx = Context::create_can_panic().unwrap();

    let red = Color::Srgb(ColorSrgb::Hex("FF0000FF".to_owned()));
    let mut failed_count = 0;
    let mut count = 0;
    for w in 3..12u32{
        for h in 3..12u32{
            let b = unsafe { &mut *BitmapBgra::create(&ctx, w, h, PixelFormat::Bgra32, Color::Black).unwrap() };

            for x in 0..w{
                for y in 0..h{
                    if x == 1 && y == 1 && w == 3 && h == 3 {
                        continue;
                        // This is a checkerboard, we don't support them
                    }

                    for size in 1..3 {
                        if x + size <= w && y + size <= h {
                            b.fill_rect(&ctx, 0, 0, w, h, &Color::Transparent).unwrap();
                            b.fill_rect(&ctx, x, y, x + size, y + size, &red).unwrap();
                            let r = ::imageflow_core::graphics::whitespace::detect_content(&b, 1).unwrap();
                            let correct = (r.x1 == x) && (r.y1 == y) && (r.x2 == x + size) && (r.y2 == y + size);
                            if !correct {
                                eprint!("Failed to correctly detect {}px dot at {},{} within {}x{}. Detected ", size, x, y, w, h);
                                if r.x1 != x { eprint!("x1={}({})", r.x1, x);}
                                if r.y1 != y { eprint!("y1={}({})", r.y1, y);}
                                if r.x2 != x + size { eprint!("Detected x2={}({})", r.x2, x + size);}
                                if r.y2 != y + size { eprint!("Detected y2={}({})", r.y2, y + size);}
                                eprintln!(".");
                                failed_count += 1;
                            }
                            count += 1;
                        }

                    }
                }
            }

            unsafe{ BitmapBgra::destroy(b, &ctx); }

        }
    }
    if failed_count > 0{
        panic!("Failed {} of {} whitespace detection tests", failed_count, count);
    }
}


#[test]
fn test_detect_whitespace_basic(){
    let ctx = Context::create_can_panic().unwrap();

    let red = Color::Srgb(ColorSrgb::Hex("FF0000FF".to_owned()));

    let b = unsafe { &mut *BitmapBgra::create(&ctx, 10, 10, PixelFormat::Bgra32, Color::Black).unwrap() };
    b.fill_rect(&ctx, 1, 1, 9, 9, &red).unwrap();
    let r = ::imageflow_core::graphics::whitespace::detect_content(&b, 1).unwrap();
    assert_eq!(r.x1,1);
    assert_eq!(r.y1,1);
    assert_eq!(r.x2,9);
    assert_eq!(r.y2,9);

    let b = unsafe { &mut *BitmapBgra::create(&ctx, 100, 100, PixelFormat::Bgra32, Color::Black).unwrap() };
    b.fill_rect(&ctx, 2, 3, 70, 70, &red).unwrap();
    let r = ::imageflow_core::graphics::whitespace::detect_content(&b, 1).unwrap();
    assert_eq!(r.x1,2);
    assert_eq!(r.y1,3);
    assert_eq!(r.x2,70);
    assert_eq!(r.y2,70);
}

//#[test]
//fn test_get_info_png_invalid() {
//    let tinypng = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00,
//                       0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00,
//                       0x00, 0x00, 0x0A, 0x49 ];
//
//    let _ = imageflow_core::clients::stateless::LibClient {}.get_image_info(&tinypng).err().expect("Should fail");
//}


fn test_idct_callback(_: &imageflow_types::ImageInfo) -> (Option<imageflow_types::DecoderCommand>, Vec<Node>)
{
    let new_w = (800 * 4 + 8 - 1) / 8;
    let new_h = (600 * 4 + 8 - 1) / 8;
    let hints = imageflow_types::JpegIDCTDownscaleHints{
        gamma_correct_for_srgb_during_spatial_luma_scaling: Some(true),
        scale_luma_spatially: Some(true),
        width: new_w,
        height: new_h
    };
    (Some(imageflow_types::DecoderCommand::JpegDownscaleHints(hints)), vec![Node::Decode{io_id:0, commands: None}])
}

fn test_idct_no_gamma_callback(info: &imageflow_types::ImageInfo) -> (Option<imageflow_types::DecoderCommand>, Vec<Node>)
{
    let new_w = (info.image_width * 6 + 8 - 1) / 8;
    let new_h = (info.image_height * 6 + 8 - 1) / 8;
    let hints = imageflow_types::JpegIDCTDownscaleHints{
        gamma_correct_for_srgb_during_spatial_luma_scaling: Some(false),
        scale_luma_spatially: Some(true),
        width: i64::from(new_w),
        height: i64::from(new_h)
    };
    //Here we send the hints via the Decode node instead.
    (Some(imageflow_types::DecoderCommand::JpegDownscaleHints(hints.clone())),
     vec![Node::Decode{io_id:0, commands: Some(vec![imageflow_types::DecoderCommand::JpegDownscaleHints(hints)])}])

}

#[test]
fn test_idct_linear(){
    let matched = test_with_callback("ScaleIDCTFastvsSlow", IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/roof_test_800x600.jpg".to_owned()),
    test_idct_callback);
    assert!(matched);
}

#[test]
fn test_idct_spatial_no_gamma(){
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


