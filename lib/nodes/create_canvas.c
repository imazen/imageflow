#include "../imageflow_private.h"
#include "definition_helpers.h"

int32_t flow_node_create_canvas(flow_c * c, struct flow_graph ** g, int32_t prev_node, flow_pixel_format format,
                                size_t width, size_t height, uint32_t bgcolor)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Create_Canvas);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_createcanvas * info
        = (struct flow_nodeinfo_createcanvas *) flow_node_get_info_pointer(*g, id);
    info->format = format;
    info->width = width;
    info->height = height;
    info->bgcolor = bgcolor;
    return id;
}

static bool stringify_canvas(flow_c * c, struct flow_graph * g, int32_t node_id, char * buffer, size_t buffer_size)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_createcanvas, info);

    char state[64];
    if (!stringify_state(state, 63, &g->nodes[node_id])) {
        FLOW_error_return(c);
    }

    flow_snprintf(buffer, buffer_size, "canvas %lux%lu %s %s", info->width, info->height,
                  flow_pixel_format_get_name(info->format, false), (const char *)&state);
    return true;
}

static bool dimensions_canvas(flow_c * c, struct flow_graph * g, int32_t node_id, bool force_estimate)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_createcanvas, info)

    struct flow_node * n = &g->nodes[node_id];

    n->result_width = (int32_t)info->width;
    n->result_height = (int32_t)info->height;
    n->result_alpha_meaningful = false;
    n->result_format = info->format;
    return true;
}

static bool execute_canvas(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_createcanvas, info)

    struct flow_node * n = &g->nodes[node_id];
    // TODO: bgcolor
    n->result_bitmap = flow_bitmap_bgra_create(c, (int)info->width, (int)info->height, true, info->format);
    if (n->result_bitmap == NULL) {
        FLOW_error_return(c);
    }
    // Uncomment to make canvas blue for debugging
    //    for (int32_t y =0; y < (int32_t)n->result_bitmap->h; y++)
    //    for (int32_t i = 0; i < (int32_t)n->result_bitmap->w; i++){
    //        n->result_bitmap->pixels[n->result_bitmap->stride * y + i * 4] = 0xFF;
    //        n->result_bitmap->pixels[n->result_bitmap->stride * y + i * 4 + 3] = 0xFF;
    //    }

    return true;
}

const struct flow_node_definition flow_define_create_canvas
    = { .type = flow_ntype_Create_Canvas,
        .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_createcanvas),
        .input_count = 0,
        .canvas_count = 0,
        .populate_dimensions = dimensions_canvas,
        .type_name = "canvas",
        .stringify = stringify_canvas,
        .execute = execute_canvas

    };
