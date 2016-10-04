extern crate imageflow_serde as s;
use daggy::{Dag, EdgeIndex, NodeIndex};
use ffi;
use std::ptr;
use ffi::{Context, Job, NodeType, EdgeKind, BitmapBgra};
use flow::definitions::*;
use flow::graph::Graph;
use petgraph;
use super::*;
use super::NodeDefHelpers;

fn bitmap_bgra_def() -> NodeDefinition{
    NodeDefinition {
        id: NodeType::primitive_bitmap_bgra_pointer,
        name: "primitive_bitmap_bgra_pointer",
        outbound_edges: true,
        inbound_edges: EdgesIn::OneOptionalInput,

        fn_estimate: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                match ctx.weight(ix).params{
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

                    },
                    _ => { panic!("Node params missing");}
                }
            }
            f
        }),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                //let ref mut weight = ctx.weight_mut(ix);
                match ctx.weight(ix).params{
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
                    },
                    _ => { panic!("Node params missing");}
                }
            }
            f
        }),
        .. Default::default()
    }
}

lazy_static! {
    pub static ref BITMAP_BGRA_POINTER: NodeDefinition = bitmap_bgra_def();
}