extern crate imageflow_serde as s;
use daggy::{Dag, EdgeIndex, NodeIndex};
use ffi;
use ffi::{Context, Job, NodeType, EdgeKind, PixelFormat};
use flow::definitions::*;
use flow::graph::Graph;
use petgraph;
use std::ptr;
use super::*;
use super::NodeDefHelpers;


fn copy_rect_def() -> NodeDefinition {
    NodeDefinition {
        id: NodeType::primitive_CopyRectToCanvas,
        name: "copy_rect",
        inbound_edges: EdgesIn::OneInputOneCanvas,
        description: "Copy Rect",
        fn_estimate:  Some(NodeDefHelpers::copy_frame_est_from_first_canvas),
        fn_execute: Some({

            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

                if let s::Node::CopyRectToCanvas {from_x, from_y, width, height, x, y} =
                    ctx.get_json_params(ix).unwrap() {
                    let input = ctx.first_parent_result_frame(ix, EdgeKind::Input).unwrap();
                    let canvas = ctx.first_parent_result_frame(ix, EdgeKind::Canvas).unwrap();

                    unsafe {
                        if (*input).fmt != (*canvas).fmt { panic!("Can't copy between bitmaps with different pixel formats")}

                        //TODO: Implement faster path for common (full clone) path
                        //    if (info->x == 0 && info->from_x == 0 && info->from_y == 0 && info->y == 0 && info->width == input->w
                        //        && info->width == canvas->w && info->height == input->h && info->height == canvas->h
                        //        && canvas->stride == input->stride) {
                        //        memcpy(canvas->pixels, input->pixels, input->stride * input->h);
                        //        canvas->alpha_meaningful = input->alpha_meaningful;

                        let bytes_pp = match (*input).fmt { PixelFormat::Gray8 => 1, PixelFormat::BGRA32 => 4, PixelFormat::BGR24 => 3};
                        for row in 0..height {
                            let from_offset = (*input).stride * (from_y + row) + bytes_pp * from_x;
                            let from_ptr = (*input).pixels.offset(from_offset as isize);
                            let to_offset = (*canvas).stride * (y + row) + bytes_pp * x;
                            let to_ptr = (*canvas).pixels.offset(to_offset as isize);
                            ptr::copy_nonoverlapping(from_ptr, to_ptr, (width * bytes_pp) as usize);
                        }


                        ctx.weight_mut(ix).result = NodeResult::Frame(canvas);
                    }

                }else{
                    panic!("Missing params")
                }
            }
            f
        }),
        .. Default::default()
    }
}

fn clone_def() -> NodeDefinition{
    NodeDefinition {
        id: NodeType::Clone,
        name: "Clone",
        description: "Clone",
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_input),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                match ctx.first_parent_input_weight(ix).unwrap().frame_est{
                    FrameEstimate::Some(FrameInfo{w,h,fmt,alpha_meaningful}) => {
                        let canvas_params = s::Node::CreateCanvas{w: w as usize, h: h as usize, format: s::PixelFormat::from(fmt), color: s::Color::Transparent };
                        let copy_params = s::Node::CopyRectToCanvas{from_x: 0, from_y: 0, x: 0, y: 0, width: w as u32, height: h as u32};
                        let canvas = ctx.graph.add_node(Node::new(&CREATE_CANVAS, NodeParams::Json(canvas_params)));
                        let copy = ctx.graph.add_node(Node::new(&COPY_RECT, NodeParams::Json(copy_params)));
                        ctx.graph.add_edge(canvas, copy, EdgeKind::Canvas).unwrap();
                        ctx.replace_node_with_existing(ix, copy);
                    }
                    _ => {panic!("")}
                }

            }
            f
        }),
        .. Default::default()
    }
}
lazy_static! {
    pub static ref CLONE: NodeDefinition = clone_def();


    pub static ref COPY_RECT: NodeDefinition = copy_rect_def();
}
