#include "imageflow_private.h"
#include "math_functions.h"
#include "nodes.h"

static size_t flow_graph_size_for(uint32_t max_edges, uint32_t max_nodes, uint32_t max_info_bytes)
{
    return sizeof(struct flow_graph) + sizeof(struct flow_edge) * max_edges + sizeof(struct flow_node) * max_nodes
           + max_info_bytes;
}

struct flow_graph * flow_graph_create(flow_c * c, uint32_t max_edges, uint32_t max_nodes, uint32_t max_info_bytes,
                                      float growth_factor)
{
    size_t total_bytes = flow_graph_size_for(max_edges, max_nodes, max_info_bytes);
    struct flow_graph * g = (struct flow_graph *)FLOW_malloc(c, total_bytes);

    if (g == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        return NULL;
    }
    g->memory_layout_version = 1;
    g->growth_factor = growth_factor;

    g->deleted_bytes = 0;
    g->max_info_bytes = max_info_bytes;
    g->next_info_byte = 0;

    g->edge_count = 0;
    g->max_edges = max_edges;
    g->next_edge_id = 0;

    g->node_count = 0;
    g->max_nodes = max_nodes;
    g->next_node_id = 0;

    g->edges = (struct flow_edge *)(((size_t)g) + sizeof(struct flow_graph));
    g->nodes = (struct flow_node *)(((size_t)g->edges) + sizeof(struct flow_edge) * max_edges);
    g->info_bytes = (uint8_t *)(((size_t)g->nodes) + sizeof(struct flow_node) * max_nodes);
    if (((size_t)g->info_bytes - (size_t)g) != total_bytes - max_info_bytes) {
        // Somehow our math was inconsistent with flow_graph_size_for()
        FLOW_error(c, flow_status_Invalid_internal_state);
        FLOW_destroy(c, g); // No destructor or child items to fail
        return NULL;
    }
    if ((size_t)&g->edges[g->max_edges - 1].info_bytes >= (size_t)&g->nodes[0].type) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        FLOW_destroy(c, g); // No destructor or child items to fail
        return NULL;
    }
    if ((size_t)&g->nodes[g->max_nodes - 1].ticks_elapsed >= (size_t)&g->info_bytes[0]) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        FLOW_destroy(c, g); // No destructor or child items to fail
        return NULL;
    }
    return g;
}

struct flow_graph * flow_graph_copy_and_resize(flow_c * c, struct flow_graph * from, uint32_t max_edges,
                                               uint32_t max_nodes, uint32_t max_info_bytes)
{
    if ((int32_t)max_edges < from->next_edge_id || (int32_t)max_nodes < from->next_node_id
        || (int32_t)max_info_bytes < from->next_info_byte) {
        FLOW_error(c, flow_status_Invalid_argument);
        return NULL;
    }
    struct flow_graph * g = flow_graph_create(c, max_edges, max_nodes, max_info_bytes, from->growth_factor);
    g->growth_factor = from->growth_factor;

    g->deleted_bytes = from->deleted_bytes;
    g->max_info_bytes = max_info_bytes;
    g->next_info_byte = from->next_info_byte;
    g->max_nodes = max_nodes;
    g->node_count = from->node_count;
    g->next_node_id = from->next_node_id;
    g->max_edges = max_edges;
    g->edge_count = from->edge_count;
    g->next_edge_id = from->next_edge_id;
    memcpy(g->info_bytes, from->info_bytes, from->next_info_byte);
    memcpy(g->edges, from->edges, from->next_edge_id * sizeof(struct flow_edge));
    memcpy(g->nodes, from->nodes, from->next_node_id * sizeof(struct flow_node));
    return g;
}

struct flow_graph * flow_graph_copy(flow_c * c, struct flow_graph * from)
{
    return flow_graph_copy_and_resize(c, from, from->max_edges, from->max_nodes, from->max_info_bytes);
}

void flow_graph_destroy(flow_c * c, struct flow_graph * g) { FLOW_free(c, g); }

int32_t flow_node_create_generic(flow_c * c, struct flow_graph ** graph_ref, int32_t prev_node, flow_ntype type)
{
    if (graph_ref == NULL || (*graph_ref) == NULL) {
        FLOW_error(c, flow_status_Null_argument);
        return -20;
    }
    int32_t nodeinfo_size = flow_node_fixed_infobyte_count(c, type);
    if (nodeinfo_size < 0) {
        FLOW_add_to_callstack(c);
        return nodeinfo_size;
    }
    if (!flow_graph_replace_if_too_small(c, graph_ref, 1, prev_node >= 0 ? 1 : 0, nodeinfo_size)) {
        FLOW_add_to_callstack(c);
        return -2;
    }
    struct flow_graph * g = *graph_ref;
    int32_t id = g->next_node_id;

    g->nodes[id].type = type;
    g->nodes[id].info_byte_index = g->next_info_byte;
    g->nodes[id].info_bytes = nodeinfo_size;
    g->nodes[id].state = flow_node_state_Blank;
    g->nodes[id].result_bitmap = NULL;
    g->nodes[id].ticks_elapsed = 0;
    g->nodes[id].result_format = flow_bgra32;
    g->nodes[id].result_width = 0;
    g->nodes[id].result_height = 0;
    g->nodes[id].result_alpha_meaningful = true;

    g->next_info_byte += g->nodes[id].info_bytes;
    g->next_node_id += 1;
    g->node_count += 1;
    if (prev_node >= 0) {
        if (flow_edge_create(c, graph_ref, prev_node, id, flow_edgetype_input) < 0) {
            FLOW_add_to_callstack(c);
            return -3;
        }
    }

    return id;
}

static void * FrameNode_get_node_info_pointer(struct flow_graph * g, int32_t node_id)
{
    return &(g->info_bytes[g->nodes[node_id].info_byte_index]);
}
int32_t flow_node_create_canvas(flow_c * c, struct flow_graph ** g, int32_t prev_node, flow_pixel_format format,
                                size_t width, size_t height, uint32_t bgcolor)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Create_Canvas);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_createcanvas * info
        = (struct flow_nodeinfo_createcanvas *)FrameNode_get_node_info_pointer(*g, id);
    info->format = format;
    info->width = width;
    info->height = height;
    info->bgcolor = bgcolor;
    return id;
}

int32_t flow_node_create_noop(flow_c * c, struct flow_graph ** g, int32_t prev_node)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Noop);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    return id;
}

int32_t flow_node_create_primitive_flip_vertical(flow_c * c, struct flow_graph ** g, int32_t prev_node)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_primitive_Flip_Vertical_Mutate);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    return id;
}
int32_t flow_node_create_primitive_flip_horizontal(flow_c * c, struct flow_graph ** g, int32_t prev_node)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_primitive_Flip_Horizontal_Mutate);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    return id;
}
int32_t flow_node_create_clone(flow_c * c, struct flow_graph ** g, int32_t prev_node)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Clone);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    return id;
}

int32_t flow_node_create_transpose(flow_c * c, struct flow_graph ** g, int32_t prev_node)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Transpose);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    return id;
}
int32_t flow_node_create_rotate_90(flow_c * c, struct flow_graph ** g, int32_t prev_node)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Rotate_90);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    return id;
}
int32_t flow_node_create_rotate_180(flow_c * c, struct flow_graph ** g, int32_t prev_node)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Rotate_180);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    return id;
}
int32_t flow_node_create_rotate_270(flow_c * c, struct flow_graph ** g, int32_t prev_node)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Rotate_270);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    return id;
}
int32_t flow_node_create_primitive_crop(flow_c * c, struct flow_graph ** g, int32_t prev_node, uint32_t x1, uint32_t y1,
                                        uint32_t x2, uint32_t y2)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_primitive_Crop_Mutate_Alias);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_crop * info = (struct flow_nodeinfo_crop *)FrameNode_get_node_info_pointer(*g, id);
    info->x1 = x1;
    info->y1 = y1;
    info->x2 = x2;
    info->y2 = y2;
    return id;
}

int32_t flow_node_create_primitive_copy_rect_to_canvas(flow_c * c, struct flow_graph ** g, int32_t prev_node,
                                                       uint32_t from_x, uint32_t from_y, uint32_t width,
                                                       uint32_t height, uint32_t x, uint32_t y)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_primitive_CopyRectToCanvas);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_copy_rect_to_canvas * info
        = (struct flow_nodeinfo_copy_rect_to_canvas *)FrameNode_get_node_info_pointer(*g, id);
    info->x = x;
    info->y = y;
    info->width = width;
    info->height = height;
    info->from_x = from_x;
    info->from_y = from_y;
    return id;
}

int32_t flow_node_create_expand_canvas(flow_c * c, struct flow_graph ** g, int32_t prev_node, uint32_t left,
                                       uint32_t top, uint32_t right, uint32_t bottom, uint32_t canvas_color_srgb)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Expand_Canvas);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_expand_canvas * info
        = (struct flow_nodeinfo_expand_canvas *)FrameNode_get_node_info_pointer(*g, id);
    info->left = left;
    info->top = top;
    info->right = right;
    info->bottom = bottom;
    info->canvas_color_srgb = canvas_color_srgb;
    return id;
}

int32_t flow_node_create_fill_rect(flow_c * c, struct flow_graph ** g, int32_t prev_node, uint32_t x1, uint32_t y1,
                                   uint32_t x2, uint32_t y2, uint32_t color_srgb)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Fill_Rect_Mutate);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_fill_rect * info = (struct flow_nodeinfo_fill_rect *)FrameNode_get_node_info_pointer(*g, id);
    info->x1 = x1;
    info->y1 = y1;
    info->x2 = x2;
    info->y2 = y2;
    info->color_srgb = color_srgb;
    return id;
}
bool flow_node_set_decoder_downscale_hint(flow_c * c, struct flow_graph * g, int32_t node_id, int64_t if_wider_than,
                                          int64_t or_taller_than, int64_t downscaled_min_width,
                                          int64_t downscaled_min_height)
{

    struct flow_nodeinfo_codec * info = (struct flow_nodeinfo_codec *)FrameNode_get_node_info_pointer(g, node_id);
    info->downscale_hints.downscaled_min_height = downscaled_min_height;
    info->downscale_hints.downscaled_min_width = downscaled_min_width;
    info->downscale_hints.downscale_if_wider_than = if_wider_than;
    info->downscale_hints.or_if_taller_than = or_taller_than;
    return true;
}

int32_t flow_node_create_scale(flow_c * c, struct flow_graph ** g, int32_t prev_node, size_t width, size_t height,
                               flow_interpolation_filter downscale_filter, flow_interpolation_filter upscale_filter) //,  flow_interpolation_filter downscale_filter,   flow_interpolation_filter upscale_filter)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Scale);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_scale * info = (struct flow_nodeinfo_scale *)FrameNode_get_node_info_pointer(*g, id);
    info->width = (int32_t)width;
    info->height = (int32_t)height;
    info->downscale_filter =downscale_filter;
    info->upscale_filter = upscale_filter;
    return id;
}

int32_t flow_node_create_decoder(flow_c * c, struct flow_graph ** g, int32_t prev_node, int32_t placeholder_id)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_decoder);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }

    struct flow_nodeinfo_codec * info = (struct flow_nodeinfo_codec *)FrameNode_get_node_info_pointer(*g, id);
    info->placeholder_id = placeholder_id;
    info->codec = NULL;
    info->downscale_hints.downscale_if_wider_than = -1;
    info->downscale_hints.or_if_taller_than = -1;
    info->downscale_hints.downscaled_min_height = -1;
    info->downscale_hints.downscaled_min_width = -1;

    return id;
}
int32_t flow_node_create_encoder_placeholder(flow_c * c, struct flow_graph ** g, int32_t prev_node,
                                             int32_t placeholder_id)
{
    return flow_node_create_encoder(c, g, prev_node, placeholder_id, 0);
}
int32_t flow_node_create_encoder(flow_c * c, struct flow_graph ** g, int32_t prev_node, int32_t placeholder_id,
                                 int64_t desired_encoder_id)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_encoder);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }

    struct flow_nodeinfo_codec * info = (struct flow_nodeinfo_codec *)FrameNode_get_node_info_pointer(*g, id);
    info->placeholder_id = placeholder_id;
    info->codec = NULL;
    info->desired_encoder_id = desired_encoder_id;
    return id;
}

int32_t flow_node_create_bitmap_bgra_reference(flow_c * c, struct flow_graph ** g, int32_t prev_node,
                                               struct flow_bitmap_bgra ** pointer_to_pointer_to_bitmap_bgra)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_primitive_bitmap_bgra_pointer);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_bitmap_bgra_pointer * info
        = (struct flow_nodeinfo_bitmap_bgra_pointer *)FrameNode_get_node_info_pointer(*g, id);
    info->ref = pointer_to_pointer_to_bitmap_bgra;
    return id;
}

int32_t flow_node_create_render_to_canvas_1d(flow_c * c, struct flow_graph ** g, int32_t prev_node,
                                             bool transpose_on_write, uint32_t canvas_x, uint32_t canvas_y,
                                             int32_t scale_to_width,
                                             flow_working_floatspace scale_and_filter_in_colorspace,
                                             float sharpen_percent, flow_compositing_mode compositing_mode,
                                             uint8_t * matte_color[4], struct flow_scanlines_filter * filter_list,
                                             flow_interpolation_filter interpolation_filter)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_primitive_RenderToCanvas1D);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_render_to_canvas_1d * info
        = (struct flow_nodeinfo_render_to_canvas_1d *)FrameNode_get_node_info_pointer(*g, id);
    info->transpose_on_write = transpose_on_write;

    info->scale_to_width = scale_to_width;
    info->interpolation_filter = interpolation_filter;
    info->scale_in_colorspace = scale_and_filter_in_colorspace;
    info->sharpen_percent_goal = sharpen_percent;
    info->compositing_mode = compositing_mode;
    info->filter_list = filter_list;
    info->canvas_x = canvas_x;
    info->canvas_y = canvas_y;
    memcpy(&info->matte_color, matte_color, 4);
    return id;
}

int32_t flow_node_create_render1d(flow_c * c, struct flow_graph ** g, int32_t prev_node, bool transpose_on_write,
                                  int32_t scale_to_width, flow_working_floatspace scale_and_filter_in_colorspace,
                                  float sharpen_percent, struct flow_scanlines_filter * filter_list,
                                  flow_interpolation_filter interpolation_filter)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Render1D);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_render_to_canvas_1d * info
        = (struct flow_nodeinfo_render_to_canvas_1d *)FrameNode_get_node_info_pointer(*g, id);
    info->transpose_on_write = transpose_on_write;

    info->scale_to_width = scale_to_width;
    info->interpolation_filter = interpolation_filter;
    info->scale_in_colorspace = scale_and_filter_in_colorspace;
    info->sharpen_percent_goal = sharpen_percent;
    info->compositing_mode = flow_compositing_mode_overwrite;
    info->filter_list = filter_list;
    info->canvas_x = 0;
    info->canvas_y = 0;
    info->matte_color[0] = 0;
    info->matte_color[1] = 0;
    info->matte_color[2] = 0;
    info->matte_color[3] = 0;
    return id;
}

bool flow_edge_delete(flow_c * c, struct flow_graph * g, int32_t edge_id)
{
    if (edge_id < 0 || edge_id >= g->next_edge_id) {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }
    struct flow_edge * e = &g->edges[edge_id];
    if (e->type == flow_edgetype_null) {
        FLOW_error(c, flow_status_Item_does_not_exist);
        return false;
    } else {
        g->deleted_bytes += e->info_bytes;
        g->edge_count--;

        e->type = flow_edgetype_null;
        e->info_byte_index = -1;
        e->info_bytes = 0;
        e->from = -1;
        e->to = -1;
        return true;
    }
}

bool flow_edge_delete_all_connected_to_node(flow_c * c, struct flow_graph * g, int32_t node_id)
{
    struct flow_edge * current_edge;
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++) {
        current_edge = &g->edges[i];
        if (current_edge->type != flow_edgetype_null) {
            if (current_edge->from == node_id || current_edge->to == node_id) {
                if (!flow_edge_delete(c, g, i)) {
                    FLOW_error_return(c);
                }
            }
        }
    }
    return true;
}

bool flow_graph_replace_if_too_small(flow_c * c, struct flow_graph ** g, uint32_t free_nodes_required,
                                     uint32_t free_edges_required, uint32_t free_bytes_required)
{
    float growth_factor = (float)fmax((*g)->growth_factor, 1.0f);
    if ((int32_t)free_nodes_required > (*g)->max_nodes - (*g)->next_node_id
        || (int32_t)free_edges_required > (*g)->max_edges - (*g)->next_edge_id
        || (int32_t)free_bytes_required > (*g)->max_info_bytes - (*g)->next_info_byte) {
        int32_t min_nodes = int_max((*g)->max_nodes, (*g)->next_node_id + free_nodes_required);
        int32_t min_edges = int_max((*g)->max_edges, (*g)->next_edge_id + free_edges_required);
        int32_t min_bytes = int_max((*g)->max_info_bytes, (*g)->next_info_byte + free_bytes_required);
        struct flow_graph * new_graph = flow_graph_copy_and_resize(
            c, (*g), (uint32_t)(growth_factor * (float)min_nodes), (uint32_t)(growth_factor * (float)min_edges),
            (uint32_t)(growth_factor * (float)min_bytes));
        if (new_graph == NULL) {
            FLOW_error_return(c);
        }
        struct flow_graph * old = *g;
        *g = new_graph; // Swap the pointer out
        flow_graph_destroy(c, old); // Delete the old graph
    }
    return true;
}

int32_t flow_graph_copy_info_bytes_to(flow_c * c, struct flow_graph * from, struct flow_graph ** to, int32_t byte_index,
                                      int32_t byte_count)
{
    if (byte_index < 0 || byte_count == 0) {
        return -1;
    }
    int32_t new_index = (*to)->next_info_byte;
    if ((*to)->max_info_bytes <= new_index + byte_count) {
        if (!flow_graph_replace_if_too_small(c, to, 0, 0, byte_count)) {
            FLOW_add_to_callstack(c); // OOM
            return -2;
        }
    }
    memcpy(&(*to)->info_bytes[new_index], &from->info_bytes[byte_index], byte_count);
    (*to)->next_info_byte += byte_count;
    return new_index;
}

int32_t flow_edge_create(flow_c * c, struct flow_graph ** g, int32_t from, int32_t to, flow_edgetype type)
{
    if ((*g)->next_edge_id >= (*g)->max_edges) {
        if (!flow_graph_replace_if_too_small(c, g, 0, 1, 0)) {
            FLOW_add_to_callstack(c); // OOM
            return -2;
        }
    }

    struct flow_edge * e = &(*g)->edges[(*g)->next_edge_id];
    e->type = type;
    e->from = from;
    e->to = to;
    e->info_bytes = 0;
    e->info_byte_index = -1;
    (*g)->edge_count++;
    (*g)->next_edge_id++;
    return (*g)->next_edge_id - 1;
}
int32_t flow_edge_duplicate(flow_c * c, struct flow_graph ** g, int32_t edge_id)
{
    struct flow_edge old_copy = (*g)->edges[edge_id];
    int32_t new_id = flow_edge_create(c, g, old_copy.from, old_copy.to, old_copy.type);
    if (new_id < 0) {
        FLOW_add_to_callstack(c);
        return -1;
    }
    struct flow_edge * e = &(*g)->edges[new_id];

    if (old_copy.info_byte_index >= 0 && old_copy.info_bytes > 0) {
        e->info_bytes = old_copy.info_bytes;
        e->info_byte_index = flow_graph_copy_info_bytes_to(c, *g, g, old_copy.info_byte_index, old_copy.info_bytes);
        if (e->info_byte_index < 0) {
            FLOW_add_to_callstack(c);
            return e->info_byte_index;
        }
    }
    return new_id;
}

bool flow_graph_duplicate_edges_to_another_node(flow_c * c, struct flow_graph ** graph_ref, int32_t from_node,
                                                int32_t to_node, bool copy_inbound, bool copy_outbound)
{
    int32_t i = -1;

    int32_t old_edge_count = (*graph_ref)->next_edge_id;
    for (i = 0; i < old_edge_count; i++) {
        // This current_edge reference becomes invalid as soon as flow_edge_duplicate is called
        struct flow_edge * current_edge = &(*graph_ref)->edges[i];
        if (current_edge->type != flow_edgetype_null) {
            if ((copy_outbound && current_edge->from == from_node) || (copy_inbound && current_edge->to == from_node)) {
                int32_t new_edge_id = flow_edge_duplicate(c, graph_ref, i);
                if (new_edge_id < 0) {
                    FLOW_add_to_callstack(c);
                    return false;
                }
                struct flow_edge * new_edge = &(*graph_ref)->edges[new_edge_id];

                if (new_edge->from == from_node) {
                    new_edge->from = to_node;
                }
                if (new_edge->to == from_node) {
                    new_edge->to = to_node;
                }
            }
        }
    }
    return true;
}

bool flow_node_delete(flow_c * c, struct flow_graph * g, int32_t node_id)
{
    if (node_id < 0 || node_id >= g->next_node_id) {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }
    struct flow_node * n = &g->nodes[node_id];
    if (n->type == flow_ntype_Null) {
        FLOW_error(c, flow_status_Item_does_not_exist);
        return false;
    } else {
        // We shouldn't be deleting nodes with frames already attached.
        if (n->result_bitmap != NULL) {
            FLOW_error(c, flow_status_Invalid_internal_state);
            return false;
        }

        if (!flow_edge_delete_all_connected_to_node(c, g, node_id)) {
            FLOW_error_return(c);
        }
        n->type = flow_ntype_Null;
        g->deleted_bytes += n->info_bytes;
        n->info_byte_index = -1;
        n->info_bytes = 0;
        n->state = flow_node_state_Blank;
        g->node_count--;
        return true;
    }
}

static bool flow_graph_walk_recursive(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref,
                                      int32_t node_id, bool * quit, flow_graph_visitor node_visitor,
                                      flow_graph_visitor edge_visitor, void * custom_data)
{
    bool skip_outbound_paths = false;
    if (node_visitor != NULL) {
        if (!node_visitor(c, job, graph_ref, node_id, quit, &skip_outbound_paths, custom_data)) {
            FLOW_error_return(c);
        }
    }
    if (skip_outbound_paths || *quit) {
        return true;
    }

    struct flow_edge * edge;
    int32_t edge_ix;
    for (edge_ix = 0; edge_ix < (*graph_ref)->next_edge_id; edge_ix++) {
        edge = &(*graph_ref)->edges[edge_ix];
        if (edge->type != flow_edgetype_null && edge->from == node_id) {

            skip_outbound_paths = false;
            if (edge_visitor != NULL) {
                if (!edge_visitor(c, job, graph_ref, edge_ix, quit, &skip_outbound_paths, custom_data)) {
                    FLOW_error_return(c);
                }
            }
            if (*quit) {
                return true;
            }
            if (!skip_outbound_paths) {
                // Recurse
                if (!flow_graph_walk_recursive(c, job, graph_ref, edge->to, quit, node_visitor, edge_visitor,
                                               custom_data)) {
                    FLOW_error_return(c);
                }
            }
            if (*quit) {
                return true;
            }
        }
    }
    return true;
}

bool flow_graph_walk(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref, flow_graph_visitor node_visitor,
                     flow_graph_visitor edge_visitor, void * custom_data)
{
    // TODO: would be good to verify graph is acyclic.

    bool quit = false;
    // We start by finding nodes with no inbound edges, then working in a direction.
    struct flow_edge * edge;
    int32_t node_ix;
    int32_t edge_ix;
    int32_t inbound_edge_count = 0;
    for (node_ix = 0; node_ix < (*graph_ref)->next_node_id; node_ix++) {
        if ((*graph_ref)->nodes[node_ix].type != flow_ntype_Null) {
            // Now count inbound edges
            inbound_edge_count = 0;
            for (edge_ix = 0; edge_ix < (*graph_ref)->next_edge_id; edge_ix++) {
                edge = &(*graph_ref)->edges[edge_ix];
                if (edge->type != flow_edgetype_null && edge->to == node_ix) {
                    inbound_edge_count++;
                }
            }
            // if zero, we have a winner
            if (inbound_edge_count == 0) {
                if (!flow_graph_walk_recursive(c, job, graph_ref, node_ix, &quit, node_visitor, edge_visitor,
                                               custom_data)) {
                    FLOW_error_return(c);
                }
                if (quit) {
                    return true;
                }
            }
        }
    }
    return true;
}

static bool flow_graph_walk_recursive_dependency_wise(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref,
                                                      int32_t node_id, bool * quit, bool * skip_return_path,
                                                      flow_graph_visitor node_visitor, flow_graph_visitor edge_visitor,
                                                      bool * visited_local, bool * visited_global, void * custom_data)
{

    // Check for cycles
    if (visited_local[node_id]) {
        FLOW_error(c, flow_status_Graph_is_cyclic); // Cycle in graph!
        return false;
    }
    visited_local[node_id] = true;

    // Skip work we did in another sibling walk
    if (visited_global[node_id]) {
        return true;
    }
    visited_global[node_id] = true;

    struct flow_edge * edge;
    int32_t edge_ix;
    for (edge_ix = 0; edge_ix < (*graph_ref)->next_edge_id; edge_ix++) {
        edge = &(*graph_ref)->edges[edge_ix];
        if (edge->type != flow_edgetype_null && edge->to == node_id) {
            bool skip_this_return_path = false;

            // Recurse, depth first
            if (!flow_graph_walk_recursive_dependency_wise(c, job, graph_ref, edge->from, quit, &skip_this_return_path,
                                                           node_visitor, edge_visitor, visited_local, visited_global,
                                                           custom_data)) {
                return false; // Actually, we *don't* want to add to the callstack. Recursion could be 30+ here. One
                // line is enough
            }
            if (*quit) {
                return true;
            }
            // We want to evaluate all branches depth first before we actually act on skip_return_path
            if (skip_this_return_path) {
                *skip_return_path = true;
            } else if (edge_visitor != NULL) {
                // If the deeper nodes didn't ask to skip, evalutate the edge
                if (!edge_visitor(c, job, graph_ref, edge_ix, quit, &skip_this_return_path, custom_data)) {
                    FLOW_error_return(c);
                }
                if (*quit) {
                    return true;
                }
                if (skip_this_return_path) {
                    *skip_return_path = true;
                }
            }
        }
    }

    // No inbound edges flagged to skip. Node gets direct influence over caller's skip_path and quit flag
    if (!*skip_return_path && node_visitor != NULL) {
        if (!node_visitor(c, job, graph_ref, node_id, quit, skip_return_path, custom_data)) {
            FLOW_error_return(c);
        }
    }
    return true;
}

bool flow_graph_walk_dependency_wise(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref,
                                     flow_graph_visitor node_visitor, flow_graph_visitor edge_visitor,
                                     void * custom_data)
{

    // visited_local checks for cycles
    // visited_global eliminates redundant work
    bool * visited_local = FLOW_calloc_array(c, (*graph_ref)->next_node_id * 2, bool);
    size_t visited_local_bytes = (*graph_ref)->next_node_id * sizeof(bool); // Just the first half of the array
    if (visited_local == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        return false;
    }
    bool * visited_global = visited_local + visited_local_bytes;

    bool quit = false;
    // We start by finding nodes with no inbound edges, then working in a direction.
    struct flow_edge * edge;
    int32_t node_ix;
    int32_t edge_ix;
    int32_t outbound_edge_count = 0;
    int32_t starting_nodes = 0;
    int32_t last_node = -1;
    for (node_ix = 0; node_ix < (*graph_ref)->next_node_id; node_ix++) {
        if ((*graph_ref)->nodes[node_ix].type != flow_ntype_Null) {
            last_node = node_ix;
            // Now count outbound edges
            outbound_edge_count = 0;
            for (edge_ix = 0; edge_ix < (*graph_ref)->next_edge_id; edge_ix++) {
                edge = &(*graph_ref)->edges[edge_ix];
                if (edge->type != flow_edgetype_null && edge->from == node_ix) {
                    outbound_edge_count++;
                }
            }
            // if zero, we have a winner
            if (outbound_edge_count == 0) {
                starting_nodes++;
                // Reset cycle check on visited_local
                memset(visited_local, 0, visited_local_bytes);
                bool skip_return_path_unused = false;
                if (!flow_graph_walk_recursive_dependency_wise(c, job, graph_ref, node_ix, &quit,
                                                               &skip_return_path_unused, node_visitor, edge_visitor,
                                                               visited_local, visited_global, custom_data)) {

                    FLOW_free(c, visited_local);
                    FLOW_error_return(c);
                }
                if (quit) {

                    FLOW_free(c, visited_local);
                    return true;
                }
            }
        }
    }
    if (last_node > -1 && starting_nodes == 0) {
        // We have at least one non-null node, but didn't visit any because there are no ending nodes. This implies a
        // cycle.
        FLOW_error(c, flow_status_Graph_is_cyclic);
        FLOW_free(c, visited_local);
        return false;
    }

    FLOW_free(c, visited_local);
    return true;
}

int32_t flow_graph_get_first_inbound_edge_of_type(flow_c * c, struct flow_graph * g, int32_t node_id,
                                                  flow_edgetype type)
{
    struct flow_edge * edge;
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++) {
        edge = &g->edges[i];
        if (edge->type == type) {
            if (edge->to == node_id) {
                return i;
            }
        }
    }
    return -404;
}
int32_t flow_graph_get_first_outbound_edge_of_type(flow_c * c, struct flow_graph * g, int32_t node_id,
                                                   flow_edgetype type)
{
    struct flow_edge * edge;
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++) {
        edge = &g->edges[i];
        if (edge->type == type) {
            if (edge->from == node_id) {
                return i;
            }
        }
    }
    return -404;
}
int32_t flow_graph_get_first_inbound_node_of_type(flow_c * c, struct flow_graph * g, int32_t node_id,
                                                  flow_edgetype type)
{
    struct flow_edge * edge;
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++) {
        edge = &g->edges[i];
        if (edge->type == type) {
            if (edge->to == node_id) {
                return edge->from;
            }
        }
    }
    return -404;
}
int32_t flow_graph_get_first_outbound_node_of_type(flow_c * c, struct flow_graph * g, int32_t node_id,
                                                   flow_edgetype type)
{
    struct flow_edge * edge;
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++) {
        edge = &g->edges[i];
        if (edge->type == type) {
            if (edge->from == node_id) {
                return edge->to;
            }
        }
    }
    return -404;
}

int32_t flow_graph_get_edge_count(flow_c * c, struct flow_graph * g, int32_t node_id, bool filter_by_edge_type,
                                  flow_edgetype type, bool include_inbound, bool include_outbound)
{
    struct flow_edge * edge;
    int32_t i;
    int32_t count = 0;
    for (i = 0; i < g->next_edge_id; i++) {
        edge = &g->edges[i];
        if (!filter_by_edge_type || edge->type == type) {
            if ((include_inbound && edge->to == node_id) || (include_outbound && edge->from == node_id)) {
                count++;
            }
        }
    }
    return count;
}

int32_t flow_graph_get_inbound_edge_count_of_type(flow_c * c, struct flow_graph * g, int32_t node_id,
                                                  flow_edgetype type)
{
    return flow_graph_get_edge_count(c, g, node_id, true, type, true, false);
}

static const char * get_format_name(flow_pixel_format f, bool alpha_meaningful)
{
    switch (f) {
        case flow_bgr24:
            return "flow_bgr24";
        case flow_bgra32:
            return alpha_meaningful ? "flow_bgra32" : "Bgr32";
        default:
            return "unknown format";
    }
}

bool flow_graph_print_to_dot(flow_c * c, struct flow_graph * g, FILE * stream, const char * image_node_filename_prefix)
{
    fprintf(stream, "digraph g {\n");
    fprintf(stream,
            "  node [shape=box, fontsize=20, fontcolor=\"#5AFA0A\" fontname=\"sans-serif bold\"]\n  size=\"12,18\"\n");
    fprintf(stream, "  edge [fontsize=20, fontname=\"sans-serif\"]\n");

    char node_label_buffer[1024];
    struct flow_edge * edge;
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++) {
        edge = &g->edges[i];
        if (edge->type != flow_edgetype_null) {
            char dimensions[64];
            struct flow_node * n = &g->nodes[edge->from];

            if (n->result_width < 0 && n->result_height < 0) {
                flow_snprintf(dimensions, 63, "?x?");
            } else {
                flow_snprintf(dimensions, 63, "%dx%d %s", n->result_width, n->result_height,
                              get_format_name(n->result_format, n->result_alpha_meaningful));
            }

            fprintf(stream, "  n%d -> n%d [label=\"e%d: %s%s\"]\n", edge->from, edge->to, i, dimensions,
                    edge->type == flow_edgetype_canvas ? " canvas" : "");
        }
    }

    uint64_t total_ticks = 0;

    struct flow_node * n;
    for (i = 0; i < g->next_node_id; i++) {
        n = &g->nodes[i];

        if (n->type != flow_ntype_Null) {
            if (!flow_node_stringify(c, g, i, node_label_buffer, 1023)) {
                FLOW_error_return(c);
            }
            // fprintf(stream, "  n%d [image=\"./node_frames/%s%d.png\", label=\"n%d: %s\"]\n", i,
            // image_node_filename_prefix, i, i, node_label_buffer); //Todo, add completion info.

            total_ticks += n->ticks_elapsed;
            double ms = n->ticks_elapsed * 1000.0 / (float)flow_get_profiler_ticks_per_second();

            if (n->result_bitmap != NULL && image_node_filename_prefix != NULL) {
                fprintf(stream, "  n%d [image=\"%s%d.png\", label=\"n%d: %s\n%.2fms\"]\n", i,
                        image_node_filename_prefix, i, i, node_label_buffer, ms);
            } else {
                fprintf(stream, "  n%d [label=\"n%d: %s\n%.2fms\"]\n", i, i, node_label_buffer,
                        ms); // Todo, add completion info.
            }
        }
    }

    double total_ms = total_ticks * 1000.0 / (float)flow_get_profiler_ticks_per_second();

    // Print graph info last so it displays right or last
    fprintf(stream, " graphinfo [label=\"");
    fprintf(stream, "%d nodes (%d/%d)\n %d edges (%d/%d)\n %d infobytes (%d/%d)\nExecution time: %.2fms", g->node_count,
            g->next_node_id, g->max_nodes, g->edge_count, g->next_edge_id, g->max_edges,
            g->next_info_byte - g->deleted_bytes, g->next_info_byte, g->max_info_bytes, total_ms);
    fprintf(stream, "\"]\n");

    fprintf(stream, "}\n");
    return true;
}

bool flow_graph_validate(flow_c * c, struct flow_graph * g)
{

    struct flow_edge * edge;
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++) {
        edge = &g->edges[i];
        if (edge->type != flow_edgetype_null) {
            if (edge->from < 0 || edge->to < 0 || edge->from >= g->next_node_id || edge->to >= g->next_node_id) {
                FLOW_error(c, flow_status_Invalid_internal_state);
                return false;
            }
            // Validate all edge from/to values are non-null nodes
            if (g->nodes[edge->from].type == flow_ntype_Null || g->nodes[edge->to].type == flow_ntype_Null) {
                FLOW_error(c, flow_status_Invalid_internal_state);
                return false;
            }

            // Validate all info bytes are within bounds (and do not overlap?)
            if (edge->info_byte_index + edge->info_bytes > g->next_info_byte) {
                FLOW_error(c, flow_status_Invalid_internal_state);
                return false;
            }
        } else {
            // validate all null nodes/edges are fully null
            if (edge->from != -1 || edge->to != -1 || edge->info_byte_index != -1 || edge->info_bytes != 0) {
                FLOW_error(c, flow_status_Invalid_internal_state);
                return false;
            }
        }
    }

    struct flow_node * node;
    for (i = 0; i < g->next_node_id; i++) {
        node = &g->nodes[i];
        if (node->type != flow_ntype_Null) {
            if (node->state > flow_node_state_Done) {
                FLOW_error(c, flow_status_Invalid_internal_state);
                return false;
            }
            if (!flow_node_validate_edges(c, g, i)) {
                FLOW_error(c, flow_status_Invalid_internal_state);
                return false;
            }
            // Validate all node types are real and have corresponding definitions

            struct flow_node_definition * def = flow_nodedef_get(c, node->type);
            if (def == NULL) {
                FLOW_error_return(c);
            }

            // Validate all info bytes are within bounds (and TODO: do not overlap?)
            if (node->info_byte_index + node->info_bytes > g->next_info_byte) {
                FLOW_error(c, flow_status_Invalid_internal_state);
                return false;
            }
        } else {
            // validate all null nodes/edges are fully null
            if (node->result_bitmap != NULL || node->state != flow_node_state_Blank || node->info_byte_index != -1
                || node->info_bytes != 0) {
                FLOW_error(c, flow_status_Invalid_internal_state);
                return false;
            }
        }
    }

    // Validate graph has no cycles, just by walking it.
    if (!flow_graph_walk_dependency_wise(c, NULL, &g, NULL, NULL, NULL)) {
        FLOW_error_return(c);
    }
    return true;
}
