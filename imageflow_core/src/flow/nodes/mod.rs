use libc::{int32_t,size_t};
use super::definitions::{Node,NodeType};
use super::graph::Graph;
use ffi::{Context,Job};
pub mod flip;

pub trait NodeMethods {
    fn stringify(&self, c: *mut Context, g: &mut Graph, node_id: int32_t, buffer: *mut u8, buffer_size: size_t) -> bool;
    fn count_infobytes(&self, c: *mut Context, g: &mut Graph, node_id: int32_t, infobytes_count_out: *mut int32_t) -> bool;
    fn populate_dimensions(&mut self, c: *mut Context, g: &mut Graph, node_id: int32_t, force_estimate: bool) -> bool;
    fn pre_optimize_flatten_complex(&mut self, c: *mut Context, g: &mut Graph, node_id: int32_t) -> bool;
    fn post_optimize_flatten_complex(&mut self, c: *mut Context, g: &mut Graph, node_id: int32_t) -> bool;
    fn pre_optimize_flatten(&mut self, c: *mut Context, g: &mut Graph, node_id: int32_t, node: &Node,
      input_node: &Node, first_replacement_node: int32_t, last_replacement_node: int32_t) -> bool;
    fn post_optimize_flatten(&mut self, c: *mut Context, g: &mut Graph, node_id: int32_t, node: &Node,
      input_node: &Node, first_replacement_node: int32_t, last_replacement_node: int32_t) -> bool;
    fn execute(&mut self, c: *mut Context, job: *mut Job, g: &mut Graph, node_id: int32_t) -> bool;
    fn estimate_cost(&self, c: *mut Context, job: *mut Job, g: &mut Graph, node_id: int32_t,
      bytes_required: *mut size_t, cpu_cost: *mut size_t) -> bool;

    fn definition(&mut self) -> &mut NodeDefinition;
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct NodeDefinition {
    pub node_type: NodeType,
    pub input_count: int32_t,
    pub prohibit_output_edges: bool,
    pub canvas_count: int32_t,
    pub type_name: String,
    pub nodeinfo_bytes_fixed: int32_t,
}

pub fn dimensions_mimic_input(c: *mut Context, g: &mut Graph, node_id: int32_t, force_estimate: bool) -> bool {
    /*FIXME: replace
    FLOW_GET_INPUT_NODE(g, node_id)
    struct flow_node * n = &g->nodes[node_id];

    n->result_width = input_node->result_width;
    n->result_height = input_node->result_height;
    n->result_alpha_meaningful = input_node->result_alpha_meaningful;
    n->result_format = input_node->result_format;
    return true;
    */
    true
}
