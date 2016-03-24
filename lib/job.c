#include <png.h>

#include "job.h"
#include "graph.h"

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
    // TODO: Free all the resources, and their codecs
    FLOW_free(c, job);
}

static int32_t flow_job_add_resource(flow_context* c, struct flow_job* job, FLOW_DIRECTION dir,
                                     int32_t graph_placeholder_id, flow_job_resource_type type, void* data)
{
    struct flow_job_resource_item* r
        = (struct flow_job_resource_item*)FLOW_malloc(c, sizeof(struct flow_job_resource_item));
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
    r->codec_type = flow_job_codec_type_null;
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
        flow_job_get_resource(c, job, id)->codec_type = flow_job_codec_type_bitmap_bgra_pointer;
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

static bool write_frame_to_disk(flow_context* c, const char* path, flow_bitmap_bgra* b)
{

    png_image target_image;
    memset(&target_image, 0, sizeof target_image);
    target_image.version = PNG_IMAGE_VERSION;
    target_image.opaque = NULL;
    target_image.width = b->w;
    target_image.height = b->h;
    target_image.format = PNG_FORMAT_BGRA;
    target_image.flags = 0;
    target_image.colormap_entries = 0;

    if (!png_image_write_to_file(&target_image, path, 0 /*convert_to_8bit*/, b->pixels, 0 /*row_stride*/,
                                 NULL /*colormap*/)) {
        FLOW_error_msg(c, flow_status_Image_encoding_failed, "Failed to export frame as png: %s  output path: %s.",
                       target_image.message, path);
        return false;
    }
    return true;
}

static bool files_identical(flow_context* c, const char* path1, const char* path2, bool* identical)
{
    FILE* fp1 = fopen(path1, "r");
    if (fp1 == NULL) {
        FLOW_error_msg(c, flow_status_IO_error, "Failed to open file A for comparison (%s).", path1);
        return false;
    }
    FILE* fp2 = fopen(path2, "r");
    if (fp2 == NULL) {
        FLOW_error_msg(c, flow_status_IO_error, "Failed to open file B for comparison (%s).", path2);
        fclose(fp1);
        return false;
    }
    int ch1 = getc(fp1);
    int ch2 = getc(fp2);

    while ((ch1 != EOF) && (ch2 != EOF) && (ch1 == ch2)) {
        ch1 = getc(fp1);
        ch2 = getc(fp2);
    }

    *identical = (ch1 == ch2);
    fclose(fp1);
    fclose(fp2);
    return true;
}
#define FLOW_MAX_GRAPH_VERSIONS 100

bool flow_job_notify_node_complete(flow_context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id)
{
    struct flow_node* n = &g->nodes[node_id];
    if (n->result_bitmap != NULL && job->record_frame_images == true) {
        char path[1024];
        flow_snprintf(path, 1023, "node_frames/job_%d_node_%d.png", job->debug_job_id, node_id);
        if (!write_frame_to_disk(c, path, n->result_bitmap)) {
            FLOW_error_return(c);
        }
    }
    return true;
}

bool flow_job_notify_graph_changed(flow_context* c, struct flow_job* job, struct flow_graph* g)
{
    if (job == NULL || !job->record_graph_versions || job->next_graph_version > FLOW_MAX_GRAPH_VERSIONS)
        return true;

    char filename[255];
    char image_prefix[255];
    char prev_filename[255];

    if (job->next_graph_version == 0) {
        // Delete existing graphs
        int32_t i = 0;
        for (i = 0; i <= FLOW_MAX_GRAPH_VERSIONS; i++) {
            flow_snprintf(filename, 254, "job_%d_graph_version_%d.dot", job->debug_job_id, i);
            remove(filename);
            flow_snprintf(filename, 254, "job_%d_graph_version_%d.dot.png", job->debug_job_id, i);
            remove(filename);
            flow_snprintf(filename, 254, "job_%d_graph_version_%d.dot.svg", job->debug_job_id, i);
            remove(filename);
            int32_t node_ix = 0;
            for (node_ix = 0; node_ix < 42; node_ix++) {
                flow_snprintf(filename, 254, "./node_frames/job_%d_node_%d.png", job->debug_job_id, node_ix);
                remove(filename);
            }
        }
    }

    int32_t prev_graph_version = job->next_graph_version - 1;
    int32_t current_graph_version = job->next_graph_version;
    job->next_graph_version++;

    flow_snprintf(filename, 254, "job_%d_graph_version_%d.dot", job->debug_job_id, current_graph_version);

    flow_snprintf(image_prefix, 254, "./node_frames/job_%d_node_", job->debug_job_id);

    FILE* f = fopen(filename, "w");
    if (f == NULL) {
        FLOW_error_msg(c, flow_status_IO_error, "Failed to open %s for graph dotfile export.", filename);
        return false;
    }
    if (!flow_graph_print_to_dot(c, g, f, image_prefix)) {
        fclose(f);
        FLOW_error_return(c);
    } else {
        fclose(f);
    }
    // Compare
    if (job->next_graph_version > 1) {
        flow_snprintf(prev_filename, 254, "job_%d_graph_version_%d.dot", job->debug_job_id, prev_graph_version);
        bool identical = false;
        if (!files_identical(c, prev_filename, filename, &identical)) {
            FLOW_error_return(c);
        }
        if (identical) {
            job->next_graph_version--; // Next time we will overwrite the duplicate graph. The last two graphs may
            // remain dupes.
            remove(filename);
        } else if (job->render_graph_versions) {
            flow_job_render_graph_to_png(c, job, g, prev_graph_version);
        }
    }

    return true;
}

bool flow_job_render_graph_to_png(flow_context* c, struct flow_job* job, struct flow_graph* g, int32_t graph_version)
{
    char filename[255];
    flow_snprintf(filename, 254, "job_%d_graph_version_%d.dot", job->debug_job_id, graph_version);

    char dotfile_command[2048];
    flow_snprintf(dotfile_command, 2048, "dot -Tpng -Gsize=11,16\\! -Gdpi=150  -O %s", filename);
    int32_t ignore = system(dotfile_command);
    ignore++;
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
