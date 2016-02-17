#include "job.h"
#include "graph.h"


static bool flow_job_node_is_completed(Context * c, struct flow_job * job,  struct flow_graph * g, int32_t node_id) {
    return g->nodes[node_id].executed;
}
bool flow_job_graph_fully_executed(Context *c, struct flow_job *job, struct flow_graph *g){
    int32_t i;
    for(i = 0; i < g->next_node_id; i ++){
        if (g->nodes[i].type != flow_ntype_Null){
            if (!flow_job_node_is_completed(c,job,g,i)){
                return false;
            }
        }
    }
    return true;
}

static bool flow_job_node_is_ready_for_execution(Context *c, struct flow_job *job, struct flow_graph *g,
                                                 int32_t node_id){
    if (flow_job_node_is_completed(c,job,g,node_id)){
        return false; //Completed already
    }
    if (g->nodes[node_id].type >= flow_ntype_non_primitive_nodes_begin ) {
        return false; //Not a primitive node. We assume if a primitive node exists, it has dimensions and has been flattened.
    }
    //For clarity, although not strictly required (AFAIK), we wait until dimensions are populated.
    if (!flow_node_input_edges_have_dimensions(c, g, node_id)){
        return false;
    }
    struct flow_edge * edge;
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++){
        edge = &g->edges[i];
        if (edge->type != flow_edgetype_null){
            if (edge->to == node_id) {
                if (!flow_job_node_is_completed(c,job,g, edge->from)) {
                    return false;
                    //One of our inputs in incomplete;
                }

            }
        }
    }
    return true;
}


static bool node_visitor_execute(Context *c, struct flow_job *job, struct flow_graph **graph_ref,
                                 int32_t node_id, bool *quit, bool *skip_outbound_paths,
                                 void *custom_data){

    if (flow_job_node_is_ready_for_execution(c,job,*graph_ref,node_id)){
        if (!flow_node_execute(c, job, *graph_ref, node_id)) {
            CONTEXT_error_return(c);
        }
    }
    if (!flow_job_node_is_completed(c,job,*graph_ref,node_id)){
        //If we couldn't complete this node yet, end this branch.
        *skip_outbound_paths = true;
    }else{
         flow_job_notify_graph_changed(c,job, *graph_ref);

    }
    return true;
}


//if no hits, search forward

bool flow_job_execute_where_certain(Context *c, struct flow_job *job, struct flow_graph **g){
    if (*g == NULL){
        CONTEXT_error(c,Null_argument);
        return false;
    }

//    //Resets and creates state tracking for this graph
//    if (!flow_job_create_state(c,job, *g)){
//        CONTEXT_error_return(c);
//    }

    if (!flow_graph_walk(c, job, g, node_visitor_execute, NULL, NULL)) {
        CONTEXT_error_return(c);
    }
    return true;
}



