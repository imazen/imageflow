use flow::graph::Graph;
use ffi::{Context,Job,NodeType};
use daggy::{Dag,EdgeIndex,NodeIndex};
use flow::definitions::*;

struct Helpers{}
impl Helpers{

    fn preserve_frame_info(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

    }
    fn flatten_flip_v(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

    }
}

const FlipV: NodeDefinition = NodeDefinition{
    id: NodeType::Flip_Vertical,
    inbound_edges: EdgesIn::OneInput,
    outbound_edges: true,
    name: "FlipV",
    description: "Flip frame vertical",
    fn_graphviz_text: None,
    fn_flatten_post_optimize: None,
    fn_execute: None,
    fn_cleanup: None,
    fn_estimate: Some(Helpers::preserve_frame_info),
    fn_flatten_pre_optimize: Some({
        fn flatten_flip_v(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

        }
        flatten_flip_v
    }),
};
