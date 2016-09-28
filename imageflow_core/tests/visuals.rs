extern crate imageflow_core;
extern crate libc;
extern crate rustc_serialize;
extern crate imageflow_serde as s;
extern crate serde;
extern crate serde_json;

use imageflow_core::Context;
use imageflow_core::parsing::BuildRequestHandler;



/// Creates a static, null-terminated Rust string, and
/// returns a ` *const libc::c_char` pointer to it.
///
/// Useful for API invocations that require a static C string

macro_rules! static_char {
    ($lit:expr) => {
        concat!($lit, "\0").as_ptr() as *const libc::c_char
    }
}

#[test]
fn try_visual(){
    let mut dest_bitmap: *mut imageflow_core::ffi::FlowBitmapBgra = std::ptr::null_mut();

    let ptr_to_ptr = &mut dest_bitmap as *mut *mut imageflow_core::ffi::FlowBitmapBgra;

    let input_io = s::IoObject {
        io_id: 0,
        direction: s::IoDirection::Input,
        checksum: None,
        io: s::IoEnum::BytesHex("FFD8FFE000104A46494600010101004800480000FFDB004300FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC2000B080001000101011100FFC40014100100000000000000000000000000000000FFDA0008010100013F10".to_owned())
    };

    let mut steps = vec![];
    steps.push(s::Node::Decode {io_id: 0});
    steps.push(s::Node::Scale{ w: 20, h: 30, down_filter: None, up_filter: None, sharpen_percent: None, flags: None });
//    steps.push(s::Node::FlipV);
//    steps.push(s::Node::FlipH);
//    steps.push(s::Node::Rotate90);
//    steps.push(s::Node::Rotate180);
//    steps.push(s::Node::Rotate270);
//    steps.push(s::Node::Transpose);
//    steps.push(s::Node::ExpandCanvas {top:2, left: 3, bottom: 4, right: 5, color: s::Color::Srgb(s::ColorSrgb::Hex("aeae22".to_owned()))});
//    steps.push(s::Node::FillRect {x1: 0, x2: 10, y1: 0, y2: 10, color: s::Color::Srgb(s::ColorSrgb::Hex("ffee00".to_owned()))});
    steps.push(s::Node::FlowBitmapBgraPtr { ptr_to_flow_bitmap_bgra_ptr: ptr_to_ptr as usize});

//    let recording = s::Build001_Graph_Recording{
//        record_graph_versions: Some(true),
//        record_frame_images: Some(false),
//        render_last_graph: Some(true),
//        render_animated_graph: Some(false),
//        render_graph_versions : Some(false),
//    };

    let build = s::Build001{
        builder_config: Some(s::Build001Config{graph_recording: None /*Some(recording)*/,
            process_all_gif_frames: Some(false),
            enable_jpeg_block_scaling: Some(false)
        }),
        io: vec![input_io],
        framewise: s::Framewise::Steps(steps)
    };

    let json_str = serde_json::to_string_pretty(&build).unwrap();

    let handler = BuildRequestHandler::new();

    let mut context = Context::create();

    let mut ctx_cell = context.unsafe_borrow_mut_context_pointer();

    //println!("{}", json_str);

    let p = std::env::current_dir().unwrap();
    println!("The current directory is {}", p.display());

    let response = handler.do_and_respond(&mut *ctx_cell, json_str.into_bytes().as_slice());

    let json_response = response.unwrap();

    unsafe {
        ctx_cell.assert_ok(None);

        println!("{:?}", **ptr_to_ptr);
    }


    let store_if_missing = true;


    //(c: *mut Context, bitmap: *mut FlowBitmapBgra, storage_name: *const libc::c_char, store_if_missing: bool, off_by_one_byte_differences_permitted: usize, caller_filename: *const libc::c_char, caller_linenumber: i32) -> bool;
    unsafe {
        //TODO: Fix link error

        let matched: bool;
        {
            matched = imageflow_core::ffi::flow_bitmap_bgra_test_compare_to_record(ctx_cell.ptr.unwrap(), *ptr_to_ptr, static_char!("rust_test_b"), store_if_missing, 500, static_char!("rust"), 0, static_char!(file!()));
        }
        println!("{:?}", **ptr_to_ptr);

        ctx_cell.assert_ok(None);

        assert!(matched);

    }
}
