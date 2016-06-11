#include "../imageflow_private.h"
#include "definition_helpers.h"

bool stringify_state(char * buffer, size_t buffer_isze, struct flow_node * n)
{
    flow_snprintf(buffer, buffer_isze, "[%d/%d]", n->state, flow_node_state_Done);
    return true;
}

char * stringify_colorspace(flow_working_floatspace space)
{
    switch (space) {
        case flow_working_floatspace_gamma:
            return "gamma";
        case flow_working_floatspace_linear:
            return "linear";
        case flow_working_floatspace_srgb:
            return "sRGB";
        default:
            return "colorspace unknown";
    }
}
char * stringify_filter(flow_interpolation_filter filter)
{
    switch (filter) {
        case flow_interpolation_filter_Robidoux:
            return "robidoux";
        default:
            return "??";
    }
}

bool set_node_optimized_and_update_state(flow_c * c, struct flow_graph * g, int32_t node_id)
{
    struct flow_node * n = &g->nodes[node_id];

    n->state = (flow_node_state)(n->state | flow_node_state_Optimized);
    if (!flow_node_update_state(c, g, node_id)) {
        FLOW_error_return(c);
    }
    return true;
}

bool dimensions_mimic_input(flow_c * c, struct flow_graph * g, int32_t node_id, bool force_estimate)
{
    FLOW_GET_INPUT_NODE(g, node_id)

    struct flow_node * n = &g->nodes[node_id];

    n->result_width = input_node->result_width;
    n->result_height = input_node->result_height;
    n->result_alpha_meaningful = input_node->result_alpha_meaningful;
    n->result_format = input_node->result_format;
    return true;
}

bool dimensions_of_canvas(flow_c * c, struct flow_graph * g, int32_t node_id, bool force_estimate)
{
    // FLOW_GET_INFOBYTES(g,node_id, flow_nodeinfo_size, info)
    FLOW_GET_CANVAS_NODE(g, node_id)

    struct flow_node * n = &g->nodes[node_id];

    n->result_format = flow_bgra32; // TODO: maybe wrong
    n->result_alpha_meaningful = true; // TODO: WRONG! Involve "input" in decision
    n->result_width = canvas_node->result_width;
    n->result_height = canvas_node->result_height;
    return true;
}

bool node_has_other_dependents(flow_c * c, struct flow_graph * g, int32_t node_id, int32_t excluding_dependent_node_id,
                               bool * has_other_dependents)
{
    // TODO: Implement tracing logic
    *has_other_dependents = true;
    return true;
}

bool flatten_delete_node(flow_c * c, struct flow_graph ** graph_ref, int32_t node_id)
{
    int32_t input_edge_id = flow_graph_get_first_inbound_edge_of_type(c, *graph_ref, node_id, flow_edgetype_input);

    int32_t output_edge_id = flow_graph_get_first_outbound_edge_of_type(c, *graph_ref, node_id, flow_edgetype_input);

    struct flow_edge * input_edge = input_edge_id < 0 ? NULL : &(*graph_ref)->edges[input_edge_id];

    struct flow_edge * output_edge = output_edge_id < 0 ? NULL : &(*graph_ref)->edges[output_edge_id];

    if (output_edge != NULL && input_edge != NULL) {
        // Clone edges
        if (!flow_graph_duplicate_edges_to_another_node(c, graph_ref, input_edge->from, output_edge->to, true, false)) {
            FLOW_error_return(c);
        }
    }

    // Delete the original
    if (!flow_node_delete(c, *graph_ref, node_id)) {
        FLOW_error_return(c);
    }
    return true;
}


