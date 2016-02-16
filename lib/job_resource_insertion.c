#include "job.h"
static int32_t create_node_for_resource(Context *c, struct flow_job_resource_item * item, struct flow_graph ** g){
    int32_t node_id = -1;
    if (item->type == flow_job_resource_bitmap_bgra){
        node_id = flow_node_create_resource_bitmap_bgra(c,g, -1, (BitmapBgra **)&item->data);
    }else{
        CONTEXT_error(c,Invalid_internal_state);
        return -404;
    }
    if (node_id < 0){
        CONTEXT_add_to_callstack(c);
    }
    return node_id;
}

//When placeholder_id == -1, the first resource placeholder is returned.
static int32_t flow_job_find_first_node_with_placeholder_id(Context *c, struct flow_graph * g, int32_t placeholder_id){
    if (g == NULL){
        CONTEXT_error(c, Null_argument);
        return -1;
    }
    int32_t i;
    for(i = 0; i < g->next_node_id; i ++){
        if (g->nodes[i].type == flow_ntype_Resource_Placeholder){
            if (placeholder_id == -1){
                return i;
            }else{
                uint8_t * info_bytes = &g->info_bytes[g->nodes[i].info_byte_index];
                struct flow_nodeinfo_index * info = (struct flow_nodeinfo_index *) info_bytes;
                if (info->index == placeholder_id){
                    return i;
                }
            }
        }
    }
    return -404;
}


static struct flow_graph *flow_job_insert_resources_into_graph_with_reuse(Context *c, struct flow_job *job,
                                                                          struct flow_graph *from,
                                                                          bool may_reuse_graph){
    if (from == NULL){
        CONTEXT_error(c, Null_argument);
        return NULL;
    }
    struct flow_graph * g = may_reuse_graph ? from : flow_graph_memcpy(c,from);
    if (g == NULL){
        CONTEXT_add_to_callstack(c);
        return NULL;
    }

    struct flow_job_resource_item * current = job->resources_head;
    int32_t next_match;
    int32_t replacement_node_id;
    int32_t match_count;
    while (current != NULL){
        match_count = 0;
        do {
            //flow_graph_print_to(c,g,stderr);

            next_match = flow_job_find_first_node_with_placeholder_id(c,g, current->graph_placeholder_id);
            if (next_match >= 0) {
                replacement_node_id = create_node_for_resource(c, current, &g);
                if (replacement_node_id < 0) {
                    CONTEXT_error_return(c);
                }
                //flow_graph_print_to(c, g, stderr);
                if (!flow_graph_duplicate_edges_to_another_node(c, &g, next_match, replacement_node_id, true, true)) {
                    CONTEXT_error_return(c);
                }
                //flow_graph_print_to(c, g, stderr);
                if (!flow_node_delete(c, g, next_match)) {
                    CONTEXT_error_return(c);
                }
                //flow_graph_print_to(c, g, stderr);
                match_count++;
                flow_job_notify_graph_changed(c,job, g);
            }
        }while(next_match >= 0);

        if (match_count == 0){
            //No matching nodes exist in the graph
            //This is a warning situation
        }

        current = current->next;
    }
    if (flow_job_find_first_node_with_placeholder_id(c,g, -1) > -1){
        //Leftover nodes with no placeholder - this is an error
        CONTEXT_error(c, Graph_could_not_be_completed);
        return NULL;
    }
    return g;
}
bool flow_job_insert_resources_into_graph(Context *c, struct flow_job *job, struct flow_graph **graph_ref){
    struct flow_graph * g = flow_job_insert_resources_into_graph_with_reuse(c, job, *graph_ref, true);
    if (g == NULL){
        CONTEXT_error_return(c);
    }
    *graph_ref = g;
    return true;
}

