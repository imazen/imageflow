#include "job.h"
#include "graph.h"


static int32_t flow_node_create_codec_on_buffer(Context *c, struct flow_job * job, struct flow_graph **graph_ref, struct flow_job_resource_buffer * buf, flow_job_codec_type codec_type){
    int32_t id = flow_node_create_generic(c, graph_ref, -1, flow_ntype_primitive_decoder);
    if (id < 0){
        CONTEXT_add_to_callstack(c);
        return id;
    }
    FLOW_GET_INFOBYTES((*graph_ref),id, flow_nodeinfo_decoder, info)
    info->type = codec_type;
    info->decoder = flow_job_acquire_decoder_over_buffer(c,job,buf,codec_type);
    return id;
}

int32_t flow_node_create_resource_bitmap_bgra(Context *c, struct flow_graph **g, int32_t prev_node, BitmapBgra ** ref){
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_primitive_bitmap_bgra_pointer);
    if (id < 0){
        CONTEXT_add_to_callstack(c);
        return id;
    }
    FLOW_GET_INFOBYTES((*g),id, flow_nodeinfo_resource_bitmap_bgra, info)
    info->ref = ref;
    return id;
}

static int32_t create_node_for_buffer(Context *c,struct flow_job * job, struct flow_job_resource_item * item, struct flow_graph ** g){
    if (item->direction != FLOW_INPUT){
        CONTEXT_error(c, Not_implemented);
        return -1;
    }
    struct flow_job_resource_buffer * buf = (struct flow_job_resource_buffer *)item->data;

    flow_job_codec_type ctype = flow_job_codec_select(c,job, (uint8_t  *)buf->buffer, buf ->buffer_size);
    if (ctype == flow_job_codec_type_null){
        //unknown
        CONTEXT_error(c, Not_implemented); //Or bad buffer, unsupported file type, etc.
        return -1;
    }
    return flow_node_create_codec_on_buffer(c, job, g, buf, ctype);
}


static int32_t create_node_for_resource(Context *c,struct flow_job * job, struct flow_job_resource_item * item, struct flow_graph ** g){

    int32_t node_id = -1;
    if (item->type == flow_job_resource_type_bitmap_bgra){
        if (item->direction != FLOW_OUTPUT){
            CONTEXT_error(c, Not_implemented);
            return false;
        }
        node_id = flow_node_create_resource_bitmap_bgra(c,g, -1, (BitmapBgra **)&item->data);
    }else if (item->type == flow_job_resource_type_buffer) {
        node_id = create_node_for_buffer(c,job, item,g);
    }else{
        CONTEXT_error(c,Not_implemented);
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
                                                                          struct flow_graph *from){
    if (from == NULL){
        CONTEXT_error(c, Null_argument);
        return NULL;
    }
    struct flow_graph * g = from;
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
                replacement_node_id = create_node_for_resource(c, job, current, &g);
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
    if (!flow_job_notify_graph_changed(c,job, *graph_ref)){
        CONTEXT_error_return(c);
    }
    struct flow_graph * g = flow_job_insert_resources_into_graph_with_reuse(c, job, *graph_ref);
    if (g == NULL){
        CONTEXT_error_return(c);
    }
    *graph_ref = g;
    return true;
}

