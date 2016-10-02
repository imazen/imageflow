use flow::graph::Graph;
use ffi::{Context,Job,NodeType};
use daggy::{Dag,EdgeIndex,NodeIndex};
use flow::definitions::*;
use petgraph;

impl OptCtxMut {
    fn replace_node(index: NodeIndex<u32>, with: Node){

    }
}
struct Helpers{}
impl Helpers{

    fn preserve_frame_info(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
        //TODO: select by EdgeKind=Input
        let input = ctx.graph.graph().neighbors_directed(ix, petgraph::EdgeDirection::Incoming).nth(0);
        match input{
            Some(input_ix) => {
                ctx.graph.node_weight_mut(ix).unwrap().frame_est = ctx.graph.node_weight(input_ix).unwrap().frame_est.clone();
            }
            None => {}
        }

    }
    fn rotate_frame_info(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
        //TODO: select by EdgeKind=Input
        let input = ctx.graph.graph().neighbors_directed(ix, petgraph::EdgeDirection::Incoming).nth(0);
        match input{
            Some(input_ix) => {
                let input_est = ctx.graph.node_weight(input_ix).unwrap().frame_est.clone();
                let mut w = ctx.graph.node_weight_mut(ix).unwrap();
                w.frame_est = match input_est{
                    FrameEstimate::Some(info) => FrameEstimate::Some(FrameInfo{w: info.h, h: info.w, .. info}),
                    FrameEstimate::UpperBound(info) => FrameEstimate::UpperBound(FrameInfo{w: info.h, h: info.w, .. info}),
                    other => other
                };
            }
            None => {}
        }
    }
    fn flatten_flip_v(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
        //ctx.graph.node_weight_mut(ix).unwrap()
    }

    fn delete_self_connect_outputs(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
        //Prefer EdgeKind=Input
        let input = ctx.graph.graph().neighbors_directed(ix, petgraph::EdgeDirection::Incoming).nth(0);
        match input {
            None => {},
            Some(from_node) => {
                let outputs = ctx.graph.graph().edges_directed(ix, petgraph::EdgeDirection::Outgoing).map(|(a,b)| (a, b.clone())).collect::<Vec<_>>();

                for (to_node, weight) in outputs {
                    ctx.graph.add_edge(from_node, to_node, weight.clone()).unwrap();
                }
                ctx.graph.remove_node(ix).unwrap();
            }
        };
    }
}
lazy_static! {
    pub static ref FLIP_V: NodeDefinition = NodeDefinition {
        id: NodeType::Flip_Vertical,
        name: "FlipV",
        description: "Flip frame vertical",
        fn_estimate: Some(Helpers::preserve_frame_info),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

            }
            f
        }),
        .. Default::default()
    };
    pub static ref FLIP_H: NodeDefinition = NodeDefinition {
        id: NodeType::Flip_Horizontal,
        name: "FlipH",
        description: "Flip frame horizontal",
        fn_estimate: Some(Helpers::preserve_frame_info),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

            }
            f
        }),
        .. Default::default()
    };
    pub static ref ROTATE_90: NodeDefinition = NodeDefinition {
        id: NodeType::Rotate_90,
        name: "Rot90",
        description: "Rotate",
        fn_estimate: Some(Helpers::rotate_frame_info),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

            }
            f
        }),
        .. Default::default()
    };
     pub static ref ROTATE_180: NodeDefinition = NodeDefinition {
        id: NodeType::Rotate_180,
        name: "Rot180",
        description: "Rotate",
        fn_estimate: Some(Helpers::preserve_frame_info),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

            }
            f
        }),
        .. Default::default()
    };
    pub static ref ROTATE_270: NodeDefinition = NodeDefinition {
        id: NodeType::Rotate_270,
        name: "Rot270",
        description: "Rotate",
        fn_estimate: Some(Helpers::rotate_frame_info),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

            }
            f
        }),
        .. Default::default()
    };
    pub static ref APPLY_ORIENTATION: NodeDefinition = NodeDefinition {
        id: NodeType::Apply_Orientation,
        name: "Apply orientation",
        description: "Apply orientation",
        fn_estimate: Some(Helpers::preserve_frame_info),
        fn_flatten_pre_optimize: Some(Helpers::delete_self_connect_outputs),
        .. Default::default()
    };

    pub static ref TRANSPOSE: NodeDefinition = NodeDefinition {
        id: NodeType::Transpose,
        name: "Transpose",
        description: "Transpose",
        fn_estimate: Some(Helpers::rotate_frame_info),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

            }
            f
        }),
        .. Default::default()
    };
}
