extern crate imageflow_core;
extern crate libc;
extern crate rustc_serialize;
extern crate imageflow_serde as s;
extern crate serde;
extern crate serde_json;

use std::ffi::CString;
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

fn compare(input: s::IoEnum, allowed_off_by_one_bytes: usize, checksum_name: String, store_if_missing: bool, debug: bool, mut steps: Vec<s::Node>) -> bool {
    let mut dest_bitmap: *mut imageflow_core::ffi::FlowBitmapBgra = std::ptr::null_mut();

    let ptr_to_ptr = &mut dest_bitmap as *mut *mut imageflow_core::ffi::FlowBitmapBgra;

    let input_io = s::IoObject {
        io_id: 0,
        direction: s::IoDirection::Input,
        checksum: None,
        io: input
    };

    steps.push(s::Node::FlowBitmapBgraPtr { ptr_to_flow_bitmap_bgra_ptr: ptr_to_ptr as usize});

        let recording = s::Build001_Graph_Recording{
            record_graph_versions: Some(true),
            record_frame_images: Some(false),
            render_last_graph: Some(true),
            render_animated_graph: Some(false),
            render_graph_versions : Some(false),
        };

    let build = s::Build001{
        builder_config: Some(s::Build001Config{graph_recording: match debug{ true => Some(recording), false => None} ,
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

    if debug {
        println!("{}", json_str);
    }

    let p = std::env::current_dir().unwrap();
    if debug {
        println!("The current directory is {}", p.display());
    }

    let response = handler.do_and_respond(&mut *ctx_cell, json_str.into_bytes().as_slice());

    let json_response = response.unwrap();

    unsafe {
        ctx_cell.assert_ok(None);

        if debug {
            println!("{:?}", *ptr_to_ptr);
        }
    }

     unsafe {
        let matched: bool;
         let c_checksum_name = CString::new(checksum_name).unwrap();
        {
            matched = imageflow_core::ffi::flow_bitmap_bgra_test_compare_to_record(ctx_cell.ptr.unwrap(), *ptr_to_ptr, c_checksum_name.as_ptr(), store_if_missing, allowed_off_by_one_bytes, static_char!(file!()), 0, static_char!(file!()));
        }
        ctx_cell.assert_ok(None);

        return matched;
    }
}


// Replaces TEST_CASE("Test scale rings", "")
#[test]
fn test_scale_rings(){
    let matched = compare(s::IoEnum::Url("http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/rings2.png".to_owned()), 500,
        "RingsDownscaling".to_owned(), true, true, vec![
        s::Node::Decode {io_id: 0},
        s::Node::Scale{ w: 400, h: 400, down_filter: Some(s::Filter::Hermite), up_filter: Some(s::Filter::Hermite), sharpen_percent: Some(0f32), flags: Some(1) }
        ]
    );
    assert!(matched);
}
