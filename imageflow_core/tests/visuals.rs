extern crate hyper;
extern crate imageflow_core;
extern crate libc;
extern crate rustc_serialize;
extern crate imageflow_types as s;
extern crate imageflow_helpers as hlp;
extern crate serde;
extern crate serde_json;

extern crate twox_hash;

use std::ffi::CString;
use std::path::Path;

use imageflow_core::{Context, JsonResponse};

fn default_build_config(debug: bool) -> s::Build001Config {
    s::Build001Config{graph_recording: match debug{ true => Some(s::Build001GraphRecording::debug_defaults()), false => None} ,
        process_all_gif_frames: Some(false),
        enable_jpeg_block_scaling: Some(false)
    }
}

const DEBUG_GRAPH: bool = false;
const POPULATE_CHECKSUMS: bool = false;


/// Creates a static, null-terminated Rust string, and
/// returns a ` *const libc::c_char` pointer to it.
///
/// Useful for API invocations that require a static C string

macro_rules! static_char {
    ($lit:expr) => {
        concat!($lit, "\0").as_ptr() as *const libc::c_char
    }
}

fn smoke_test(input: Option<s::IoEnum>, output: Option<s::IoEnum>,  debug: bool, steps: Vec<s::Node>){
    let mut io_list = Vec::new();
    if input.is_some() {
        io_list.push(s::IoObject {
            io_id: 0,
            direction: s::IoDirection::In,

            io: input.unwrap()
        });
    }
    if output.is_some() {
        io_list.push(s::IoObject {
            io_id: 1,
            direction: s::IoDirection::Out,

            io: output.unwrap()
        });
    }
    let build = s::Build001{
        builder_config: Some(default_build_config(debug)),
        io: io_list,
        framewise: s::Framewise::Steps(steps)
    };
    let mut context = Context::create().unwrap();
    context.message("v0.1/build", &serde_json::to_vec(&build).unwrap()).unwrap();
}

fn compare(input: Option<s::IoEnum>, allowed_off_by_one_bytes: usize, checksum_name: String, store_if_missing: bool, debug: bool, mut steps: Vec<s::Node>) -> bool {
    let mut dest_bitmap: *mut imageflow_core::ffi::BitmapBgra = std::ptr::null_mut();

    let ptr_to_ptr = &mut dest_bitmap as *mut *mut imageflow_core::ffi::BitmapBgra;

    let mut inputs = Vec::new();
    if input.is_some() {
        inputs.push(s::IoObject {
            io_id: 0,
            direction: s::IoDirection::In,

            io: input.unwrap()
        });
    }

    steps.push(s::Node::FlowBitmapBgraPtr { ptr_to_flow_bitmap_bgra_ptr: ptr_to_ptr as usize });

    {
        //println!("{}", serde_json::to_string_pretty(&steps).unwrap());
    }

    let build = s::Build001 {
        builder_config: Some(s::Build001Config {
            graph_recording: match debug {
                true => Some(s::Build001GraphRecording::debug_defaults()),
                false => None
            },
            process_all_gif_frames: Some(false),
            enable_jpeg_block_scaling: Some(false)
        }),
        io: inputs,
        framewise: s::Framewise::Steps(steps)
    };


    if debug {
        println!("{}", serde_json::to_string_pretty(&build).unwrap());
    }


    let mut context = Context::create().unwrap();

    context.message("v0.1/build", &serde_json::to_vec(&build).unwrap()).unwrap();

    unsafe {
        if debug {
            println!("{:?}", dest_bitmap);
        }

        let mut ctx = checkums_ctx_for(&context);
        ctx.create_if_missing = store_if_missing;
        ctx.max_off_by_one_ratio = allowed_off_by_one_bytes as f32 / ((*dest_bitmap).h * (*dest_bitmap).stride) as f32;
        regression_check(&ctx, dest_bitmap, &checksum_name)
    }
}
fn checkums_ctx_for<'a>(c: &'a Context) -> ChecksumCtx<'a>{
    let visuals = Path::new(env!("CARGO_MANIFEST_DIR")).join(Path::new("tests")).join(Path::new("visuals"));
    std::fs::create_dir_all(&visuals).unwrap();
    ChecksumCtx {
        c: c,
        visuals_dir: visuals.clone(),
        cache_dir: visuals.join(Path::new("cache")),
        create_if_missing: true,
        checksum_file: visuals.join(Path::new("checksums.json")),
        max_off_by_one_ratio: 0.01
    }
}

#[test]
fn test_fill_rect(){
    let matched = compare(None, 500,
                          "FillRectEECCFF".to_owned(), POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
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
                          "FillRectEECCFFExpand2233AAFF".to_owned(), POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
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
                              "FillRectAndCrop".to_owned(), POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
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
        "RingsDownscaling".to_owned(), POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
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
    let matched = compare(None, 1,
                          "FillRect".to_owned(), POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
        s::Node::CreateCanvas {w: 400, h: 300, format: s::PixelFormat::Bgra32, color: s::Color::Transparent},
        s::Node::FillRect{x1:0, y1:0, x2:50, y2:100, color: blue},
        ]
    );
    assert!(matched);
}

fn request_1d_twice_mode() -> s::ResampleHints {
    s::ResampleHints {
        sharpen_percent: None,
        prefer_1d_twice: Some(true)
    }
}

#[test]
fn test_scale_image() {
    let matched = compare(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned())), 500,
                          "ScaleTheHouse".to_owned(), POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
        s::Node::Decode {io_id: 0, commands: None},
        s::Node::Resample2D{ w: 400, h: 300, down_filter: Some(s::Filter::Robidoux), up_filter: Some(s::Filter::Robidoux), hints: Some(request_1d_twice_mode()), scaling_colorspace: None }
        ]
    );
    assert!(matched);
}



#[test]
fn test_white_balance_image() {
    let matched = compare(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/red-night.png".to_owned())), 500,
                          "WhiteBalanceNight".to_owned(), POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            s::Node::Decode {io_id: 0, commands: None},
            s::Node::WhiteBalanceHistogramAreaThresholdSrgb { low_threshold: None, high_threshold: None}
        ]
    );
    assert!(matched);
}
#[test]
fn test_read_gif() {
    let matched = compare(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/mountain_800.gif".to_owned())), 500,
                          "mountain_gif_scaled400".to_owned(), POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            s::Node::Decode {io_id: 0, commands: None},
            s::Node::Resample2D{ w: 400, h: 300, down_filter: Some(s::Filter::Robidoux), up_filter: Some(s::Filter::Robidoux), hints: Some(request_1d_twice_mode()), scaling_colorspace: None }
        ]
    );
    assert!(matched);
}



#[test]
fn test_jpeg_icc2_color_profile() {
    let matched = compare(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_tagged.jpg".to_owned())), 500,
                          "MarsRGB_ICC_Scaled400300".to_owned(), POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
s::Node::Decode {io_id: 0, commands: None},
s::Node::Resample2D{ w: 400, h: 300, down_filter: Some(s::Filter::Robidoux), up_filter: Some(s::Filter::Robidoux), hints: Some(request_1d_twice_mode()), scaling_colorspace: None }
]
    );
    assert!(matched);
}

#[test]
fn test_jpeg_icc4_color_profile() {
    let matched = compare(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_v4_sYCC_8bit.jpg".to_owned())), 500,
                          "MarsRGB_ICCv4_Scaled400300".to_owned(), POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
s::Node::Decode {io_id: 0, commands: None},
s::Node::Resample2D{ w: 400, h: 300, down_filter: Some(s::Filter::Robidoux), up_filter: Some(s::Filter::Robidoux), hints: Some(request_1d_twice_mode()), scaling_colorspace: None }
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
            let matched = compare(Some(s::IoEnum::Url(url)), 500, title, POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![s::Node::Decode {io_id: 0, commands: None}, s::Node::Constrain(s::Constraint::Within{w: Some(70), h: Some(70), hints: None})]);
            assert!(matched);
        }
    }

}


#[test]
fn test_jpeg_crop() {
    let matched = compare(Some(s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned())), 500,
                          "jpeg_crop".to_owned(), false, false, vec![
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
//                              "Read".to_owned(), true, DEBUG_GRAPH, vec![
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
fn test_encode_jpeg_smoke() {
    let steps = vec![
    s::Node::Decode {io_id: 0, commands: None},
    s::Node::Resample2D{ w: 400, h: 300, down_filter: Some(s::Filter::Robidoux), up_filter: Some(s::Filter::Robidoux), hints: None, scaling_colorspace: None },
    s::Node::Encode{ io_id: 1, preset: s::EncoderPreset::LibjpegTurbo {quality: Some(100)}}
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

fn get_result_dimensions(steps: Vec<s::Node>, io: Vec<s::IoObject>, debug: bool) -> (u32, u32) {
    let mut steps = steps.clone();

    let mut dest_bitmap: *mut imageflow_core::ffi::BitmapBgra = std::ptr::null_mut();
    let ptr_to_ptr = &mut dest_bitmap as *mut *mut imageflow_core::ffi::BitmapBgra;

    steps.push(s::Node::FlowBitmapBgraPtr { ptr_to_flow_bitmap_bgra_ptr: ptr_to_ptr as usize});

    let build = s::Build001{
        builder_config: Some(default_build_config(debug)),
        io: io,
        framewise: s::Framewise::Steps(steps)
    };
    let mut context = Context::create().unwrap();
    context.message("v0.1/build", &serde_json::to_vec(&build).unwrap()).unwrap();
    unsafe { ((*dest_bitmap).w, (*dest_bitmap).h) }
}


#[test]
fn test_dimensions(){
    let steps = vec![
    s::Node::CreateCanvas{w: 638, h: 423, format: s::PixelFormat::Bgra32, color: s::Color::Black},
    //s::Node::Crop { x1: 0, y1: 0, x2: 638, y2: 423},
    s::Node::Resample2D{w:200,h:133, down_filter: None, up_filter: None, hints: None, scaling_colorspace: None},
    s::Node::ExpandCanvas{left:1, top: 0, right:0, bottom: 0, color: s::Color::Transparent},
    ];
    let (w, h) = get_result_dimensions(steps, vec![], DEBUG_GRAPH);
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
    let (w, h) = get_result_dimensions(steps, vec![png], false);
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
    //let white = s::Color::Srgb(s::ColorSrgb::Hex("FFFFFFFF".to_owned()));
    let blue = s::Color::Srgb(s::ColorSrgb::Hex("0000FFFF".to_owned()));
    let matched = compare(None, 1,
                          "DetectWhitespace".to_owned(), POPULATE_CHECKSUMS, DEBUG_GRAPH, vec![
            s::Node::CreateCanvas {w: 400, h: 300, format: s::PixelFormat::Bgra32, color: s::Color::Transparent},
            s::Node::FillRect{x1:0, y1:0, x2:50, y2:100, color: blue},
            s::Node::CropWhitespace {threshold: 80, percent_padding: 0f32}
        ]
    );
    assert!(matched);
}


//#[test]
//fn test_get_info_png_invalid() {
//    let tinypng = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00,
//                       0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00,
//                       0x00, 0x00, 0x0A, 0x49 ];
//
//    let _ = imageflow_core::clients::stateless::LibClient {}.get_image_info(&tinypng).err().expect("Should fail");
//}


fn test_idct_callback(_: s::ImageInfo) -> (Option<s::DecoderCommand>, Vec<s::Node>)
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

fn test_idct_no_gamma_callback(info: s::ImageInfo) -> (Option<s::DecoderCommand>, Vec<s::Node>)
{
    let new_w = (info.image_width * 6 + 8 - 1) / 8;
    let new_h = (info.image_height * 6 + 8 - 1) / 8;
    let hints = s::JpegIDCTDownscaleHints{
        gamma_correct_for_srgb_during_spatial_luma_scaling: Some(false),
        scale_luma_spatially: Some(true),
        width: new_w as i64,
        height: new_h as i64
    };
    //Here we send the hints via the Decode node instead.
    (Some(s::DecoderCommand::JpegDownscaleHints(hints.clone())),
     vec![s::Node::Decode{io_id:0, commands: Some(vec![s::DecoderCommand::JpegDownscaleHints(hints)])}])

}

#[test]
fn test_idct_linear(){
    let matched = test_with_callback("ScaleIDCTFastvsSlow".to_owned(), s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/roof_test_800x600.jpg".to_owned()),
    test_idct_callback);
    assert!(matched);
}

#[test]
fn test_idct_spatial_no_gamma(){
    let matched = test_with_callback("ScaleIDCT_approx_gamma".to_owned(), s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/roof_test_800x600.jpg".to_owned()),
                                     test_idct_no_gamma_callback);
    assert!(matched);
}
//
//#[test]
//fn test_fail(){
//    let matched = test_with_callback("ScaleIDCTFastvsSlow".to_owned(), s::IoEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/roof_test_800x600.jpg".to_owned()),
//                                     test_idct_callback_no_gamma);
//    assert!(matched);
//}

fn test_with_callback(checksum_name: String, input: s::IoEnum, callback: fn(s::ImageInfo) -> (Option<s::DecoderCommand>, Vec<s::Node>) ) -> bool{
    let context = Context::create().unwrap();
    let matched:bool;

    unsafe {
        let mut job = context.create_job();
        //Add input
        ::imageflow_core::parsing::IoTranslator::new(&context).add_to_job(&mut *job, vec![s::IoObject{ io_id:0, direction: s::IoDirection::In, io: input}]);


        let info_blob: JsonResponse = job.message("v0.1/get_image_info", "{\"io_id\": 0}".as_bytes()).unwrap();
        let info_response: s::Response001 = serde_json::from_slice(info_blob.response_json.as_ref()).unwrap();
        if !info_response.success {
            panic!("get_image_info failed: {:?}",info_response);
        }
        let image_info = match info_response.data {
            s::ResponsePayload::ImageInfo(info) => info,
            _ => panic!("")
        };

        let (tell_decoder, mut steps): (Option<s::DecoderCommand>, Vec<s::Node>) = callback(image_info);

        if let Some(what) = tell_decoder {
            let send_hints = s::TellDecoder001 {
                io_id: 0,
                command: what
            };
            let send_hints_str = serde_json::to_string_pretty(&send_hints).unwrap();
            job.message("v0.1/tell_decoder", send_hints_str.as_bytes()).unwrap().assert_ok();
        }

        let mut dest_bitmap: *mut imageflow_core::ffi::BitmapBgra = std::ptr::null_mut();

        let ptr_to_ptr = &mut dest_bitmap as *mut *mut imageflow_core::ffi::BitmapBgra;

        steps.push(s::Node::FlowBitmapBgraPtr { ptr_to_flow_bitmap_bgra_ptr: ptr_to_ptr as usize});


        let send_execute = s::Execute001{
            framewise: s::Framewise::Steps(steps),
            graph_recording: None
        };

        let send_execute_str = serde_json::to_string_pretty(&send_execute).unwrap();
        job.message("v0.1/execute", send_execute_str.as_bytes()).unwrap().assert_ok();



        let ctx = checkums_ctx_for(&context);
        matched = regression_check(&ctx, *ptr_to_ptr, &checksum_name)


    }
    context.destroy_allowing_panics();
    matched
}

fn djb2(bytes: &[u8]) -> u64{
    bytes.iter().fold(5381u64, |hash, c| ((hash << 5).wrapping_add(hash)).wrapping_add(*c as u64))
}

use imageflow_core::ffi::BitmapBgra;
use std::collections::HashMap;
use ::std::fs::File;
use ::std::path::{PathBuf};
use ::std::io::Write;
use twox_hash::XxHash;
use std::hash::Hasher;

fn checksum_bitmap(bitmap: &BitmapBgra) -> String {
    unsafe {
        let info = format!("{}x{} fmt={} alpha={}", bitmap.w, bitmap.h, bitmap.fmt as i32, bitmap.alpha_meaningful as i32);
        let width_bytes = bitmap.w as usize * if bitmap.fmt == ::imageflow_core::ffi::PixelFormat::Bgra32 { 4} else { 3 };
        let mut hash = XxHash::with_seed(0x8ed12ad9483d28a0);
        for h in 0isize..(bitmap.h as isize){
            let row_slice = ::std::slice::from_raw_parts(bitmap.pixels.offset(h * bitmap.stride as isize), width_bytes);
            hash.write(row_slice)
        }
        return format!("{:02$X}_{:02$X}",hash.finish(), djb2(info.as_bytes()),17)
    }
}

struct ChecksumCtx<'a>{
    c: &'a Context,
    checksum_file: PathBuf,
    visuals_dir: PathBuf,
    #[allow(dead_code)]
    cache_dir: PathBuf,
    create_if_missing: bool,
    max_off_by_one_ratio: f32
}
#[macro_use]
extern crate lazy_static;

use std::sync::RwLock;

lazy_static! {
    static ref CHECKSUM_FILE: RwLock<()> = RwLock::new(());
}

fn load_list(c: &ChecksumCtx) -> Result<HashMap<String,String>,()>{
    if c.checksum_file.exists() {
        let map: HashMap<String, String> = ::serde_json::from_reader(::std::fs::File::open(&c.checksum_file).unwrap()).unwrap();
        Ok(map)
    }else{
        Ok(HashMap::new())
    }
}
fn save_list(c: &ChecksumCtx, map: &HashMap<String,String>) -> Result<(),()>{
    let mut f = ::std::fs::File::create(&c.checksum_file).unwrap();
    ::serde_json::to_writer_pretty(&mut f, map).unwrap();

    f.sync_all().unwrap();
    Ok(())
}

#[allow(unused_variables)]
fn load_checksum(c: &ChecksumCtx, name: &str) -> Option<String>{
    #[allow(unused_variables)]
    let lock = CHECKSUM_FILE.read().unwrap();
    load_list(c).unwrap().get(name).and_then(|v|Some(v.to_owned()))
}
#[allow(unused_variables)]
fn save_checksum(c: &ChecksumCtx, name: String, checksum: String) -> Result<(),()>{
    #[allow(unused_variables)]
    let lock = CHECKSUM_FILE.write().unwrap();
    let mut map = load_list(c).unwrap();
    map.insert(name,checksum);
    save_list(c,&map).unwrap();
    Ok(())
}

fn fetch_bytes(url: &str) -> Vec<u8> {
    hlp::fetching::fetch_bytes(url).expect("Did you forget to upload {} to s3?")
}

fn download(c: &ChecksumCtx, checksum: &str){
    let dest_path = c.visuals_dir.as_path().join(Path::new(&format!("{}.png", checksum)));
    let source_url = format!("https://s3-us-west-2.amazonaws.com/imageflow-resources/visual_test_checksums/{}.png",checksum);
    if dest_path.exists() {
        println!("{} (trusted) exists", checksum);
    }else{
        println!("Fetching {} to {:?}", &source_url, &dest_path);
        File::create(&dest_path).unwrap().write_all(&fetch_bytes(&source_url)).unwrap();
    }
}
fn save_visual(c: &ChecksumCtx, bit: &BitmapBgra){
    let checksum =checksum_bitmap(bit);
    let dest_path = c.visuals_dir.as_path().join(Path::new(&format!("{}.png", &checksum)));
    if !dest_path.exists(){
        println!("Writing {:?}", &dest_path);
        let dest_cpath = CString::new(dest_path.into_os_string().into_string().unwrap()).unwrap();
        unsafe {
            if !::imageflow_core::ffi::flow_bitmap_bgra_save_png(c.c.flow_c(), bit as *const BitmapBgra, dest_cpath.as_ptr()){
                c.c.error().assert_ok();
            }
        }

    }
}

fn load_visual(c: &ChecksumCtx, checksum: &str) -> *const BitmapBgra{
    unsafe {
        let path = c.visuals_dir.as_path().join(Path::new(&format!("{}.png", &checksum)));
        let cpath = CString::new(path.into_os_string().into_string().unwrap()).unwrap();
        let mut b: *const BitmapBgra = std::ptr::null();
        if !::imageflow_core::ffi::flow_bitmap_bgra_load_png(c.c.flow_c(), &mut b as *mut *const BitmapBgra, cpath.as_ptr()) {
            c.c.error().assert_ok();
        }
        b
    }
}
/// Returns the number of bytes that differ, followed by the total value of all differences
/// If these are equal, then only off-by-one errors are occurring
fn diff_bytes(a: &[u8], b: &[u8]) ->(i64,i64){
    a.iter().zip(b.iter()).fold((0,0), |(count, delta), (a,b)| if a != b { (count + 1, delta + (*a as i64 - *b as i64).abs()) } else { (count,delta)})
}


fn diff_bitmap_bytes(a: &BitmapBgra, b: &BitmapBgra) -> (i64,i64){
    if a.w != b.w || a.h != b.h || a.fmt != b.fmt { panic!("Bitmap dimensions differ"); }

    let width_bytes = a.w as usize * if a.fmt == ::imageflow_core::ffi::PixelFormat::Bgra32 { 4} else { 3 };

    (0isize..a.h as isize).map(|h| {
        let a_contents_slice = unsafe { ::std::slice::from_raw_parts(a.pixels.offset(h * a.stride as isize), width_bytes) };
        let b_contents_slice = unsafe { ::std::slice::from_raw_parts(b.pixels.offset(h * b.stride as isize), width_bytes) };
        diff_bytes(a_contents_slice, b_contents_slice)
    }).fold((0,0), |(a,b),(c,d)| (a + c,b + d))
}

fn regression_check(c: &ChecksumCtx, bitmap: *const BitmapBgra, name: &str) -> bool{
    let bitmap_ref =unsafe{&*bitmap};

    // Always write a copy if it doesn't exist
    save_visual(c, bitmap_ref);

    if bitmap.is_null(){panic!("");}
    let trusted = load_checksum(c, name);
    let current = checksum_bitmap(bitmap_ref);
    if trusted == None {
        if c.create_if_missing {
            println!("====================\n{}\nStoring checksum {}", name, &current);
            save_checksum(c, name.to_owned(), current.clone()).unwrap();
        } else {
            panic!("There is no stored checksum for {}; rerun with create_if_missing=true", name);
        }
        true
    }else if Some(&current) != trusted.as_ref() {
        download(c, trusted.as_ref().unwrap());
        println!("====================\n{}\nThe stored checksum {} differs from the current one {}", name, trusted.as_ref().unwrap(), &current);

        let trusted_bit = load_visual(c,trusted.as_ref().unwrap());
        let (count, delta) = diff_bitmap_bytes(bitmap_ref, unsafe{ &*trusted_bit});
        unsafe{
            ::imageflow_core::ffi::flow_destroy(c.c.flow_c(), trusted_bit as *const libc::c_void, std::ptr::null(), 0);
        }
        if count != delta{
            panic!("Not just off-by-one errors! count={} delta={}", count, delta);
        }
        let allowed_errors = ((bitmap_ref.w * bitmap_ref.stride) as f32 * c.max_off_by_one_ratio) as i64;
        if delta  > allowed_errors{
            panic!("There were {} off-by-one errors, more than the {} ({}%) allowed.", delta, allowed_errors, c.max_off_by_one_ratio * 100f32);
        }
        true
        //Optionally run dssim/imagemagick
    }else{
        true //matched! yay!
    }
}

