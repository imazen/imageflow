extern crate imageflow_serde as s;
use daggy::{Dag, EdgeIndex, NodeIndex};
use ffi;
use ffi::{Context, Job, NodeType, EdgeKind};
use flow::definitions::*;
use flow::graph::Graph;
use petgraph;
use super::*;
use super::NodeDefHelpers;

lazy_static! {

    pub static ref CREATE_CANVAS: NodeDefinition = NodeDefinition {
        id: NodeType::Create_Canvas,
        name: "create_canvas",
        description: "Create Canvas",
        fn_estimate: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                let ref mut weight = ctx.weight_mut(ix);
                match weight.params{
                    NodeParams::Json(s::Node::CreateCanvas{ ref format, ref  w, ref h, ref color}) => {
                        weight.frame_est = FrameEstimate::Some(FrameInfo{w: *w as i32, h: *h as i32, fmt: ffi::PixelFormat::from(format), alpha_meaningful: true});
                    },
                    _ => { panic!("Node params missing");}
                }
            }
            f
        }),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                let c = ctx.c;
                let ref mut weight = ctx.weight_mut(ix);
                match  weight.params{
// TODO: support color
                    NodeParams::Json(s::Node::CreateCanvas{ ref format, ref  w, ref h, ref color}) => unsafe {
                        weight.result = NodeResult::Frame(::ffi::flow_bitmap_bgra_create(c, *w as i32, *h as i32, true, ffi::PixelFormat::from(format)))
                    },
                    _ => { panic!("Node params missing");}
                }

            }
            f
        }),
        .. Default::default()
    };


}