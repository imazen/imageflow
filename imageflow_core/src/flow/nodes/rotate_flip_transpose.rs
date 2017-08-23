use super::internal_prelude::*;

fn apply_orientation_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.apply_orientation",
        name: "Apply orientation",
        fn_estimate: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                NodeDefHelpers::copy_frame_est_from_first_input(ctx, ix);
                let weight = &mut ctx.weight_mut(ix);
                match weight.params {
                    NodeParams::Json(s::Node::ApplyOrientation { ref flag }) => {
                        let swap = *flag >= 5 && *flag <= 8;
                        if let FrameEstimate::Some(frame_info) = weight.frame_est {
                            weight.frame_est = FrameEstimate::Some(FrameInfo {
                                w: if swap {
                                    frame_info.h
                                } else { frame_info.w },
                                h: if swap {
                                    frame_info.w
                                } else { frame_info.h },
                                ..frame_info
                            });
                        }
                    }
                    _ => {
                        panic!("Node params missing");
                    }
                }
            }
            f
        }),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                if let NodeParams::Json(s::Node::ApplyOrientation { flag }) = ctx.weight(ix)
                    .params {
                    let replacement_nodes: Vec<&NodeDefinition> = match flag {
                        7 => vec![&ROTATE_180, &TRANSPOSE],
                        8 => vec![&ROTATE_90],
                        6 => vec![&ROTATE_270],
                        5 => vec![&TRANSPOSE],
                        4 => vec![&FLIP_V],
                        3 => vec![&ROTATE_180],
                        2 => vec![&FLIP_H],
                        _ => vec![],
                    };
                    ctx.replace_node(ix,
                                     replacement_nodes.iter()
                                         .map(|v| Node::new(v, NodeParams::None))
                                         .collect());
                } else {
                    panic!("");
                }
            }
            f
        }),
        ..Default::default()
    }
}
fn transpose_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.transpose",
        name: "Transpose",
        fn_estimate: Some(NodeDefHelpers::rotate_frame_info),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                match ctx.first_parent_input_weight(ix).unwrap().frame_est {
                    FrameEstimate::Some(FrameInfo { w, h, fmt, alpha_meaningful }) => {
                            let canvas_params = s::Node::CreateCanvas {
                                w: h as usize,
                                h: w as usize,
                                format: s::PixelFormat::from(fmt),
                                color: s::Color::Transparent,
                            };
                            let canvas = ctx.graph
                                .add_node(Node::n(&CREATE_CANVAS,
                                                    NodeParams::Json(canvas_params)));
                            let copy = ctx.graph
                                .add_node(Node::new(&TRANSPOSE_MUT, NodeParams::None));
                            ctx.graph.add_edge(canvas, copy, EdgeKind::Canvas).unwrap();
                            ctx.replace_node_with_existing(ix, copy);

                    }
                    _ => panic!(""),
                }
            }
            f
        }),

        ..Default::default()
    }
}

fn transpose_mut_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.transpose_to_canvas",
        name: "transpose_to_canvas",
        inbound_edges: EdgesIn::OneInputOneCanvas,
        description: "Transpose To",
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_canvas),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                let input: *mut ::ffi::BitmapBgra =
                    ctx.first_parent_result_frame(ix, EdgeKind::Input).unwrap();
                let canvas: *mut ::ffi::BitmapBgra =
                    ctx.first_parent_result_frame(ix, EdgeKind::Canvas).unwrap();

                unsafe {
                    if (*input).fmt != (*canvas).fmt {
                        panic!("Can't copy between bitmaps with different pixel formats")
                    }
                    if input == canvas {
                        panic!("Canvas and input must be different bitmaps for transpose to work!")
                    }

                    if !::ffi::flow_bitmap_bgra_transpose(ctx.flow_c(), input, canvas) {
                        panic!("Failed to transpose bitmap")
                    }

                    ctx.weight_mut(ix).result = NodeResult::Frame(canvas);
                }
            }
            f
        }),
        ..Default::default()
    }
}
fn no_op_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.noop",
        name: "NoOp",
        description: "Does nothing; pass-through node",
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_input),
        fn_flatten_pre_optimize: Some(NodeDefHelpers::delete_node_and_snap_together),
        ..Default::default()
    }
}
fn flip_v_p_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.flip_vertical_mutate",
        name: "FlipVPrimitive",
        description: "Flip frame vertical",
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_input),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                let from_node = ctx.first_parent_input_weight(ix).unwrap().clone();
                match from_node.result {
                    NodeResult::Frame(bitmap) => {
                        unsafe {
                            ::ffi::flow_bitmap_bgra_flip_vertical(ctx.flow_c(), bitmap);
                        }
                        ctx.weight_mut(ix).result = NodeResult::Frame(bitmap);
                        ctx.first_parent_input_weight_mut(ix).unwrap().result =
                            NodeResult::Consumed;
                    }
                    _ => {
                        panic!{"Previous node not ready"}
                    }
                }
            }
            f
        }),
        ..Default::default()
    }
}
fn flip_h_p_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.flip_horizontal_mutate",
        name: "FlipHPrimitive",
        description: "Flip frame horizontal",
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_input),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                let from_node = ctx.first_parent_input_weight(ix).unwrap().clone();
                match from_node.result {
                    NodeResult::Frame(bitmap) => {
                        unsafe {
                            ::ffi::flow_bitmap_bgra_flip_horizontal(ctx.flow_c(), bitmap);
                        }
                        ctx.weight_mut(ix).result = NodeResult::Frame(bitmap);
                        ctx.first_parent_input_weight_mut(ix).unwrap().result =
                            NodeResult::Consumed;
                    }
                    _ => {
                        panic!{"Previous node not ready"}
                    }
                }
            }
            f
        }),
        ..Default::default()
    }
}
fn flip_v_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.flipv",
        name: "FlipV",
        description: "Flip frame vertical",
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_input),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                let mut new_nodes = Vec::with_capacity(2);
                if ctx.has_other_children(ctx.first_parent_input(ix).unwrap(), ix) {
                    new_nodes.push(Node::new(&CLONE, NodeParams::None));
                }
                new_nodes.push(Node::new(&FLIP_V_PRIMITIVE, NodeParams::None));
                ctx.replace_node(ix, new_nodes);
            }
            f
        }),
        ..Default::default()
    }
}
fn flip_h_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.fliph",
        name: "FlipH",
        description: "Flip frame horizontal",
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_input),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                let mut new_nodes = Vec::with_capacity(2);
                if ctx.has_other_children(ctx.first_parent_input(ix).unwrap(), ix) {
                    new_nodes.push(Node::new(&CLONE, NodeParams::None));
                }
                new_nodes.push(Node::new(&FLIP_H_PRIMITIVE, NodeParams::None));
                ctx.replace_node(ix, new_nodes);
            }
            f
        }),

        ..Default::default()
    }
}
fn rotate90_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.rot90",
        name: "Rot90",
        fn_estimate: Some(NodeDefHelpers::rotate_frame_info),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                ctx.replace_node(ix,
                                 vec![
                Node::new(&TRANSPOSE, NodeParams::None),
                Node::new(&FLIP_V, NodeParams::None),
                ]);
            }
            f
        }),
        ..Default::default()
    }
}
fn rotate180_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.rot180",
        name: "Rot180",
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_input),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                ctx.replace_node(ix,
                                 vec![
                Node::new(&FLIP_V, NodeParams::None),
                Node::new(&FLIP_H, NodeParams::None),
                ]);
            }
            f
        }),
        ..Default::default()
    }
}

fn rotate270_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.rot270",
        name: "Rot270",
        fn_estimate: Some(NodeDefHelpers::rotate_frame_info),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                ctx.replace_node(ix,
                                 vec![
                Node::new(&FLIP_V, NodeParams::None),
                Node::new(&TRANSPOSE, NodeParams::None),
                ]);
            }
            f
        }),
        ..Default::default()
    }
}
lazy_static! {
    pub static ref NO_OP: NodeDefinition = no_op_def();

   pub static ref FLIP_V_PRIMITIVE: NodeDefinition = flip_v_p_def() ;
    pub static ref FLIP_V: NodeDefinition = flip_v_def();
     pub static ref FLIP_H_PRIMITIVE: NodeDefinition = flip_h_p_def();
    pub static ref FLIP_H: NodeDefinition = flip_h_def();
    pub static ref ROTATE_90: NodeDefinition = rotate90_def();
     pub static ref ROTATE_180: NodeDefinition = rotate180_def();
    pub static ref ROTATE_270: NodeDefinition = rotate270_def();
    pub static ref APPLY_ORIENTATION: NodeDefinition = apply_orientation_def();

    pub static ref TRANSPOSE: NodeDefinition = transpose_def();

    pub static ref TRANSPOSE_MUT: NodeDefinition = transpose_mut_def();
}
