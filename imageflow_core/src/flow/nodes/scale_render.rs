extern crate imageflow_serde as s;
use daggy::{Dag, EdgeIndex, NodeIndex};
use ffi;
use ffi::{Context, Job, NodeType, EdgeKind};
use flow::definitions::*;
use flow::graph::Graph;
use petgraph;
use super::*;
use super::NodeDefHelpers;

struct ScaleRenderHelpers {}
impl ScaleRenderHelpers {

    fn scale_size_but_input_format(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

        let input_info = ctx.first_parent_frame_info_some(ix).unwrap();

        let ref mut weight = ctx.weight_mut(ix);
        match weight.params{
            NodeParams::Json(s::Node::Scale{ref  w, ref h, ..}) => {
                weight.frame_est = FrameEstimate::Some(
                    FrameInfo{
                        w: *w as i32,
                        h: *h as i32,
                        fmt: ffi::PixelFormat::from(input_info.fmt),
                        alpha_meaningful: input_info.alpha_meaningful});
            },
            _ => { panic!("Node params missing");}
        }
    }

    fn render1d_size(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

        let input_info = ctx.first_parent_frame_info_some(ix).unwrap();

        let ref mut weight = ctx.weight_mut(ix);
        match weight.params{
            NodeParams::Json(s::Node::Render1D{ref  scale_to_width, ref transpose_on_write, ref interpolation_filter}) => {
                let w = match *transpose_on_write { true => input_info.h, false => *scale_to_width as i32};
                let h = match *transpose_on_write { true => *scale_to_width as i32, false => input_info.h};

                weight.frame_est = FrameEstimate::Some(
                FrameInfo{
                        w: w as i32,
                        h: h as i32,
                        fmt: ffi::PixelFormat::from(input_info.fmt),
                        alpha_meaningful: input_info.alpha_meaningful});
            },
            _ => { panic!("Node params missing");}
        }
    }

}

fn scale_def() -> NodeDefinition {
    NodeDefinition {
        id: NodeType::Scale,
        name: "scale",
        inbound_edges: EdgesIn::OneInput,
        description: "scale",
        fn_estimate: Some(ScaleRenderHelpers::scale_size_but_input_format),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                let input = ctx.first_parent_frame_info_some(ix).unwrap();

                if let s::Node::Scale {w, h, down_filter, up_filter, sharpen_percent, flags } =
                ctx.get_json_params(ix).unwrap() {

                    let filter =  if input.w < w as i32 || input.h < h as i32 { up_filter }
                        else { down_filter};

                    match flags {
                        Some(1) => {
                            let canvas_params = s::Node::CreateCanvas{w: w as usize, h: h as usize, format: s::PixelFormat::from(input.fmt), color: s::Color::Transparent };
                            //TODO: Not the right params!
                            let scale2d_params = s::Node::Scale{ w: w, h: h, up_filter: up_filter, down_filter: down_filter, flags: Some(1), sharpen_percent: None};
                            let canvas = ctx.graph.add_node(Node::new(&CREATE_CANVAS, NodeParams::Json(canvas_params)));
                            let scale2d = ctx.graph.add_node(Node::new(&SCALE_2D_RENDER_TO_CANVAS_1D, NodeParams::Json(scale2d_params)));
                            ctx.graph.add_edge(canvas, scale2d, EdgeKind::Canvas).unwrap();
                            ctx.replace_node_with_existing(ix, scale2d);
                        },
                        _ => {

                            let scalew_params = s::Node::Render1D{ scale_to_width: w, interpolation_filter: filter, transpose_on_write: true };
                            let scaleh_params = s::Node::Render1D{ scale_to_width: h, interpolation_filter: filter, transpose_on_write: true };
                            let scalew = Node::new(&SCALE_1D, NodeParams::Json(scalew_params));
                            let scaleh = Node::new(&SCALE_1D, NodeParams::Json(scaleh_params));
                            ctx.replace_node(ix, vec![scalew,scaleh]);
                        }
                    }

                }

            }
            f
        }),
        .. Default::default()
    }
}

fn render1d_def() -> NodeDefinition {
    NodeDefinition {
        id: NodeType::Render1D,
        name: "render1d",
        inbound_edges: EdgesIn::OneInput,
        fn_estimate: Some(ScaleRenderHelpers::render1d_size),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {

                if let FrameEstimate::Some(est) = ctx.weight(ix).frame_est {
                    let canvas_params = s::Node::CreateCanvas { w: est.w as usize, h: est.h as usize, format: s::PixelFormat::from(est.fmt), color: s::Color::Transparent };
                    let render1d_params = ctx.get_json_params(ix).unwrap();
                    let canvas = ctx.graph.add_node(Node::new(&CREATE_CANVAS, NodeParams::Json(canvas_params)));
                    let scale1d = ctx.graph.add_node(Node::new(&SCALE_1D_TO_CANVAS_1D, NodeParams::Json(render1d_params)));
                    ctx.graph.add_edge(canvas, scale1d, EdgeKind::Canvas).unwrap();
                    ctx.replace_node_with_existing(ix, scale1d);
                }else{
                    panic!("");
                }
            }
            f
        }),
        ..Default::default()
    }
}
fn render1d_to_canvas_def() -> NodeDefinition {
    NodeDefinition {
        id: NodeType::primitive_RenderToCanvas1D,
        name: "render1d_p",
        inbound_edges: EdgesIn::OneInputOneCanvas,
        fn_estimate: Some(ScaleRenderHelpers::render1d_size),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                if let s::Node::Render1D{ref  scale_to_width, ref transpose_on_write, ref interpolation_filter} =
                ctx.get_json_params(ix).unwrap() {
                    let input = ctx.first_parent_result_frame(ix, EdgeKind::Input).unwrap();
                    let canvas = ctx.first_parent_result_frame(ix, EdgeKind::Canvas).unwrap();

                    unsafe {

                        //                        if (*canvas).w as usize != w || (*canvas).h as usize != h {
                        //                            panic!("Inconsistent dimensions between {:?} and {:?}", ctx.get_json_params(ix).unwrap(), *canvas);
                        //                        }


                        //                        let picked_filter = if w > (*input).w as usize || h > (*input).h as usize {up_filter} else {down_filter};


                        let ffi_struct = ffi::RenderToCanvas1d{
                            interpolation_filter: ffi::Filter::from((*interpolation_filter).unwrap_or(s::Filter::Robidoux)),
                            scale_to_width: *scale_to_width as i32,
                            transpose_on_write: *transpose_on_write,

                        };


                        if !::ffi::flow_node_execute_render_to_canvas_1d(ctx.c,
                                                                         input, canvas, &ffi_struct as *const ffi::RenderToCanvas1d) {
                            //ctx.c.assert_ok();

                            ::ContextPtr::from_ptr(ctx.c).assert_ok(Some(ctx.graph));
                            panic!("TODO: print context error");
                        }
                    }

                    ctx.weight_mut(ix).result = NodeResult::Frame(canvas);
                    //TODO: consume canvas, if we mutated it
                }else{
                    panic!("Invalid params {:?}", ctx.get_json_params(ix));
                }
            }
            f
        }),
        .. Default::default()
    }
}
fn scale2d_render_def() ->NodeDefinition{
    NodeDefinition {
        id: NodeType::primitive_Scale2D_RenderToCanvas1D,
        name: "scale2d_p",
        inbound_edges: EdgesIn::OneInputOneCanvas,
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_canvas),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                if let s::Node::Scale {w, h, down_filter, up_filter, sharpen_percent, flags } =
                ctx.get_json_params(ix).unwrap() {
                    let input = ctx.first_parent_result_frame(ix, EdgeKind::Input).unwrap();
                    let canvas = ctx.first_parent_result_frame(ix, EdgeKind::Canvas).unwrap();

                    unsafe {

                        if (*canvas).w as usize != w || (*canvas).h as usize != h {
                            panic!("Inconsistent dimensions between {:?} and {:?}", ctx.get_json_params(ix).unwrap(), *canvas);
                        }


                        let picked_filter = if w > (*input).w as usize || h > (*input).h as usize {up_filter} else {down_filter};


                        let ffi_struct = ffi::Scale2dRenderToCanvas1d{
                            interpolation_filter: ffi::Filter::from(picked_filter.unwrap_or(s::Filter::Robidoux)),
                            scale_to_width: w as i32,
                            scale_to_height: h as i32,
                            sharpen_percent_goal: sharpen_percent.unwrap_or(0f32),
                            scale_in_colorspace: ffi::Floatspace::linear,
                        };


                        if !::ffi::flow_node_execute_scale2d_render1d(ctx.c,
                                                                      input, canvas, &ffi_struct as *const ffi::Scale2dRenderToCanvas1d) {
                            //ctx.c.assert_ok();

                            ::ContextPtr::from_ptr(ctx.c).assert_ok(Some(ctx.graph));
                            panic!("TODO: print context error");
                        }
                    }

                    ctx.weight_mut(ix).result = NodeResult::Frame(canvas);
                    //TODO: consume canvas, if we mutated it
                }else{
                    panic!("Invalid params {:?}", ctx.get_json_params(ix));
                }
            }
            f
        }),
        .. Default::default()
    }
}

lazy_static! {
    pub static ref SCALE: NodeDefinition = scale_def();
    pub static ref SCALE_1D: NodeDefinition = render1d_def();
    pub static ref SCALE_1D_TO_CANVAS_1D: NodeDefinition = render1d_to_canvas_def();
    pub static ref SCALE_2D_RENDER_TO_CANVAS_1D: NodeDefinition = scale2d_render_def();
}