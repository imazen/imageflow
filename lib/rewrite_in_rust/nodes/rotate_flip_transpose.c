#include "lib/imageflow_private.h"
#include "definition_helpers.h"

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

int32_t flow_node_create_apply_orientation(flow_c * c, struct flow_graph ** g, int32_t prev_node, int32_t exif_orientation_flag)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Apply_Orientation);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_apply_orientation * info = (struct flow_nodeinfo_apply_orientation *)flow_node_get_info_pointer(*g, id);
    info->orientation = exif_orientation_flag;
    return id;
}


static bool dimensions_transpose(flow_c * c, struct flow_graph * g, int32_t node_id, bool force_estimate)
{
    FLOW_GET_INPUT_NODE(g, node_id)

    struct flow_node * n = &g->nodes[node_id];

    n->result_width = input_node->result_height; // we just swap with and height
    n->result_height = input_node->result_width;
    n->result_alpha_meaningful = input_node->result_alpha_meaningful;
    n->result_format = input_node->result_format;
    return true;
}

static bool flatten_transpose(flow_c * c, struct flow_graph ** g, int32_t node_id, struct flow_node * node,
                              struct flow_node * input_node, int32_t * first_replacement_node,
                              int32_t * last_replacement_node)
{

    int32_t canvas = flow_node_create_canvas(c, g, -1, input_node->result_format, input_node->result_height,
                                             input_node->result_width, 0);
    if (canvas < 0) {
        FLOW_error_return(c);
    }

    *first_replacement_node = create_primitve_render_to_canvas_1d_node(
        c, g, *first_replacement_node, input_node->result_width, true, flow_interpolation_filter_Robidoux);
    if (*first_replacement_node < 0) {
        FLOW_error_return(c);
    }
    if (flow_edge_create(c, g, canvas, *first_replacement_node, flow_edgetype_canvas) < 0) {
        FLOW_error_return(c);
    }

    *last_replacement_node = *first_replacement_node;
    return true;
}

static bool dimensions_apply_orientation(flow_c * c, struct flow_graph * g, int32_t node_id, bool force_estimate)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_apply_orientation, info)


    FLOW_GET_INPUT_NODE(g, node_id)

    struct flow_node * n = &g->nodes[node_id];

    if (info->orientation >= 5 && info->orientation <= 8) {
        n->result_width = input_node->result_height; // we just swap with and height
        n->result_height = input_node->result_width;
    }else{
        n->result_width = input_node->result_width;
        n->result_height = input_node->result_height;
    }
    n->result_alpha_meaningful = input_node->result_alpha_meaningful;
    n->result_format = input_node->result_format;
    return true;
}

static bool flatten_apply_orientation(flow_c * c, struct flow_graph ** g, int32_t node_id, struct flow_node * node,
                              struct flow_node * input_node, int32_t * first_replacement_node,
                              int32_t * last_replacement_node)
{
    FLOW_GET_INFOBYTES((*g), node_id, flow_nodeinfo_apply_orientation, info)
    int32_t orientation = info->orientation;

    if (orientation < 2 || orientation > 8){
        //Replace with flow_ntype_Noop
        *first_replacement_node = flow_node_create_noop(c, g, -1);
        *last_replacement_node = *first_replacement_node;
        if (*last_replacement_node < 0) {
            FLOW_error_return(c);
        }
        return true;
    }
    int32_t last_node = -1;

    //2,3, 6, 7, 8, need rotate 180
    if (orientation == 2){
        last_node = flow_node_create_generic(c, g, -1, flow_ntype_Flip_Horizontal);
    }
    if (orientation == 3){
        last_node = flow_node_create_rotate_180(c, g, -1);
    }
    if (orientation == 4){
        last_node = flow_node_create_generic(c, g, -1, flow_ntype_Flip_Vertical);
    }
    if (orientation == 5){
        last_node = flow_node_create_transpose(c, g, -1);
    }
    if (orientation == 6){
        last_node = flow_node_create_rotate_270(c, g, -1);
    }
    if (orientation == 8){
        last_node = flow_node_create_rotate_90(c, g, -1);
    }
    if (orientation != 7) {
        if (last_node < 0) {
            FLOW_error_return(c);
        }
        *first_replacement_node = last_node;
        *last_replacement_node = last_node;
        return true;
    }else {
        *first_replacement_node = flow_node_create_rotate_180(c, g, -1);
        *last_replacement_node = flow_node_create_transpose(c, g, *first_replacement_node);
        return true;
    }


}

static bool stringify_apply_orientation(flow_c * c, struct flow_graph * g, int32_t node_id, char * buffer, size_t buffer_size)
{
    FLOW_GET_INFOBYTES((g), node_id, flow_nodeinfo_apply_orientation, info)
    int32_t orientation = info->orientation;

    char state[64];
    if (!stringify_state(state, 63, &g->nodes[node_id])) {
        FLOW_error_return(c);
    }

    flow_snprintf(buffer, buffer_size, "apply_orientation(%d) %s", orientation, (const char *)&state);

    return true;
}


static bool flatten_rotate_90(flow_c * c, struct flow_graph ** g, int32_t node_id, struct flow_node * node,
                              struct flow_node * input_node, int32_t * first_replacement_node,
                              int32_t * last_replacement_node)
{

    *first_replacement_node = flow_node_create_transpose(c, g, -1);
    if (*first_replacement_node < 0) {
        FLOW_error_return(c);
    }
    *last_replacement_node = flow_node_create_generic(c, g, *first_replacement_node, flow_ntype_Flip_Vertical);
    if (*last_replacement_node < 0) {
        FLOW_error_return(c);
    }
    return true;
}
static bool flatten_rotate_270(flow_c * c, struct flow_graph ** g, int32_t node_id, struct flow_node * node,
                               struct flow_node * input_node, int32_t * first_replacement_node,
                               int32_t * last_replacement_node)
{

    *first_replacement_node = flow_node_create_generic(c, g, -1, flow_ntype_Flip_Vertical);
    if (*first_replacement_node < 0) {
        FLOW_error_return(c);
    }
    *last_replacement_node = flow_node_create_transpose(c, g, *first_replacement_node);
    if (*last_replacement_node < 0) {
        FLOW_error_return(c);
    }
    return true;
}
static bool flatten_rotate_180(flow_c * c, struct flow_graph ** g, int32_t node_id, struct flow_node * node,
                               struct flow_node * input_node, int32_t * first_replacement_node,
                               int32_t * last_replacement_node)
{

    *first_replacement_node = flow_node_create_generic(c, g, -1, flow_ntype_Flip_Vertical);
    if (*first_replacement_node < 0) {
        FLOW_error_return(c);
    }
    *last_replacement_node = flow_node_create_generic(c, g, *first_replacement_node, flow_ntype_Flip_Horizontal);
    if (*last_replacement_node < 0) {
        FLOW_error_return(c);
    }
    return true;
}

static bool flatten_flip_v(flow_c * c, struct flow_graph ** g, int32_t node_id, struct flow_node * node,
                           struct flow_node * input_node, int32_t * first_replacement_node,
                           int32_t * last_replacement_node)
{
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
        = flow_node_create_generic(c, g, *first_replacement_node, flow_ntype_primitive_Flip_Vertical_Mutate);
    if (*last_replacement_node < 0) {
        FLOW_error_return(c);
    }
    if (!must_clone) {
        *first_replacement_node = *last_replacement_node;
    }
    return true;
}

static bool flatten_flip_h(flow_c * c, struct flow_graph ** g, int32_t node_id, struct flow_node * node,
                           struct flow_node * input_node, int32_t * first_replacement_node,
                           int32_t * last_replacement_node)
{

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
        = flow_node_create_generic(c, g, *first_replacement_node, flow_ntype_primitive_Flip_Horizontal_Mutate);
    if (*last_replacement_node < 0) {
        FLOW_error_return(c);
    }
    if (!must_clone) {
        *first_replacement_node = *last_replacement_node;
    }
    return true;
}

static bool execute_flip_vertical(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id)
{
    FLOW_GET_INPUT_EDGE(g, node_id)
    struct flow_node * n = &g->nodes[node_id];
    n->result_bitmap = g->nodes[input_edge->from].result_bitmap;
    flow_bitmap_bgra_flip_vertical(c, n->result_bitmap);
    return true;
}

static bool execute_flip_horizontal(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id)
{
    FLOW_GET_INPUT_EDGE(g, node_id)
    struct flow_node * n = &g->nodes[node_id];
    n->result_bitmap = g->nodes[input_edge->from].result_bitmap;
    flow_bitmap_bgra_flip_horizontal(c, n->result_bitmap);
    return true;
}

const struct flow_node_definition flow_define_rotate_90 = { .type = flow_ntype_Rotate_90,
                                                            .nodeinfo_bytes_fixed = 0,
                                                            .input_count = 1,
                                                            .canvas_count = 0,
                                                            .populate_dimensions = dimensions_transpose,
                                                            .type_name = "rotate 90",
                                                            .pre_optimize_flatten = flatten_rotate_90 };

const struct flow_node_definition flow_define_rotate_180 = { .type = flow_ntype_Rotate_180,
                                                             .nodeinfo_bytes_fixed = 0,
                                                             .input_count = 1,
                                                             .canvas_count = 0,
                                                             .populate_dimensions = dimensions_mimic_input,
                                                             .type_name = "rotate 180",
                                                             .pre_optimize_flatten = flatten_rotate_180 };

const struct flow_node_definition flow_define_rotate_270 = { .type = flow_ntype_Rotate_270,
                                                             .nodeinfo_bytes_fixed = 0,
                                                             .input_count = 1,
                                                             .canvas_count = 0,
                                                             .populate_dimensions = dimensions_transpose,
                                                             .type_name = "rotate 270",
                                                             .pre_optimize_flatten = flatten_rotate_270 };

// Optimizable (non-mutating)
const struct flow_node_definition flow_define_flip_v = { .type = flow_ntype_Flip_Vertical,
                                                         .nodeinfo_bytes_fixed = 0,
                                                         .input_count = 1,
                                                         .canvas_count = 0,
                                                         .populate_dimensions = dimensions_mimic_input,
                                                         .type_name = "flip vertical",
                                                         .post_optimize_flatten = flatten_flip_v };

const struct flow_node_definition flow_define_flip_h = { .type = flow_ntype_Flip_Horizontal,
                                                         .nodeinfo_bytes_fixed = 0,
                                                         .input_count = 1,
                                                         .canvas_count = 0,
                                                         .populate_dimensions = dimensions_mimic_input,
                                                         .type_name = "flip horizontal",
                                                         .post_optimize_flatten = flatten_flip_h };

const struct flow_node_definition flow_define_transpose = { .type = flow_ntype_Transpose,
                                                            .nodeinfo_bytes_fixed = 0,
                                                            .input_count = 1,
                                                            .canvas_count = 0,
                                                            .populate_dimensions = dimensions_transpose,
                                                            .type_name = "transpose",
                                                            .post_optimize_flatten = flatten_transpose };

const struct flow_node_definition flow_define_flip_v_primitive = { .type = flow_ntype_primitive_Flip_Vertical_Mutate,
                                                                   .nodeinfo_bytes_fixed = 0,
                                                                   .input_count = 1,
                                                                   .canvas_count = 0,
                                                                   .populate_dimensions = dimensions_mimic_input,
                                                                   .type_name = "flip vertical mutate",
                                                                   .execute = execute_flip_vertical };
const struct flow_node_definition flow_define_flip_h_primitive = { .type = flow_ntype_primitive_Flip_Horizontal_Mutate,
                                                                   .nodeinfo_bytes_fixed = 0,
                                                                   .input_count = 1,
                                                                   .canvas_count = 0,
                                                                   .populate_dimensions = dimensions_mimic_input,
                                                                   .type_name = "flip horizontal mutate",
                                                                   .execute = execute_flip_horizontal };
const struct flow_node_definition flow_define_apply_orientation = { .type = flow_ntype_Apply_Orientation,
                                                                .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_apply_orientation),
                                                                .input_count = 1,
                                                                .canvas_count = 0,
                                                                .stringify = stringify_apply_orientation,
                                                                .populate_dimensions = dimensions_apply_orientation,
                                                                .type_name = "apply_orientation",
                                                                .post_optimize_flatten = flatten_apply_orientation };
