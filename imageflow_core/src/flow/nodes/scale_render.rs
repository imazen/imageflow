use super::internal_prelude::*;


lazy_static! {
    pub static ref SCALE: NodeDefinition = scale_def();
    pub static ref SCALE_1D: NodeDefinition = render1d_def();
    pub static ref SCALE_1D_TO_CANVAS_1D: NodeDefinition = render1d_to_canvas_def();
    pub static ref SCALE_2D_RENDER_TO_CANVAS_1D: NodeDefinition = scale2d_render_def();
}


struct ScaleRenderHelpers {}
impl ScaleRenderHelpers {
    fn scale_size_but_input_format(ctx: &mut OpCtxMut, ix: NodeIndex) {

        let input_info = ctx.first_parent_frame_info_some(ix).unwrap();

        let weight = &mut ctx.weight_mut(ix);
        match weight.params {
            NodeParams::Json(s::Node::Resample2D { ref w, ref h, .. }) => {
                weight.frame_est = FrameEstimate::Some(FrameInfo {
                    w: *w as i32,
                    h: *h as i32,
                    fmt: ffi::PixelFormat::from(input_info.fmt),
                });
            }
            _ => {
                panic!("Node params missing");
            }
        }
    }

    fn render1d_size(ctx: &mut OpCtxMut, ix: NodeIndex) {

        let input_info = ctx.first_parent_frame_info_some(ix).unwrap();

        let weight = &mut ctx.weight_mut(ix);
        match weight.params {
            NodeParams::Json(s::Node::Resample1D { ref scale_to_width,
                                                   ref transpose_on_write,
                                                   ref interpolation_filter, .. }) => {
                let w = if *transpose_on_write {
                    input_info.h
                } else {
                    *scale_to_width as i32
                };
                let h = if *transpose_on_write {
                    *scale_to_width as i32
                } else {
                    input_info.h
                };

                weight.frame_est = FrameEstimate::Some(FrameInfo {
                    w: w as i32,
                    h: h as i32,
                    fmt: ffi::PixelFormat::from(input_info.fmt),
                });
            }
            _ => {
                panic!("Node params missing");
            }
        }
    }
}

fn scale_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.scale",
        name: "scale",
        inbound_edges: EdgesIn::OneInput,
        description: "scale",
        fn_estimate: Some(ScaleRenderHelpers::scale_size_but_input_format),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex) {
                let input = ctx.first_parent_frame_info_some(ix).unwrap();

                if let s::Node::Resample2D { w, h, down_filter, up_filter, scaling_colorspace, hints } =
                       ctx.get_json_params(ix).unwrap() {
                    let filter = if input.w < w as i32 || input.h < h as i32 {
                        up_filter
                    } else {
                        down_filter
                    };

                    let old_style = if let Some(s::ResampleHints {
                                                    prefer_1d_twice,
                                                    sharpen_percent
                                                }) = hints {
                        prefer_1d_twice == Some(true)
                    } else {
                        false
                    };
                    if !old_style {
                        let canvas_params = s::Node::CreateCanvas {
                            w: w as usize,
                            h: h as usize,
                            format: s::PixelFormat::from(input.fmt),
                            color: s::Color::Transparent,
                        };
                        // TODO: Not the right params!
                        let scale2d_params = s::Node::Resample2D {
                            w: w,
                            h: h,
                            up_filter: up_filter,
                            down_filter: down_filter,
                            scaling_colorspace: scaling_colorspace,
                            hints: hints,
                        };
                        let canvas = ctx.graph
                            .add_node(Node::n(&CREATE_CANVAS,
                                                NodeParams::Json(canvas_params)));
                        let scale2d = ctx.graph
                            .add_node(Node::new(&SCALE_2D_RENDER_TO_CANVAS_1D,
                                                NodeParams::Json(scale2d_params)));
                        ctx.graph.add_edge(canvas, scale2d, EdgeKind::Canvas).unwrap();
                        ctx.replace_node_with_existing(ix, scale2d);
                    } else {
                        let scalew_params = s::Node::Resample1D {
                            scale_to_width: w,
                            interpolation_filter: filter,
                            transpose_on_write: true,
                            scaling_colorspace: scaling_colorspace
                        };
                        let scaleh_params = s::Node::Resample1D {
                            scale_to_width: h,
                            interpolation_filter: filter,
                            transpose_on_write: true,

                            scaling_colorspace: scaling_colorspace
                        };
                        let scalew = Node::new(&SCALE_1D, NodeParams::Json(scalew_params));
                        let scaleh = Node::new(&SCALE_1D, NodeParams::Json(scaleh_params));
                        ctx.replace_node(ix, vec![scalew, scaleh]);
                    }
                }

            }
            f
        }),
        ..Default::default()
    }
}

fn render1d_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.render1d",
        name: "render1d",
        inbound_edges: EdgesIn::OneInput,
        fn_estimate: Some(ScaleRenderHelpers::render1d_size),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex) {

                if let FrameEstimate::Some(est) = ctx.weight(ix).frame_est {
                    let canvas_params = s::Node::CreateCanvas {
                        w: est.w as usize,
                        h: est.h as usize,
                        format: s::PixelFormat::from(est.fmt),
                        color: s::Color::Transparent,
                    };
                    let render1d_params = ctx.get_json_params(ix).unwrap();
                    let canvas = ctx.graph
                        .add_node(Node::n(&CREATE_CANVAS, NodeParams::Json(canvas_params)));
                    let scale1d = ctx.graph.add_node(Node::new(&SCALE_1D_TO_CANVAS_1D,
                                                               NodeParams::Json(render1d_params)));
                    ctx.graph.add_edge(canvas, scale1d, EdgeKind::Canvas).unwrap();
                    ctx.replace_node_with_existing(ix, scale1d);
                } else {
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
        fqn: "imazen.render1d_to_canvas",
        name: "render1d_p",
        inbound_edges: EdgesIn::OneInputOneCanvas,
        fn_estimate: Some(ScaleRenderHelpers::render1d_size),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex) {
                if let s::Node::Resample1D { ref scale_to_width,
                                             ref transpose_on_write,
                                             ref interpolation_filter,
                                             ref scaling_colorspace } = ctx.get_json_params(ix)
                    .unwrap() {
                    let input = ctx.first_parent_result_frame(ix, EdgeKind::Input).unwrap();
                    let canvas = ctx.first_parent_result_frame(ix, EdgeKind::Canvas).unwrap();

                    unsafe {

                        //                        if (*canvas).w as usize != w || (*canvas).h as usize != h {
                        //                            panic!("Inconsistent dimensions between {:?} and {:?}", ctx.get_json_params(ix).unwrap(), *canvas);
                        //                        }


                        //                        let picked_filter = if w > (*input).w as usize || h > (*input).h as usize {up_filter} else {down_filter};

                        let downscaling = *scale_to_width < (*input).w as usize;
                        let default_colorspace = ffi::Floatspace::Linear; // if downscaling { ffi::Floatspace::Linear} else {ffi::Floatspace::Srgb}

                        let ffi_struct = ffi::RenderToCanvas1d {
                            interpolation_filter: ffi::Filter::from((*interpolation_filter)
                                .unwrap_or(s::Filter::Robidoux)),
                            scale_to_width: *scale_to_width as i32,
                            transpose_on_write: *transpose_on_write,
                            scale_in_colorspace: match *scaling_colorspace {
                                Some(s::ScalingFloatspace::Srgb) => ffi::Floatspace::Srgb,
                                Some(s::ScalingFloatspace::Linear) => ffi::Floatspace::Linear,
                                _ => default_colorspace
                            }
                        };


                        if !::ffi::flow_node_execute_render_to_canvas_1d(ctx.flow_c(),
                                                                         input, canvas, &ffi_struct as *const ffi::RenderToCanvas1d) {
                            cerror!(ctx.c).panic();
                        }
                    }

                    ctx.weight_mut(ix).result = NodeResult::Frame(canvas);
                    // TODO: consume canvas, if we mutated it
                } else {
                    panic!("Invalid params {:?}", ctx.get_json_params(ix));
                }
            }
            f
        }),
        ..Default::default()
    }
}
fn scale2d_render_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.scale2d_render_to_canvas_1d",
        name: "scale2d_p",
        inbound_edges: EdgesIn::OneInputOneCanvas,
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_canvas),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex) {
                if let s::Node::Resample2D { w, h, down_filter, up_filter, hints, scaling_colorspace } =
                       ctx.get_json_params(ix).unwrap() {
                    let input = ctx.first_parent_result_frame(ix, EdgeKind::Input).unwrap();
                    let canvas = ctx.first_parent_result_frame(ix, EdgeKind::Canvas).unwrap();

                    unsafe {

                        if (*canvas).w as usize != w || (*canvas).h as usize != h {
                            panic!("Inconsistent dimensions between {:?} and {:?}",
                                   ctx.get_json_params(ix).unwrap(),
                                   *canvas);
                        }

                        let upscaling = w > (*input).w as usize || h > (*input).h as usize;
                        let downscaling = w < (*input).w as usize || h < (*input).h as usize;

                        let picked_filter = if w > (*input).w as usize || h > (*input).h as usize {
                            up_filter
                        } else {
                            down_filter
                        };

                        let sharpen_percent = hints.and_then(|h| h.sharpen_percent);

                        let default_colorspace = ffi::Floatspace::Linear; //  if downscaling { ffi::Floatspace::Linear} else {ffi::Floatspace::Srgb}

                        let ffi_struct = ffi::Scale2dRenderToCanvas1d {
                            interpolation_filter:
                                ffi::Filter::from(picked_filter.unwrap_or(s::Filter::Robidoux)),
                            scale_to_width: w as i32,
                            scale_to_height: h as i32,
                            sharpen_percent_goal: sharpen_percent.unwrap_or(0f32),
                            scale_in_colorspace:  match scaling_colorspace {
                                Some(s::ScalingFloatspace::Srgb) => ffi::Floatspace::Srgb,
                                Some(s::ScalingFloatspace::Linear) => ffi::Floatspace::Linear,
                                _ => default_colorspace
                            }
                        };


                        if !::ffi::flow_node_execute_scale2d_render1d(ctx.flow_c(),
                                                                      input, canvas, &ffi_struct as *const ffi::Scale2dRenderToCanvas1d) {
                            cerror!(ctx.c).panic();
                        }
                    }

                    ctx.weight_mut(ix).result = NodeResult::Frame(canvas);
                    // TODO: consume canvas, if we mutated it
                } else {
                    panic!("Invalid params {:?}", ctx.get_json_params(ix));
                }
            }
            f
        }),
        ..Default::default()
    }
}
