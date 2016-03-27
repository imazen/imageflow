#include <png.h>


#include "imageflow_private.h"
#include "nodes.h"
#include "codecs.h"

struct flow_job* flow_job_create(flow_context* c)
{

    struct flow_job* job = (struct flow_job*)FLOW_malloc(c, sizeof(struct flow_job));
    if (job == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        return NULL;
    }
    static int32_t job_id = 0;
    flow_job_configure_recording(c, job, false, false, false, false, false);
    job->next_graph_version = 0;
    job->debug_job_id = job_id++;
    job->next_resource_id = 0x800;
    job->resources_head = NULL;
    job->max_calc_flatten_execute_passes = 6;
    return job;
}

bool flow_job_configure_recording(flow_context* c, struct flow_job* job, bool record_graph_versions,
                                  bool record_frame_images, bool render_last_graph, bool render_graph_versions,
                                  bool render_animated_graph)
{
    job->record_frame_images = record_frame_images;
    job->record_graph_versions = record_graph_versions;
    job->render_last_graph = render_last_graph;
    job->render_graph_versions = render_graph_versions && record_graph_versions;
    job->render_animated_graph = render_animated_graph && job->render_graph_versions;
    return true;
}
void flow_job_destroy(flow_context* c, struct flow_job* job)
{
    FLOW_destroy(c, job);
}

static int32_t flow_job_add_resource(flow_context* c, struct flow_job* job, FLOW_DIRECTION dir,
                                     int32_t graph_placeholder_id, flow_job_resource_type type, void* data)
{
    struct flow_job_resource_item* r
        = (struct flow_job_resource_item*)FLOW_malloc_owned(c, sizeof(struct flow_job_resource_item), job);
    if (r == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        return -2;
    }
    r->next = NULL;
    r->id = job->next_resource_id;
    job->next_resource_id++;
    r->graph_placeholder_id = graph_placeholder_id;
    r->data = data;
    r->direction = dir;
    r->type = type;
    r->codec_state = NULL;
    r->codec_type = flow_codec_type_null;
    if (job->resources_head == NULL) {
        job->resources_head = r;
        job->resources_tail = r;
    } else {
        job->resources_tail->next = r;
        job->resources_tail = r;
    }
    return r->id;
}

static struct flow_job_resource_item* flow_job_get_resource(flow_context* c, struct flow_job* job, int32_t resource_id)
{
    struct flow_job_resource_item* current = job->resources_head;
    while (current != NULL) {
        if (current->id == resource_id) {
            return current;
        }
        current = current->next;
    }
    return NULL;
}
int32_t flow_job_add_bitmap_bgra(flow_context* c, struct flow_job* job, FLOW_DIRECTION dir,
                                 int32_t graph_placeholder_id, flow_bitmap_bgra* bitmap)
{
    int32_t id = flow_job_add_resource(c, job, dir, graph_placeholder_id, flow_job_resource_type_bitmap_bgra, bitmap);
    if (id >= 0) {
        flow_job_get_resource(c, job, id)->codec_type = flow_codec_type_bitmap_bgra_pointer;
    }
    return id;
}

flow_bitmap_bgra* flow_job_get_bitmap_bgra(flow_context* c, struct flow_job* job, int32_t resource_id)
{
    struct flow_job_resource_item* r = flow_job_get_resource(c, job, resource_id);
    if (r == NULL || r->data == NULL)
        return NULL;
    return (flow_bitmap_bgra*)r->data;
}

struct flow_job_resource_buffer* flow_job_get_buffer(flow_context* c, struct flow_job* job, int32_t resource_id)
{
    struct flow_job_resource_item* r = flow_job_get_resource(c, job, resource_id);
    if (r == NULL || r->data == NULL)
        return NULL;
    return (struct flow_job_resource_buffer*)r->data;
}

int32_t flow_job_add_buffer(flow_context* c, struct flow_job* job, FLOW_DIRECTION dir, int32_t graph_placeholder_id,
                            void* buffer, size_t buffer_size, bool owned_by_job)
{
    struct flow_job_resource_buffer* resource
        = (struct flow_job_resource_buffer*)FLOW_calloc(c, 1, sizeof(struct flow_job_resource_buffer));

    resource->buffer = buffer;
    resource->buffer_size = buffer_size;
    resource->owned_by_job = owned_by_job;
    resource->codec_state = NULL;

    int32_t id = flow_job_add_resource(c, job, dir, graph_placeholder_id, flow_job_resource_type_buffer, resource);
    return id;
}

bool flow_job_execute(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref)
{
    if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
        FLOW_error_return(c);
    }
    // States for a node
    // New
    // OutboundDimensionsKnown
    // Flattened
    // Optimized
    // LockedForExecution
    // Executed
    int32_t passes = 0;
    while (!flow_job_graph_fully_executed(c, job, *graph_ref)) {
        if (passes >= job->max_calc_flatten_execute_passes) {
            FLOW_error(c, flow_status_Maximum_graph_passes_exceeded);
            return false;
        }
        if (!flow_job_populate_dimensions_where_certain(c, job, graph_ref)) {
            FLOW_error_return(c);
        }
        if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
            FLOW_error_return(c);
        }
        if (!flow_graph_pre_optimize_flatten(c, graph_ref)) {
            FLOW_error_return(c);
        }
        if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
            FLOW_error_return(c);
        }
        if (!flow_job_populate_dimensions_where_certain(c, job, graph_ref)) {
            FLOW_error_return(c);
        }
        if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
            FLOW_error_return(c);
        }
        if (!flow_graph_optimize(c, job, graph_ref)) {
            FLOW_error_return(c);
        }
        if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
            FLOW_error_return(c);
        }
        if (!flow_job_populate_dimensions_where_certain(c, job, graph_ref)) {
            FLOW_error_return(c);
        }
        if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
            FLOW_error_return(c);
        }
        if (!flow_graph_post_optimize_flatten(c, job, graph_ref)) {
            FLOW_error_return(c);
        }
        if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
            FLOW_error_return(c);
        }
        if (!flow_job_populate_dimensions_where_certain(c, job, graph_ref)) {
            FLOW_error_return(c);
        }
        if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
            FLOW_error_return(c);
        }
        if (!flow_job_execute_where_certain(c, job, graph_ref)) {
            FLOW_error_return(c);
        }
        passes++;

        if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
            FLOW_error_return(c);
        }
    }
    if (job->next_graph_version > 0 && job->render_last_graph
        && !flow_job_render_graph_to_png(c, job, *graph_ref, job->next_graph_version - 1)) {
        FLOW_error_return(c);
    }
    return true;
}

static bool node_visitor_post_optimize_flatten(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref,
                                               int32_t node_id, bool* quit, bool* skip_outbound_paths,
                                               void* custom_data)
{

    if (!flow_node_update_state(c, *graph_ref, node_id)) {
        FLOW_error_return(c);
    }
    struct flow_node* n = &(*graph_ref)->nodes[node_id];

    // If input nodes are populated
    if (n->state == flow_node_state_ReadyForPostOptimizeFlatten) {
        if (!flow_node_post_optimize_flatten(c, graph_ref, node_id)) {
            FLOW_error_return(c);
        }
        if (!flow_graph_validate(c, *graph_ref)) {
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

bool flow_graph_post_optimize_flatten(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref)
{
    if (*graph_ref == NULL) {
        FLOW_error(c, flow_status_Null_argument);
        return false;
    }
    bool re_walk;
    do {
        re_walk = false;
        if (!flow_graph_walk(c, job, graph_ref, node_visitor_post_optimize_flatten, NULL, &re_walk)) {
            FLOW_error_return(c);
        }
    } while (re_walk);
    return true;
}

static bool node_visitor_optimize(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref, int32_t node_id,
                                  bool* quit, bool* skip_outbound_paths, void* custom_data)
{

    struct flow_node* node = &(*graph_ref)->nodes[node_id];
    if (node->state == flow_node_state_ReadyForOptimize) {
        node->state = (flow_node_state)(node->state | flow_node_state_Optimized);
    }

    // Implement optimizations
    return true;
}

bool flow_graph_optimize(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref)
{
    if (*graph_ref == NULL) {
        FLOW_error(c, flow_status_Null_argument);
        return false;
    }
    bool re_walk;
    do {
        re_walk = false;
        if (!flow_graph_walk(c, job, graph_ref, node_visitor_optimize, NULL, &re_walk)) {
            FLOW_error_return(c);
        }
    } while (re_walk);
    return true;
}

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

static bool flow_job_node_is_executed(flow_context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id)
{
    return (g->nodes[node_id].state & flow_node_state_Executed) > 0;
}
bool flow_job_graph_fully_executed(flow_context* c, struct flow_job* job, struct flow_graph* g)
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

static bool node_visitor_execute(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref, int32_t node_id,
                                 bool* quit, bool* skip_outbound_paths, void* custom_data)
{

    if (!flow_node_update_state(c, *graph_ref, node_id)) {
        FLOW_error_return(c);
    }
    struct flow_node* n = &(*graph_ref)->nodes[node_id];

    if (!flow_job_node_is_executed(c, job, *graph_ref, node_id) && n->state == flow_node_state_ReadyForExecution) {
        uint64_t now = flow_get_high_precision_ticks();
        if (!flow_node_execute(c, job, *graph_ref, node_id)) {
            FLOW_error_return(c);
        } else {
            (*graph_ref)->nodes[node_id].ticks_elapsed += flow_get_high_precision_ticks() - now;
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

bool flow_job_execute_where_certain(flow_context* c, struct flow_job* job, struct flow_graph** g)
{
    if (*g == NULL) {
        FLOW_error(c, flow_status_Null_argument);
        return false;
    }

    //    //Resets and creates state tracking for this graph
    //    if (!flow_job_create_state(c,job, *g)){
    //        FLOW_error_return(c);
    //    }

    if (!flow_graph_walk_dependency_wise(c, job, g, node_visitor_execute, NULL, NULL)) {
        FLOW_error_return(c);
    }
    return true;
}

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
        flow_codec_type ctype = flow_job_codec_select(c, job, (uint8_t*)buf->buffer, buf->buffer_size);
        if (ctype == flow_codec_type_null) {
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

        flow_codec_type codec_type = flow_codec_type_encode_png;

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
    int remaining_placeholder_node = flow_job_find_first_node_with_placeholder_id(c, *graph_ref, -1);
    if (remaining_placeholder_node > -1) {
        // Leftover nodes with no placeholder - this is an error
        FLOW_error_msg(c, flow_status_Graph_invalid, "No matching job resource found for placeholder id %d (node #%d).",
                       get_placeholder_id_for_node(c, *graph_ref, remaining_placeholder_node),
                       remaining_placeholder_node);
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
