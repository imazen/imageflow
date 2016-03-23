#include "job.h"
#include "graph.h"

static int32_t flow_node_create_codec_on_buffer(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref,
                                                struct flow_job_resource_item* resource_item, flow_ntype node_type)
{
    int32_t id = flow_node_create_generic(c, graph_ref, -1, node_type);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    FLOW_GET_INFOBYTES((*graph_ref), id, flow_nodeinfo_codec, info)
    info->type = resource_item->codec_type;
    info->codec_state = resource_item->codec_state;
    return id;
}

int32_t flow_node_create_resource_bitmap_bgra(flow_context* c, struct flow_graph** g, int32_t prev_node,
                                              flow_bitmap_bgra** ref)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_primitive_bitmap_bgra_pointer);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    FLOW_GET_INFOBYTES((*g), id, flow_nodeinfo_resource_bitmap_bgra, info)
    info->ref = ref;
    return id;
}

bool flow_job_initialize_input_resource(flow_context* c, struct flow_job* job, struct flow_job_resource_item* item)
{
    if (item->direction != FLOW_INPUT) {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }
    if (item->type == flow_job_resource_type_buffer) {
        if (item->codec_state != NULL) {
            return true; // Already done
        }
        struct flow_job_resource_buffer* buf = (struct flow_job_resource_buffer*)item->data;
        flow_job_codec_type ctype = flow_job_codec_select(c, job, (uint8_t*)buf->buffer, buf->buffer_size);
        if (ctype == flow_job_codec_type_null) {
            // unknown
            FLOW_error(c, flow_status_Not_implemented); // Or bad buffer, unsupported file type, etc.
            return -1;
        }
        item->codec_type = ctype;
        item->codec_state = flow_job_acquire_codec_over_buffer(c, job, buf, item->codec_type);
    } else if (item->type == flow_job_resource_type_bitmap_bgra) {
        // Nothing required.
    }
    return true;
}

static int32_t create_node_for_buffer(flow_context* c, struct flow_job* job, struct flow_job_resource_item* item,
                                      struct flow_graph** g, int32_t placeholder_node_index)
{

    struct flow_job_resource_buffer* buf = (struct flow_job_resource_buffer*)item->data;
    int32_t id;
    if (item->direction == FLOW_INPUT) {
        if (!flow_job_initialize_input_resource(c, job, item)) {
            FLOW_add_to_callstack(c);
            return -1;
        }
        id = flow_node_create_codec_on_buffer(c, job, g, item, flow_ntype_primitive_decoder);
        if (id < 0) {
            FLOW_add_to_callstack(c);
        }
    } else {

        flow_job_codec_type codec_type = flow_job_codec_type_encode_png;

        // If an encoder placeholder is used we can get specifics
        if ((*g)->nodes[placeholder_node_index].type == flow_ntype_Encoder_Placeholder) {
            FLOW_GET_INFOBYTES((*g), placeholder_node_index, flow_nodeinfo_encoder_placeholder, info);
            codec_type = info->codec_type;
        }
        item->codec_type = codec_type;
        item->codec_state = flow_job_acquire_codec_over_buffer(c, job, buf, codec_type);
        id = flow_node_create_codec_on_buffer(c, job, g, item, flow_ntype_primitive_encoder);
        if (id < 0) {
            FLOW_add_to_callstack(c);
        }
    }
    return id;
}

static int32_t create_node_for_resource(flow_context* c, struct flow_job* job, struct flow_job_resource_item* item,
                                        struct flow_graph** g, int32_t placeholder_node_index)
{

    int32_t node_id = -1;
    if (item->type == flow_job_resource_type_bitmap_bgra) {
        node_id = flow_node_create_resource_bitmap_bgra(c, g, -1, (flow_bitmap_bgra**)&item->data);
    } else if (item->type == flow_job_resource_type_buffer) {
        node_id = create_node_for_buffer(c, job, item, g, placeholder_node_index);
    } else {
        FLOW_error(c, flow_status_Not_implemented);
        return -404;
    }
    if (node_id < 0) {
        FLOW_add_to_callstack(c);
    }
    return node_id;
}

static int32_t get_placeholder_id_for_node(flow_context* c, struct flow_graph* g, int32_t node_id)
{
    uint8_t* info_bytes = &g->info_bytes[g->nodes[node_id].info_byte_index];
    struct flow_nodeinfo_index* info = (struct flow_nodeinfo_index*)info_bytes;
    return info->index;
}

// When placeholder_id == -1, the first resource placeholder is returned.
static int32_t flow_job_find_first_node_with_placeholder_id(flow_context* c, struct flow_graph* g,
                                                            int32_t placeholder_id)
{
    if (g == NULL) {
        FLOW_error(c, flow_status_Null_argument);
        return -1;
    }
    int32_t i;
    for (i = 0; i < g->next_node_id; i++) {
        if (g->nodes[i].type == flow_ntype_Resource_Placeholder || g->nodes[i].type == flow_ntype_Encoder_Placeholder) {
            if (placeholder_id == -1) {
                return i;
            } else {
                if (get_placeholder_id_for_node(c, g, i) == placeholder_id) {
                    return i;
                }
            }
        }
    }
    return -404;
}

static bool flow_job_insert_resources_into_graph_with_reuse(flow_context* c, struct flow_job* job,
                                                            struct flow_graph** graph_ref)
{
    if (graph_ref == NULL || *graph_ref == NULL) {
        FLOW_error(c, flow_status_Null_argument);
        return false;
    }

    struct flow_job_resource_item* current = job->resources_head;
    int32_t next_match;
    int32_t replacement_node_id;
    int32_t match_count;
    while (current != NULL) {
        match_count = 0;
        do {
            // flow_graph_print_to(c,g,stderr);

            next_match = flow_job_find_first_node_with_placeholder_id(c, *graph_ref, current->graph_placeholder_id);
            if (next_match >= 0) {
                replacement_node_id = create_node_for_resource(c, job, current, graph_ref, next_match);
                if (replacement_node_id < 0) {
                    FLOW_error_return(c);
                }
                // flow_graph_print_to(c, g, stderr);
                if (!flow_graph_duplicate_edges_to_another_node(c, graph_ref, next_match, replacement_node_id, true,
                                                                true)) {
                    FLOW_error_return(c);
                }
                // flow_graph_print_to(c, g, stderr);
                if (!flow_node_delete(c, *graph_ref, next_match)) {
                    FLOW_error_return(c);
                }
                // flow_graph_print_to(c, g, stderr);
                match_count++;
                flow_job_notify_graph_changed(c, job, *graph_ref);
            }
        } while (next_match >= 0);

        if (match_count == 0) {
            // No matching nodes exist in the graph
            // This is a warning situation
        }

        current = current->next;
    }
    if (flow_job_find_first_node_with_placeholder_id(c, *graph_ref, -1) > -1) {
        // Leftover nodes with no placeholder - this is an error
        FLOW_error(c, flow_status_Graph_could_not_be_completed);
        return false;
    }
    return true;
}

bool flow_job_insert_resources_into_graph(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref)
{
    if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
        FLOW_error_return(c);
    }
    if (!flow_job_insert_resources_into_graph_with_reuse(c, job, graph_ref)) {
        FLOW_error_return(c);
    }
    return true;
}
