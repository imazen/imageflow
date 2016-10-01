use libc::{int32_t,size_t};
use flow::definitions::{Node,NodeType};
use flow::graph::Graph;
use ffi::{Context,Job};
use super::{NodeDefinition,NodeMethods};

pub struct FlipVertical {
    definition: NodeDefinition,
}

impl FlipVertical {
    pub fn new() -> FlipVertical {
        FlipVertical {
            definition: NodeDefinition {
                node_type: NodeType::Flip_Vertical,
                nodeinfo_bytes_fixed: 0,
                input_count: 1,
                canvas_count: 0,
                type_name: String::from("flip vertical"),
                prohibit_output_edges: false, //FIXME: default value here?
            }
        }
    }
}

impl NodeMethods for FlipVertical {
    fn stringify(&self, c: *mut Context, g: &mut Graph, node_id: int32_t, buffer: *mut u8, buffer_size: size_t) -> bool {
        //placeholder
        true
    }

    fn count_infobytes(&self, c: *mut Context, g: &mut Graph, node_id: int32_t, infobytes_count_out: *mut int32_t) -> bool {
        //placeholder
        true
    }

    fn populate_dimensions(&mut self, c: *mut Context, g: &mut Graph, node_id: int32_t, force_estimate: bool) -> bool {
      super::dimensions_mimic_input(c, g, node_id, force_estimate)
    }

    fn pre_optimize_flatten_complex(&mut self, c: *mut Context, g: &mut Graph, node_id: int32_t) -> bool {
        //placeholder
        true
    }

    fn post_optimize_flatten_complex(&mut self, c: *mut Context, g: &mut Graph, node_id: int32_t) -> bool {
        //placeholder
        true
    }

    fn pre_optimize_flatten(&mut self, c: *mut Context, g: &mut Graph, node_id: int32_t, node: &Node,
      input_node: &Node, first_replacement_node: int32_t, last_replacement_node: int32_t) -> bool {
        //placeholder
        true
    }

    fn post_optimize_flatten(&mut self, c: *mut Context, g: &mut Graph, node_id: int32_t, node: &Node,
      input_node: &Node, first_replacement_node: int32_t, last_replacement_node: int32_t) -> bool {
        /*FIXME: replace
        FLOW_GET_INPUT_EDGE((*g), node_id);
        bool must_clone = false;
        if (!node_has_other_dependents(c, *g, input_edge->from, node_id, &must_clone)) {
            FLOW_error_return(c);
        }
        if (must_clone) {
            *first_replacement_node = flow_node_create_clone(c, g, -1);
            if (*first_replacement_node < 0) {
                FLOW_error_return(c);
            }
        } else {
            *first_replacement_node = -1;
        }
        *last_replacement_node
            = flow_node_create_generic(c, g, *first_replacement_node, flow_ntype_primitive_Flip_Vertical_Mutate);
        if (*last_replacement_node < 0) {
            FLOW_error_return(c);
        }
        if (!must_clone) {
            *first_replacement_node = *last_replacement_node;
        }
        */

        true
    }

    fn execute(&mut self, c: *mut Context, job: *mut Job, g: &mut Graph, node_id: int32_t) -> bool {
        //placeholder
        true
    }

    fn estimate_cost(&self, c: *mut Context, job: *mut Job, g: &mut Graph, node_id: int32_t,
      bytes_required: *mut size_t, cpu_cost: *mut size_t) -> bool {
        //placeholder
        true
    }

    fn definition(&mut self) -> &mut NodeDefinition {
      &mut self.definition
    }
}
