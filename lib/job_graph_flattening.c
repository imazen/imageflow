#include "job.h"
#include "graph.h"

static bool node_visitor_flatten(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref, int32_t node_id,
                                 bool* quit, bool* skip_outbound_paths, void* custom_data)
{

    if (!flow_node_update_state(c, *graph_ref, node_id)) {
        FLOW_error_return(c);
    }
    struct flow_node* n = &(*graph_ref)->nodes[node_id];

    // If input nodes are populated
    if (n->state == flow_node_state_ReadyForPreOptimizeFlatten) {
        if (!flow_node_pre_optimize_flatten(c, graph_ref, node_id)) {
            FLOW_error_return(c);
        }
        *quit = true;
        *((bool*)custom_data) = true;
    } else if ((n->state & flow_node_state_InputDimensionsKnown) == 0) {
        // we can't flatten past missing dimensions
        *skip_outbound_paths = true;
    }
    return true;
}

bool flow_graph_pre_optimize_flatten(flow_context* c, struct flow_graph** graph_ref)
{
    if (*graph_ref == NULL) {
        FLOW_error(c, flow_status_Null_argument);
        return false;
    }
    bool re_walk;
    do {
        re_walk = false;
        if (!flow_graph_walk_dependency_wise(c, NULL, graph_ref, node_visitor_flatten, NULL, &re_walk)) {
            FLOW_error_return(c);
        }
    } while (re_walk);
    return true;
}
