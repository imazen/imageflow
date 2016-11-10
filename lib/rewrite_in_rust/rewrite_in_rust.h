#pragma once
#define PUB FLOW_EXPORT

#ifdef __cplusplus
extern "C" {
#endif

struct flow_node;
struct flow_edge;
struct flow_graph;

typedef enum flow_edgetype {
    flow_edgetype_null = 0,
    flow_edgetype_input = 1,
    flow_edgetype_canvas = 2,
    flow_edgetype_info = 3,
    flow_edgetype_FORCE_ENUM_SIZE_INT32 = 2147483647
} flow_edgetype;

typedef enum flow_ntype {
    flow_ntype_Null = 0,
    flow_ntype_primitive_Flip_Vertical_Mutate = 1,
    flow_ntype_primitive_Flip_Horizontal_Mutate = 2,
    flow_ntype_primitive_Crop_Mutate_Alias = 3,
    flow_ntype_primitive_CopyRectToCanvas = 4, // Overwrite only, no compositing
    flow_ntype_Create_Canvas = 5,
    flow_ntype_primitive_RenderToCanvas1D = 6,
    flow_ntype_primitive_Scale2D_RenderToCanvas1D = 7,
    flow_ntype_primitive_bitmap_bgra_pointer,
    flow_ntype_primitive_decoder,
    flow_ntype_primitive_encoder,

    flow_ntype_Fill_Rect_Mutate,
    flow_ntype_non_primitive_nodes_begin = 256,

    flow_ntype_Expand_Canvas,
    flow_ntype_Transpose,
    flow_ntype_Flip_Vertical,
    flow_ntype_Flip_Horizontal,
    flow_ntype_Render1D,
    flow_ntype_Crop,
    flow_ntype_Apply_Orientation,
    flow_ntype_non_optimizable_nodes_begin = 512,

    flow_ntype_Clone,
    flow_ntype_decoder,
    flow_ntype_encoder,

    flow_ntype_Rotate_90,
    flow_ntype_Rotate_180,
    flow_ntype_Rotate_270,
    flow_ntype_Scale, //(preserve colorspace), interpolation filter
    flow_ntype_Noop,

    // Not implemented below here:
    flow_ntype_Rotate_Flip_Per_Orientation,
    flow_ntype_Crop_Percentage,
    flow_ntype_Crop_Percentage_Infinite_Canvas, // canvas_color
    flow_ntype_Crop_Rectangle,
    flow_ntype_Constrain, //(mode=pad|max|crop|stretch) (width, height) (scale=down|up|both|canvas) (anchor=9 points)
    flow_ntype_Matte,
    flow_ntype_EnlargeCanvas,
    flow_ntype_Sharpen,
    flow_ntype_Blur,
    flow_ntype_Convolve_Custom,
    flow_ntype_AdjustContrast,
    flow_ntype_AdjustSaturation,
    flow_ntype_AdjustBrightness,
    flow_ntype_CropWhitespace, // tolerances and padding
    flow_ntype_Opacity,
    flow_ntype_Sepia,
    flow_ntype_Grayscale, // true|y|ry|ntsc|bt709|flat
    flow_ntype_DrawImage,
    flow_ntype_RemoveNoise,
    flow_ntype_ColorMatrixsRGB,
    flow_ntype__FORCE_ENUM_SIZE_INT32 = 2147483647
} flow_ntype;

typedef enum flow_node_state {
    flow_node_state_Blank = 0,
    flow_node_state_InputDimensionsKnown = 1,
    flow_node_state_ReadyForPreOptimizeFlatten = 1,
    flow_node_state_PreOptimizeFlattened = 2,
    flow_node_state_ReadyForOptimize = 3,
    flow_node_state_Optimized = 4,
    flow_node_state_ReadyForPostOptimizeFlatten = 7,
    flow_node_state_PostOptimizeFlattened = 8,
    flow_node_state_InputsExecuted = 16,
    flow_node_state_ReadyForExecution = 31,
    flow_node_state_Executed = 32,
    flow_node_state_Done = 63
} flow_node_state;

// RN: this is the entry point that you will replace in rust, with rust
PUB bool flow_job_execute(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref);

// This call will, of course, turn into {:?} (.fmt())
PUB bool flow_graph_print_to_stdout(flow_c * c, struct flow_graph * g);

// RN: nothing else matters, delete all apis  in this file and those marked in src/ffi.rs

PUB struct flow_graph * flow_graph_create(flow_c * c, uint32_t max_edges, uint32_t max_nodes, uint32_t max_info_bytes,
                                          float growth_factor);

PUB void flow_graph_destroy(flow_c * c, struct flow_graph * target);

PUB bool flow_graph_replace_if_too_small(flow_c * c, struct flow_graph ** g, uint32_t free_nodes_required,
                                         uint32_t free_edges_required, uint32_t free_bytes_required);
PUB struct flow_graph * flow_graph_copy_and_resize(flow_c * c, struct flow_graph * from, uint32_t max_edges,
                                                   uint32_t max_nodes, uint32_t max_info_bytes);

PUB struct flow_graph * flow_graph_copy(flow_c * c, struct flow_graph * from);

PUB int32_t flow_node_create_decoder(flow_c * c, struct flow_graph ** g, int32_t prev_node, int32_t placeholder_id);

PUB int32_t flow_node_create_canvas(flow_c * c, struct flow_graph ** g, int32_t prev_node, flow_pixel_format format,
                                    size_t width, size_t height, uint32_t bgcolor);
PUB int32_t flow_node_create_scale(flow_c * c, struct flow_graph ** g, int32_t prev_node, size_t width, size_t height,
                                   flow_interpolation_filter downscale_filter, flow_interpolation_filter upscale_filter,
                                   size_t flags, float sharpen);

PUB int32_t flow_node_create_primitive_flip_vertical(flow_c * c, struct flow_graph ** g, int32_t prev_node);
PUB int32_t flow_node_create_primitive_flip_horizontal(flow_c * c, struct flow_graph ** g, int32_t prev_node);
PUB int32_t flow_node_create_clone(flow_c * c, struct flow_graph ** g, int32_t prev_node);
PUB int32_t flow_node_create_expand_canvas(flow_c * c, struct flow_graph ** g, int32_t prev_node, uint32_t left,
                                           uint32_t top, uint32_t right, uint32_t bottom, uint32_t canvas_color_srgb);
PUB int32_t flow_node_create_fill_rect(flow_c * c, struct flow_graph ** g, int32_t prev_node, uint32_t x1, uint32_t y1,
                                       uint32_t x2, uint32_t y2, uint32_t color_srgb);
PUB int32_t flow_node_create_transpose(flow_c * c, struct flow_graph ** g, int32_t prev_node);

PUB int32_t flow_node_create_apply_orientation(flow_c * c, struct flow_graph ** g, int32_t prev_node,
                                               int32_t exif_orientation_flag);

PUB int32_t flow_node_create_rotate_90(flow_c * c, struct flow_graph ** g, int32_t prev_node);

PUB int32_t flow_node_create_rotate_180(flow_c * c, struct flow_graph ** g, int32_t prev_node);

PUB int32_t flow_node_create_rotate_270(flow_c * c, struct flow_graph ** g, int32_t prev_node);

PUB int32_t
    flow_node_create_encoder_placeholder(flow_c * c, struct flow_graph ** g, int32_t prev_node, int32_t output_slot_id);

PUB int32_t flow_node_create_encoder(flow_c * c, struct flow_graph ** g, int32_t prev_node, int32_t placeholder_id,
                                     int64_t desired_encoder_id, struct flow_encoder_hints * hints);

PUB int32_t flow_node_create_noop(flow_c * c, struct flow_graph ** g, int32_t prev_node);

PUB int32_t flow_node_create_bitmap_bgra_reference(flow_c * c, struct flow_graph ** g, int32_t prev_node,
                                                   struct flow_bitmap_bgra ** pointer_to_pointer_to_bitmap_bgra);

PUB int32_t flow_node_create_primitive_copy_rect_to_canvas(flow_c * c, struct flow_graph ** g, int32_t prev_node,
                                                           uint32_t from_x, uint32_t from_y, uint32_t width,
                                                           uint32_t height, uint32_t x, uint32_t y);

PUB int32_t flow_node_create_primitive_crop(flow_c * c, struct flow_graph ** g, int32_t prev_node, uint32_t x1,
                                            uint32_t x2, uint32_t y1, uint32_t y2);

PUB int32_t flow_node_create_render_to_canvas_1d(flow_c * c, struct flow_graph ** g, int32_t prev_node,
                                                 bool transpose_on_write, uint32_t canvas_x, uint32_t canvas_y,
                                                 int32_t scale_to_width,
                                                 flow_working_floatspace scale_and_filter_in_colorspace,
                                                 float sharpen_percent, flow_compositing_mode compositing_mode,
                                                 uint8_t * matte_color[4], struct flow_scanlines_filter * filter_list,
                                                 flow_interpolation_filter interpolation_filter);

PUB int32_t flow_node_create_scale_2d(flow_c * c, struct flow_graph ** g, int32_t prev_node, int32_t scale_to_width,
                                      int32_t scale_to_height, flow_working_floatspace scale_and_filter_in_colorspace,
                                      float sharpen_percent, flow_interpolation_filter interpolation_filter);

PUB int32_t flow_edge_create(flow_c * c, struct flow_graph ** g, int32_t from, int32_t to, flow_edgetype type);

PUB int32_t flow_node_create_render1d(flow_c * c, struct flow_graph ** g, int32_t prev_node, bool transpose_on_write,
                                      int32_t scale_to_width, flow_working_floatspace scale_and_filter_in_colorspace,
                                      float sharpen_percent, struct flow_scanlines_filter * filter_list,
                                      flow_interpolation_filter interpolation_filter);
PUB int32_t flow_node_create_generic(flow_c * c, struct flow_graph ** graph_ref, int32_t prev_node, flow_ntype type);

PUB int32_t flow_node_create_generic_with_data(flow_c * c, struct flow_graph ** graph_ref, int32_t prev_node,
                                               flow_ntype type, uint8_t * bytes, size_t byte_count);

PUB bool flow_graph_validate(flow_c * c, struct flow_graph * g);
////////////////////////////////////////////
// from imageflow_advanced.h Deal with graphs

typedef bool (*flow_graph_visitor)(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref, int32_t id,
                                   bool * quit, bool * skip_outbound_paths, void * custom_data);

PUB bool flow_graph_walk(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref,
                         flow_graph_visitor node_visitor, flow_graph_visitor edge_visitor, void * custom_data);

PUB bool flow_node_delete(flow_c * c, struct flow_graph * g, int32_t node_id);

PUB bool flow_edge_delete(flow_c * c, struct flow_graph * g, int32_t edge_id);

PUB bool flow_edge_delete_all_connected_to_node(flow_c * c, struct flow_graph * g, int32_t node_id);
PUB bool flow_edge_delete_connected_to_node(flow_c * c, struct flow_graph * g, int32_t node_id, bool inbound,
                                            bool outbound);

PUB int32_t
    flow_graph_get_inbound_edge_count_of_type(flow_c * c, struct flow_graph * g, int32_t node_id, flow_edgetype type);
PUB int32_t
    flow_graph_get_first_inbound_edge_of_type(flow_c * c, struct flow_graph * g, int32_t node_id, flow_edgetype type);

PUB int32_t
    flow_graph_get_first_outbound_edge_of_type(flow_c * c, struct flow_graph * g, int32_t node_id, flow_edgetype type);

PUB int32_t
    flow_graph_get_first_inbound_node_of_type(flow_c * c, struct flow_graph * g, int32_t node_id, flow_edgetype type);

PUB int32_t
    flow_graph_get_first_outbound_node_of_type(flow_c * c, struct flow_graph * g, int32_t node_id, flow_edgetype type);

PUB bool flow_node_has_dimensions(flow_c * c, struct flow_graph * g, int32_t node_id);

PUB bool flow_node_inputs_have_dimensions(flow_c * c, struct flow_graph * g, int32_t node_id);
PUB bool flow_graph_duplicate_edges_to_another_node(flow_c * c, struct flow_graph ** graph_ref, int32_t from_node,
                                                    int32_t to_node, bool copy_inbound, bool copy_outbound);

PUB int32_t flow_graph_copy_info_bytes_to(flow_c * c, struct flow_graph * from, struct flow_graph ** to,
                                          int32_t byte_index, int32_t byte_count);

PUB int32_t flow_edge_duplicate(flow_c * c, struct flow_graph ** g, int32_t edge_id);

PUB bool flow_graph_print_to_dot(flow_c * c, struct flow_graph * g, FILE * stream,
                                 const char * image_node_filename_prefix);

PUB bool flow_job_populate_dimensions_where_certain(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref);
// For doing execution cost estimates, we force estimate, then flatten, then calculate cost
PUB bool flow_job_force_populate_dimensions(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref);
PUB bool flow_job_execute_where_certain(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref);
PUB bool flow_job_graph_fully_executed(flow_c * c, struct flow_job * job, struct flow_graph * g);

PUB bool flow_job_notify_graph_changed(flow_c * c, struct flow_job * job, struct flow_graph * g);

PUB bool flow_graph_post_optimize_flatten(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref);

PUB bool flow_graph_optimize(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref);
PUB bool flow_graph_pre_optimize_flatten(flow_c * c, struct flow_graph ** graph_ref);
PUB int32_t flow_graph_get_edge_count(flow_c * c, struct flow_graph * g, int32_t node_id, bool filter_by_edge_type,
                                      flow_edgetype type, bool include_inbound, bool include_outbound);

PUB bool flow_node_post_optimize_flatten(flow_c * c, struct flow_graph ** graph_ref, int32_t node_id);

PUB bool flow_graph_walk_dependency_wise(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref,
                                         flow_graph_visitor node_visitor, flow_graph_visitor edge_visitor,
                                         void * custom_data);

PUB bool flow_job_render_graph_to_png(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t graph_version);
PUB bool flow_job_notify_node_complete(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id);

PUB bool flow_job_link_codecs(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref);

typedef bool (*flow_nodedef_fn_stringify)(flow_c * c, struct flow_graph * g, int32_t node_id, char * buffer,
                                          size_t buffer_size);

typedef bool (*flow_nodedef_fn_infobyte_count)(flow_c * c, struct flow_graph * g, int32_t node_id,
                                               int32_t * infobytes_count_out);

typedef bool (*flow_nodedef_fn_populate_dimensions)(flow_c * c, struct flow_graph * g, int32_t node_id,
                                                    bool force_estimate);

typedef bool (*flow_nodedef_fn_flatten)(flow_c * c, struct flow_graph ** graph_ref, int32_t node_id);

typedef bool (*flow_nodedef_fn_flatten_shorthand)(flow_c * c, struct flow_graph ** graph_ref, int32_t node_id,
                                                  struct flow_node * node, struct flow_node * input_node,
                                                  int32_t * first_replacement_node, int32_t * last_replacement_node);

typedef bool (*flow_nodedef_fn_execute)(flow_c * c, struct flow_graph * g, int32_t node_id);

typedef bool (*flow_nodedef_fn_estimate_cost)(flow_c * c, struct flow_graph * g, int32_t node_id,
                                              size_t * bytes_required, size_t * cpu_cost);

struct flow_graph {
    uint32_t memory_layout_version; // This progresses differently from the library version, as internals are subject to
    // refactoring. If we are given a graph to copy, we check this number.
    struct flow_edge * edges;
    int32_t edge_count;
    int32_t next_edge_id;
    int32_t max_edges;

    struct flow_node * nodes;
    int32_t node_count;
    int32_t next_node_id;
    int32_t max_nodes;

    uint8_t * info_bytes;
    int32_t max_info_bytes;
    int32_t next_info_byte;
    int32_t deleted_bytes;

    float growth_factor;
};

struct flow_nodeinfo_index;
struct flow_nodeinfo_createcanvas;
struct flow_nodeinfo_crop;
struct flow_nodeinfo_copy_rect_to_canvas;
struct flow_nodeinfo_expand_canvas;
struct flow_nodeinfo_fill_rect;
struct flow_nodinfo_size;
struct flow_nodeinfo_bitmap_bgra_pointer;
struct flow_nodeinfo_codec;
struct flow_nodeinfo_render_to_canvas_1d;
struct flow_nodeinfo_scale2d_render_to_canvas1d;

struct flow_edge {
    flow_edgetype type;
    int32_t from;
    int32_t to;
    int32_t info_byte_index;
    int32_t info_bytes;
};

struct flow_node {
    flow_ntype type;
    int32_t info_byte_index;
    int32_t info_bytes;
    flow_node_state state;
    int32_t result_width;
    int32_t result_height;
    flow_pixel_format result_format;
    bool result_alpha_meaningful;
    struct flow_bitmap_bgra * result_bitmap;
    uint32_t ticks_elapsed;
};

struct flow_nodeinfo_index {
    int32_t index;
};

struct flow_nodeinfo_encoder_placeholder {
    struct flow_nodeinfo_index index; // MUST BE FIRST
    flow_codec_type codec_type;
};

struct flow_nodeinfo_createcanvas {
    flow_pixel_format format;
    size_t width;
    size_t height;
    uint32_t bgcolor;
};

struct flow_nodeinfo_crop {
    uint32_t x1;
    uint32_t x2;
    uint32_t y1;
    uint32_t y2;
};

struct flow_nodeinfo_copy_rect_to_canvas {
    uint32_t x;
    uint32_t y;
    uint32_t from_x;
    uint32_t from_y;
    uint32_t width;
    uint32_t height;
};
struct flow_nodeinfo_expand_canvas {
    uint32_t left;
    uint32_t top;
    uint32_t right;
    uint32_t bottom;
    uint32_t canvas_color_srgb;
};
struct flow_nodeinfo_fill_rect {
    uint32_t x1;
    uint32_t y1;
    uint32_t x2;
    uint32_t y2;
    uint32_t color_srgb;
};
struct flow_nodeinfo_size {
    int32_t width;
    int32_t height;
};
struct flow_nodeinfo_scale {
    int32_t width;
    int32_t height;
    flow_interpolation_filter downscale_filter;
    flow_interpolation_filter upscale_filter;
    size_t flags;
    float sharpen;
};
struct flow_nodeinfo_bitmap_bgra_pointer {
    struct flow_bitmap_bgra ** ref;
};

struct flow_nodeinfo_codec {
    int32_t placeholder_id;
    struct flow_codec_instance * codec;
    // For encoders
    int64_t desired_encoder_id;
    // For decdoers
    struct flow_decoder_downscale_hints downscale_hints;
    struct flow_encoder_hints encoder_hints;
};

struct flow_nodeinfo_apply_orientation {
    int32_t orientation; // Values 1-8, per the EXIF Orientation spec
};

struct flow_context_node_set {
    struct flow_node_definition * node_definitions;
    size_t node_definitions_count;
};

struct flow_context_node_set * flow_context_get_default_node_set(void);

struct flow_node_definition {
    flow_ntype type;
    int32_t input_count;
    bool prohibit_output_edges;
    int32_t canvas_count;
    const char * type_name;

    flow_nodedef_fn_stringify stringify;
    flow_nodedef_fn_infobyte_count count_infobytes;
    int32_t nodeinfo_bytes_fixed;
    flow_nodedef_fn_populate_dimensions populate_dimensions;
    flow_nodedef_fn_flatten pre_optimize_flatten_complex;
    flow_nodedef_fn_flatten_shorthand pre_optimize_flatten;
    flow_nodedef_fn_flatten post_optimize_flatten_complex;
    flow_nodedef_fn_flatten_shorthand post_optimize_flatten;
    flow_nodedef_fn_execute execute;
    flow_nodedef_fn_estimate_cost estimate_cost;
};

struct flow_node_definition * flow_nodedef_get(flow_c * c, flow_ntype type);

//!Throws an error and returns null if node_id does not represent a valid, non-null node
struct flow_node * flow_node_get(flow_c * c, struct flow_graph * g, int32_t node_id);

//!Throws an error if node_id does not represent a valid, non-null node, or if there are no infobytes, or the infobyte
// size does not match sizeof_infobytes_struct
void * flow_node_get_infobytes_pointer(flow_c * c, struct flow_graph * g, int32_t node_id,
                                       size_t sizeof_infobytes_struct);

bool flow_node_stringify(flow_c * c, struct flow_graph * g, int32_t node_id, char * buffer, size_t buffer_size);

int32_t flow_node_fixed_infobyte_count(flow_c * c, flow_ntype type);
bool flow_node_infobyte_count(flow_c * c, struct flow_graph * g, int32_t node_id, int32_t * infobytes_count_out);
bool flow_node_populate_dimensions(flow_c * c, struct flow_graph * g, int32_t node_id, bool force_estimate);
bool flow_node_pre_optimize_flatten(flow_c * c, struct flow_graph ** graph_ref, int32_t node_id);
bool flow_node_execute(flow_c * c, struct flow_graph * g, int32_t node_id);
bool flow_node_estimate_execution_cost(flow_c * c, struct flow_graph * g, int32_t node_id, size_t * bytes_required,
                                       size_t * cpu_cost);
bool flow_node_validate_edges(flow_c * c, struct flow_graph * g, int32_t node_id);
bool flow_node_update_state(flow_c * c, struct flow_graph * g, int32_t node_id);

#ifdef __cplusplus
}
#endif
