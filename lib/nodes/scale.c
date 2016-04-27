#include "../imageflow_private.h"
#include "nodes_private.h"

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
