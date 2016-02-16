#include "job.h"



static bool flow_job_node_is_completed(Context * c, struct flow_job * job,  struct flow_graph * g, int32_t node_id) {
    return job->job_state.node_completed[node_id];
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




static bool flow_job_complete_node(Context *c, struct flow_job * job, struct flow_graph * g, int32_t node_id ) {

    struct flow_node * n = &g->nodes[node_id];
    uint8_t * bytes = &g->info_bytes[n->info_byte_index];

    BitmapBgra * b = NULL;
    if (n->type == flow_ntype_Create_Canvas) {
        if (flow_graph_get_inbound_edge_count_of_type(c, g, node_id, flow_edgetype_input) +
            flow_graph_get_inbound_edge_count_of_type(c, g, node_id, flow_edgetype_canvas) != 0){
            CONTEXT_error(c, Invalid_inputs_to_node);
            return false;
        }
        struct flow_nodeinfo_createcanvas *info = (struct flow_nodeinfo_createcanvas *) bytes;
        b = BitmapBgra_create(c, info->width, info->height, true, info->format);
        if (b == NULL){
            CONTEXT_error_return(c);
        }
        //b->matte_color = info->bgcolor;
    }else if(n->type == flow_ntype_primitive_bitmap_bgra_pointer){
        if (flow_graph_get_inbound_edge_count_of_type(c, g, node_id, flow_edgetype_input) != 1 &&
            flow_graph_get_inbound_edge_count_of_type(c, g, node_id, flow_edgetype_canvas) != 0){
            CONTEXT_error(c, Invalid_inputs_to_node);
            return false;
        }
        struct flow_nodeinfo_resource_bitmap_bgra *info = (struct flow_nodeinfo_resource_bitmap_bgra *) bytes;

        int32_t input_node_id = flow_graph_get_first_inbound_edge_of_type(c, g, node_id, flow_edgetype_input);
        //get input
        b = job->job_state.node_bitmap[input_node_id];
        //Update pointer
        *info->ref = b;
    }else{
        if (n->type < flow_ntype_non_primitive_nodes_begin) {
            CONTEXT_error(c, Not_implemented);
        }else{
            CONTEXT_error(c, Graph_not_flattened);
        }
        return false;
    }


    job->job_state.node_completed[node_id] = true;
    job->job_state.node_bitmap[node_id] = b;
    return true;
}


static bool node_visitor_execute(Context *c, struct flow_job *job, struct flow_graph **graph_ref,
                                 int32_t node_id, bool *quit, bool *skip_outbound_paths,
                                 void *custom_data){

    if (flow_job_node_is_ready_for_execution(c,job,*graph_ref,node_id)){
        if (!flow_job_complete_node(c, job, *graph_ref, node_id)) {
            CONTEXT_error_return(c);
        }
    }
    if (!flow_job_node_is_completed(c,job,*graph_ref,node_id)){
        //If we couldn't complete this node yet, end this branch.
        *skip_outbound_paths = true;
    }
    return true;
}


//if no hits, search forward

bool flow_job_execute_where_certain(Context *c, struct flow_job *job, struct flow_graph **g){
    if (*g == NULL){
        CONTEXT_error(c,Null_argument);
        return false;
    }

    //Resets and creates state tracking for this graph
    if (!flow_job_create_state(c,job, *g)){
        CONTEXT_error_return(c);
    }

    if (!flow_graph_walk(c, job, g, node_visitor_execute, NULL, NULL)) {
        CONTEXT_error_return(c);
    }
    return true;
}



