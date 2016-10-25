extern crate imageflow_serde as s;
use daggy::{Dag, EdgeIndex, NodeIndex};
use ffi;
use ffi::{Context, Job, NodeType, EdgeKind};
use flow::definitions::*;
use flow::graph::Graph;
use petgraph;
use super::*;
use super::NodeDefHelpers;


fn apply_orientation_def() -> NodeDefinition {
    NodeDefinition {
        id: NodeType::Apply_Orientation,
        name: "Apply orientation",
        fn_estimate: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                NodeDefHelpers::copy_frame_est_from_first_input(ctx, ix);
                let ref mut weight = ctx.weight_mut(ix);
                match weight.params {
                    NodeParams::Json(s::Node::ApplyOrientation { ref flag }) => {
                        let swap = *flag >= 5 && *flag <= 8;
                        if let FrameEstimate::Some(frame_info) = weight.frame_est {
                            weight.frame_est = FrameEstimate::Some(FrameInfo {
                                w: match swap {
                                    true => frame_info.h,
                                    _ => frame_info.w
                                },
                                h: match swap {
                                    true => frame_info.w,
                                    _ => frame_info.h
                                },
                                ..frame_info
                            });
                        }
                    },
                    _ => { panic!("Node params missing"); }
                }
            }
            f
        }),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                if let NodeParams::Json(s::Node::ApplyOrientation { flag }) = ctx.weight(ix).params {
                    let replacement_nodes: Vec<&NodeDefinition> = match flag {
                        7 => vec![&ROTATE_180, &TRANSPOSE],
                        8 => vec![&ROTATE_90],
                        6 => vec![&ROTATE_270],
                        5 => vec![&TRANSPOSE],
                        4 => vec![&FLIP_V],
                        3 => vec![&ROTATE_180],
                        2 => vec![&FLIP_H],
                        _ => vec![]
                    };
                    ctx.replace_node(ix, replacement_nodes.iter().map(|v| Node::new(v, NodeParams::None)).collect());
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
        id: NodeType::Transpose,
        name: "Transpose",
        fn_estimate: Some(NodeDefHelpers::rotate_frame_info),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                if let FrameEstimate::Some(FrameInfo{h, ..}) = ctx.weight(ix).frame_est {
                    //TODO: Shouldn't the filter be triangle, or (better) not be a filter at all?
                    let scale_params = s::Node::Render1D{ scale_to_width: h as usize, interpolation_filter: Some(s::Filter::Robidoux), transpose_on_write: true };
                    ctx.replace_node(ix, vec![
                        Node::new(&SCALE_1D, NodeParams::Json(scale_params)),
                    ]);
                }else{
                    panic!("");
                }
            }
            f
        }),
        .. Default::default()
    }
}


lazy_static! {
    pub static ref NO_OP: NodeDefinition = NodeDefinition {
        id: NodeType::Noop,
        name: "NoOp",
        description: "Does nothing; pass-through node",
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_input),
        fn_flatten_pre_optimize: Some(NodeDefHelpers::delete_node_and_snap_together),
        .. Default::default()};



   pub static ref FLIP_V_PRIMITIVE: NodeDefinition = NodeDefinition {
        id: NodeType::primitive_Flip_Vertical_Mutate,
        name: "FlipVPrimitive",
        description: "Flip frame vertical",
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_input),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                let from_node = ctx.first_parent_input_weight(ix).unwrap().clone();
                match from_node.result {
                    NodeResult::Frame(bitmap) => {
                        unsafe {::ffi::flow_bitmap_bgra_flip_vertical(ctx.c, bitmap); }
                        ctx.weight_mut(ix).result = NodeResult::Frame(bitmap);
                        ctx.first_parent_input_weight_mut(ix).unwrap().result = NodeResult::Consumed;
                    }
                    _ => {panic!{"Previous node not ready"}}
                }
            }
            f
        }),
        .. Default::default()
    };
    pub static ref FLIP_V: NodeDefinition = NodeDefinition {
        id: NodeType::Flip_Vertical,
        name: "FlipV",
        description: "Flip frame vertical",
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_input),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                let mut new_nodes = Vec::with_capacity(2);
                if ctx.has_other_children(ctx.first_parent_input(ix).unwrap(), ix) {
                    new_nodes.push(Node::new(&CLONE, NodeParams::None));
                }
                new_nodes.push(Node::new(&FLIP_V_PRIMITIVE, NodeParams::None));
                ctx.replace_node(ix, new_nodes);
            }
            f
        }),
        .. Default::default()
    };
     pub static ref FLIP_H_PRIMITIVE: NodeDefinition = NodeDefinition {
        id: NodeType::primitive_Flip_Horizontal_Mutate,
        name: "FlipHPrimitive",
        description: "Flip frame horizontal",
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_input),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                let from_node = ctx.first_parent_input_weight(ix).unwrap().clone();
                match from_node.result {
                    NodeResult::Frame(bitmap) => {
                        unsafe {::ffi::flow_bitmap_bgra_flip_horizontal(ctx.c, bitmap); }
                        ctx.weight_mut(ix).result = NodeResult::Frame(bitmap);
                        ctx.first_parent_input_weight_mut(ix).unwrap().result = NodeResult::Consumed;
                    }
                    _ => {panic!{"Previous node not ready"}}
                }
            }
            f
        }),
        .. Default::default()
    };
    pub static ref FLIP_H: NodeDefinition = NodeDefinition {
        id: NodeType::Flip_Horizontal,
        name: "FlipH",
        description: "Flip frame horizontal",
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_input),
         fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                let mut new_nodes = Vec::with_capacity(2);
                if ctx.has_other_children(ctx.first_parent_input(ix).unwrap(), ix) {
                    new_nodes.push(Node::new(&CLONE, NodeParams::None));
                }
                new_nodes.push(Node::new(&FLIP_H_PRIMITIVE, NodeParams::None));
                ctx.replace_node(ix, new_nodes);
            }
            f
        }),

        .. Default::default()
    };
    pub static ref ROTATE_90: NodeDefinition = NodeDefinition {
        id: NodeType::Rotate_90,
        name: "Rot90",
        description: "Rotate",
        fn_estimate: Some(NodeDefHelpers::rotate_frame_info),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                ctx.replace_node(ix, vec![
                    Node::new(&TRANSPOSE, NodeParams::None),
                    Node::new(&FLIP_V, NodeParams::None),
                ]);
            }
            f
        }),
        .. Default::default()
    };
     pub static ref ROTATE_180: NodeDefinition = NodeDefinition {
        id: NodeType::Rotate_180,
        name: "Rot180",
        description: "Rotate",
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_input),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                ctx.replace_node(ix, vec![
                    Node::new(&FLIP_V, NodeParams::None),
                    Node::new(&FLIP_H, NodeParams::None),
                ]);
            }
            f
        }),
        .. Default::default()
    };
    pub static ref ROTATE_270: NodeDefinition = NodeDefinition {
        id: NodeType::Rotate_270,
        name: "Rot270",
        fn_estimate: Some(NodeDefHelpers::rotate_frame_info),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                ctx.replace_node(ix, vec![
                    Node::new(&FLIP_V, NodeParams::None),
                    Node::new(&TRANSPOSE, NodeParams::None),
                ]);
            }
            f
        }),
        .. Default::default()
    };
    pub static ref APPLY_ORIENTATION: NodeDefinition = apply_orientation_def();

    pub static ref TRANSPOSE: NodeDefinition = transpose_def();
}
