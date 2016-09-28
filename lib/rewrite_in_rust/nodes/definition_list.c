#include "lib/imageflow_private.h"
#include "definition_helpers.h"

extern const struct flow_node_definition flow_define_scale;
extern const struct flow_node_definition flow_define_create_canvas;
extern const struct flow_node_definition flow_define_decoder;
extern const struct flow_node_definition flow_define_primitive_decoder;
extern const struct flow_node_definition flow_define_primitive_encoder;
extern const struct flow_node_definition flow_define_encoder;
extern const struct flow_node_definition flow_define_apply_orientation;
extern const struct flow_node_definition flow_define_rotate_90;
extern const struct flow_node_definition flow_define_rotate_180;
extern const struct flow_node_definition flow_define_rotate_270;
extern const struct flow_node_definition flow_define_flip_v;
extern const struct flow_node_definition flow_define_flip_h;
extern const struct flow_node_definition flow_define_flip_v_primitive;
extern const struct flow_node_definition flow_define_flip_h_primitive;
extern const struct flow_node_definition flow_define_transpose;
extern const struct flow_node_definition flow_define_clone;
extern const struct flow_node_definition flow_define_crop;
extern const struct flow_node_definition flow_define_expand_canvas;
extern const struct flow_node_definition flow_define_crop_mutate;
extern const struct flow_node_definition flow_define_fill_rect;
extern const struct flow_node_definition flow_define_copy_rect;
extern const struct flow_node_definition flow_define_bitmap_bgra_pointer;
extern const struct flow_node_definition flow_define_render_to_canvas_1d;
extern const struct flow_node_definition flow_define_render_to_canvas_1d_primitive;
extern const struct flow_node_definition flow_define_scale2d_render_to_canvas1d;

const struct flow_node_definition flow_define_noop = {
    .type = flow_ntype_Noop,
    .input_count = 1,
    .canvas_count = 0,
    .type_name = "no-op",
    .nodeinfo_bytes_fixed = 0,
    .populate_dimensions = dimensions_mimic_input,
    .pre_optimize_flatten_complex = flatten_delete_node,
};
const struct flow_node_definition flow_define_null = {
    .type = flow_ntype_Null, .type_name = "(null)", .input_count = 0, .canvas_count = 0, .prohibit_output_edges = true,
};

struct flow_context_node_set * flow_context_get_default_node_set()
{
    static struct flow_context_node_set cached_default_node_set;
    static struct flow_node_definition cached_default_set[27];

    int i = 0;
    cached_default_set[i++] = flow_define_null;
    cached_default_set[i++] = flow_define_noop;
    cached_default_set[i++] = flow_define_scale;
    cached_default_set[i++] = flow_define_create_canvas;
    cached_default_set[i++] = flow_define_decoder;
    cached_default_set[i++] = flow_define_primitive_decoder;
    cached_default_set[i++] = flow_define_primitive_encoder;
    cached_default_set[i++] = flow_define_encoder;
    cached_default_set[i++] = flow_define_apply_orientation;
    cached_default_set[i++] = flow_define_rotate_90;
    cached_default_set[i++] = flow_define_rotate_180;
    cached_default_set[i++] = flow_define_rotate_270;
    cached_default_set[i++] = flow_define_flip_v;
    cached_default_set[i++] = flow_define_flip_h;
    cached_default_set[i++] = flow_define_flip_v_primitive;
    cached_default_set[i++] = flow_define_flip_h_primitive;
    cached_default_set[i++] = flow_define_transpose;
    cached_default_set[i++] = flow_define_clone;
    cached_default_set[i++] = flow_define_crop;
    cached_default_set[i++] = flow_define_expand_canvas;
    cached_default_set[i++] = flow_define_crop_mutate;
    cached_default_set[i++] = flow_define_fill_rect;
    cached_default_set[i++] = flow_define_copy_rect;
    cached_default_set[i++] = flow_define_bitmap_bgra_pointer;
    cached_default_set[i++] = flow_define_render_to_canvas_1d;
    cached_default_set[i++] = flow_define_render_to_canvas_1d_primitive;
    cached_default_set[i++] = flow_define_scale2d_render_to_canvas1d;

    cached_default_node_set.node_definitions = &cached_default_set[0];
    cached_default_node_set.node_definitions_count = sizeof(cached_default_set) / sizeof(struct flow_node_definition);
    return &cached_default_node_set;
}
