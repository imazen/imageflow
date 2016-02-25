#include "graph.h"
#include "job.h"

//
// typedef bool (*flow_nodedef_fn_stringify)(Context *c, struct flow_graph *g, int32_t node_id, char * buffer, size_t
// buffer_size);
//
//
//
// typedef bool (*flow_nodedef_fn_infobyte_count)(Context *c, struct flow_graph *g, int32_t node_id, int32_t *
// infobytes_count_out);
//
// typedef bool (*flow_nodedef_fn_populate_dimensions)(Context *c, struct flow_graph *g, int32_t node_id, int32_t
// outbound_edge_id);
//
//
// typedef bool (*flow_nodedef_fn_flatten)(Context *c, struct flow_graph **graph_ref, int32_t node_id);
//
// typedef bool (*flow_nodedef_fn_execute)(Context *c, struct flow_graph *g, int32_t node_id);
//
//
//
//
// struct flow_node_definition{
//    flow_ntype type;
//    int32_t input_count;
//    int32_t canvas_count;
//    const char * type_name;
//
//    flow_nodedef_fn_stringify stringify;
//    flow_nodedef_fn_infobyte_count count_infobytes;
//    int32_t nodeinfo_bytes_fixed;
//    flow_nodedef_fn_populate_dimensions populate_dimensions;
//    flow_nodedef_fn_flatten flatten;
//    flow_nodedef_fn_execute execute;
//
//};

#define FLOW_GET_INPUT_EDGE(g, node_id)                                                                                \
    int32_t input_edge_id = flow_graph_get_first_inbound_edge_of_type(c, g, node_id, flow_edgetype_input);             \
    if (input_edge_id < 0) {                                                                                           \
        CONTEXT_error(c, Invalid_inputs_to_node);                                                                      \
        return false;                                                                                                  \
    }                                                                                                                  \
    struct flow_edge* input_edge = &g->edges[input_edge_id];

#define FLOW_GET_CANVAS_EDGE(g, node_id)                                                                               \
    int32_t canvas_edge_id = flow_graph_get_first_inbound_edge_of_type(c, g, node_id, flow_edgetype_canvas);           \
    if (canvas_edge_id < 0) {                                                                                          \
        CONTEXT_error(c, Invalid_inputs_to_node);                                                                      \
        return false;                                                                                                  \
    }                                                                                                                  \
    struct flow_edge* canvas_edge = &g->edges[canvas_edge_id];

static bool stringify_state(char* buffer, size_t buffer_isze, struct flow_node* n)
{
    snprintf(buffer, buffer_isze, "[%d/%d]", n->state, flow_node_state_Done);
    return true;
}

static const char* get_format_name(BitmapPixelFormat f, bool alpha_meaningful)
{
    switch (f) {
    case Bgr24:
        return "Bgr24";
    case Bgra32:
        return alpha_meaningful ? "Bgra32" : "Bgr32";
    default:
        return "?";
    }
}

static bool stringify_scale(Context* c, struct flow_graph* g, int32_t node_id, char* buffer, size_t buffer_size)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_size, info);

    char state[64];
    if (!stringify_state(state, 63, &g->nodes[node_id])) {
        CONTEXT_error_return(c);
    }

    snprintf(buffer, buffer_size, "scale %lux%lu %s", info->width, info->height, (const char*)(const char*) & state);
    return true;
}

static bool stringify_canvas(Context* c, struct flow_graph* g, int32_t node_id, char* buffer, size_t buffer_size)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_createcanvas, info);

    char state[64];
    if (!stringify_state(state, 63, &g->nodes[node_id])) {
        CONTEXT_error_return(c);
    }

    snprintf(buffer, buffer_size, "canvas %lux%lu %s %s", info->width, info->height,
             get_format_name(info->format, false), (const char*)&state);
    return true;
}
static char* stringify_colorspace(WorkingFloatspace space)
{
    switch (space) {
    case Floatspace_gamma:
        return "gamma";
    case Floatspace_linear:
        return "linear";
    case Floatspace_srgb:
        return "sRGB";
    default:
        return "colorspace unknown";
    }
}
static char* stringify_filter(InterpolationFilter filter)
{
    switch (filter) {
    case Filter_Robidoux:
        return "robidoux";
    default:
        return "??";
    }
}
static bool stringify_render1d(Context* c, struct flow_graph* g, int32_t node_id, char* buffer, size_t buffer_size)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_render_to_canvas_1d, info);
    char state[64];
    if (!stringify_state(state, 63, &g->nodes[node_id])) {
        CONTEXT_error_return(c);
    }

    snprintf(buffer, buffer_size, "render1d x%d %s %s\nat %d,%d. %s sharp%d%%. %s", info->scale_to_width,
             stringify_filter(info->interpolation_filter), (const char*)&state, info->canvas_x, info->canvas_y,
             info->transpose_on_write ? "transpose. " : "", (int)info->sharpen_percent_goal,
             stringify_colorspace(info->scale_in_colorspace));
    return true;
}

static bool stringify_placeholder(Context* c, struct flow_graph* g, int32_t node_id, char* buffer, size_t buffer_size)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_index, info);

    snprintf(buffer, buffer_size, "placeholder #%d", info->index);
    return true;
}

static bool stringify_bitmap_bgra_pointer(Context* c, struct flow_graph* g, int32_t node_id, char* buffer,
                                          size_t buffer_size)
{
    snprintf(buffer, buffer_size, "* BitmapBgra");
    return true;
}

static bool stringify_decode(Context* c, struct flow_graph* g, int32_t node_id, char* buffer, size_t buffer_size)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_codec, info);

    struct flow_job_codec_definition* def = flow_job_get_codec_definition(c, info->type);

    // TODO FIX job null
    if (def->stringify == NULL) {
        if (def->name == NULL) {
            CONTEXT_error(c, Not_implemented);
            return false;
        } else {

            char state[64];
            if (!stringify_state(state, 63, &g->nodes[node_id])) {
                CONTEXT_error_return(c);
            }

            snprintf(buffer, buffer_size, "%s %s", def->name, (const char*)&state);
        }
    } else {
        def->stringify(c, NULL, info->codec_state, buffer, buffer_size);
    }
    return true;
}

static bool stringify_encode(Context* c, struct flow_graph* g, int32_t node_id, char* buffer, size_t buffer_size)
{
    return stringify_decode(c, g, node_id, buffer, buffer_size);
}

static bool dimensions_scale(Context* c, struct flow_graph* g, int32_t node_id, int32_t outbound_edge_id,
                             bool force_estimate)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_size, info)
    FLOW_GET_INPUT_EDGE(g, node_id)

    struct flow_edge* output = &g->edges[outbound_edge_id];

    output->from_width = info->width;
    output->from_height = info->height;
    output->from_alpha_meaningful = input_edge->from_alpha_meaningful;
    output->from_format = input_edge->from_format;
    return true;
}

static bool dimensions_bitmap_bgra_pointer(Context* c, struct flow_graph* g, int32_t node_id, int32_t outbound_edge_id,
                                           bool force_estimate)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_resource_bitmap_bgra, info)

    if (*info->ref == NULL) {
        CONTEXT_error(c, Invalid_inputs_to_node);
        return false; // If this is acting as an source node, info->data MUST be populated
    }
    BitmapBgra* b = *info->ref;

    struct flow_edge* output = &g->edges[outbound_edge_id];
    output->from_width = b->w;
    output->from_height = b->h;
    output->from_alpha_meaningful = b->alpha_meaningful;
    output->from_format = b->fmt;
    return true;
}

static bool dimensions_mimic_input(Context* c, struct flow_graph* g, int32_t node_id, int32_t outbound_edge_id,
                                   bool force_estimate)
{
    FLOW_GET_INPUT_EDGE(g, node_id)

    struct flow_edge* output = &g->edges[outbound_edge_id];

    output->from_width = input_edge->from_width;
    output->from_height = input_edge->from_height;
    output->from_alpha_meaningful = input_edge->from_alpha_meaningful;
    output->from_format = input_edge->from_format;
    return true;
}
static bool dimensions_transpose(Context* c, struct flow_graph* g, int32_t node_id, int32_t outbound_edge_id,
                                 bool force_estimate)
{
    FLOW_GET_INPUT_EDGE(g, node_id)

    struct flow_edge* output = &g->edges[outbound_edge_id];

    output->from_width = input_edge->from_height; // we just swap with and height
    output->from_height = input_edge->from_width;
    output->from_alpha_meaningful = input_edge->from_alpha_meaningful;
    output->from_format = input_edge->from_format;
    return true;
}

static bool dimensions_copy_rect(Context* c, struct flow_graph* g, int32_t node_id, int32_t outbound_edge_id,
                                 bool force_estimate)
{
    FLOW_GET_CANVAS_EDGE(g, node_id)

    struct flow_edge* output = &g->edges[outbound_edge_id];

    // TODO: implement validation of all coordinates here.
    output->from_width = canvas_edge->from_width;
    output->from_height = canvas_edge->from_height;
    output->from_alpha_meaningful = canvas_edge->from_alpha_meaningful;
    output->from_format = canvas_edge->from_format;
    return true;
}

static bool dimensions_crop(Context* c, struct flow_graph* g, int32_t node_id, int32_t outbound_edge_id,
                            bool force_estimate)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_crop, info)
    FLOW_GET_INPUT_EDGE(g, node_id)

    struct flow_edge* output = &g->edges[outbound_edge_id];

    output->from_width = info->x2 - info->x1;
    output->from_height = info->y2 - info->y1;
    if (output->from_width < 1 || output->from_height < 1) {
        CONTEXT_error(c, Invalid_argument);
        return false;
    }
    if ((int32_t)info->x1 >= input_edge->from_width || (int32_t)info->x2 > input_edge->from_width) {
        CONTEXT_error(c, Invalid_argument);
        return false;
    }
    if ((int32_t)info->y1 >= input_edge->from_height || (int32_t)info->y2 > input_edge->from_height) {
        CONTEXT_error(c, Invalid_argument);
        return false;
    }
    output->from_alpha_meaningful = input_edge->from_alpha_meaningful;
    output->from_format = input_edge->from_format;
    return true;
}

static bool dimensions_canvas(Context* c, struct flow_graph* g, int32_t node_id, int32_t outbound_edge_id,
                              bool force_estimate)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_createcanvas, info)

    struct flow_edge* output = &g->edges[outbound_edge_id];

    output->from_width = info->width;
    output->from_height = info->height;
    output->from_alpha_meaningful = false;
    output->from_format = info->format;
    return true;
}

static bool dimensions_render1d(Context* c, struct flow_graph* g, int32_t node_id, int32_t outbound_edge_id,
                                bool force_estimate)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_render_to_canvas_1d, info)
    FLOW_GET_INPUT_EDGE(g, node_id)

    struct flow_edge* output = &g->edges[outbound_edge_id];

    output->from_format = Bgra32; // TODO: maybe wrong
    output->from_alpha_meaningful = true; // TODO: WRONG! Involve "input" in decision
    output->from_width = info->transpose_on_write ? input_edge->from_height : info->scale_to_width;
    output->from_height = info->transpose_on_write ? info->scale_to_width : input_edge->from_height;
    return true;
}

static bool dimensions_render_to_canvas_1d(Context* c, struct flow_graph* g, int32_t node_id, int32_t outbound_edge_id,
                                           bool force_estimate)
{
    // FLOW_GET_INFOBYTES(g,node_id, flow_nodeinfo_size, info)
    FLOW_GET_CANVAS_EDGE(g, node_id)

    struct flow_edge* output = &g->edges[outbound_edge_id];

    output->from_format = Bgra32; // TODO: maybe wrong
    output->from_alpha_meaningful = true; // TODO: WRONG! Involve "input" in decision
    output->from_width = canvas_edge->from_width;
    output->from_height = canvas_edge->from_height;
    return true;
}

static bool dimensions_decode(Context* c, struct flow_graph* g, int32_t node_id, int32_t outbound_edge_id,
                              bool force_estimate)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_codec, info)

    struct flow_edge* output = &g->edges[outbound_edge_id];

    struct flow_job_codec_definition* def = flow_job_get_codec_definition(c, info->type);

    if (def == NULL || def->get_frame_info == NULL) {
        CONTEXT_error(c, Not_implemented);
        return false;
    }
    struct decoder_frame_info frame_info;

    if (!def->get_frame_info(c, NULL, info->codec_state, &frame_info)) {
        CONTEXT_error_return(c);
    }

    output->from_width = frame_info.w;
    output->from_height = frame_info.h;
    output->from_alpha_meaningful = true; // TODO Wrong
    output->from_format = Bgra32;
    return true;
}

static int32_t create_primitve_render_to_canvas_1d_node(Context* c, struct flow_graph** g, int32_t last,
                                                        int32_t to_width, bool transpose, InterpolationFilter filter)
{

    WorkingFloatspace floatspace = Floatspace_linear;
    flow_compositing_mode mode = flow_compositing_mode_overwrite;
    uint8_t* matte_color[4];
    float sharpen_percent = 0;

    int32_t id = flow_node_create_render_to_canvas_1d(c, g, last, transpose, 0, 0, to_width, floatspace,
                                                      sharpen_percent, mode, matte_color, NULL, filter);
    if (id < 0) {
        CONTEXT_add_to_callstack(c);
    }
    return id;
}

static int32_t create_render1d_node(Context* c, struct flow_graph** g, int32_t last, int32_t to_width, bool transpose,
                                    InterpolationFilter filter)
{

    WorkingFloatspace floatspace = Floatspace_linear;
    float sharpen_percent = 0;

    int32_t id = flow_node_create_render1d(c, g, last, transpose, to_width, floatspace, sharpen_percent, NULL, filter);
    if (id < 0) {
        CONTEXT_add_to_callstack(c);
    }
    return id;
}
static bool flatten_scale(Context* c, struct flow_graph** g, int32_t node_id, struct flow_node* node,
                          struct flow_edge* input_edge, int32_t* first_replacement_node, int32_t* last_replacement_node)
{
    FLOW_GET_INFOBYTES((*g), node_id, flow_nodeinfo_size, size)

    InterpolationFilter filter = Filter_Robidoux;
    *first_replacement_node = create_render1d_node(c, g, -1, size->width, true, filter);
    if (*first_replacement_node < 0) {
        CONTEXT_error_return(c);
    }

    *last_replacement_node = create_render1d_node(c, g, *first_replacement_node, size->height, true, filter);
    if (*last_replacement_node < 0) {
        CONTEXT_error_return(c);
    }
    return true;
}

static bool flatten_transpose(Context* c, struct flow_graph** g, int32_t node_id, struct flow_node* node,
                              struct flow_edge* input_edge, int32_t* first_replacement_node,
                              int32_t* last_replacement_node)
{

    int32_t canvas = flow_node_create_canvas(c, g, -1, input_edge->from_format, input_edge->from_height,
                                             input_edge->from_width, 0);
    if (canvas < 0) {
        CONTEXT_error_return(c);
    }

    *first_replacement_node = create_primitve_render_to_canvas_1d_node(c, g, *first_replacement_node,
                                                                       input_edge->from_width, true, Filter_Robidoux);
    if (*first_replacement_node < 0) {
        CONTEXT_error_return(c);
    }
    if (flow_edge_create(c, g, canvas, *first_replacement_node, flow_edgetype_canvas) < 0) {
        CONTEXT_error_return(c);
    }

    *last_replacement_node = *first_replacement_node;
    return true;
}

static bool flatten_rotate_90(Context* c, struct flow_graph** g, int32_t node_id, struct flow_node* node,
                              struct flow_edge* input_edge, int32_t* first_replacement_node,
                              int32_t* last_replacement_node)
{

    *first_replacement_node = flow_node_create_transpose(c, g, -1);
    if (*first_replacement_node < 0) {
        CONTEXT_error_return(c);
    }
    *last_replacement_node = flow_node_create_generic(c, g, *first_replacement_node, flow_ntype_Flip_Vertical);
    if (*last_replacement_node < 0) {
        CONTEXT_error_return(c);
    }
    return true;
}
static bool flatten_rotate_270(Context* c, struct flow_graph** g, int32_t node_id, struct flow_node* node,
                               struct flow_edge* input_edge, int32_t* first_replacement_node,
                               int32_t* last_replacement_node)
{

    *first_replacement_node = flow_node_create_generic(c, g, -1, flow_ntype_Flip_Vertical);
    if (*first_replacement_node < 0) {
        CONTEXT_error_return(c);
    }
    *last_replacement_node = flow_node_create_transpose(c, g, *first_replacement_node);
    if (*last_replacement_node < 0) {
        CONTEXT_error_return(c);
    }
    return true;
}
static bool flatten_rotate_180(Context* c, struct flow_graph** g, int32_t node_id, struct flow_node* node,
                               struct flow_edge* input_edge, int32_t* first_replacement_node,
                               int32_t* last_replacement_node)
{

    *first_replacement_node = flow_node_create_generic(c, g, -1, flow_ntype_Flip_Vertical);
    if (*first_replacement_node < 0) {
        CONTEXT_error_return(c);
    }
    *last_replacement_node = flow_node_create_generic(c, g, *first_replacement_node, flow_ntype_Flip_Horizontal);
    if (*last_replacement_node < 0) {
        CONTEXT_error_return(c);
    }
    return true;
}

static bool node_has_other_dependents(Context* c, struct flow_graph* g, int32_t node_id,
                                      int32_t excluding_dependent_node_id, bool* has_other_dependents)
{
    // TODO: Implement tracing logic
    *has_other_dependents = true;
    return true;
}

static bool set_node_optimized_and_update_state(Context* c, struct flow_graph* g, int32_t node_id)
{
    struct flow_node* n = &g->nodes[node_id];

    n->state = (flow_node_state)(n->state | flow_node_state_Optimized);
    if (!flow_node_update_state(c, g, node_id)) {
        CONTEXT_error_return(c);
    }
    return true;
}

static bool flatten_flip_v(Context* c, struct flow_graph** g, int32_t node_id, struct flow_node* node,
                           struct flow_edge* input_edge, int32_t* first_replacement_node,
                           int32_t* last_replacement_node)
{
    bool must_clone = false;
    if (!node_has_other_dependents(c, *g, input_edge->from, node_id, &must_clone)) {
        CONTEXT_error_return(c);
    }
    if (must_clone) {
        *first_replacement_node = flow_node_create_clone(c, g, -1);
        if (*first_replacement_node < 0) {
            CONTEXT_error_return(c);
        }
    } else {
        *first_replacement_node = -1;
    }
    *last_replacement_node
        = flow_node_create_generic(c, g, *first_replacement_node, flow_ntype_primitive_Flip_Vertical_Mutate);
    if (*last_replacement_node < 0) {
        CONTEXT_error_return(c);
    }
    if (!must_clone) {
        *first_replacement_node = *last_replacement_node;
    }
    return true;
}

static bool flatten_flip_h(Context* c, struct flow_graph** g, int32_t node_id, struct flow_node* node,
                           struct flow_edge* input_edge, int32_t* first_replacement_node,
                           int32_t* last_replacement_node)
{

    bool must_clone = false;
    if (!node_has_other_dependents(c, *g, input_edge->from, node_id, &must_clone)) {
        CONTEXT_error_return(c);
    }
    if (must_clone) {
        *first_replacement_node = flow_node_create_clone(c, g, -1);
        if (*first_replacement_node < 0) {
            CONTEXT_error_return(c);
        }
    } else {
        *first_replacement_node = -1;
    }
    *last_replacement_node
        = flow_node_create_generic(c, g, *first_replacement_node, flow_ntype_primitive_Flip_Horizontal_Mutate);
    if (*last_replacement_node < 0) {
        CONTEXT_error_return(c);
    }
    if (!must_clone) {
        *first_replacement_node = *last_replacement_node;
    }
    return true;
}

static bool flatten_crop(Context* c, struct flow_graph** g, int32_t node_id, struct flow_node* node,
                         struct flow_edge* input_edge, int32_t* first_replacement_node, int32_t* last_replacement_node)
{

    FLOW_GET_INFOBYTES((*g), node_id, flow_nodeinfo_crop, info)

    bool must_clone = false;
    if (!node_has_other_dependents(c, *g, input_edge->from, node_id, &must_clone)) {
        CONTEXT_error_return(c);
    }
    if (must_clone) {
        *first_replacement_node = flow_node_create_clone(c, g, -1);
        if (*first_replacement_node < 0) {
            CONTEXT_error_return(c);
        }
    } else {
        *first_replacement_node = -1;
    }
    *last_replacement_node
        = flow_node_create_primitive_crop(c, g, *first_replacement_node, info->x1, info->y1, info->x2, info->y2);
    if (*last_replacement_node < 0) {
        CONTEXT_error_return(c);
    }
    if (!must_clone) {
        *first_replacement_node = *last_replacement_node;
    }
    return true;
}

static bool flatten_render1d(Context* c, struct flow_graph** g, int32_t node_id, struct flow_node* node,
                             struct flow_edge* input_edge, int32_t* first_replacement_node,
                             int32_t* last_replacement_node)
{

    FLOW_GET_INFOBYTES((*g), node_id, flow_nodeinfo_render_to_canvas_1d, info)

    int32_t c_h = info->transpose_on_write ? info->scale_to_width : input_edge->from_height;
    int32_t c_w = info->transpose_on_write ? input_edge->from_height : info->scale_to_width;

    int32_t canvas = flow_node_create_canvas(c, g, -1, input_edge->from_format, c_w, c_h, 0);
    if (canvas < 0) {
        CONTEXT_error_return(c);
    }

    if (!set_node_optimized_and_update_state(c, *g, canvas)) {
        CONTEXT_error_return(c);
    }

    *first_replacement_node = create_primitve_render_to_canvas_1d_node(c, g, *first_replacement_node,
                                                                       info->scale_to_width, true, Filter_Robidoux);
    if (*first_replacement_node < 0) {
        CONTEXT_error_return(c);
    }
    if (flow_edge_create(c, g, canvas, *first_replacement_node, flow_edgetype_canvas) < 0) {
        CONTEXT_error_return(c);
    }

    *last_replacement_node = *first_replacement_node;
    return true;
}

static bool flatten_decode(Context* c, struct flow_graph** g, int32_t node_id, struct flow_node* node,
                           struct flow_edge* input_edge, int32_t* first_replacement_node,
                           int32_t* last_replacement_node)
{

    node->type = flow_ntype_primitive_decoder;

    *first_replacement_node = *last_replacement_node = node_id;
    // TODO, inject color space correction and other filters
    return true;
}

static bool flatten_encode(Context* c, struct flow_graph** g, int32_t node_id, struct flow_node* node,
                           struct flow_edge* input_edge, int32_t* first_replacement_node,
                           int32_t* last_replacement_node)
{

    node->type = flow_ntype_primitive_encoder;

    *first_replacement_node = *last_replacement_node = node_id;
    // TODO, inject color space correction and other filters
    return true;
}

static bool flatten_clone(Context* c, struct flow_graph** g, int32_t node_id, struct flow_node* node,
                          struct flow_edge* input_edge, int32_t* first_replacement_node, int32_t* last_replacement_node)
{

    // create canvas
    int32_t canvas = flow_node_create_canvas(c, g, -1, input_edge->from_format, input_edge->from_width,
                                             input_edge->from_height, 0);
    if (canvas < 0) {
        CONTEXT_error_return(c);
    }
    // Blit from image
    *first_replacement_node = flow_node_create_primitive_copy_rect_to_canvas(c, g, -1, 0, 0, input_edge->from_width,
                                                                             input_edge->from_height, 0, 0);
    if (*first_replacement_node < 0) {
        CONTEXT_error_return(c);
    }
    // blit to canvas
    if (flow_edge_create(c, g, canvas, *first_replacement_node, flow_edgetype_canvas) < 0) {
        CONTEXT_error_return(c);
    }

    *last_replacement_node = *first_replacement_node;
    return true;
}

static bool execute_canvas(Context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_createcanvas, info)

    struct flow_node* n = &g->nodes[node_id];
    // TODO: bgcolor
    n->result_bitmap = BitmapBgra_create(c, info->width, info->height, true, info->format);
    if (n->result_bitmap == NULL) {
        CONTEXT_error_return(c);
    }
    // Uncomment to make canvas blue for debugging
    //    for (int32_t y =0; y < (int32_t)n->result_bitmap->h; y++)
    //    for (int32_t i = 0; i < (int32_t)n->result_bitmap->w; i++){
    //        n->result_bitmap->pixels[n->result_bitmap->stride * y + i * 4] = 0xFF;
    //        n->result_bitmap->pixels[n->result_bitmap->stride * y + i * 4 + 3] = 0xFF;
    //    }

    return true;
}

static bool execute_flip_vertical(Context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id)
{
    FLOW_GET_INPUT_EDGE(g, node_id)
    struct flow_node* n = &g->nodes[node_id];
    n->result_bitmap = g->nodes[input_edge->from].result_bitmap;
    BitmapBgra_flip_vertical(c, n->result_bitmap);
    return true;
}

static bool execute_flip_horizontal(Context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id)
{
    FLOW_GET_INPUT_EDGE(g, node_id)
    struct flow_node* n = &g->nodes[node_id];
    n->result_bitmap = g->nodes[input_edge->from].result_bitmap;
    BitmapBgra_flip_horizontal(c, n->result_bitmap);
    return true;
}

static bool execute_crop(Context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_crop, info)
    FLOW_GET_INPUT_EDGE(g, node_id)
    struct flow_node* n = &g->nodes[node_id];

    BitmapBgra* original = g->nodes[input_edge->from].result_bitmap;
    ;
    BitmapBgra* b = BitmapBgra_create_header(c, info->x2 - info->x1, info->y2 - info->y1);
    if (b == NULL) {
        CONTEXT_error_return(c);
    }
    b->alpha_meaningful = original->alpha_meaningful;
    b->borrowed_pixels = true;
    b->can_reuse_space = false;
    b->compositing_mode = original->compositing_mode;
    b->fmt = original->fmt;
    memcpy(&b->matte_color, &original->matte_color, 4);
    b->stride = original->stride;
    b->pixels = original->pixels + (original->stride * info->y1)
                + BitmapPixelFormat_bytes_per_pixel(original->fmt) * info->x1;

    n->result_bitmap = b;
    return true;
}

static bool execute_bitmap_bgra_pointer(Context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_resource_bitmap_bgra, info)
    struct flow_node* n = &g->nodes[node_id];

    int count = flow_graph_get_inbound_edge_count_of_type(c, g, node_id, flow_edgetype_input);
    if (count == 1) {
        FLOW_GET_INPUT_EDGE(g, node_id)
        *info->ref = n->result_bitmap = g->nodes[input_edge->from].result_bitmap;
    } else {
        n->result_bitmap = *info->ref;
        if (*info->ref == NULL) {
            CONTEXT_error(c, Invalid_inputs_to_node);
            return false;
        }
    }
    return true;
}

static bool execute_render1d(Context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_render_to_canvas_1d, info)
    FLOW_GET_INPUT_EDGE(g, node_id)
    FLOW_GET_CANVAS_EDGE(g, node_id)
    struct flow_node* n = &g->nodes[node_id];

    BitmapBgra* input = g->nodes[input_edge->from].result_bitmap;
    BitmapBgra* canvas = g->nodes[canvas_edge->from].result_bitmap;

    if (!flow_node_execute_render_to_canvas_1d(c, job, input, canvas, info)) {
        CONTEXT_error_return(c);
    }
    n->result_bitmap = canvas;
    return true;
}

static bool execute_copy_rect(Context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_copy_rect_to_canvas, info)
    FLOW_GET_INPUT_EDGE(g, node_id)
    FLOW_GET_CANVAS_EDGE(g, node_id)
    struct flow_node* n = &g->nodes[node_id];

    BitmapBgra* input = g->nodes[input_edge->from].result_bitmap;
    BitmapBgra* canvas = g->nodes[canvas_edge->from].result_bitmap;

    // TODO: implement bounds checks!!!
    if (input->fmt != canvas->fmt) {
        CONTEXT_error(c, Invalid_argument);
        return false;
    }
    if (info->x == 0 && info->from_x == 0 && info->from_y == 0 && info->y == 0 && info->width == input->w
        && info->width == canvas->w && info->height == input->h && info->height == canvas->h
        && canvas->stride == input->stride) {
        memcpy(canvas->pixels, input->pixels, input->stride * input->h);
        canvas->alpha_meaningful = input->alpha_meaningful;
    } else {
        int32_t bytes_pp = BitmapPixelFormat_bytes_per_pixel(input->fmt);
        for (uint32_t y = 0; y < info->height; y++) {
            void* from_ptr = input->pixels + (size_t)(input->stride * (info->from_y + y) + bytes_pp * info->from_x);
            void* to_ptr = canvas->pixels + (size_t)(canvas->stride * (info->y + y) + bytes_pp * info->x);
            memcpy(to_ptr, from_ptr, info->width * bytes_pp);
        }
    }
    n->result_bitmap = canvas;
    return true;
}

static bool execute_decode(Context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_codec, info)

    struct flow_node* n = &g->nodes[node_id];

    struct flow_job_codec_definition* def = flow_job_get_codec_definition(c, info->type);

    if (def == NULL || def->get_frame_info == NULL || def->read_frame == NULL) {
        CONTEXT_error(c, Not_implemented);
        return false;
    }
    struct decoder_frame_info frame_info;
    if (!def->get_frame_info(c, NULL, info->codec_state, &frame_info)) {
        CONTEXT_error_return(c);
    }

    n->result_bitmap = BitmapBgra_create(c, frame_info.w, frame_info.h, true, Bgra32);
    if (n->result_bitmap == NULL) {
        CONTEXT_error_return(c);
    }
    if (!def->read_frame(c, NULL, info->codec_state, n->result_bitmap)) {
        CONTEXT_error_return(c);
    }
    return true;
}

static bool execute_encode(Context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_codec, info)
    FLOW_GET_INPUT_EDGE(g, node_id)
    struct flow_node* n = &g->nodes[node_id];
    n->result_bitmap = g->nodes[input_edge->from].result_bitmap;

    struct flow_job_codec_definition* def = flow_job_get_codec_definition(c, info->type);

    if (def == NULL || def->write_frame == NULL) {
        CONTEXT_error(c, Not_implemented);
        return false;
    }

    if (!def->write_frame(c, NULL, info->codec_state, n->result_bitmap)) {
        CONTEXT_error_return(c);
    }
    return true;
}

struct flow_node_definition flow_node_defs[] = {
    // High level (non-executable). These *flatten* into more primitive nodes
    {
      .type = flow_ntype_Scale,
      .input_count = 1,
      .canvas_count = 0,
      .type_name = "scale",
      .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_size),
      .stringify = stringify_scale,
      .populate_dimensions = dimensions_scale,
      .pre_optimize_flatten = flatten_scale,

    },
    {
      .type = flow_ntype_Resource_Placeholder,
      .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_index),
      .type_name = "placeholder",
      .input_count = -1,
      .canvas_count = 0,
      .stringify = stringify_placeholder
      // Placeholders aren't *flattened*, per se - they are swapped out prior to the execution loop by the job.
    },
    { // Should be useless once we finish function/mutate logic
      .type = flow_ntype_Clone,
      .nodeinfo_bytes_fixed = 0,
      .input_count = 1,
      .canvas_count = 0,
      .populate_dimensions = dimensions_mimic_input,
      .type_name = "clone",
      .pre_optimize_flatten = flatten_clone
    },
    { .type = flow_ntype_Rotate_90,
      .nodeinfo_bytes_fixed = 0,
      .input_count = 1,
      .canvas_count = 0,
      .populate_dimensions = dimensions_transpose,
      .type_name = "rotate 90",
      .pre_optimize_flatten = flatten_rotate_90 },
    { .type = flow_ntype_Rotate_180,
      .nodeinfo_bytes_fixed = 0,
      .input_count = 1,
      .canvas_count = 0,
      .populate_dimensions = dimensions_mimic_input,
      .type_name = "rotate 180",
      .pre_optimize_flatten = flatten_rotate_180 },
    { .type = flow_ntype_Rotate_270,
      .nodeinfo_bytes_fixed = 0,
      .input_count = 1,
      .canvas_count = 0,
      .populate_dimensions = dimensions_transpose,
      .type_name = "rotate 270",
      .pre_optimize_flatten = flatten_rotate_270 },
    {
      .type = flow_ntype_decoder,
      .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_codec),
      .type_name = "decode",
      .input_count = 0,
      .canvas_count = 0, //?
      .stringify = stringify_decode,
      .populate_dimensions = dimensions_decode,
      .pre_optimize_flatten = flatten_decode,
    },
    {
      .type = flow_ntype_encoder,
      .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_codec),
      .type_name = "encode",
      .input_count = 1,
      .canvas_count = 0, //?
      .stringify = stringify_encode,
      .pre_optimize_flatten = flatten_encode,
    },
    // Optimizable (non-mutating)
    { .type = flow_ntype_Flip_Vertical,
      .nodeinfo_bytes_fixed = 0,
      .input_count = 1,
      .canvas_count = 0,
      .populate_dimensions = dimensions_mimic_input,
      .type_name = "flip vertical",
      .post_optimize_flatten = flatten_flip_v },
    { .type = flow_ntype_Flip_Horizontal,
      .nodeinfo_bytes_fixed = 0,
      .input_count = 1,
      .canvas_count = 0,
      .populate_dimensions = dimensions_mimic_input,
      .type_name = "flip horizontal",
      .post_optimize_flatten = flatten_flip_h },
    { .type = flow_ntype_Transpose,
      .nodeinfo_bytes_fixed = 0,
      .input_count = 1,
      .canvas_count = 0,
      .populate_dimensions = dimensions_transpose,
      .type_name = "transpose",
      .post_optimize_flatten = flatten_transpose },
    {
      .type = flow_ntype_Crop,
      .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_crop),
      .input_count = 1,
      .canvas_count = 0,
      .populate_dimensions = dimensions_crop,
      .type_name = "crop",
      .post_optimize_flatten = flatten_crop,
    },

    {
      .type = flow_ntype_Render1D,
      .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_render_to_canvas_1d),
      .type_name = "render1d",
      .input_count = 1,
      .canvas_count = 0,
      .stringify = stringify_render1d,
      .populate_dimensions = dimensions_render1d,
      .post_optimize_flatten = flatten_render1d,

    },

    // Non-optimizable primitives
    { .type = flow_ntype_primitive_RenderToCanvas1D,
      .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_render_to_canvas_1d),
      .type_name = "render1d_p",
      .input_count = 1,
      .canvas_count = 1,
      .stringify = stringify_render1d,
      .populate_dimensions = dimensions_render_to_canvas_1d,
      .execute = execute_render1d

    },

    { .type = flow_ntype_Create_Canvas,
      .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_createcanvas),
      .input_count = 0,
      .canvas_count = 0,
      .populate_dimensions = dimensions_canvas,
      .type_name = "canvas",
      .stringify = stringify_canvas,
      .execute = execute_canvas

    },
    { .type = flow_ntype_primitive_Flip_Vertical_Mutate,
      .nodeinfo_bytes_fixed = 0,
      .input_count = 1,
      .canvas_count = 0,
      .populate_dimensions = dimensions_mimic_input,
      .type_name = "flip vertical mutate",
      .execute = execute_flip_vertical },
    { .type = flow_ntype_primitive_Flip_Horizontal_Mutate,
      .nodeinfo_bytes_fixed = 0,
      .input_count = 1,
      .canvas_count = 0,
      .populate_dimensions = dimensions_mimic_input,
      .type_name = "flip horizontal mutate",
      .execute = execute_flip_horizontal },

    { .type = flow_ntype_primitive_Crop_Mutate_Alias,
      .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_crop),
      .input_count = 1,
      .canvas_count = 0,
      .populate_dimensions = dimensions_crop,
      .type_name = "crop mutate/alias",
      .execute = execute_crop },

    { .type = flow_ntype_primitive_CopyRectToCanvas,
      .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_copy_rect_to_canvas),
      .input_count = 1,
      .canvas_count = 1,
      .populate_dimensions = dimensions_copy_rect,
      .type_name = "copy rect",
      .execute = execute_copy_rect },

    { .type = flow_ntype_primitive_bitmap_bgra_pointer,
      .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_resource_bitmap_bgra),
      .type_name = "BitmapBgra ptr",
      .input_count = -1,
      .canvas_count = 0,
      .stringify = stringify_bitmap_bgra_pointer,
      .execute = execute_bitmap_bgra_pointer,
      .populate_dimensions = dimensions_bitmap_bgra_pointer

    },

    {
      .type = flow_ntype_primitive_decoder,
      .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_codec),
      .type_name = "decode",
      .input_count = 0,
      .canvas_count = 0, //?
      .stringify = stringify_decode,
      .populate_dimensions = dimensions_decode,
      .execute = execute_decode,

    },
    {
      .type = flow_ntype_primitive_encoder,
      .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_codec),
      .type_name = "encode",
      .input_count = 1,
      .canvas_count = 0, //?
      .stringify = stringify_encode,
      .execute = execute_encode,

    },
    {
      .type = flow_ntype_Null, .type_name = "(null)", .input_count = 0, .canvas_count = 0,

    }
};
int32_t flow_node_defs_count = sizeof(flow_node_defs) / sizeof(struct flow_node_definition);

struct flow_node_definition* flow_nodedef_get(Context* c, flow_ntype type)
{
    int i = 0;
    for (i = 0; i < flow_node_defs_count; i++) {
        if (flow_node_defs[i].type == type)
            return &flow_node_defs[i];
    }
    CONTEXT_error(c, Not_implemented);
    return NULL;
}

bool flow_node_stringify(Context* c, struct flow_graph* g, int32_t node_id, char* buffer, size_t buffer_size)
{
    struct flow_node* node = &g->nodes[node_id];
    struct flow_node_definition* def = flow_nodedef_get(c, node->type);
    if (def == NULL) {
        CONTEXT_error_return(c);
    }
    if (def->stringify == NULL) {
        if (def->type_name == NULL) {
            CONTEXT_error(c, Not_implemented);
            return false;
        }
        char state[64];
        if (!stringify_state(state, 63, &g->nodes[node_id])) {
            CONTEXT_error_return(c);
        }

        snprintf(buffer, buffer_size, "%s %s", def->type_name, (const char*)&state);
    } else {
        if (!def->stringify(c, g, node_id, buffer, buffer_size)) {
            CONTEXT_error_return(c);
        }
    }
    return true;
}
int32_t flow_node_fixed_infobyte_count(Context* c, flow_ntype type)
{
    struct flow_node_definition* def = flow_nodedef_get(c, type);
    if (def == NULL) {
        CONTEXT_add_to_callstack(c);
        return -1;
    }
    if (def->nodeinfo_bytes_fixed < 0) {
        CONTEXT_error(c, Not_implemented);
    }
    return def->nodeinfo_bytes_fixed;
}
bool flow_node_infobyte_count(Context* c, struct flow_graph* g, int32_t node_id, int32_t* infobytes_count_out)
{
    struct flow_node* node = &g->nodes[node_id];
    struct flow_node_definition* def = flow_nodedef_get(c, node->type);
    if (def == NULL) {
        CONTEXT_error_return(c);
    }
    if (def->count_infobytes == NULL) {
        *infobytes_count_out = flow_node_fixed_infobyte_count(c, node->type);
        if (*infobytes_count_out < 0) {
            CONTEXT_error_return(c);
        }
    } else {
        def->count_infobytes(c, g, node_id, infobytes_count_out);
    }
    return true;
}

bool flow_node_validate_inputs(Context* c, struct flow_graph* g, int32_t node_id)
{
    struct flow_node* node = &g->nodes[node_id];
    struct flow_node_definition* def = flow_nodedef_get(c, node->type);
    if (def == NULL) {
        CONTEXT_error_return(c);
    }

    int32_t input_edge_count = flow_graph_get_inbound_edge_count_of_type(c, g, node_id, flow_edgetype_input);
    int32_t canvas_edge_count = flow_graph_get_inbound_edge_count_of_type(c, g, node_id, flow_edgetype_canvas);

    if (def->input_count > -1 && def->input_count != input_edge_count) {
        CONTEXT_error(c, Invalid_inputs_to_node);
        return false;
    }
    if (def->canvas_count > -1 && def->canvas_count != canvas_edge_count) {
        CONTEXT_error(c, Invalid_inputs_to_node);
        return false;
    }
    return true;
}

static bool flow_node_all_types_inputs_executed(Context* c, struct flow_graph* g, int32_t node_id)
{
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++) {
        if (g->edges[i].type != flow_edgetype_null && g->edges[i].to == node_id) {
            if ((g->nodes[g->edges[i].from].state & flow_node_state_Executed) == 0) {
                return false;
            }
        }
    }
    return true;
}

bool flow_node_update_state(Context* c, struct flow_graph* g, int32_t node_id)
{

    // Ready flags are cumulative.
    // 1. If you don't have input dimensions, you're not ready for anything (although you may have already been
    // optimized, as optimization or flattening can leave the graph inconsistent.
    // 2. If you aren't a primitive or optimizable node type, you're not ready for optimizing, or post flattening or
    // executing
    // 3. If you're not optimized, you're not ready for post flattening or executing
    // 4. If you're not a primitve, or haven't been optimized, you're not ready for executing
    // 5. If your input edges haven't executed, you're not ready for executing

    struct flow_node* n = &g->nodes[node_id];

    bool input_dimensions_known = flow_node_input_edges_have_dimensions(c, g, node_id);
    bool optimization_allowed = n->type < flow_ntype_non_optimizable_nodes_begin;
    bool optimized = (n->state & flow_node_state_Optimized) > 0;
    bool is_executable_primitive = n->type < flow_ntype_non_primitive_nodes_begin;
    bool executed = (n->state & flow_node_state_Executed) > 0;

    n->state = flow_node_state_Blank;

    //#1
    if (input_dimensions_known) {
        n->state = (flow_node_state)(n->state | flow_node_state_InputDimensionsKnown);
    } else {
        if (executed) {
            CONTEXT_error(c, Invalid_internal_state);
            return false;
        }
        // One can be optimized or flattened, yet be *newly* missing input dimensions due to said processes
    }
    //#2
    if (!optimization_allowed) {
        // If it's not optimizable or executable, nothing else is relevant
        if (optimized || executed || is_executable_primitive) {
            CONTEXT_error(c, Invalid_internal_state);
            return false;
        }
        return true;
    }
    // Only pre-optimize-flattened nodes make it to this point
    n->state = (flow_node_state)(n->state | flow_node_state_PreOptimizeFlattened);

    //#3
    if (!optimized) {
        // If it's not optimizable or executable, nothing else is relevant
        if (executed) {
            CONTEXT_error(c, Invalid_internal_state);
            return false;
        }
        return true;
    }
    n->state = (flow_node_state)(n->state | flow_node_state_Optimized);

    //#4
    if (!is_executable_primitive) {
        if (executed) {
            CONTEXT_error(c, Invalid_internal_state);
            return false;
        }
        return true;
    }
    n->state = (flow_node_state)(n->state | flow_node_state_PostOptimizeFlattened);

    //#5
    bool inputs_executed = flow_node_all_types_inputs_executed(c, g, node_id);
    if (!inputs_executed) {
        if (executed) {
            CONTEXT_error(c, Invalid_internal_state);
            return false;
        }
        return true;
    }
    n->state = (flow_node_state)(n->state | flow_node_state_InputsExecuted);

    if (!executed) {
        return true;
    }
    n->state = (flow_node_state)(n->state | flow_node_state_Executed);

    return true;
}

bool flow_node_populate_dimensions_to_edge(Context* c, struct flow_graph* g, int32_t node_id, int32_t outbound_edge_id,
                                           bool force_estimate)
{
    if (!flow_node_validate_inputs(c, g, node_id)) {
        CONTEXT_error_return(c);
    }
    struct flow_node* node = &g->nodes[node_id];
    struct flow_node_definition* def = flow_nodedef_get(c, node->type);
    if (def == NULL) {
        CONTEXT_error_return(c);
    }
    if (def->populate_dimensions == NULL) {
        CONTEXT_error(c, Not_implemented);
        return false;
    } else {
        def->populate_dimensions(c, g, node_id, outbound_edge_id, force_estimate);
    }
    return true;
}
static bool flow_node_flatten_generic(Context* c, struct flow_graph** graph_ref, int32_t node_id, bool post_optimize)
{
    if (!flow_node_validate_inputs(c, *graph_ref, node_id)) {
        CONTEXT_error_return(c);
    }
    struct flow_node* node = &(*graph_ref)->nodes[node_id];
    struct flow_node_definition* def = flow_nodedef_get(c, node->type);
    if (def == NULL) {
        CONTEXT_error_return(c);
    }
    if ((post_optimize ? def->post_optimize_flatten_complex : def->pre_optimize_flatten_complex) == NULL) {
        if ((post_optimize ? def->post_optimize_flatten : def->pre_optimize_flatten) == NULL) {
            CONTEXT_error(c, Not_implemented);
            return false;
        } else {
            int32_t first_replacement_node = -1;
            int32_t last_replacement_node = -1;

            int32_t input_edge_id
                = flow_graph_get_first_inbound_edge_of_type(c, *graph_ref, node_id, flow_edgetype_input);
            struct flow_edge* input_edge = input_edge_id < 0 ? NULL : &(*graph_ref)->edges[input_edge_id];

            (post_optimize ? def->post_optimize_flatten : def->pre_optimize_flatten)(
                c, graph_ref, node_id, node, input_edge, &first_replacement_node, &last_replacement_node);

            if (first_replacement_node == last_replacement_node && last_replacement_node == node_id) {
                // do nothing
            } else {
                // Clone edges
                if (!flow_graph_duplicate_edges_to_another_node(c, graph_ref, node_id, first_replacement_node, true,
                                                                false)) {
                    CONTEXT_error_return(c);
                }
                if (!flow_graph_duplicate_edges_to_another_node(c, graph_ref, node_id, last_replacement_node, false,
                                                                true)) {
                    CONTEXT_error_return(c);
                }

                // Delete the original
                if (!flow_node_delete(c, *graph_ref, node_id)) {
                    CONTEXT_error_return(c);
                }
            }
        }
    } else {
        (post_optimize ? def->post_optimize_flatten_complex : def->pre_optimize_flatten_complex)(c, graph_ref, node_id);
    }
    return true;
}
bool flow_node_pre_optimize_flatten(Context* c, struct flow_graph** graph_ref, int32_t node_id)
{
    if (!flow_node_flatten_generic(c, graph_ref, node_id, false)) {
        CONTEXT_error_return(c);
    }
    return true;
}
bool flow_node_post_optimize_flatten(Context* c, struct flow_graph** graph_ref, int32_t node_id)
{
    if (!flow_node_flatten_generic(c, graph_ref, node_id, true)) {
        CONTEXT_error_return(c);
    }
    return true;
}
bool flow_node_execute(Context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id)
{
    if (!flow_node_validate_inputs(c, g, node_id)) {
        CONTEXT_error_return(c);
    }
    struct flow_node* node = &g->nodes[node_id];
    struct flow_node_definition* def = flow_nodedef_get(c, node->type);
    if (def == NULL) {
        CONTEXT_error_return(c);
    }
    if (def->execute == NULL) {
        CONTEXT_error(c, Not_implemented);
        return false;
    } else {
        if (!def->execute(c, job, g, node_id)) {
            CONTEXT_error_return(c);
        } else {
            node->state = (flow_node_state)(node->state | flow_node_state_Executed);
            if (!flow_node_update_state(c, g, node_id)) {
                CONTEXT_error_return(c);
            }
        }
    }
    return true;
}
bool flow_node_estimate_execution_cost(Context* c, struct flow_graph* g, int32_t node_id, size_t* bytes_required,
                                       size_t* cpu_cost)
{
    CONTEXT_error(c, Not_implemented);
    return false;
}
