#include "job.h"
#include "graph.h"

static bool flow_job_node_is_executed(Context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id)
{
    return (g->nodes[node_id].state & flow_node_state_Executed) > 0;
}
bool flow_job_graph_fully_executed(Context* c, struct flow_job* job, struct flow_graph* g)
{
    int32_t i;
    for (i = 0; i < g->next_node_id; i++) {
        if (g->nodes[i].type != flow_ntype_Null) {
            if (!flow_job_node_is_executed(c, job, g, i)) {
                return false;
            }
        }
    }
    return true;
}

static bool node_visitor_execute(Context* c, struct flow_job* job, struct flow_graph** graph_ref, int32_t node_id,
                                 bool* quit, bool* skip_outbound_paths, void* custom_data)
{

    if (!flow_node_update_state(c, *graph_ref, node_id)) {
        CONTEXT_error_return(c);
    }
    struct flow_node* n = &(*graph_ref)->nodes[node_id];

    if (!flow_job_node_is_executed(c, job, *graph_ref, node_id) && n->state == flow_node_state_ReadyForExecution) {
        uint64_t now = get_high_precision_ticks();
        if (!flow_node_execute(c, job, *graph_ref, node_id)) {
            CONTEXT_error_return(c);
        } else {
            (*graph_ref)->nodes[node_id].ticks_elapsed += get_high_precision_ticks() - now;
            n->state = (flow_node_state)(n->state | flow_node_state_Executed);
            flow_job_notify_node_complete(c, job, *graph_ref, node_id);
        }
    }
    if (!flow_job_node_is_executed(c, job, *graph_ref, node_id)) {
        // If we couldn't complete this node yet, end this branch.
        *skip_outbound_paths = true;
    } else {
        flow_job_notify_graph_changed(c, job, *graph_ref);
    }
    return true;
}

// if no hits, search forward

bool flow_job_execute_where_certain(Context* c, struct flow_job* job, struct flow_graph** g)
{
    if (*g == NULL) {
        CONTEXT_error(c, Null_argument);
        return false;
    }

    //    //Resets and creates state tracking for this graph
    //    if (!flow_job_create_state(c,job, *g)){
    //        CONTEXT_error_return(c);
    //    }

    if (!flow_graph_walk_dependency_wise(c, job, g, node_visitor_execute, NULL, NULL)) {
        CONTEXT_error_return(c);
    }
    return true;
}
