#include "../imageflow_private.h"
#include "definition_helpers.h"

int32_t flow_node_create_clone(flow_c * c, struct flow_graph ** g, int32_t prev_node)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Clone);
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

static bool dimensions_copy_rect(flow_c * c, struct flow_graph * g, int32_t node_id, bool force_estimate)
{
    FLOW_GET_CANVAS_NODE(g, node_id)

    struct flow_node * n = &g->nodes[node_id];

    // TODO: implement validation of all coordinates here.
    n->result_width = canvas_node->result_width;
    n->result_height = canvas_node->result_height;
    n->result_alpha_meaningful = canvas_node->result_alpha_meaningful;
    n->result_format = canvas_node->result_format;
    return true;
}

static bool dimensions_crop(flow_c * c, struct flow_graph * g, int32_t node_id, bool force_estimate)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_crop, info)
    FLOW_GET_INPUT_NODE(g, node_id)

    struct flow_node * n = &g->nodes[node_id];

    n->result_width = info->x2 - info->x1;
    n->result_height = info->y2 - info->y1;
    if (n->result_width < 1 || n->result_height < 1) {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }
    if ((int32_t)info->x1 >= input_node->result_width || (int32_t)info->x2 > input_node->result_width) {
        FLOW_error_msg(c, flow_status_Invalid_argument, "crop arguments x1=%i, x2=%i are outside the width bound of "
                                                        "the input frame (%i)",
                       info->x1, info->x2, input_node->result_width);
        return false;
    }
    if ((int32_t)info->y1 >= input_node->result_height || (int32_t)info->y2 > input_node->result_height) {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }
    n->result_alpha_meaningful = input_node->result_alpha_meaningful;
    n->result_format = input_node->result_format;
    return true;
}

static bool dimensions_expand_canvas(flow_c * c, struct flow_graph * g, int32_t node_id, bool force_estimate)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_expand_canvas, info)
    FLOW_GET_INPUT_NODE(g, node_id)

    struct flow_node * n = &g->nodes[node_id];

    n->result_width = input_node->result_width + info->left + info->right;
    n->result_height = input_node->result_height + info->top + info->bottom;

    if (n->result_width < 1 || n->result_height < 1) {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }
    // TODO: If we were passed a transparent background color, we should upgrade the format to have alpha.
    n->result_alpha_meaningful = input_node->result_alpha_meaningful;
    n->result_format = input_node->result_format;
    return true;
}

static bool flatten_crop(flow_c * c, struct flow_graph ** g, int32_t node_id, struct flow_node * node,
                         struct flow_node * input_node, int32_t * first_replacement_node,
                         int32_t * last_replacement_node)
{

    struct flow_nodeinfo_crop info;

    memcpy(&info, &(*g)->info_bytes[(*g)->nodes[node_id].info_byte_index], (*g)->nodes[node_id].info_bytes);

    FLOW_GET_INPUT_EDGE((*g), node_id);

    bool must_clone = false;
    if (!node_has_other_dependents(c, *g, input_edge->from, node_id, &must_clone)) {
        FLOW_error_return(c);
    }
    if (must_clone) {
        *first_replacement_node = flow_node_create_clone(c, g, -1);
        if (*first_replacement_node < 0) {
            FLOW_error_return(c);
        }
    } else {
        *first_replacement_node = -1;
    }
    *last_replacement_node
        = flow_node_create_primitive_crop(c, g, *first_replacement_node, info.x1, info.y1, info.x2, info.y2);
    if (*last_replacement_node < 0) {
        FLOW_error_return(c);
    }
    if (!must_clone) {
        *first_replacement_node = *last_replacement_node;
    }
    return true;
}

static bool flatten_expand_canvas(flow_c * c, struct flow_graph ** g, int32_t node_id, struct flow_node * node,
                                  struct flow_node * input_node, int32_t * first_replacement_node,
                                  int32_t * last_replacement_node)
{

    struct flow_nodeinfo_expand_canvas info;

    memcpy(&info, &(*g)->info_bytes[(*g)->nodes[node_id].info_byte_index], (*g)->nodes[node_id].info_bytes);

    // TODO: If edges are all zero, replace this node with a nullop
    int input_width = input_node->result_width;
    int input_height = input_node->result_height;
    int canvas_width = input_node->result_width + info.left + info.right;
    int canvas_height = input_node->result_height + info.top + info.bottom;

    int canvas_node_id = flow_node_create_canvas(c, g, -1, input_node->result_format, canvas_width, canvas_height, 0);
    if (canvas_node_id < 0) {
        FLOW_error_return(c);
    }

    *first_replacement_node = flow_node_create_primitive_copy_rect_to_canvas(
        c, g, *first_replacement_node, 0, 0, input_width, input_height, info.left, info.top);
    if (*first_replacement_node < 0) {
        FLOW_error_return(c);
    }
    // Ad canvas edge
    if (flow_edge_create(c, g, canvas_node_id, *first_replacement_node, flow_edgetype_canvas) < 0) {
        FLOW_error_return(c);
    }

    *last_replacement_node = *first_replacement_node;

    if (info.left > 0)
        *last_replacement_node = flow_node_create_fill_rect(c, g, *first_replacement_node, 0, 0, info.left,
                                                            canvas_height, info.canvas_color_srgb);
    if (*last_replacement_node < 0) {
        FLOW_error_return(c);
    }

    if (info.top > 0)
        *last_replacement_node = flow_node_create_fill_rect(c, g, *last_replacement_node, info.left, 0, canvas_width,
                                                            info.top, info.canvas_color_srgb);
    if (*last_replacement_node < 0) {
        FLOW_error_return(c);
    }
    if (info.bottom > 0)
        *last_replacement_node
            = flow_node_create_fill_rect(c, g, *last_replacement_node, info.left, canvas_height - info.bottom,
                                         canvas_width, canvas_height, info.canvas_color_srgb);
    if (*last_replacement_node < 0) {
        FLOW_error_return(c);
    }
    if (info.right > 0)
        *last_replacement_node
            = flow_node_create_fill_rect(c, g, *last_replacement_node, canvas_width - info.left, info.top, canvas_width,
                                         canvas_height - info.top, info.canvas_color_srgb);
    if (*last_replacement_node < 0) {
        FLOW_error_return(c);
    }

    return true;
}

static bool flatten_clone(flow_c * c, struct flow_graph ** g, int32_t node_id, struct flow_node * node,
                          struct flow_node * input_node, int32_t * first_replacement_node,
                          int32_t * last_replacement_node)
{

    if (input_node == NULL) {
        FLOW_error_msg(c, flow_status_Graph_invalid, "A Clone node must have one input.");
        return false;
    }
    int32_t rw = input_node->result_width;
    int32_t rh = input_node->result_height;
    flow_pixel_format rf = input_node->result_format;
    // create canvas
    int32_t canvas = flow_node_create_canvas(c, g, -1, rf, rw, rh, 0);
    if (canvas < 0) {
        FLOW_error_return(c);
    }
    // Blit from image
    *first_replacement_node = flow_node_create_primitive_copy_rect_to_canvas(c, g, -1, 0, 0, rw, rh, 0, 0);
    if (*first_replacement_node < 0) {
        FLOW_error_return(c);
    }
    // blit to canvas
    if (flow_edge_create(c, g, canvas, *first_replacement_node, flow_edgetype_canvas) < 0) {
        FLOW_error_return(c);
    }

    *last_replacement_node = *first_replacement_node;
    return true;
}

static bool execute_crop(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_crop, info)
    FLOW_GET_INPUT_EDGE(g, node_id)
    struct flow_node * n = &g->nodes[node_id];

    struct flow_bitmap_bgra * original = g->nodes[input_edge->from].result_bitmap;
    struct flow_bitmap_bgra * b = flow_bitmap_bgra_create_header(c, info->x2 - info->x1, info->y2 - info->y1);
    if (b == NULL) {
        FLOW_error_return(c);
    }
    b->alpha_meaningful = original->alpha_meaningful;
    b->borrowed_pixels = true;
    b->can_reuse_space = false;
    b->compositing_mode = original->compositing_mode;
    b->fmt = original->fmt;
    memcpy(&b->matte_color, &original->matte_color, 4);
    b->stride = original->stride;
    b->pixels = original->pixels + (original->stride * info->y1)
                + flow_pixel_format_bytes_per_pixel(original->fmt) * info->x1;

    n->result_bitmap = b;
    return true;
}

static bool execute_fill_rect(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_fill_rect, info)
    FLOW_GET_INPUT_EDGE(g, node_id)
    struct flow_node * n = &g->nodes[node_id];

    struct flow_bitmap_bgra * b = g->nodes[input_edge->from].result_bitmap;

    if (!flow_bitmap_bgra_fill_rect(c, b, info->x1, info->y1, info->x2, info->y2, info->color_srgb)) {
        FLOW_error_return(c);
    }
    n->result_bitmap = b;
    return true;
}

bool flow_bitmap_bgra_fill_rect(flow_c * c, struct flow_bitmap_bgra * b, uint32_t x1, uint32_t y1, uint32_t x2,
                                uint32_t y2, uint32_t color_srgb_argb)
{
    if (x1 >= x2 || y1 >= y2 || y2 > b->h || x2 > b->w) {
        FLOW_error(c, flow_status_Invalid_argument);
        // Either out of bounds or has a width or height of zero.
        return false;
    }

    uint8_t step = flow_pixel_format_bytes_per_pixel(b->fmt);

    uint8_t * topleft = b->pixels + (b->stride * y1) + step * x1;

    size_t rect_width_bytes = step * (x2 - x1);

    uint32_t color = color_srgb_argb;
    if (step == 1) {
        // TODO: use gamma-correct grayscale conversion
        FLOW_error(c, flow_status_Not_implemented);
        return false;
    } else if (step == 3) {
        color = color >> 8; // Drop the alpha bits
    }
    for (uint32_t byte_offset = 0; byte_offset < rect_width_bytes; byte_offset += step) {
        memcpy(topleft + byte_offset, &color, step);
    }
    // Copy downwards
    for (uint32_t y = 1; y < (y2 - y1); y++) {
        memcpy(topleft + (b->stride * y), topleft, rect_width_bytes);
    }
    return true;
}

static bool execute_copy_rect(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_copy_rect_to_canvas, info)
    FLOW_GET_INPUT_EDGE(g, node_id)
    FLOW_GET_CANVAS_EDGE(g, node_id)
    struct flow_node * n = &g->nodes[node_id];

    struct flow_bitmap_bgra * input = g->nodes[input_edge->from].result_bitmap;
    struct flow_bitmap_bgra * canvas = g->nodes[canvas_edge->from].result_bitmap;

    // TODO: implement bounds checks!!!
    if (input->fmt != canvas->fmt) {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }
    if (info->x == 0 && info->from_x == 0 && info->from_y == 0 && info->y == 0 && info->width == input->w
        && info->width == canvas->w && info->height == input->h && info->height == canvas->h
        && canvas->stride == input->stride) {
        memcpy(canvas->pixels, input->pixels, input->stride * input->h);
        canvas->alpha_meaningful = input->alpha_meaningful;
    } else {
        int32_t bytes_pp = flow_pixel_format_bytes_per_pixel(input->fmt);
        for (uint32_t y = 0; y < info->height; y++) {
            void * from_ptr = input->pixels + (size_t)(input->stride * (info->from_y + y) + bytes_pp * info->from_x);
            void * to_ptr = canvas->pixels + (size_t)(canvas->stride * (info->y + y) + bytes_pp * info->x);
            memcpy(to_ptr, from_ptr, info->width * bytes_pp);
        }
    }
    n->result_bitmap = canvas;
    return true;
}

const struct flow_node_definition flow_define_clone = { // Should be useless once we finish function/mutate logic
    .type = flow_ntype_Clone,
    .nodeinfo_bytes_fixed = 0,
    .input_count = 1,
    .canvas_count = 0,
    .populate_dimensions = dimensions_mimic_input,
    .type_name = "clone",
    .pre_optimize_flatten = flatten_clone
};

const struct flow_node_definition flow_define_crop = {
    .type = flow_ntype_Crop,
    .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_crop),
    .input_count = 1,
    .canvas_count = 0,
    .populate_dimensions = dimensions_crop,
    .type_name = "crop",
    .post_optimize_flatten = flatten_crop,
};

const struct flow_node_definition flow_define_expand_canvas = {
    .type = flow_ntype_Expand_Canvas,
    .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_expand_canvas),
    .input_count = 1,
    .canvas_count = 0,
    .populate_dimensions = dimensions_expand_canvas,
    .type_name = "expand_canvas",
    .post_optimize_flatten = flatten_expand_canvas,
};

const struct flow_node_definition flow_define_crop_mutate = { .type = flow_ntype_primitive_Crop_Mutate_Alias,
                                                              .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_crop),
                                                              .input_count = 1,
                                                              .canvas_count = 0,
                                                              .populate_dimensions = dimensions_crop,
                                                              .type_name = "crop mutate/alias",
                                                              .execute = execute_crop };

const struct flow_node_definition flow_define_fill_rect
    = { .type = flow_ntype_Fill_Rect_Mutate,
        .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_fill_rect),
        .input_count = 1,
        .canvas_count = 0,
        .populate_dimensions = dimensions_mimic_input,
        .type_name = "fill rect",
        .execute = execute_fill_rect };

const struct flow_node_definition flow_define_copy_rect
    = { .type = flow_ntype_primitive_CopyRectToCanvas,
        .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_copy_rect_to_canvas),
        .input_count = 1,
        .canvas_count = 1,
        .populate_dimensions = dimensions_copy_rect,
        .type_name = "copy rect",
        .execute = execute_copy_rect };
