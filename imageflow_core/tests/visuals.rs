#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate imageflow_core;
extern crate imageflow_types as s;
extern crate imageflow_helpers as hlp;
extern crate serde_json;
extern crate smallvec;

mod common;
use common::*;

use imageflow_core::{Context, ErrorKind, FlowError, CodeLocation};

const DEBUG_GRAPH: bool = false;
const POPULATE_CHECKSUMS: bool = true;



#[test]
fn test_encode_gradients() {
    let steps = vec![
        s::Node::Decode { io_id: 0, commands: None },
        s::Node::Encode {
            io_id: 1,
            preset: s::EncoderPreset::libpng32()
        }
    ];

    compare_encoded(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/gradients.png".to_owned())),
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
fn test_encode_frymire() {
    let steps = vec![
        s::Node::Decode { io_id: 0, commands: None },
        s::Node::Encode {
            io_id: 1,
            preset: s::EncoderPreset::Lodepng
        }
    ];

    compare_encoded_to_source(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/frymire.png".to_owned()),
                              DEBUG_GRAPH,
                              Constraints {
                                  max_file_size: Some(390_000),
                                  similarity: Similarity::AllowDssimMatch(0.0, 0.0),
                              },
                              steps
    );
}


#[test]
fn test_encode_pngquant() {
    let steps = vec![
        s::Node::Decode { io_id: 0, commands: None },
        s::Node::Encode {
            io_id: 1,
            preset: s::EncoderPreset::Pngquant {
                speed: None,
                quality: Some((0, 100)),
            }
        }
    ];

    compare_encoded_to_source(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/frymire.png".to_owned()),
                              DEBUG_GRAPH,
                              Constraints {
                                  max_file_size: Some(280_000),
                                  similarity: Similarity::AllowDssimMatch(0.005, 0.008),
                              },
                              steps
    );
}

#[test]
fn test_encode_pngquant_fallback() {
    let steps = vec![
        s::Node::Decode { io_id: 0, commands: None },
        s::Node::Encode {
            io_id: 1,
            preset: s::EncoderPreset::Pngquant {
                speed: None,
                quality: Some((99, 100)),
            }
        }
    ];

    compare_encoded_to_source(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/frymire.png".to_owned()),
                              DEBUG_GRAPH,
                              Constraints {
                                  max_file_size: None,
                                  similarity: Similarity::AllowDssimMatch(0.000, 0.001),
                              },
                              steps
    );
}


#[test]
fn test_encode_lodepng() {
    let steps = vec![
        s::Node::Decode { io_id: 0, commands: None },
        s::Node::Encode {
            io_id: 1,
            preset: s::EncoderPreset::Lodepng,
        }
    ];

    compare_encoded_to_source(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/frymire.png".to_owned()),
                              DEBUG_GRAPH,
                              Constraints {
                                  max_file_size: Some(390_000),
                                  similarity: Similarity::AllowDssimMatch(0., 0.),
                              },
                              steps
    );
}


#[test]
fn test_encode_mozjpeg() {
    let steps = vec![
        s::Node::Decode { io_id: 0, commands: None },
        s::Node::Encode {
            io_id: 1,
            preset: s::EncoderPreset::Mozjpeg {
                progressive: None,
                quality: Some(50),
            },
        },
    ];

    compare_encoded_to_source(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/frymire.png".to_owned()),
                              DEBUG_GRAPH,
                              Constraints {
                                  max_file_size: Some(205_000),
                                  similarity: Similarity::AllowDssimMatch(0.04, 0.06),
                              },
                              steps
    );
}


#[test]
fn test_fill_rect(){
    let matched = compare(None, 500,
                          "FillRectEECCFF", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
        s::Node::CreateCanvas {w: 200, h: 200, format: s::PixelFormat::Bgra32, color: s::Color::Transparent},
        s::Node::FillRect{x1:0, y1:0, x2:100, y2:100, color: s::Color::Srgb(s::ColorSrgb::Hex("EECCFFFF".to_owned()))},
        s::Node::Resample2D{ w: 400, h: 400, down_filter: Some(s::Filter::Hermite), up_filter: Some(s::Filter::Hermite), hints: None, scaling_colorspace: None }
        ]
    );
    assert!(matched);
}

#[test]
fn test_expand_rect(){
    let matched = compare(None, 500,
                          "FillRectEECCFFExpand2233AAFF", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
        s::Node::CreateCanvas {w: 200, h: 200, format: s::PixelFormat::Bgra32, color: s::Color::Transparent},
        s::Node::FillRect{x1:0, y1:0, x2:100, y2:100, color: s::Color::Srgb(s::ColorSrgb::Hex("EECCFFFF".to_owned()))},
        s::Node::ExpandCanvas{left: 10, top: 15, right: 20, bottom: 25, color: s::Color::Srgb(s::ColorSrgb::Hex("2233AAFF".to_owned()))},
        s::Node::Resample2D{ w: 400, h: 400, down_filter: Some(s::Filter::Hermite), up_filter: Some(s::Filter::Hermite), hints: None, scaling_colorspace: Some(s::ScalingFloatspace::Linear) }
        ]
    );
    assert!(matched);
}


#[test]
fn test_crop(){
    for _ in 1..100 {
        let matched = compare(None, 500,
                              "FillRectAndCrop", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            s::Node::CreateCanvas { w: 200, h: 200, format: s::PixelFormat::Bgra32, color: s::Color::Srgb(s::ColorSrgb::Hex("FF5555FF".to_owned())) },
            s::Node::FillRect { x1: 0, y1: 0, x2: 10, y2: 100, color: s::Color::Srgb(s::ColorSrgb::Hex("0000FFFF".to_owned())) },
            s::Node::Crop { x1: 0, y1: 50, x2: 100, y2: 100 }
            ]
        );
        assert!(matched);
    }
}



//  Replaces TEST_CASE("Test scale rings", "")
#[test]
fn test_scale_rings(){
    let matched = compare(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/rings2.png".to_owned())), 500,
        "RingsDownscaling", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
        s::Node::Decode {io_id: 0, commands: None},
        s::Node::Resample2D{ w: 400, h: 400, down_filter: Some(s::Filter::Hermite), up_filter: Some(s::Filter::Hermite), hints: None, scaling_colorspace: None }
        ]
    );
    assert!(matched);
}


#[test]
fn test_fill_rect_original(){
    //let white = s::Color::Srgb(s::ColorSrgb::Hex("FFFFFFFF".to_owned()));
    let blue = s::Color::Srgb(s::ColorSrgb::Hex("0000FFFF".to_owned()));
    let matched = compare(None, 1, "FillRect", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
        s::Node::CreateCanvas {w: 400, h: 300, format: s::PixelFormat::Bgra32, color: s::Color::Transparent},
        s::Node::FillRect{x1:0, y1:0, x2:50, y2:100, color: blue},
        ]
    );
    assert!(matched);
}

#[test]
fn test_scale_image() {
    let matched = compare(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned())), 500,
                          "ScaleTheHouse", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
        s::Node::Decode {io_id: 0, commands: None},
        s::Node::Resample2D{ w: 400, h: 300, down_filter: Some(s::Filter::Robidoux), up_filter: Some(s::Filter::Robidoux), hints: None, scaling_colorspace: None }
        ]
    );
    assert!(matched);
}



#[test]
fn test_white_balance_image() {
    let matched = compare(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/red-night.png".to_owned())), 500,
                          "WhiteBalanceNight", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            s::Node::Decode {io_id: 0, commands: None},
            s::Node::WhiteBalanceHistogramAreaThresholdSrgb { threshold: None}
        ]
    );
    assert!(matched);
}
#[test]
fn test_read_gif() {
    let matched = compare(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/mountain_800.gif".to_owned())), 500,
                          "mountain_gif_scaled400", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            s::Node::Decode {io_id: 0, commands: None},
            s::Node::Resample2D{ w: 400, h: 300, down_filter: Some(s::Filter::Robidoux), up_filter: Some(s::Filter::Robidoux), hints: None, scaling_colorspace: None }
        ]
    );
    assert!(matched);
}



#[test]
fn test_jpeg_icc2_color_profile() {
    let matched = compare(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_tagged.jpg".to_owned())), 500,
                          "MarsRGB_ICC_Scaled400300", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
s::Node::Decode {io_id: 0, commands: None},
s::Node::Resample2D{ w: 400, h: 300, down_filter: Some(s::Filter::Robidoux), up_filter: Some(s::Filter::Robidoux), hints: None, scaling_colorspace: None }
]
    );
    assert!(matched);
}

#[test]
fn test_jpeg_icc4_color_profile() {
    let matched = compare(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_v4_sYCC_8bit.jpg".to_owned())), 500,
                          "MarsRGB_ICCv4_Scaled400300", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
s::Node::Decode {io_id: 0, commands: None},
s::Node::Resample2D{ w: 400, h: 300, down_filter: Some(s::Filter::Robidoux), up_filter: Some(s::Filter::Robidoux), hints: None, scaling_colorspace: None }
]
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
            let matched = compare(Some(s::IoEnum::Url(url)), 500, &title, POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![s::Node::Decode {io_id: 0, commands: None}, s::Node::Constrain(s::Constraint::Within{w: Some(70), h: Some(70), hints: None})]);
            assert!(matched);
        }
    }

}


#[test]
fn test_jpeg_crop() {
    let matched = compare(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned())), 500,
                          "jpeg_crop", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            s::Node::CommandString{
                kind: s::CommandStringKind::ImageResizer4,
                value: "width=100&height=200&mode=crop".to_owned(),
                decode: Some(0),
                encode: None
            }
        ]
    );
    assert!(matched);
}

//
//#[test]
//fn test_gif_ir4(){
//        let matched = compare(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned())), 500,
//                              "Read", true, DEBUG_GRAPH, vec![
//                s::Node::CommandString{
//                    kind: s::CommandStringKind::ImageResizer4,
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
//        s::Node::CommandString{
//            kind: s::CommandStringKind::ImageResizer4,
//            value: "width=200&height=200&format=gif".to_owned(),
//            decode: Some(0),
//            encode: Some(1)
//        }
//    ];
//
//    smoke_test(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned())),
//               Some(s::IoEnum::OutputBuffer),
//               DEBUG_GRAPH,
//               steps,
//    );
//}

#[test]
fn smoke_test_gif_ir4(){

    let steps = vec![
        s::Node::CommandString{
            kind: s::CommandStringKind::ImageResizer4,
            value: "width=200&height=200&format=gif".to_owned(),
            decode: Some(0),
            encode: Some(1)
        }
    ];

    smoke_test(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned())),
               Some(s::IoEnum::OutputBuffer),
               DEBUG_GRAPH,
               steps,
    );
}

#[test]
fn smoke_test_png_ir4(){

    let steps = vec![
        s::Node::CommandString{
            kind: s::CommandStringKind::ImageResizer4,
            value: "width=200&height=200&format=png".to_owned(),
            decode: Some(0),
            encode: Some(1)
        }
    ];

    smoke_test(Some(s::IoEnum::Url("https://user-images.githubusercontent.com/2650124/31182064-e1c54784-a8f0-11e7-8bb3-833bba872975.png".to_owned())),
               Some(s::IoEnum::OutputBuffer),
               DEBUG_GRAPH,
               steps,
    );
}




#[test]
fn test_encode_jpeg_smoke() {
    let steps = vec![
    s::Node::Decode {io_id: 0, commands: None},
    s::Node::Resample2D{ w: 400, h: 300, down_filter: Some(s::Filter::Robidoux), up_filter: Some(s::Filter::Robidoux), hints: None, scaling_colorspace: None },
    s::Node::Encode{ io_id: 1, preset: s::EncoderPreset::LibjpegTurbo {quality: Some(100), progressive: None, optimize_huffman_coding: None}}
    ];

    smoke_test(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_v4_sYCC_8bit.jpg".to_owned())),
               Some(s::IoEnum::OutputBuffer),
               DEBUG_GRAPH,
               steps,
    );
}

#[test]
fn test_encode_gif_smoke() {
    let steps = vec![
        s::Node::Decode {io_id: 0, commands: None},
        s::Node::Resample2D{ w: 400, h: 300, down_filter: Some(s::Filter::Robidoux), up_filter: Some(s::Filter::Robidoux), hints: None, scaling_colorspace: None },
        s::Node::Encode{ io_id: 1, preset: s::EncoderPreset::Gif}
    ];

    smoke_test(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_v4_sYCC_8bit.jpg".to_owned())),
               Some(s::IoEnum::OutputBuffer),
               DEBUG_GRAPH,
               steps,
    );
}

#[test]
fn test_encode_png32_smoke() {
    let steps = vec![
    s::Node::Decode {io_id: 0, commands: None},
    s::Node::Resample2D{ w: 400, h: 300, down_filter: Some(s::Filter::Robidoux), up_filter: Some(s::Filter::Robidoux), hints: None, scaling_colorspace: None },
    s::Node::FlipV,
    s::Node::Crop{ x1: 20, y1: 20, x2: 380, y2: 280},
    s::Node::Encode{ io_id: 1, preset: s::EncoderPreset::Libpng {depth: Some(s::PngBitDepth::Png32), matte: None,  zlib_compression: None}}
    ];

    smoke_test(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_v4_sYCC_8bit.jpg".to_owned())),
               Some(s::IoEnum::OutputBuffer),
               DEBUG_GRAPH,
               steps,
    );
}


#[test]
fn test_dimensions(){
    let steps = vec![
    s::Node::CreateCanvas{w: 638, h: 423, format: s::PixelFormat::Bgra32, color: s::Color::Black},
    //s::Node::Crop { x1: 0, y1: 0, x2: 638, y2: 423},
    s::Node::Resample2D{w:200,h:133, down_filter: None, up_filter: None, hints: None, scaling_colorspace: None},
    s::Node::ExpandCanvas{left:1, top: 0, right:0, bottom: 0, color: s::Color::Transparent},
    ];
    let (w, h) = get_result_dimensions(&steps, vec![], DEBUG_GRAPH);
    assert_eq!(w,201);
    assert_eq!(h,133);

}




#[test]
fn test_decode_png_and_scale_dimensions(){

    let tinypng = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00,
    0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00,
    0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
        0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82 ];

    let png = s::IoObject{
        io_id: 0,
        direction: s::IoDirection::In,

        io: s::IoEnum::ByteArray(tinypng)
    };
    let steps = vec![
    s::Node::Decode{io_id: 0, commands: None},
    //s::Node::Crop { x1: 0, y1: 0, x2: 638, y2: 423},
    s::Node::Resample2D{w:300,h:200,  down_filter: None, up_filter: None, hints: None, scaling_colorspace: None},
    ];
    let (w, h) = get_result_dimensions(&steps, vec![png], false);
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

//#[test]
//fn test_detect_whitespace(){
//    //let white = s::Color::Srgb(s::ColorSrgb::Hex("FFFFFFFF".to_owned()));
//    let blue = s::Color::Srgb(s::ColorSrgb::Hex("0000FFFF".to_owned()));
//    let matched = compare(None, 1,
//                          "DetectWhitespace", POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
//            s::Node::CreateCanvas {w: 400, h: 300, format: s::PixelFormat::Bgra32, color: s::Color::Transparent},
//            s::Node::FillRect{x1:0, y1:0, x2:50, y2:100, color: blue},
//            s::Node::CropWhitespace {threshold: 80, percent_padding: 0f32}
//        ]
//    );
//    assert!(matched);
//}


//#[test]
//fn test_get_info_png_invalid() {
//    let tinypng = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00,
//                       0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00,
//                       0x00, 0x00, 0x0A, 0x49 ];
//
//    let _ = imageflow_core::clients::stateless::LibClient {}.get_image_info(&tinypng).err().expect("Should fail");
//}


fn test_idct_callback(_: &s::ImageInfo) -> (Option<s::DecoderCommand>, Vec<s::Node>)
{
    let new_w = (800 * 4 + 8 - 1) / 8;
    let new_h = (600 * 4 + 8 - 1) / 8;
    let hints = s::JpegIDCTDownscaleHints{
        gamma_correct_for_srgb_during_spatial_luma_scaling: Some(true),
        scale_luma_spatially: Some(true),
        width: new_w,
        height: new_h
    };
    (Some(s::DecoderCommand::JpegDownscaleHints(hints)), vec![s::Node::Decode{io_id:0, commands: None}])
}

fn test_idct_no_gamma_callback(info: &s::ImageInfo) -> (Option<s::DecoderCommand>, Vec<s::Node>)
{
    let new_w = (info.image_width * 6 + 8 - 1) / 8;
    let new_h = (info.image_height * 6 + 8 - 1) / 8;
    let hints = s::JpegIDCTDownscaleHints{
        gamma_correct_for_srgb_during_spatial_luma_scaling: Some(false),
        scale_luma_spatially: Some(true),
        width: i64::from(new_w),
        height: i64::from(new_h)
    };
    //Here we send the hints via the Decode node instead.
    (Some(s::DecoderCommand::JpegDownscaleHints(hints.clone())),
     vec![s::Node::Decode{io_id:0, commands: Some(vec![s::DecoderCommand::JpegDownscaleHints(hints)])}])

}

#[test]
fn test_idct_linear(){
    let matched = test_with_callback("ScaleIDCTFastvsSlow", s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/roof_test_800x600.jpg".to_owned()),
    test_idct_callback);
    assert!(matched);
}

#[test]
fn test_idct_spatial_no_gamma(){
    let matched = test_with_callback("ScaleIDCT_approx_gamma", s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/roof_test_800x600.jpg".to_owned()),
                                     test_idct_no_gamma_callback);
    assert!(matched);
}
//
//#[test]
//fn test_fail(){
//    let matched = test_with_callback("ScaleIDCTFastvsSlow", s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/roof_test_800x600.jpg".to_owned()),
//                                     test_idct_callback_no_gamma);
//    assert!(matched);
//}

fn test_with_callback(checksum_name: &str, input: s::IoEnum, callback: fn(&s::ImageInfo) -> (Option<s::DecoderCommand>, Vec<s::Node>) ) -> bool{
    let mut context = Context::create().unwrap();
    let matched:bool;

    unsafe {
        ::imageflow_core::parsing::IoTranslator{}.add_all(&mut context, vec![s::IoObject{ io_id:0, direction: s::IoDirection::In, io: input}]).unwrap();


        let image_info = context.get_image_info(0).unwrap();

        let (tell_decoder, mut steps): (Option<s::DecoderCommand>, Vec<s::Node>) = callback(&image_info);

        if let Some(what) = tell_decoder {
            let send_hints = s::TellDecoder001 {
                io_id: 0,
                command: what
            };
            let send_hints_str = serde_json::to_string_pretty(&send_hints).unwrap();
            context.message("v0.1/tell_decoder", send_hints_str.as_bytes()).1.unwrap();
        }


        let mut bit = BitmapBgraContainer::empty();
        steps.push(bit.get_node());

        let send_execute = s::Execute001{
            framewise: s::Framewise::Steps(steps),
            graph_recording: None
        };
        context.execute_1(send_execute).unwrap();

        let ctx = ChecksumCtx::visuals(&context);
        matched = bitmap_regression_check(&ctx, bit.bitmap(&context).unwrap(), checksum_name, 500)
    }
    context.destroy().unwrap();
    matched
}

