#include "job.h"
#include "graph.h"

static bool flow_job_populate_outbound_dimensions_for_edge(flow_context* c, struct flow_job* job, struct flow_graph* g,
                                                           int32_t outbound_edge_id, bool force_estimate)
{

    struct flow_edge* edge = &g->edges[outbound_edge_id];

    uint64_t now = flow_get_high_precision_ticks();

    if (!flow_node_populate_dimensions_to_edge(c, g, edge->from, outbound_edge_id, force_estimate)) {
        FLOW_error_return(c);
    }
    g->nodes[edge->from].ticks_elapsed += flow_get_high_precision_ticks() - now;
    return true;
}

bool flow_edge_has_dimensions(flow_context* c, struct flow_graph* g, int32_t edge_id)
{
    struct flow_edge* edge = &g->edges[edge_id];
    return edge->from_width > 0;
}
bool flow_node_input_edges_have_dimensions(flow_context* c, struct flow_graph* g, int32_t node_id)
{
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++) {
        if (g->edges[i].type != flow_edgetype_null && g->edges[i].to == node_id) {
            if (!flow_edge_has_dimensions(c, g, i)) {
                return false;
            }
        }
    }
    return true;
}

static bool edge_visitor_populate_outbound_dimensions(flow_context* c, struct flow_job* job,
                                                      struct flow_graph** graph_ref, int32_t edge_id, bool* quit,
                                                      bool* skip_outbound_paths, void* custom_data)
{

    int32_t node_id = (*graph_ref)->edges[edge_id].from;
    // Only populate if empty
    if (!flow_edge_has_dimensions(c, *graph_ref, edge_id)) {
        if (!flow_node_update_state(c, *graph_ref, node_id)) {
            FLOW_error_return(c);
        }

        struct flow_node* n = &(*graph_ref)->nodes[node_id];
        // If input nodes are populated
        if ((n->state & flow_node_state_InputDimensionsKnown) > 0) {
            if (!flow_job_populate_outbound_dimensions_for_edge(c, job, *graph_ref, edge_id, (bool)custom_data)) {
                FLOW_error_return(c);
            }
        }
        if (!flow_edge_has_dimensions(c, *graph_ref, edge_id)) {
            // We couldn't populate this edge, so we sure can't populate others in this direction.
            // Stop this branch of recursion
            *skip_outbound_paths = true;
        } else {
            flow_job_notify_graph_changed(c, job, *graph_ref);
        }
    }

    return true;
}

bool flow_job_populate_dimensions_where_certain(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref)
{
    // TODO: would be good to verify graph is acyclic.
    if (!flow_graph_walk_dependency_wise(c, job, graph_ref, NULL, edge_visitor_populate_outbound_dimensions,
                                         (void*)false)) {
        FLOW_error_return(c);
    }
    return true;
}

bool flow_job_force_populate_dimensions(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref)
{
    // TODO: would be good to verify graph is acyclic.
    if (!flow_graph_walk(c, job, graph_ref, NULL, edge_visitor_populate_outbound_dimensions, (void*)true)) {
        FLOW_error_return(c);
    }
    return true;
}
