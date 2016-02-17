#include "job.h"
#include "graph.h"


static bool node_visitor_flatten(Context *c, struct flow_job *job, struct flow_graph **graph_ref,
                                                      int32_t node_id, bool *quit, bool *skip_outbound_paths,
                                                      void *custom_data){

    struct flow_node * node =&(*graph_ref)->nodes[node_id];

    //If input nodes are populated
    if (flow_node_input_edges_have_dimensions(c,*graph_ref,node_id)){
        if (node->type >= flow_ntype_non_primitive_nodes_begin ) {
            if (!flow_node_flatten(c, graph_ref, node_id)) {
                CONTEXT_error_return(c);
            }
            *quit = true;
            *((bool *)custom_data) = true;
        }
    }else{
        //we can't flatten past missing dimensions
        *skip_outbound_paths = true;
    }
    return true;
}

bool flow_graph_flatten_where_certain(Context *c, struct flow_graph ** graph_ref){
    if (*graph_ref == NULL){
        CONTEXT_error(c,Null_argument);
        return false;
    }
    bool re_walk;
    do {
        re_walk = false;
        if (!flow_graph_walk(c, NULL, graph_ref, node_visitor_flatten, NULL, &re_walk)) {
            CONTEXT_error_return(c);
        }
    }while(re_walk);
    return true;
}
