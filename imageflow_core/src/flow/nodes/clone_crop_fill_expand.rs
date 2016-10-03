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
    pub static ref CLONE: NodeDefinition = NodeDefinition {
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
    };


    pub static ref COPY_RECT: NodeDefinition = NodeDefinition {
        id: NodeType::primitive_CopyRectToCanvas,
        name: "copy_rect",
        inbound_edges: EdgesIn::OneInputOneCanvas,
        description: "Copy Rect",
        fn_estimate:  Some(NodeDefHelpers::copy_frame_est_from_first_canvas),
        fn_execute: Some({

            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

            //              FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_copy_rect_to_canvas, info)
            //    FLOW_GET_INPUT_EDGE(g, node_id)
            //    FLOW_GET_CANVAS_EDGE(g, node_id)
            //    struct flow_node * n = &g->nodes[node_id];
            //
            //    struct flow_bitmap_bgra * input = g->nodes[input_edge->from].result_bitmap;
            //    struct flow_bitmap_bgra * canvas = g->nodes[canvas_edge->from].result_bitmap;
            //
            //    // TODO: implement bounds checks!!!
            //    if (input->fmt != canvas->fmt) {
            //        FLOW_error(c, flow_status_Invalid_argument);
            //        return false;
            //    }
            //    if (info->x == 0 && info->from_x == 0 && info->from_y == 0 && info->y == 0 && info->width == input->w
            //        && info->width == canvas->w && info->height == input->h && info->height == canvas->h
            //        && canvas->stride == input->stride) {
            //        memcpy(canvas->pixels, input->pixels, input->stride * input->h);
            //        canvas->alpha_meaningful = input->alpha_meaningful;
            //    } else {
            //        int32_t bytes_pp = flow_pixel_format_bytes_per_pixel(input->fmt);
            //        for (uint32_t y = 0; y < info->height; y++) {
            //            void * from_ptr = input->pixels + (size_t)(input->stride * (info->from_y + y) + bytes_pp * info->from_x);
            //            void * to_ptr = canvas->pixels + (size_t)(canvas->stride * (info->y + y) + bytes_pp * info->x);
            //            memcpy(to_ptr, from_ptr, info->width * bytes_pp);
            //        }
            //    }
            //    n->result_bitmap = canvas;
            //                let ref mut weight = ctx.weight_mut(ix);
            //                match weight.params{
            //                    NodeParams::Json(s::Node::CreateCanvas{format,w,h,color}) => {
            //                        weight.result = NodeResult::Frame(::ffi::flow_bitmap_bgra_create(ctx.c, w as i32, h as i32, true, ffi::PixelFormat::from(format)))
            //                    },
            //                    _ => { panic!("Node params missing");}
            //                }

            }
            f
        }),
        .. Default::default()
    };
}
