extern crate imageflow_serde as s;
use daggy::{Dag, EdgeIndex, NodeIndex};
use ffi;
use ffi::{Context, Job, EdgeKind};
use flow::definitions::*;
use flow::graph::Graph;
use petgraph;
use super::*;
use super::NodeDefHelpers;

fn create_canvas_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.create_canvas",
        name: "create_canvas",
        fn_estimate: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                let ref mut weight = ctx.weight_mut(ix);
                match weight.params {
                    NodeParams::Json(s::Node::CreateCanvas { ref format,
                                                             ref w,
                                                             ref h,
                                                             ref color }) => {
                        weight.frame_est = FrameEstimate::Some(FrameInfo {
                            w: *w as i32,
                            h: *h as i32,
                            fmt: ffi::PixelFormat::from(format),
                            alpha_meaningful: true,
                        });
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
                let c = ctx.c;
                let ref mut weight = ctx.weight_mut(ix);
                match weight.params {
                    NodeParams::Json(s::Node::CreateCanvas { ref format,
                                                             ref w,
                                                             ref h,
                                                             ref color }) => unsafe {
                        // TODO: handle creation failure. Most likely OOM in entire codebase
                        let ptr = ::ffi::flow_bitmap_bgra_create(c,
                                                                 *w as i32,
                                                                 *h as i32,
                                                                 true,
                                                                 ffi::PixelFormat::from(format));
                        let color_val = color.clone();
                        if color_val != s::Color::Transparent {
                            if !ffi::flow_bitmap_bgra_fill_rect(c,
                                                                ptr,
                                                                0,
                                                                0,
                                                                *w as u32,
                                                                *h as u32,
                                                                color_val.to_u32_bgra().unwrap()) {
                                panic!("failed to fill rect. epic.");
                            }
                        }
                        weight.result = NodeResult::Frame(ptr);
                    },
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

lazy_static! {
    pub static ref CREATE_CANVAS: NodeDefinition = create_canvas_def();
}
