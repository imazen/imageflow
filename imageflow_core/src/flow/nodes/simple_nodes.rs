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
    pub static ref APPLY_ORIENTATION: NodeDefinition = NodeDefinition {
        id: NodeType::Apply_Orientation,
        name: "Apply orientation",
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_input),
        fn_flatten_pre_optimize: Some(NodeDefHelpers::delete_node_and_snap_together),
        .. Default::default()
    };

    pub static ref TRANSPOSE: NodeDefinition = NodeDefinition {
        id: NodeType::Transpose,
        name: "Transpose",
        fn_estimate: Some(NodeDefHelpers::rotate_frame_info),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

            }
            f
        }),
        .. Default::default()
    };

    //TODO: Render1D
    //TODO: APPLY_ORIENTATION
    //RENDER2d
    //BitmapBgra
    //Encoder
    //Decoder
    //Crop
    //Fill
    //Expand


}
