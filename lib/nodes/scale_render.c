#include "../imageflow_private.h"
#include "definition_helpers.h"

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
    if (matte_color != NULL) {
        memset(&info->matte_color[0], 0, 4);
    } else {
        info->matte_color[0] = 0;
        info->matte_color[1] = 0;
        info->matte_color[2] = 0;
        info->matte_color[3] = 0;
    }
    return id;
}

int32_t flow_node_create_scale_2d(flow_c * c, struct flow_graph ** g, int32_t prev_node, int32_t scale_to_width,
                                  int32_t scale_to_height, flow_working_floatspace scale_and_filter_in_colorspace,
                                  float sharpen_percent, flow_interpolation_filter interpolation_filter)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_primitive_Scale2D_RenderToCanvas1D);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_scale2d_render_to_canvas1d * info
        = (struct flow_nodeinfo_scale2d_render_to_canvas1d *)FrameNode_get_node_info_pointer(*g, id);
    info->scale_to_width = scale_to_width;
    info->scale_to_height = scale_to_height;
    info->interpolation_filter = interpolation_filter;
    info->scale_in_colorspace = scale_and_filter_in_colorspace;
    info->sharpen_percent_goal = sharpen_percent;
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

int32_t flow_node_create_scale(flow_c * c, struct flow_graph ** g, int32_t prev_node, size_t width, size_t height,
                               flow_interpolation_filter downscale_filter, flow_interpolation_filter upscale_filter,
                               size_t flags)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Scale);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_scale * info = (struct flow_nodeinfo_scale *)FrameNode_get_node_info_pointer(*g, id);
    info->width = (int32_t)width;
    info->height = (int32_t)height;
    info->downscale_filter = downscale_filter;
    info->upscale_filter = upscale_filter;
    info->flags = flags;
    return id;
}

static int32_t create_render1d_node(flow_c * c, struct flow_graph ** g, int32_t last, int32_t to_width, bool transpose,
                                    flow_interpolation_filter filter)
{

    flow_working_floatspace floatspace = flow_working_floatspace_linear;
    float sharpen_percent = 0;

    int32_t id = flow_node_create_render1d(c, g, last, transpose, to_width, floatspace, sharpen_percent, NULL, filter);
    if (id < 0) {
        FLOW_add_to_callstack(c);
    }
    return id;
}

static bool stringify_scale(flow_c * c, struct flow_graph * g, int32_t node_id, char * buffer, size_t buffer_size)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_scale, info);

    char state[64];
    if (!stringify_state(state, 63, &g->nodes[node_id])) {
        FLOW_error_return(c);
    }

    flow_snprintf(buffer, buffer_size, "scale %lux%lu %s", info->width, info->height,
                  (const char *)(const char *) & state);
    return true;
}

static bool dimensions_scale(flow_c * c, struct flow_graph * g, int32_t node_id, bool force_estimate)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_size, info)
    FLOW_GET_INPUT_NODE(g, node_id)

    struct flow_node * n = &g->nodes[node_id];

    n->result_width = info->width;
    n->result_height = info->height;
    n->result_alpha_meaningful = input_node->result_alpha_meaningful;
    n->result_format = input_node->result_format;
    return true;
}

static bool flatten_scale(flow_c * c, struct flow_graph ** g, int32_t node_id, struct flow_node * node,
                          struct flow_node * input_node, int32_t * first_replacement_node,
                          int32_t * last_replacement_node)
{
    if ((*g)->nodes[node_id].type != flow_ntype_Scale
        || (*g)->nodes[node_id].info_byte_index > (int64_t)((*g)->next_info_byte - sizeof(struct flow_nodeinfo_size))) {
        FLOW_error(c, flow_status_Graph_invalid);
        return false;
    }
    // INFOBYTES ARE INVALID AFTER THE FIRST create_*_nodec call, since the graph has been swapped out.
    // TODO: check them all
    FLOW_GET_INFOBYTES((*g), node_id, flow_nodeinfo_scale, size)
    int32_t height = size->height;
    int32_t width = size->width;
    // TODO: swap out for upscale filter
    flow_interpolation_filter filter = size->downscale_filter;

    if ((size->flags & flow_scale_flags_use_scale2d) > 0) {

        flow_pixel_format input_format = input_node->result_format;

        int32_t canvas = flow_node_create_canvas(c, g, -1, input_format, width, height, 0);
        if (canvas < 0) {
            FLOW_error_return(c);
        }

        if (!set_node_optimized_and_update_state(c, *g, canvas)) {
            FLOW_error_return(c);
        }

        *first_replacement_node
            = flow_node_create_scale_2d(c, g, *first_replacement_node, width, height, (flow_working_floatspace_as_is),
                                        0, (flow_interpolation_filter_Robidoux));
        if (*first_replacement_node < 0) {
            FLOW_error_return(c);
        }
        if (flow_edge_create(c, g, canvas, *first_replacement_node, flow_edgetype_canvas) < 0) {
            FLOW_error_return(c);
        }

        *last_replacement_node = *first_replacement_node;
        return true;
    } else {
        *first_replacement_node = create_render1d_node(c, g, -1, width, true, filter);
        if (*first_replacement_node < 0) {
            FLOW_error_return(c);
        }

        int32_t copy = *first_replacement_node;

        *last_replacement_node = create_render1d_node(c, g, copy, height, true, filter);
        if (*last_replacement_node < 0) {
            FLOW_error_return(c);
        }
    }
    return true;
}

static bool stringify_render1d(flow_c * c, struct flow_graph * g, int32_t node_id, char * buffer, size_t buffer_size)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_render_to_canvas_1d, info);
    FLOW_GET_INPUT_NODE(g, node_id)

    char state[64];
    if (!stringify_state(state, 63, &g->nodes[node_id])) {
        FLOW_error_return(c);
    }

    flow_snprintf(buffer, buffer_size, "render1d %d -> %d %s %s\nat %d,%d. %s sharp%d%%. %s", input_node->result_width,
                  info->scale_to_width, stringify_filter(info->interpolation_filter), (const char *)&state,
                  info->canvas_x, info->canvas_y, info->transpose_on_write ? "transpose. " : "",
                  (int)info->sharpen_percent_goal, stringify_colorspace(info->scale_in_colorspace));
    return true;
}

static bool stringify_scale2d(flow_c * c, struct flow_graph * g, int32_t node_id, char * buffer, size_t buffer_size)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_scale2d_render_to_canvas1d, info);
    FLOW_GET_INPUT_NODE(g, node_id)

    char state[64];
    if (!stringify_state(state, 63, &g->nodes[node_id])) {
        FLOW_error_return(c);
    }

    flow_snprintf(buffer, buffer_size, "scale2d %dx%d -> %dx%d %s %s sharp%d%%. %s", input_node->result_width,
                  input_node->result_height, info->scale_to_width, info->scale_to_height,
                  stringify_filter(info->interpolation_filter), (const char *)&state,

                  (int)info->sharpen_percent_goal, stringify_colorspace(info->scale_in_colorspace));
    return true;
}

static bool dimensions_render1d(flow_c * c, struct flow_graph * g, int32_t node_id, bool force_estimate)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_render_to_canvas_1d, info)
    FLOW_GET_INPUT_NODE(g, node_id)

    struct flow_node * n = &g->nodes[node_id];

    n->result_format = flow_bgra32; // TODO: maybe wrong
    n->result_alpha_meaningful = true; // TODO: WRONG! Involve "input" in decision
    n->result_width = info->transpose_on_write ? input_node->result_height : info->scale_to_width;
    n->result_height = info->transpose_on_write ? info->scale_to_width : input_node->result_height;
    return true;
}

int32_t create_primitve_render_to_canvas_1d_node(flow_c * c, struct flow_graph ** g, int32_t last, int32_t to_width,
                                                 bool transpose, flow_interpolation_filter filter)
{

    flow_working_floatspace floatspace = flow_working_floatspace_linear;
    flow_compositing_mode mode = flow_compositing_mode_overwrite;
    uint8_t * matte_color[4];
    float sharpen_percent = 0;

    int32_t id = flow_node_create_render_to_canvas_1d(c, g, last, transpose, 0, 0, to_width, floatspace,
                                                      sharpen_percent, mode, matte_color, NULL, filter);
    if (id < 0) {
        FLOW_add_to_callstack(c);
    }
    return id;
}

static bool flatten_render1d(flow_c * c, struct flow_graph ** g, int32_t node_id, struct flow_node * node,
                             struct flow_node * input_node, int32_t * first_replacement_node,
                             int32_t * last_replacement_node)
{

    FLOW_GET_INFOBYTES((*g), node_id, flow_nodeinfo_render_to_canvas_1d, info)

    int32_t c_h = info->transpose_on_write ? info->scale_to_width : input_node->result_height;
    int32_t c_w = info->transpose_on_write ? input_node->result_height : info->scale_to_width;
    flow_pixel_format input_format = input_node->result_format;
    int32_t scale_to_width = info->scale_to_width;

    int32_t canvas = flow_node_create_canvas(c, g, -1, input_format, c_w, c_h, 0);
    if (canvas < 0) {
        FLOW_error_return(c);
    }

    if (!set_node_optimized_and_update_state(c, *g, canvas)) {
        FLOW_error_return(c);
    }

    *first_replacement_node = create_primitve_render_to_canvas_1d_node(c, g, *first_replacement_node, scale_to_width,
                                                                       true, flow_interpolation_filter_Robidoux);
    if (*first_replacement_node < 0) {
        FLOW_error_return(c);
    }
    if (flow_edge_create(c, g, canvas, *first_replacement_node, flow_edgetype_canvas) < 0) {
        FLOW_error_return(c);
    }

    *last_replacement_node = *first_replacement_node;
    return true;
}

static bool execute_render1d(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_render_to_canvas_1d, info)
    FLOW_GET_INPUT_EDGE(g, node_id)
    FLOW_GET_CANVAS_EDGE(g, node_id)
    struct flow_node * n = &g->nodes[node_id];

    struct flow_bitmap_bgra * input = g->nodes[input_edge->from].result_bitmap;
    struct flow_bitmap_bgra * canvas = g->nodes[canvas_edge->from].result_bitmap;

    if (!flow_node_execute_render_to_canvas_1d(c, job, input, canvas, info)) {
        FLOW_error_return(c);
    }
    n->result_bitmap = canvas;
    return true;
}

static bool execute_scale2d(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id)
{

    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_scale2d_render_to_canvas1d, info)
    FLOW_GET_INPUT_EDGE(g, node_id)
    FLOW_GET_CANVAS_EDGE(g, node_id)
    struct flow_node * n = &g->nodes[node_id];

    struct flow_bitmap_bgra * input = g->nodes[input_edge->from].result_bitmap;
    struct flow_bitmap_bgra * canvas = g->nodes[canvas_edge->from].result_bitmap;

    if (!flow_node_execute_scale2d_render1d(c, job, input, canvas, info)) {
        FLOW_error_return(c);
    }
    n->result_bitmap = canvas;
    return true;
}

const struct flow_node_definition flow_define_scale = {
    .type = flow_ntype_Scale,
    .input_count = 1,
    .canvas_count = 0,
    .type_name = "scale",
    .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_scale),
    .stringify = stringify_scale,
    .populate_dimensions = dimensions_scale,
    .pre_optimize_flatten = flatten_scale,
};

const struct flow_node_definition flow_define_render_to_canvas_1d = {
    .type = flow_ntype_Render1D,
    .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_render_to_canvas_1d),
    .type_name = "render1d",
    .input_count = 1,
    .canvas_count = 0,
    .stringify = stringify_render1d,
    .populate_dimensions = dimensions_render1d,
    .post_optimize_flatten = flatten_render1d,

};

const struct flow_node_definition flow_define_render_to_canvas_1d_primitive
    = { .type = flow_ntype_primitive_RenderToCanvas1D,
        .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_render_to_canvas_1d),
        .type_name = "render1d_p",
        .input_count = 1,
        .canvas_count = 1,
        .stringify = stringify_render1d,
        .populate_dimensions = dimensions_of_canvas,
        .execute = execute_render1d

    };
const struct flow_node_definition flow_define_scale2d_render_to_canvas1d
    = { .type = flow_ntype_primitive_Scale2D_RenderToCanvas1D,
        .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_scale2d_render_to_canvas1d),
        .type_name = "scale2d_p",
        .input_count = 1,
        .canvas_count = 1,
        .stringify = stringify_scale2d,
        .populate_dimensions = dimensions_of_canvas,
        .execute = execute_scale2d

    };
