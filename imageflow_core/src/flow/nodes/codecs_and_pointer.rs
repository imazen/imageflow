extern crate imageflow_serde as s;
use daggy::{Dag, EdgeIndex, NodeIndex};
use ffi;
use ffi::{Context, Job, NodeType, EdgeKind, BitmapBgra};
use flow::definitions::*;
use flow::graph::Graph;
use petgraph;
use petgraph::EdgeDirection;
use std::ptr;
use super::*;
use super::NodeDefHelpers;

fn bitmap_bgra_def() -> NodeDefinition {
    NodeDefinition {
        id: NodeType::primitive_bitmap_bgra_pointer,
        name: "primitive_bitmap_bgra_pointer",
        outbound_edges: true,
        inbound_edges: EdgesIn::OneOptionalInput,

        fn_estimate: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                match ctx.weight(ix).params {
                    NodeParams::Json(s::Node::FlowBitmapBgraPtr{ptr_to_flow_bitmap_bgra_ptr}) => {
                        let ptr: *mut *mut BitmapBgra = ptr_to_flow_bitmap_bgra_ptr as *mut *mut BitmapBgra;
                        unsafe {
                            if ptr == ptr::null_mut() {
                                panic!("Must be a valid pointer to a pointer to BitmapBgra");
                            }

                            if *ptr == ptr::null_mut() {
                                NodeDefHelpers::copy_frame_est_from_first_input(ctx, ix);
                            } else {
                                let ref mut weight = ctx.weight_mut(ix);
                                let ref b = **ptr;
                                weight.frame_est = FrameEstimate::Some(FrameInfo {
                                    w: b.w as i32,
                                    h: b.h as i32,
                                    fmt: b.fmt,
                                    alpha_meaningful: b.alpha_meaningful
                                });
                            }
                        }

                    }
                    _ => {
                        panic!("Node params missing");
                    }
                }
            }
            f
        }),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                // let ref mut weight = ctx.weight_mut(ix);
                match ctx.weight(ix).params {
                    NodeParams::Json(s::Node::FlowBitmapBgraPtr{ptr_to_flow_bitmap_bgra_ptr}) => {
                        let ptr: *mut *mut BitmapBgra = ptr_to_flow_bitmap_bgra_ptr as *mut *mut BitmapBgra;
                        unsafe {
                            if ptr == ptr::null_mut() {
                                panic!("Must be a valid pointer to a pointer to BitmapBgra");
                            }

                            let frame =     ctx.first_parent_result_frame(ix, EdgeKind::Input);
                            let ref mut weight = ctx.weight_mut(ix);
                            match frame {
                                Some(input_ptr) => {
                                    *ptr = input_ptr;
                                    weight.result = NodeResult::Frame(input_ptr);
                                },
                                None => {
                                    if *ptr == ptr::null_mut() {
                                        panic!("When serving as an input node, FlowBitmapBgraPtr must point to a valid BitmapBgra. Found null.");
                                    }
                                    weight.result = NodeResult::Frame(*ptr);
                                }
                            }
                        }
                    }
                    _ => {
                        panic!("Node params missing");
                    }
                }
            }
            f
        }),
        ..Default::default()
    }
}

fn decoder_io_id(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) -> Option<i32> {
    match ctx.weight(ix).params {
        NodeParams::Json(s::Node::Decode { io_id }) => Some(io_id),
        _ => None,
    }
}

fn decoder_estimate(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {

    let codec = ctx.weight(ix).custom_state as *mut ffi::CodecInstance;

    let io_id = decoder_io_id(ctx, ix).unwrap();
    let mut frame_info: ffi::DecoderInfo = Default::default();
    unsafe {
        if !ffi::flow_job_get_decoder_info(ctx.c,
                                           ctx.job,
                                           io_id,
                                           &mut frame_info as *mut ffi::DecoderInfo) {
            ctx.assert_ok();
        }
    }

    ctx.weight_mut(ix).frame_est = FrameEstimate::Some(FrameInfo {
        fmt: frame_info.frame0_post_decode_format,
        w: frame_info.frame0_width,
        h: frame_info.frame0_height,
        alpha_meaningful: true, // WRONG
    });
}

// Todo list codec name in stringify

fn decoder_def() -> NodeDefinition {
    NodeDefinition {
        id: NodeType::decoder,
        name: "decoder",
        outbound_edges: true,
        inbound_edges: EdgesIn::NoInput,
        fn_estimate: Some(decoder_estimate),

        // Allow link-up
        fn_link_state_to_this_io_id: Some(decoder_io_id),
        fn_flatten_pre_optimize: {
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {

                // Mutate instead of replace (custom_state is populated)
                ctx.weight_mut(ix).def = &PRIMITIVE_DECODER;


                let exif_flag = unsafe {ffi::flow_codecs_jpg_decoder_get_exif(ctx.c, ctx.weight(ix).custom_state as *mut ffi::CodecInstance) };
                if exif_flag > 0 {
                    let new_node = ctx.graph
                        .add_node(Node::new(&APPLY_ORIENTATION,
                                            NodeParams::Json(s::Node::ApplyOrientation {
                                                flag: exif_flag,
                                            })));
                    ctx.copy_edges_to(ix, new_node, EdgeDirection::Outgoing);
                    ctx.delete_child_edges_for(ix);
                    ctx.graph.add_edge(ix, new_node, EdgeKind::Input).unwrap();
                }
            }
            Some(f)
        },
        ..Default::default()
    }
}
fn primitive_decoder_def() -> NodeDefinition {
    NodeDefinition {
        id: NodeType::primitive_decoder,
        name: "primitive_decoder",
        outbound_edges: true,
        inbound_edges: EdgesIn::NoInput,
        fn_estimate: Some(decoder_estimate),

        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                let codec = ctx.weight(ix).custom_state as *mut ffi::CodecInstance;

                unsafe {
                    let result = ffi::flow_codec_execute_read_frame(ctx.c, codec);
                    if result == ptr::null_mut() {
                        ctx.assert_ok();
                    } else {
                        ctx.weight_mut(ix).result = NodeResult::Frame(result);
                    }
                }
            }
            f
        }),
        ..Default::default()
    }
}


lazy_static! {
    pub static ref BITMAP_BGRA_POINTER: NodeDefinition = bitmap_bgra_def();
    pub static ref DECODER: NodeDefinition = decoder_def();
    pub static ref PRIMITIVE_DECODER: NodeDefinition = primitive_decoder_def();
}
