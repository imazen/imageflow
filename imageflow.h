#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <stdio.h>

// Version selection is not implemented within imageflow, instead, we let callers do that logic:
// Expose API to evaluate graph and suggest minimum source dimensions.
// Returns "indeterminate" if face or whitespace cropping is in use, or any other conditionals.

// Source/output files and I/O interfaces must be registered with the context. They can survive multiple ImageJobs.

// ImageJobs may eventually associate an opaque cache for dimensions, metadata, and (potentially) bitmap data with these
// I/O
// interfaces.

// * We heavily experimented with LUV and XYZ color spaces, but determined that better results occur using RGB linear.
// * A custom sigmoidized color space could perhaps improve things, but would introduce significant overhead.

typedef enum flow_ntype {
    flow_ntype_Null = 0,
    flow_ntype_primitive_Flip_Vertical_Mutate = 1,
    flow_ntype_primitive_Flip_Horizontal_Mutate = 1,
    flow_ntype_primitive_Crop_Mutate_Alias = 2,
    flow_ntype_primitive_CopyRectToCanvas = 3, // Overwrite only, no compositing
    flow_ntype_Create_Canvas = 4,
    flow_ntype_primitive_RenderToCanvas1D = 5,

    flow_ntype_primitive_bitmap_bgra_pointer,
    flow_ntype_primitive_decoder,
    flow_ntype_primitive_encoder,

    flow_ntype_non_primitive_nodes_begin = 256,

    flow_ntype_Transpose,
    flow_ntype_Flip_Vertical,
    flow_ntype_Flip_Horizontal,
    flow_ntype_Render1D,
    flow_ntype_Crop,
    flow_ntype_non_optimizable_nodes_begin = 512,

    flow_ntype_Clone,
    flow_ntype_decoder,
    flow_ntype_encoder,

    flow_ntype_Rotate_90,
    flow_ntype_Rotate_180,
    flow_ntype_Rotate_270,
    flow_ntype_Scale, //(preserve colorspace), interpolation filter

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
    flow_ntype_Resource_Placeholder,
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

typedef enum flow_edgetype {
    flow_edgetype_null,
    flow_edgetype_input,
    flow_edgetype_canvas,
    flow_edgetype_info,
    flow_edgetype_FORCE_ENUM_SIZE_INT32 = 2147483647
} flow_edgetype;

typedef enum flow_compositing_mode {
    flow_compositing_mode_overwrite,
    flow_compositing_mode_compose,
    flow_compositing_mode_blend_with_matte
} flow_compositing_mode;

struct flow_job;

typedef enum flow_job_resource_type {
    flow_job_resource_type_bitmap_bgra = 1,
    flow_job_resource_type_buffer = 2

} flow_job_resource_type;

typedef enum flow_job_codec_type {
    flow_job_codec_type_null,
    flow_job_codec_type_bitmap_bgra_pointer,
    flow_job_codec_type_decode_png,
    flow_job_codec_type_encode_png
} flow_job_codec_type;

typedef enum flow_scanlines_filter_type {
    flow_scanlines_filter_Sharpen, // 3x3, percentage-based
    flow_scanlines_filter_Blur, // 3x box blur to simulate guassian
    flow_scanlines_filter_Convolve, // Apply convolution kernel
    flow_scanlines_filter_ColorMatrix, // Apply color matrix
    flow_scanlines_filter_ToLinear,
    flow_scanlines_filter_ToSrgb,
    flow_scanlines_filter_Custom, // Execute custom callback.,
    flow_scanlines_filter__FORCE_ENUM_SIZE_INT32 = 2147483647
} flow_scanlines_filter_type;

typedef enum flow_status_code {
    flow_status_No_Error = 0,
    flow_status_Out_of_memory = 1,
    flow_status_Invalid_BitmapBgra_dimensions,
    flow_status_Invalid_BitmapFloat_dimensions,
    flow_status_Unsupported_pixel_format,
    flow_status_Invalid_internal_state,
    flow_status_Transpose_not_permitted_in_place,
    flow_status_Invalid_interpolation_filter,
    flow_status_Invalid_argument,
    flow_status_Null_argument,
    flow_status_Interpolation_details_missing,
    flow_status_Node_already_deleted,
    flow_status_Edge_already_deleted,
    flow_status_Graph_could_not_be_completed,
    flow_status_Not_implemented,
    flow_status_Invalid_inputs_to_node,
    flow_status_Graph_not_flattened,
    flow_status_Failed_to_open_file,
    flow_status_Graph_could_not_be_executed,
    flow_status_Png_decoding_failed,
    flow_status_Png_encoding_failed,
    flow_status_Graph_is_cyclic,
} flow_status_code;

typedef enum flow_interpolation_filter {
    flow_interpolation_filter_RobidouxFast = 1,
    flow_interpolation_filter_Robidoux = 2,
    flow_interpolation_filter_RobidouxSharp = 3,
    flow_interpolation_filter_Ginseng,
    flow_interpolation_filter_GinsengSharp,
    flow_interpolation_filter_Lanczos,
    flow_interpolation_filter_LanczosSharp,
    flow_interpolation_filter_Lanczos2,
    flow_interpolation_filter_Lanczos2Sharp,
    flow_interpolation_filter_CubicFast,
    flow_interpolation_filter_Cubic,
    flow_interpolation_filter_CubicSharp,
    flow_interpolation_filter_CatmullRom,
    flow_interpolation_filter_Mitchell,

    flow_interpolation_filter_CubicBSpline,
    flow_interpolation_filter_Hermite,
    flow_interpolation_filter_Jinc,
    flow_interpolation_filter_RawLanczos3,
    flow_interpolation_filter_RawLanczos3Sharp,
    flow_interpolation_filter_RawLanczos2,
    flow_interpolation_filter_RawLanczos2Sharp,
    flow_interpolation_filter_Triangle,
    flow_interpolation_filter_Linear,
    flow_interpolation_filter_Box,
    flow_interpolation_filter_CatmullRomFast,
    flow_interpolation_filter_CatmullRomFastSharp,

    flow_interpolation_filter_Fastest,

    flow_interpolation_filter_MitchellFast
} flow_interpolation_filter;

typedef enum flow_profiling_entry_flags {
    flow_profiling_entry_start = 2,
    flow_profiling_entry_start_allow_recursion = 6,
    flow_profiling_entry_stop = 8,
    flow_profiling_entry_stop_assert_started = 24,
    flow_profiling_entry_stop_children = 56
} flow_profiling_entry_flags;

typedef enum flow_pixel_format { flow_bgr24 = 3, flow_bgra32 = 4, flow_gray8 = 1 } flow_pixel_format;

typedef enum flow_bitmap_compositing_mode {
    flow_bitmap_compositing_replace_self = 0,
    flow_bitmap_compositing_blend_with_self = 1,
    flow_bitmap_compositing_blend_with_matte = 2
} flow_bitmap_compositing_mode;

typedef enum flow_working_floatspace {
    flow_working_floatspace_srgb = 0,
    flow_working_floatspace_as_is = 0,
    flow_working_floatspace_linear = 1,
    flow_working_floatspace_gamma = 2
} flow_working_floatspace;

typedef struct flow_context_struct flow_context;

/** flow_context: flow_profiling_log **/

typedef struct {
    int64_t time;
    const char* name;
    flow_profiling_entry_flags flags;
} flow_profiling_entry;

typedef struct {
    flow_profiling_entry* log;
    uint32_t count;
    uint32_t capacity;
    int64_t ticks_per_second;
} flow_profiling_log;

flow_profiling_log* flow_context_get_profiler_log(flow_context* context);

flow_context* flow_context_create(void);
void flow_context_destroy(flow_context* context);
void flow_context_free_all_allocations(flow_context* context);
void flow_context_print_memory_info(flow_context* context);

const char* flow_context_error_message(flow_context* context, char* buffer, size_t buffer_size);

const char* flow_context_stacktrace(flow_context* context, char* buffer, size_t buffer_size);

bool flow_context_has_error(flow_context* context);
int flow_context_error_reason(flow_context* context);

void flow_context_free_static_caches(void);

bool flow_context_print_and_exit_if_err(flow_context* c);

void flow_context_clear_error(flow_context* context);

void flow_context_print_error_to(flow_context* c, FILE* stream);

// non-indexed bitmap
typedef struct flow_bitmap_bgra_struct {

    // bitmap width in pixels
    uint32_t w;
    // bitmap height in pixels
    uint32_t h;
    // byte length of each row (may include any amount of padding)
    uint32_t stride;
    // pointer to pixel 0,0; should be of length > h * stride
    unsigned char* pixels;
    // If true, we don't dispose of *pixels when we dispose the struct
    bool borrowed_pixels;
    // If false, we can even ignore the alpha channel on 4bpp
    bool alpha_meaningful;
    // If false, we can edit pixels without affecting the stride
    bool pixels_readonly;
    // If false, we can change the stride of the image.
    bool stride_readonly;

    // If true, we can reuse the allocated memory for other purposes.
    bool can_reuse_space;

    flow_pixel_format fmt;

    // When using compositing mode blend_with_matte, this color will be used. We should probably define this as always
    // being sRGBA, 4 bytes.
    uint8_t matte_color[4];

    flow_bitmap_compositing_mode compositing_mode;

} flow_bitmap_bgra;

float flow_context_byte_to_floatspace(flow_context* c, uint8_t srgb_value);
uint8_t flow_context_floatspace_to_byte(flow_context* c, float space_value);

void flow_context_set_floatspace(flow_context* context, flow_working_floatspace space, float a, float b, float c);

typedef struct flow_RendererStruct flow_Renderer;

struct flow_interpolation_details_struct;
typedef double (*flow_detailed_interpolation_method)(const struct flow_interpolation_details_struct*, double);

typedef struct flow_interpolation_details_struct {
    // 1 is the default; near-zero overlapping between windows. 2 overlaps 50% on each side.
    double window;
    // Coefficients for bucubic weighting
    double p1, p2, p3, q1, q2, q3, q4;
    // Blurring factor when > 1, sharpening factor when < 1. Applied to weights.
    double blur;

    // pointer to the weight calculation function
    flow_detailed_interpolation_method filter;
    // How much sharpening we are requesting
    float sharpen_percent_goal;

} flow_interpolation_details;

typedef struct flow_convolution_kernel {
    float* kernel;
    uint32_t width;
    uint32_t radius;
    float threshold_min_change; // These change values are on a somewhat arbitrary scale between 0 and 4;
    float threshold_max_change;
    float* buffer;
} flow_convolution_kernel;

typedef struct flow_RenderDetailsStruct {
    // Interpolation and scaling details
    flow_interpolation_details* interpolation;
    // How large the interoplation window needs to be before we even attempt to apply a sharpening
    // percentage to the given filter
    float minimum_sample_window_to_interposharpen;

    // If possible to do correctly, halve the image until it is [interpolate_last_percent] times larger than needed. 3
    // or greater reccomended. Specify -1 to disable halving.
    float interpolate_last_percent;

    // The number of pixels (in target canvas coordinates) that it is acceptable to discard for better halving
    // performance
    float havling_acceptable_pixel_loss;

    // The actual halving factor to use.
    uint32_t halving_divisor;

    // The first convolution to apply
    flow_convolution_kernel* kernel_a;
    // A second convolution to apply
    flow_convolution_kernel* kernel_b;

    // If greater than 0, a percentage to sharpen the result along each axis;
    float sharpen_percent_goal;

    // If true, we should apply the color matrix
    bool apply_color_matrix;

    float color_matrix_data[25];
    float* color_matrix[5];

    // Transpose, flipx, flipy - combined, these give you all 90 interval rotations
    bool post_transpose;
    bool post_flip_x;
    bool post_flip_y;

    // Enables profiling
    bool enable_profiling;

} flow_RenderDetails;

flow_bitmap_bgra* flow_bitmap_bgra_create(flow_context* context, int sx, int sy, bool zeroed, flow_pixel_format format);
flow_bitmap_bgra* flow_bitmap_bgra_create_header(flow_context* context, int sx, int sy);
void flow_bitmap_bgra_destroy(flow_context* context, flow_bitmap_bgra* im);
bool flow_bitmap_bgra_flip_horizontal(flow_context* context, flow_bitmap_bgra* b);
bool flow_bitmap_bgra_compare(flow_context* c, flow_bitmap_bgra* a, flow_bitmap_bgra* b, bool* equal_out);

flow_RenderDetails* flow_RenderDetails_create(flow_context* context);
flow_RenderDetails* flow_RenderDetails_create_with(flow_context* context, flow_interpolation_filter filter);

bool flow_RenderDetails_render(flow_context* context, flow_RenderDetails* details, flow_bitmap_bgra* source,
                               flow_bitmap_bgra* canvas);
bool flow_RenderDetails_render_in_place(flow_context* context, flow_RenderDetails* details,
                                        flow_bitmap_bgra* edit_in_place);
void flow_RenderDetails_destroy(flow_context* context, flow_RenderDetails* d);

bool flow_interpolation_filter_exists(flow_interpolation_filter filter);
flow_interpolation_details* flow_interpolation_details_create(flow_context* context);
flow_interpolation_details* flow_interpolation_details_create_bicubic_custom(flow_context* context, double window,
                                                                             double blur, double B, double C);
flow_interpolation_details* flow_interpolation_details_create_custom(flow_context* context, double window, double blur,
                                                                     flow_detailed_interpolation_method filter);
flow_interpolation_details* flow_interpolation_details_create_from(flow_context* context,
                                                                   flow_interpolation_filter filter);
double flow_interpolation_details_percent_negative_weight(const flow_interpolation_details* details);
void flow_interpolation_details_destroy(flow_context* context, flow_interpolation_details*);

uint32_t flow_pixel_format_bytes_per_pixel(flow_pixel_format format);

typedef struct {
    float* Weights; /* Normalized weights of neighboring pixels */
    int Left; /* Bounds of source pixels window */
    int Right;
} flow_interpolation_pixel_contributions; /* Contirbution information for a single pixel */

typedef struct {
    flow_interpolation_pixel_contributions* ContribRow; /* Row (or column) of contribution weights */
    uint32_t WindowSize; /* Filter window size (of affecting source pixels) */
    uint32_t LineLength; /* Length of line (no. or rows / cols) */
    double percent_negative; /* Estimates the sharpening effect actually applied*/
} flow_interpolation_line_contributions;

flow_interpolation_line_contributions*
flow_interpolation_line_contributions_create(flow_context* context, const uint32_t output_line_size,
                                             const uint32_t input_line_size, const flow_interpolation_details* details);
void flow_interpolation_line_contributions_destroy(flow_context* context, flow_interpolation_line_contributions* p);

flow_convolution_kernel* flow_convolution_kernel_create(flow_context* context, uint32_t radius);
void flow_convolution_kernel_destroy(flow_context* context, flow_convolution_kernel* kernel);

flow_convolution_kernel* flow_convolution_kernel_create_guassian(flow_context* context, double stdDev, uint32_t radius);
// The only error these 2 could generate would be a null pointer. Should they have a context just for this?
double flow_convolution_kernel_sum(flow_convolution_kernel* kernel);
void flow_convolution_kernel_normalize(flow_convolution_kernel* kernel, float desiredSum);
flow_convolution_kernel* flow_convolution_kernel_create_guassian_normalized(flow_context* context, double stdDev,
                                                                            uint32_t radius);
flow_convolution_kernel* flow_convolution_kernel_create_guassian_sharpen(flow_context* context, double stdDev,
                                                                         uint32_t radius);

bool flow_bitmap_bgra_populate_histogram(flow_context* context, flow_bitmap_bgra* bmp, uint64_t* histograms,
                                         uint32_t histogram_size_per_channel, uint32_t histogram_count,
                                         uint64_t* pixels_sampled);

struct flow_scanlines_filter;

struct flow_scanlines_filter {
    flow_scanlines_filter_type type;
    struct flow_scanlines_filter* next;
};
//
// struct flow_frame_info{
//    int32_t w;
//    int32_t h;
//    flow_pixel_format fmt;
//    bool alpha_meaningful;
//};

struct flow_edge {
    flow_edgetype type;
    int32_t from;
    int32_t to;
    int32_t from_width;
    int32_t from_height;
    flow_pixel_format from_format;
    bool from_alpha_meaningful;
    int32_t info_byte_index;
    int32_t info_bytes;
};

struct flow_node {
    flow_ntype type;
    int32_t info_byte_index;
    int32_t info_bytes;
    flow_node_state state;
    flow_bitmap_bgra* result_bitmap;
    uint32_t ticks_elapsed;
};

struct flow_graph {
    uint32_t memory_layout_version; // This progresses differently from the library version, as internals are subject to
    // refactoring. If we are given a graph to copy, we check this number.
    struct flow_edge* edges;
    int32_t edge_count;
    int32_t next_edge_id;
    int32_t max_edges;

    struct flow_node* nodes;
    int32_t node_count;
    int32_t next_node_id;
    int32_t max_nodes;

    uint8_t* info_bytes;
    int32_t max_info_bytes;
    int32_t next_info_byte;
    int32_t deleted_bytes;

    float growth_factor;
};

struct flow_graph* flow_graph_create(flow_context* c, uint32_t max_edges, uint32_t max_nodes, uint32_t max_info_bytes,
                                     float growth_factor);

void flow_graph_destroy(flow_context* c, struct flow_graph* target);

bool flow_graph_replace_if_too_small(flow_context* c, struct flow_graph** g, uint32_t free_nodes_required,
                                     uint32_t free_edges_required, uint32_t free_bytes_required);
struct flow_graph* flow_graph_copy_and_resize(flow_context* c, struct flow_graph* from, uint32_t max_edges,
                                              uint32_t max_nodes, uint32_t max_info_bytes);

int32_t flow_graph_copy_info_bytes_to(flow_context* c, struct flow_graph* from, struct flow_graph** to,
                                      int32_t byte_index, int32_t byte_count);

int32_t flow_edge_duplicate(flow_context* c, struct flow_graph** g, int32_t edge_id);

/*
 * flow_Graph
 * flow_Node
 * flow_Edge
 * flow_edgetype
 * flow_ntype
 *
 * flow_node_create_canvas
 * flow_
 */

int32_t flow_node_create_canvas(flow_context* c, struct flow_graph** g, int32_t prev_node, flow_pixel_format format,
                                size_t width, size_t height, uint32_t bgcolor);
int32_t flow_node_create_scale(flow_context* c, struct flow_graph** g, int32_t prev_node, size_t width, size_t height);

int32_t flow_node_create_primitive_flip_vertical(flow_context* c, struct flow_graph** g, int32_t prev_node);
int32_t flow_node_create_primitive_flip_horizontal(flow_context* c, struct flow_graph** g, int32_t prev_node);
int32_t flow_node_create_clone(flow_context* c, struct flow_graph** g, int32_t prev_node);

int32_t flow_node_create_transpose(flow_context* c, struct flow_graph** g, int32_t prev_node);

int32_t flow_node_create_rotate_90(flow_context* c, struct flow_graph** g, int32_t prev_node);

int32_t flow_node_create_rotate_180(flow_context* c, struct flow_graph** g, int32_t prev_node);

int32_t flow_node_create_rotate_270(flow_context* c, struct flow_graph** g, int32_t prev_node);

int32_t flow_node_create_resource_placeholder(flow_context* c, struct flow_graph** g, int32_t prev_node,
                                              int32_t output_slot_id);

int32_t flow_node_create_resource_bitmap_bgra(flow_context* c, struct flow_graph** graph_ref, int32_t prev_node,
                                              flow_bitmap_bgra** ref);

int32_t flow_node_create_primitive_copy_rect_to_canvas(flow_context* c, struct flow_graph** g, int32_t prev_node,
                                                       uint32_t from_x, uint32_t from_y, uint32_t width,
                                                       uint32_t height, uint32_t x, uint32_t y);

int32_t flow_node_create_primitive_crop(flow_context* c, struct flow_graph** g, int32_t prev_node, uint32_t x1,
                                        uint32_t x2, uint32_t y1, uint32_t y2);

int32_t flow_node_create_render_to_canvas_1d(flow_context* c, struct flow_graph** g, int32_t prev_node,
                                             bool transpose_on_write, uint32_t canvas_x, uint32_t canvas_y,
                                             int32_t scale_to_width,
                                             flow_working_floatspace scale_and_filter_in_colorspace,
                                             float sharpen_percent, flow_compositing_mode compositing_mode,
                                             uint8_t* matte_color[4], struct flow_scanlines_filter* filter_list,
                                             flow_interpolation_filter interpolation_filter);

bool flow_node_delete(flow_context* c, struct flow_graph* g, int32_t node_id);

bool flow_edge_delete(flow_context* c, struct flow_graph* g, int32_t edge_id);

bool flow_edge_delete_all_connected_to_node(flow_context* c, struct flow_graph* g, int32_t node_id);

int32_t flow_graph_get_inbound_edge_count_of_type(flow_context* c, struct flow_graph* g, int32_t node_id,
                                                  flow_edgetype type);
int32_t flow_graph_get_first_inbound_edge_of_type(flow_context* c, struct flow_graph* g, int32_t node_id,
                                                  flow_edgetype type);

bool flow_edge_has_dimensions(flow_context* c, struct flow_graph* g, int32_t edge_id);
bool flow_node_input_edges_have_dimensions(flow_context* c, struct flow_graph* g, int32_t node_id);
bool flow_graph_duplicate_edges_to_another_node(flow_context* c, struct flow_graph** graph_ref, int32_t from_node,
                                                int32_t to_node, bool copy_inbound, bool copy_outbound);

int32_t flow_edge_create(flow_context* c, struct flow_graph** g, int32_t from, int32_t to, flow_edgetype type);

typedef bool (*flow_graph_visitor)(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref, int32_t id,
                                   bool* quit, bool* skip_outbound_paths, void* custom_data);

bool flow_graph_walk(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref,
                     flow_graph_visitor node_visitor, flow_graph_visitor edge_visitor, void* custom_data);

struct flow_nodeinfo_index {
    int32_t index;
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

struct flow_nodeinfo_size {
    size_t width;
    size_t height;
};

struct flow_nodeinfo_resource_bitmap_bgra {
    flow_bitmap_bgra** ref;
};

struct flow_nodeinfo_codec {
    void* codec_state;
    flow_job_codec_type type;
};

struct flow_nodeinfo_render_to_canvas_1d {
    // There will need to be consistency checks against the createcanvas node

    flow_interpolation_filter interpolation_filter;
    // flow_interpolation_details * interpolationDetails;
    int32_t scale_to_width;
    uint32_t canvas_x;
    uint32_t canvas_y;
    bool transpose_on_write;
    flow_working_floatspace scale_in_colorspace;

    float sharpen_percent_goal;

    flow_compositing_mode compositing_mode;
    // When using compositing mode blend_with_matte, this color will be used. We should probably define this as always
    // being sRGBA, 4 bytes.
    uint8_t matte_color[4];

    struct flow_scanlines_filter* filter_list;
};

bool flow_node_execute_render_to_canvas_1d(flow_context* c, struct flow_job* job, flow_bitmap_bgra* input,
                                           flow_bitmap_bgra* canvas, struct flow_nodeinfo_render_to_canvas_1d* info);

int32_t flow_node_create_render1d(flow_context* c, struct flow_graph** g, int32_t prev_node, bool transpose_on_write,
                                  int32_t scale_to_width, flow_working_floatspace scale_and_filter_in_colorspace,
                                  float sharpen_percent, struct flow_scanlines_filter* filter_list,
                                  flow_interpolation_filter interpolation_filter);

typedef enum FLOW_DIRECTION { FLOW_OUTPUT = 8, FLOW_INPUT = 4 } FLOW_DIRECTION;

struct flow_job* flow_job_create(flow_context* c);
void flow_job_destroy(flow_context* c, struct flow_job* job);
bool flow_job_configure_recording(flow_context* c, struct flow_job* job, bool record_graph_versions,
                                  bool record_frame_images, bool render_last_graph, bool render_graph_versions,
                                  bool render_animated_graph);

bool flow_job_insert_resources_into_graph(flow_context* c, struct flow_job* job, struct flow_graph** graph);

bool flow_job_populate_dimensions_where_certain(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref);
// For doing execution cost estimates, we force estimate, then flatten, then calculate cost
bool flow_job_force_populate_dimensions(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref);
bool flow_job_execute_where_certain(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref);
bool flow_job_graph_fully_executed(flow_context* c, struct flow_job* job, struct flow_graph* g);

bool flow_job_notify_graph_changed(flow_context* c, struct flow_job* job, struct flow_graph* g);
bool flow_job_execute(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref);
bool flow_graph_post_optimize_flatten(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref);

bool flow_graph_optimize(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref);
bool flow_graph_pre_optimize_flatten(flow_context* c, struct flow_graph** graph_ref);
int32_t flow_graph_get_edge_count(flow_context* c, struct flow_graph* g, int32_t node_id, bool filter_by_edge_type,
                                  flow_edgetype type, bool include_inbound, bool include_outbound);

bool flow_graph_validate(flow_context* c, struct flow_graph* g);

int32_t flow_job_add_bitmap_bgra(flow_context* c, struct flow_job* job, FLOW_DIRECTION dir,
                                 int32_t graph_placeholder_id, flow_bitmap_bgra* bitmap);

int32_t flow_job_add_buffer(flow_context* c, struct flow_job* job, FLOW_DIRECTION dir, int32_t graph_placeholder_id,
                            void* buffer, size_t buffer_size, bool owned_by_job);

int32_t flow_node_create_generic(flow_context* c, struct flow_graph** graph_ref, int32_t prev_node, flow_ntype type);

bool flow_graph_print_to_dot(flow_context* c, struct flow_graph* g, FILE* stream,
                             const char* image_node_filename_prefix);

flow_bitmap_bgra* flow_job_get_bitmap_bgra(flow_context* c, struct flow_job* job, int32_t resource_id);
struct flow_job_resource_buffer* flow_job_get_buffer(flow_context* c, struct flow_job* job, int32_t resource_id);

void flow_graph_print_to(flow_context* c, struct flow_graph* g, FILE* stream);

struct flow_job_resource_buffer {
    void* buffer;
    size_t buffer_size;
    bool owned_by_job;
    void* codec_state;
};

struct flow_job_png_encoder_state {
    flow_context* context;
    char* buffer;
    size_t size;
    struct flow_job_resource_buffer* output_resource;
};

bool flow_bitmap_bgra_write_png(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra* frame);

bool flow_node_post_optimize_flatten(flow_context* c, struct flow_graph** graph_ref, int32_t node_id);

// Multi-frame/multi-page images are not magically handled.
// We require one frame graph per frame/page to be created by the client after metadata is parsed for that frame/page.

/*
 * output format:
 *
 *
 * png -> png
 * png -> jpeg
 * jpeg -> png
 * gif -> png
 * agif -> agif
 *
 *
 *
 *
 */

// Imageflow makes multiple passes over each graph
// 1. Parse headers on every source node. Resolve conditionals based on that data.
// 2. ...

// n-1: Collapse frame graph into primitives

// Source node - can be asked for different I/O interfaces. May always upgrade instead. Header should be implemented for
// safety (avoiding expensive loads)
// header, length -> random access -> buffer

// Output node - callback
// Output node - metadata

// File output nodes
// Output node - buffer
// Output node - random read/write

// MetadataCache

// SelectFrame
// PerFrameFlow - contains subgraph, which has an FrameOutput endpoint.

// Pick frame
// decoding
// encoding
// quantization

//
//| VFlip | Format agnostic | In Place
//| Crop  | Format agnostic | In Place
//| CopyRect  | Format agnostic | New Frame
//| CreateCanvas |
//| RenderToCanvas1D (scale (flow_interpolation_details), compose (InPlace, Copy, Blende, Matte[color]), bool transpose,
//[list
// of convolution & pixel filters], working_floatspace)
//
//

#ifdef __cplusplus
}
#endif
