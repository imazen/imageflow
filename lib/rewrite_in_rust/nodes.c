#include "../imageflow_private.h"
#include "./nodes/definition_helpers.h"
#include "../codecs.h"

struct flow_node_definition * flow_nodedef_get(flow_c * c, flow_ntype type)
{
    size_t i = 0;
    for (i = 0; i < c->node_set->node_definitions_count; i++) {
        if (c->node_set->node_definitions[i].type == type)
            return &c->node_set->node_definitions[i];
    }
    FLOW_error(c, flow_status_Not_implemented);
    return NULL;
}

bool flow_node_stringify(flow_c * c, struct flow_graph * g, int32_t node_id, char * buffer, size_t buffer_size)
{
    struct flow_node * node = &g->nodes[node_id];
    struct flow_node_definition * def = flow_nodedef_get(c, node->type);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (def->stringify == NULL) {
        if (def->type_name == NULL) {
            FLOW_error(c, flow_status_Not_implemented);
            return false;
        }
        char state[64];
        if (!stringify_state(state, 63, &g->nodes[node_id])) {
            FLOW_error_return(c);
        }

        flow_snprintf(buffer, buffer_size, "%s %s", def->type_name, (const char *)&state);
    } else {
        if (!def->stringify(c, g, node_id, buffer, buffer_size)) {
            FLOW_error_return(c);
        }
    }
    return true;
}

int32_t flow_node_fixed_infobyte_count(flow_c * c, flow_ntype type)
{
    struct flow_node_definition * def = flow_nodedef_get(c, type);
    if (def == NULL) {
        FLOW_add_to_callstack(c);
        return -1;
    }
    if (def->nodeinfo_bytes_fixed < 0) {
        FLOW_error(c, flow_status_Not_implemented);
    }
    return def->nodeinfo_bytes_fixed;
}

bool flow_node_infobyte_count(flow_c * c, struct flow_graph * g, int32_t node_id, int32_t * infobytes_count_out)
{
    struct flow_node * node = &g->nodes[node_id];
    struct flow_node_definition * def = flow_nodedef_get(c, node->type);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (def->count_infobytes == NULL) {
        *infobytes_count_out = flow_node_fixed_infobyte_count(c, node->type);
        if (*infobytes_count_out < 0) {
            FLOW_error_return(c);
        }
    } else {
        def->count_infobytes(c, g, node_id, infobytes_count_out);
    }
    return true;
}

bool flow_node_validate_edges(flow_c * c, struct flow_graph * g, int32_t node_id)
{
    struct flow_node * node = &g->nodes[node_id];
    struct flow_node_definition * def = flow_nodedef_get(c, node->type);
    if (def == NULL) {
        FLOW_error_return(c);
    }

    int32_t input_edge_count = flow_graph_get_inbound_edge_count_of_type(c, g, node_id, flow_edgetype_input);
    int32_t canvas_edge_count = flow_graph_get_inbound_edge_count_of_type(c, g, node_id, flow_edgetype_canvas);

    if (def->input_count > -1 && def->input_count != input_edge_count) {
        FLOW_error(c, flow_status_Invalid_inputs_to_node);
        return false;
    }
    if (def->canvas_count > -1 && def->canvas_count != canvas_edge_count) {
        FLOW_error(c, flow_status_Invalid_inputs_to_node);
        return false;
    }

    if (def->prohibit_output_edges) {
        int32_t outbound_edge_count = flow_graph_get_edge_count(c, g, node_id, false, flow_edgetype_null, false, true);
        if (outbound_edge_count > 0) {
            FLOW_error_msg(c, flow_status_Graph_invalid, "This node (%s) cannot have outbound edges - found %i.",
                           def->type_name, outbound_edge_count);
            return false;
        }
    }
    return true;
}

static bool flow_node_all_types_inputs_executed(flow_c * c, struct flow_graph * g, int32_t node_id)
{
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++) {
        if (g->edges[i].type != flow_edgetype_null && g->edges[i].to == node_id) {
            if ((g->nodes[g->edges[i].from].state & flow_node_state_Executed) == 0) {
                return false;
            }
        }
    }
    return true;
}

bool flow_node_update_state(flow_c * c, struct flow_graph * g, int32_t node_id)
{

    // Ready flags are cumulative.
    // 1. If you don't have input dimensions, you're not ready for anything (although you may have already been
    // optimized, as optimization or flattening can leave the graph inconsistent.
    // 2. If you aren't a primitive or optimizable node type, you're not ready for optimizing, or post flattening or
    // executing
    // 3. If you're not optimized, you're not ready for post flattening or executing
    // 4. If you're not a primitve, or haven't been optimized, you're not ready for executing
    // 5. If your input edges haven't executed, you're not ready for executing

    struct flow_node * n = &g->nodes[node_id];

    bool input_dimensions_known = flow_node_inputs_have_dimensions(c, g, node_id);
    bool optimization_allowed = n->type < flow_ntype_non_optimizable_nodes_begin;
    bool optimized = (n->state & flow_node_state_Optimized) > 0;
    bool is_executable_primitive = n->type < flow_ntype_non_primitive_nodes_begin;
    bool executed = (n->state & flow_node_state_Executed) > 0;

    n->state = flow_node_state_Blank;

    //#1
    if (input_dimensions_known) {
        n->state = (flow_node_state)(n->state | flow_node_state_InputDimensionsKnown);
    } else {
        if (executed) {
            FLOW_error(c, flow_status_Invalid_internal_state);
            return false;
        }
        // One can be optimized or flattened, yet be *newly* missing input dimensions due to said processes
    }
    //#2
    if (!optimization_allowed) {
        // If it's not optimizable or executable, nothing else is relevant
        if (optimized || executed || is_executable_primitive) {
            FLOW_error(c, flow_status_Invalid_internal_state);
            return false;
        }
        return true;
    }
    // Only pre-optimize-flattened nodes make it to this point
    n->state = (flow_node_state)(n->state | flow_node_state_PreOptimizeFlattened);

    //#3
    if (!optimized) {
        // If it's not optimizable or executable, nothing else is relevant
        if (executed) {
            FLOW_error(c, flow_status_Invalid_internal_state);
            return false;
        }
        return true;
    }
    n->state = (flow_node_state)(n->state | flow_node_state_Optimized);

    //#4
    if (!is_executable_primitive) {
        if (executed) {
            FLOW_error(c, flow_status_Invalid_internal_state);
            return false;
        }
        return true;
    }
    n->state = (flow_node_state)(n->state | flow_node_state_PostOptimizeFlattened);

    //#5
    bool inputs_executed = flow_node_all_types_inputs_executed(c, g, node_id);
    if (!inputs_executed) {
        if (executed) {
            FLOW_error(c, flow_status_Invalid_internal_state);
            return false;
        }
        return true;
    }
    n->state = (flow_node_state)(n->state | flow_node_state_InputsExecuted);

    if (!executed) {
        return true;
    }
    n->state = (flow_node_state)(n->state | flow_node_state_Executed);

    return true;
}

bool flow_node_populate_dimensions(flow_c * c, struct flow_graph * g, int32_t node_id, bool force_estimate)
{
    if (!flow_node_validate_edges(c, g, node_id)) {
        FLOW_error_return(c);
    }
    struct flow_node * node = &g->nodes[node_id];
    struct flow_node_definition * def = flow_nodedef_get(c, node->type);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (def->populate_dimensions == NULL) {
        FLOW_error_msg(c, flow_status_Not_implemented, "populate_dimensions is not implemented for node type %s",
                       def->type_name);
        return false;
    } else {
        if (!def->populate_dimensions(c, g, node_id, force_estimate)) {
            FLOW_error_return(c);
        }
    }
    return true;
}

static bool flow_node_flatten_generic(flow_c * c, struct flow_graph ** graph_ref, int32_t node_id, bool post_optimize)
{
    if (!flow_node_validate_edges(c, *graph_ref, node_id)) {
        FLOW_error_return(c);
    }
    struct flow_node * node = &(*graph_ref)->nodes[node_id];
    struct flow_node_definition * def = flow_nodedef_get(c, node->type);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if ((post_optimize ? def->post_optimize_flatten_complex : def->pre_optimize_flatten_complex) == NULL) {
        if ((post_optimize ? def->post_optimize_flatten : def->pre_optimize_flatten) == NULL) {
            FLOW_error_msg(c, flow_status_Not_implemented,
                           post_optimize ? "post_optimize flattening not implemented for node %s"
                                         : "pre_optimize flattening not implemented for node %s",
                           def->type_name);
            return false;
        } else {
            int32_t first_replacement_node = -1;
            int32_t last_replacement_node = -1;

            int32_t input_node_id
                = flow_graph_get_first_inbound_node_of_type(c, *graph_ref, node_id, flow_edgetype_input);
            // TODO - check bounds
            struct flow_node * input_node = input_node_id < 0 ? NULL : &(*graph_ref)->nodes[input_node_id];

            (post_optimize ? def->post_optimize_flatten : def->pre_optimize_flatten)(
                c, graph_ref, node_id, node, input_node, &first_replacement_node, &last_replacement_node);

            if (first_replacement_node == last_replacement_node && last_replacement_node == node_id) {
                // do nothing
            } else if (first_replacement_node == node_id || last_replacement_node == node_id) {
                FLOW_error_msg(c, flow_status_Invalid_inputs_to_node,
                               "You may not reuse the original node AND add additional nodes.");
                return false;
            } else {

                // Clone inbound edges
                if (!flow_graph_duplicate_edges_to_another_node(c, graph_ref, node_id, first_replacement_node, true,
                                                                false)) {
                    FLOW_error_return(c);
                }
                // Clone outbound edges
                if (!flow_graph_duplicate_edges_to_another_node(c, graph_ref, node_id, last_replacement_node, false,
                                                                true)) {
                    FLOW_error_return(c);
                }
                // Delete the original
                if (!flow_node_delete(c, *graph_ref, node_id)) {
                    FLOW_error_return(c);
                }
            }
        }
    } else {
        (post_optimize ? def->post_optimize_flatten_complex : def->pre_optimize_flatten_complex)(c, graph_ref, node_id);
    }
    return true;
}
bool flow_node_pre_optimize_flatten(flow_c * c, struct flow_graph ** graph_ref, int32_t node_id)
{
    if (!flow_node_flatten_generic(c, graph_ref, node_id, false)) {
        FLOW_error_return(c);
    }
    return true;
}
bool flow_node_post_optimize_flatten(flow_c * c, struct flow_graph ** graph_ref, int32_t node_id)
{
    if (!flow_node_flatten_generic(c, graph_ref, node_id, true)) {
        FLOW_error_return(c);
    }
    return true;
}
bool flow_node_execute(flow_c * c, struct flow_graph * g, int32_t node_id)
{
    if (!flow_node_validate_edges(c, g, node_id)) {
        FLOW_error_return(c);
    }
    struct flow_node * node = &g->nodes[node_id];
    struct flow_node_definition * def = flow_nodedef_get(c, node->type);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (def->execute == NULL) {
        FLOW_error(c, flow_status_Not_implemented);
        return false;
    } else {
        if (!def->execute(c, g, node_id)) {
            FLOW_error_return(c);
        } else {
            node->state = (flow_node_state)(node->state | flow_node_state_Executed);
            if (!flow_node_update_state(c, g, node_id)) {
                FLOW_error_return(c);
            }
        }
    }
    return true;
}
bool flow_node_estimate_execution_cost(flow_c * c, struct flow_graph * g, int32_t node_id, size_t * bytes_required,
                                       size_t * cpu_cost)
{
    FLOW_error(c, flow_status_Not_implemented);
    return false;
}
