#include "../imageflow_private.h"
#include "definition_helpers.h"
#include "../codecs.h"

int32_t flow_node_create_bitmap_bgra_reference(flow_c * c, struct flow_graph ** g, int32_t prev_node,
                                               struct flow_bitmap_bgra ** pointer_to_pointer_to_bitmap_bgra)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_primitive_bitmap_bgra_pointer);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_bitmap_bgra_pointer * info
        = (struct flow_nodeinfo_bitmap_bgra_pointer *) flow_node_get_info_pointer(*g, id);
    info->ref = pointer_to_pointer_to_bitmap_bgra;
    return id;
}

int32_t flow_node_create_decoder(flow_c * c, struct flow_graph ** g, int32_t prev_node, int32_t placeholder_id)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_decoder);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }

    struct flow_nodeinfo_codec * info = (struct flow_nodeinfo_codec *) flow_node_get_info_pointer(*g, id);
    info->placeholder_id = placeholder_id;
    info->codec = NULL;
    info->downscale_hints.downscale_if_wider_than = -1;
    info->downscale_hints.or_if_taller_than = -1;
    info->downscale_hints.downscaled_min_height = -1;
    info->downscale_hints.downscaled_min_width = -1;
    info->downscale_hints.gamma_correct_for_srgb_during_spatial_luma_scaling = false;
    info->downscale_hints.scale_luma_spatially = false;
    info->encoder_hints.jpeg_encode_quality = 0;

    return id;
}


int32_t flow_node_create_encoder_placeholder(flow_c * c, struct flow_graph ** g, int32_t prev_node,
                                             int32_t placeholder_id)
{
    return flow_node_create_encoder(c, g, prev_node, placeholder_id, 0, NULL);
}
int32_t flow_node_create_encoder(flow_c * c, struct flow_graph ** g, int32_t prev_node, int32_t placeholder_id,
                                 int64_t desired_encoder_id, struct flow_encoder_hints * hints)
{
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_encoder);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }

    struct flow_nodeinfo_codec * info = (struct flow_nodeinfo_codec *) flow_node_get_info_pointer(*g, id);
    info->placeholder_id = placeholder_id;
    info->codec = NULL;
    info->desired_encoder_id = desired_encoder_id;

    info->encoder_hints.jpeg_encode_quality = 90;
    if (hints != NULL){
        memcpy(&info->encoder_hints, hints, sizeof(struct flow_encoder_hints));
    }
    return id;
}

static bool dimensions_decode(flow_c * c, struct flow_graph * g, int32_t node_id, bool force_estimate)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_codec, info)

    struct flow_node * n = &g->nodes[node_id];

    struct flow_decoder_frame_info frame_info;

    if (!flow_job_decoder_get_frame_info(c, NULL, info->codec->codec_state, info->codec->codec_id, &frame_info)) {
        FLOW_error_return(c);
    }

    n->result_width = frame_info.w;
    n->result_height = frame_info.h;
    n->result_alpha_meaningful = true; // TODO Wrong
    n->result_format = frame_info.format;
    return true;
}

static bool stringify_decode(flow_c * c, struct flow_graph * g, int32_t node_id, char * buffer, size_t buffer_size)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_codec, info);
    // TODO - fix when codec_id == 0

    if (info->codec == NULL) {
        flow_snprintf(buffer, buffer_size, "(codec undetermined)");
        return true;
    }
    struct flow_codec_definition * def = flow_job_get_codec_definition(c, info->codec->codec_id);
    if (def == NULL) {
        FLOW_error_return(c);
    }

    // TODO FIX job null
    if (def->stringify == NULL) {
        if (def->name == NULL) {
            FLOW_error(c, flow_status_Not_implemented);
            return false;
        } else {

            char state[64];
            if (!stringify_state(state, 63, &g->nodes[node_id])) {
                FLOW_error_return(c);
            }

            flow_snprintf(buffer, buffer_size, "%s %s", def->name, (const char *)&state);
        }
    } else {
        def->stringify(c, NULL, info->codec->codec_state, buffer, buffer_size);
    }
    return true;
}

static bool stringify_encode(flow_c * c, struct flow_graph * g, int32_t node_id, char * buffer, size_t buffer_size)
{
    return stringify_decode(c, g, node_id, buffer, buffer_size);
}

static bool flatten_decode(flow_c * c, struct flow_graph ** g, int32_t node_id, struct flow_node * node,
                           struct flow_node * input_node, int32_t * first_replacement_node,
                           int32_t * last_replacement_node)
{

    node->type = flow_ntype_primitive_decoder;

    *first_replacement_node = *last_replacement_node = node_id;
    // TODO, inject color space correction and other filters
    return true;
}

static bool flatten_encode(flow_c * c, struct flow_graph ** g, int32_t node_id, struct flow_node * node,
                           struct flow_node * input_node, int32_t * first_replacement_node,
                           int32_t * last_replacement_node)
{

    node->type = flow_ntype_primitive_encoder;
    FLOW_GET_INFOBYTES((*g), node_id, flow_nodeinfo_codec, info)

    if (info->codec->codec_state == NULL) {
        // Not yet initialized.
        // Don't overwrite the current ID if we're using 0 - that means we're in placeholder mode
        if (info->desired_encoder_id != 0) {
            info->codec->codec_id = info->desired_encoder_id;
        }
        // TODO: establish NULL as a valid flow_job * value for initialize_codec?
        if (!flow_job_initialize_codec(c, NULL, info->codec)) {
            FLOW_add_to_callstack(c);
            return false;
        }
    }

    *first_replacement_node = *last_replacement_node = node_id;
    // TODO, inject color space correction and other filters
    return true;
}

static bool execute_decode(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_codec, info)

    struct flow_node * n = &g->nodes[node_id];

    struct flow_codec_definition * def = flow_job_get_codec_definition(c, info->codec->codec_id);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (def->get_frame_info == NULL || def->read_frame == NULL) {
        FLOW_error(c, flow_status_Not_implemented);
        return false;
    }
    struct flow_decoder_frame_info frame_info;
    if (!def->get_frame_info(c, NULL, info->codec->codec_state, &frame_info)) {
        FLOW_error_return(c);
    }

    n->result_bitmap = flow_bitmap_bgra_create(c, frame_info.w, frame_info.h, true, frame_info.format);
    if (n->result_bitmap == NULL) {
        FLOW_error_return(c);
    }
    if (!def->read_frame(c, NULL, info->codec->codec_state, n->result_bitmap)) {
        FLOW_error_return(c);
    }
    return true;
}

static bool execute_encode(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_codec, info)
    FLOW_GET_INPUT_EDGE(g, node_id)
    struct flow_node * n = &g->nodes[node_id];
    n->result_bitmap = g->nodes[input_edge->from].result_bitmap;

    struct flow_codec_definition * def = flow_job_get_codec_definition(c, info->codec->codec_id);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (def->write_frame == NULL) {
        FLOW_error(c, flow_status_Not_implemented);
        return false;
    }

    if (!def->write_frame(c, NULL,  info->codec->codec_state, n->result_bitmap, &info->encoder_hints)) {
        FLOW_error_return(c);
    }
    return true;
}

static bool stringify_bitmap_bgra_pointer(flow_c * c, struct flow_graph * g, int32_t node_id, char * buffer,
                                          size_t buffer_size)
{
    flow_snprintf(buffer, buffer_size, "* flow_bitmap_bgra");
    return true;
}

static bool dimensions_bitmap_bgra_pointer(flow_c * c, struct flow_graph * g, int32_t node_id, bool force_estimate)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_bitmap_bgra_pointer, info)
    struct flow_node * n = &g->nodes[node_id];

    if (*info->ref == NULL) {
        // Could be acting as a pass-through node. If bitmap is null, require an input be present
        FLOW_GET_INPUT_NODE(g, node_id)

        n->result_height = input_node->result_height;
        n->result_width = input_node->result_width;
        n->result_alpha_meaningful = input_node->result_alpha_meaningful;
        n->result_format = input_node->result_format;
    } else {
        struct flow_bitmap_bgra * b = *info->ref;
        n->result_width = b->w;
        n->result_height = b->h;
        n->result_alpha_meaningful = b->alpha_meaningful;
        n->result_format = b->fmt;
    }
    return true;
}

static bool execute_bitmap_bgra_pointer(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id)
{
    FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_bitmap_bgra_pointer, info)
    struct flow_node * n = &g->nodes[node_id];

    int count = flow_graph_get_inbound_edge_count_of_type(c, g, node_id, flow_edgetype_input);
    if (count == 1) {
        FLOW_GET_INPUT_EDGE(g, node_id)
        *info->ref = n->result_bitmap = g->nodes[input_edge->from].result_bitmap;
    } else {
        n->result_bitmap = *info->ref;
        if (*info->ref == NULL) {
            FLOW_error(c, flow_status_Invalid_inputs_to_node);
            return false;
        }
    }
    return true;
}

const struct flow_node_definition flow_define_decoder = {
    .type = flow_ntype_decoder,
    .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_codec),
    .type_name = "decode",
    .input_count = 0,
    .canvas_count = 0, //?
    .stringify = stringify_decode,
    .populate_dimensions = dimensions_decode,
    .pre_optimize_flatten = flatten_decode,
};
const struct flow_node_definition flow_define_primitive_decoder = {
    .type = flow_ntype_primitive_decoder,
    .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_codec),
    .type_name = "decode",
    .input_count = 0,
    .canvas_count = 0, //?
    .stringify = stringify_decode,
    .populate_dimensions = dimensions_decode,
    .execute = execute_decode,

};
const struct flow_node_definition flow_define_primitive_encoder = {
    .type = flow_ntype_primitive_encoder,
    .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_codec),
    .type_name = "encode",
    .input_count = 1,
    .canvas_count = 0, //?
    .stringify = stringify_encode,
    .execute = execute_encode,
    .prohibit_output_edges = true,
};

const struct flow_node_definition flow_define_encoder = {
    .type = flow_ntype_encoder,
    .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_codec),
    .type_name = "encode",
    .input_count = 1,
    .canvas_count = 0, //?
    .stringify = stringify_encode,
    .pre_optimize_flatten = flatten_encode,
    .prohibit_output_edges = true,

};

const struct flow_node_definition flow_define_bitmap_bgra_pointer
    = { .type = flow_ntype_primitive_bitmap_bgra_pointer,
        .nodeinfo_bytes_fixed = sizeof(struct flow_nodeinfo_bitmap_bgra_pointer),
        .type_name = "flow_bitmap_bgra ptr",
        .input_count = -1,
        .canvas_count = 0,
        .stringify = stringify_bitmap_bgra_pointer,
        .execute = execute_bitmap_bgra_pointer,
        .populate_dimensions = dimensions_bitmap_bgra_pointer

    };
