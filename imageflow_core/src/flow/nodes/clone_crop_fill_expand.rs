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



fn fill_rect_def() -> NodeDefinition {
    NodeDefinition {
        id: NodeType::Fill_Rect_Mutate,
        name: "fill_rect",
        inbound_edges: EdgesIn::OneInput,
        fn_estimate:  Some(NodeDefHelpers::copy_frame_est_from_first_input),
        fn_execute: Some({

            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

                if let s::Node::FillRect {x1,x2,y1,y2, color} =
                ctx.get_json_params(ix).unwrap() {


                    let input = ctx.first_parent_result_frame(ix, EdgeKind::Input).unwrap();
                    unsafe {

                        if !ffi::flow_bitmap_bgra_fill_rect(ctx.c, input, x1, y1, x2, y2, color.to_u32_bgra().unwrap()) {
                            panic!("failed to fill rect. epic.");
                        }

                        ctx.weight_mut(ix).result = NodeResult::Frame(input);
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
fn expand_canvas_size(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

    let input_info = ctx.first_parent_frame_info_some(ix).unwrap();

    let ref mut weight = ctx.weight_mut(ix);
    match weight.params{
        NodeParams::Json(s::Node::ExpandCanvas{ref  left, ref top, ref bottom, ref right, ref color}) => {
            weight.frame_est = FrameEstimate::Some(
                FrameInfo{
                    w: input_info.w + *left as i32 + *right as i32,
                    h: input_info.h + *top as i32 + *bottom as i32,
                    fmt: ffi::PixelFormat::from(input_info.fmt),
                    alpha_meaningful: input_info.alpha_meaningful}); //TODO: May change if color has alpha
        },
        _ => { panic!("Node params missing");}
    }
}

fn expand_canvas_def() -> NodeDefinition {
    NodeDefinition {
        id: NodeType::Expand_Canvas,
        name: "expand canvas",
        inbound_edges: EdgesIn::OneInput,
        description: "Expand Canvas",
        fn_estimate:  Some(expand_canvas_size),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                match ctx.first_parent_input_weight(ix).unwrap().frame_est{
                    FrameEstimate::Some(FrameInfo{w,h,fmt,alpha_meaningful}) => {
                        if let s::Node::ExpandCanvas {left, top, right, bottom, color} =
                            ctx.get_json_params(ix).unwrap() {
                            let new_w = w as usize + left as usize + right as usize;
                            let new_h = h as usize + top as usize + bottom as usize;
                            let canvas_params = s::Node::CreateCanvas{w: new_w as usize, h: new_h as usize, format: s::PixelFormat::from(fmt), color: color };
                            let copy_params = s::Node::CopyRectToCanvas{from_x: 0, from_y: 0, x: left, y: top, width: w as u32, height: h as u32};
                            let canvas = ctx.graph.add_node(Node::new(&CREATE_CANVAS, NodeParams::Json(canvas_params)));
                            let copy = ctx.graph.add_node(Node::new(&COPY_RECT, NodeParams::Json(copy_params)));
                            ctx.graph.add_edge(canvas, copy, EdgeKind::Canvas).unwrap();
                            ctx.replace_node_with_existing(ix, copy);
                        }
                    }
                    _ => {panic!("")}
                }
            }
            f
        }),
        .. Default::default()
    }
}


fn crop_size(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

    let input_info = ctx.first_parent_frame_info_some(ix).unwrap_or_else(|| {
        println!("{}", ctx.graph_to_str());
        panic!("");
    });

    let ref mut weight = ctx.weight_mut(ix);
    match weight.params{
        NodeParams::Json(s::Node::Crop{ref  x1, ref y1, ref x2, ref y2}) => {
            weight.frame_est = FrameEstimate::Some(
                FrameInfo{
                    w: *x2 as i32 - *x1 as i32,
                    h: *y2 as i32 - *y1 as i32,
                    fmt: ffi::PixelFormat::from(input_info.fmt),
                    alpha_meaningful: input_info.alpha_meaningful});
        },
        _ => { panic!("Node params missing");}
    }
}
fn crop_mutate_def() -> NodeDefinition {
    NodeDefinition { //TODO: As a mutating node, shouldn't this verify no siblings exist? 'Consumed' might be non-deterministic
        id: NodeType::Crop,
        name: "crop_mutate",
        inbound_edges: EdgesIn::OneInput,
        fn_estimate:  Some(crop_size),
        fn_execute: Some({

            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

                if let s::Node::Crop {x1,x2,y1,y2} =
                ctx.get_json_params(ix).unwrap() {

                    let input = ctx.first_parent_result_frame(ix, EdgeKind::Input).unwrap();
                    unsafe {
                        println!("Cropping {}x{} to ({},{}) ({},{})", (*input).w, (*input).h, x1, y1, x2, y2);

                        let bytes_pp = match (*input).fmt { PixelFormat::Gray8 => 1, PixelFormat::BGRA32 => 4, PixelFormat::BGR24 => 3};
                        let offset = (*input).stride as isize * y1 as isize + bytes_pp * x1 as isize;
                        (*input).pixels = (*input).pixels.offset(offset);
                        (*input).w = x2 - x1;
                        (*input).h = y2 - y1;
                        println!("Changing pointer by {}, w{}, h{}", offset, (*input).w, (*input).h);


                        ctx.weight_mut(ix).result = NodeResult::Frame(input);
                        ctx.first_parent_input_weight_mut(ix).unwrap().result = NodeResult::Consumed;
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

fn crop_def() -> NodeDefinition {
    NodeDefinition {
        id: NodeType::Crop,
        name: "crop",
        inbound_edges: EdgesIn::OneInput,
        fn_estimate:  Some(crop_size),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                let mut new_nodes = Vec::with_capacity(2);
                if ctx.has_other_children(ctx.first_parent_input(ix).unwrap(), ix) {
                    new_nodes.push(Node::new(&CLONE, NodeParams::None));
                }
                new_nodes.push(Node::new(&CROP_MUTATE, NodeParams::Json(ctx.get_json_params(ix).unwrap())));
                ctx.replace_node(ix, new_nodes);
            }
            f
        }),
        .. Default::default()
    }
}
lazy_static! {
    pub static ref CLONE: NodeDefinition = clone_def();
    pub static ref CROP_MUTATE: NodeDefinition = crop_mutate_def();
    pub static ref CROP: NodeDefinition = crop_def();
    pub static ref EXPAND_CANVAS: NodeDefinition = expand_canvas_def();

    pub static ref COPY_RECT: NodeDefinition = copy_rect_def();
    pub static ref FILL_RECT: NodeDefinition = fill_rect_def();
}
