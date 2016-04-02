#pragma once
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <stdio.h>
#include <setjmp.h>

#ifdef __cplusplus
extern "C" {
#endif
#if defined(_WIN32)
#if defined(imageflow_EXPORTS)
/* Cmake will define imageflow_EXPORTS on Windows when it
configures to build a shared library.*/
#define FLOW_EXPORT __declspec(dllexport)
#else
#define FLOW_EXPORT __declspec(dllimport)
#endif /* imageflow_EXPORTS */
#else /* defined (_WIN32) */
#define FLOW_EXPORT
#endif

#define PUB FLOW_EXPORT

// Version selection is not implemented within imageflow, instead, we let callers do that logic:
// Expose API to evaluate graph and suggest minimum source dimensions.
// Returns "indeterminate" if face or whitespace cropping is in use, or any other conditionals.

// Source/output files and I/O interfaces must be registered with the context. They can survive multiple ImageJobs.

// ImageJobs may eventually associate an opaque cache for dimensions, metadata, and (potentially) bitmap data with these
// I/O
// interfaces.

// * We heavily experimented with LUV and XYZ color spaces, but determined that better results occur using RGB linear.
// * A custom sigmoidized color space could perhaps improve things, but would introduce significant overhead.

typedef enum FLOW_DIRECTION { FLOW_OUTPUT = 8, FLOW_INPUT = 4 } FLOW_DIRECTION;

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

    flow_ntype_Fill_Rect_Mutate,
    flow_ntype_non_primitive_nodes_begin = 256,

    flow_ntype_Expand_Canvas,
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

typedef enum flow_codec_type {
    flow_codec_type_null,
    flow_codec_type_decode_png,
    flow_codec_type_encode_png,
    flow_codec_type_decode_jpeg,
    flow_codec_type_encode_jpeg,
    flow_codec_type_decode_gif
} flow_codec_type;

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
    flow_status_Not_implemented,
    flow_status_Unsupported_pixel_format,
    flow_status_Null_argument,
    flow_status_Invalid_argument,
    flow_status_Invalid_dimensions,
    flow_status_Invalid_internal_state,
    flow_status_IO_error,
    flow_status_Image_decoding_failed,
    flow_status_Image_encoding_failed,
    flow_status_Item_does_not_exist,
    flow_status_Graph_invalid,
    flow_status_Invalid_inputs_to_node,
    flow_status_Maximum_graph_passes_exceeded,
    flow_status_Graph_is_cyclic,
    flow_status_Other_error,
    flow_status____Last_library_error,
    flow_status_First_user_defined_error = 1025,
    flow_status_Last_user_defined_error = 2147483647
} flow_status_code;

static const char * const flow_status_code_strings[] = {
    "No error",      "Out Of Memory",          "Not implemented",               "Pixel format unsupported by algorithm",
    "Null argument", "Invalid argument",       "Invalid dimensions",            "Internal state invalid",
    "I/O error",     "Image decoding failed",  "Image encoding failed",         "Item does not exist",
    "Graph invalid", "Invalid inputs to node", "Maximum graph passes exceeded", "Graph is cyclic",
    "Other error:",
};

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

// TODO: So many more - 8-bit, compressed data,
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

typedef enum flow_io_mode {
    flow_io_mode_null = 0,
    flow_io_mode_read_sequential = 1,
    flow_io_mode_write_sequential = 2,
    flow_io_mode_read_seekable = 5, // 1 | 4,
    flow_io_mode_write_seekable = 6, // 2 | 4,
    flow_io_mode_read_write_seekable = 15 // 1 | 2 | 4 | 8
} flow_io_mode;

typedef struct flow_context flow_c;
struct flow_codec_definition;
struct flow_codec_instance; // All methods should center around this
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
struct flow_scanlines_filter;
struct flow_decoder_downscale_hints;
struct flow_node;
struct flow_edge;
struct flow_graph;
struct flow_bitmap_bgra;

PUB flow_c * flow_context_create(void);

// When you need to control 100% of heap operations, you can allocate
// flow_context_sizeof_context() bytes and initialize them with flow_context_initialize,
// then call flow_heap_set_custom. Use flow_context_terminate and your matching free() function instead of
// flow_context_destroy
PUB size_t flow_context_sizeof_context_struct(void);
PUB void flow_context_initialize(flow_c * c);
// Want to ensure there were no memory leaks due to incorrect API use, and that all files flushed and close
// successfully?
// Call begin_terminate, then check on error status and memory stats.
PUB bool flow_context_begin_terminate(flow_c * c);
PUB void flow_context_end_terminate(flow_c * c);

PUB void flow_context_destroy(flow_c * c); // Don't pass this a pointer on the stack! use begin/end terminate

PUB int64_t flow_context_error_and_stacktrace(flow_c * c, char * buffer, size_t buffer_size, bool full_file_path);
PUB int64_t flow_context_error_message(flow_c * c, char * buffer, size_t buffer_size);

PUB int64_t flow_context_stacktrace(flow_c * c, char * buffer, size_t buffer_size, bool full_file_path);

PUB bool flow_context_has_error(flow_c * c);
PUB int flow_context_error_reason(flow_c * c);

PUB bool flow_context_print_and_exit_if_err(flow_c * c);

PUB void flow_context_clear_error(flow_c * c);

PUB void flow_context_print_error_to(flow_c * c, FILE * stream);

PUB void flow_context_print_memory_info(flow_c * c);

// Flush buffers; close files     ; release underlying resources - the job has been ended.
typedef bool (*flow_destructor_function)(flow_c * c, void * thing);

// Assuming, here, that we never get a pointer to address 42 in memory.
#define FLOW_OWNER_IMMORTAL ((void *)42)

PUB struct flow_io * flow_io_create_for_file(flow_c * c, flow_io_mode mode, const char * filename, void * owner);
PUB struct flow_io * flow_io_create_from_memory(flow_c * c, flow_io_mode mode, uint8_t * memory, size_t length,
                                                void * owner, flow_destructor_function memory_free);
PUB struct flow_io * flow_io_create_for_output_buffer(flow_c * c, void * owner);

// Returns false if the flow_io struct is disposed or not an output buffer type (or for any other error)
PUB bool flow_io_get_output_buffer(flow_c * c, struct flow_io * io, uint8_t ** out_pointer_to_buffer,
                                   size_t * out_length);
PUB struct flow_io * flow_job_get_io(flow_c * c, struct flow_job * job, int32_t placeholder_id);

PUB bool flow_job_get_output_buffer(flow_c * c, struct flow_job * job, int32_t placeholder_id,
                                    uint8_t ** out_pointer_to_buffer, size_t * out_length);
PUB bool flow_io_write_output_buffer_to_file(flow_c * c, struct flow_io * io, const char * file_path);

PUB bool flow_job_initialize_encoder(flow_c * c, struct flow_job * job, int32_t by_placeholder_id, int64_t codec_id);

PUB bool flow_job_add_io(flow_c * c, struct flow_job * job, struct flow_io * io, int32_t placeholder_id,
                         FLOW_DIRECTION direction);

PUB struct flow_codec_instance * flow_job_get_codec_instance(flow_c * c, struct flow_job * job,
                                                             int32_t by_placeholder_id);
PUB bool flow_job_set_default_encoder(flow_c * c, struct flow_job * job, int32_t by_placeholder_id,
                                      int64_t default_encoder_id);

PUB bool flow_node_set_decoder_downscale_hint(flow_c * c, struct flow_graph * g, int32_t node_id, int64_t if_wider_than,
                                              int64_t or_taller_than, int64_t downscaled_min_width,
                                              int64_t downscaled_min_height);

PUB bool flow_job_decoder_set_downscale_hints_by_placeholder_id(flow_c * c, struct flow_job * job,
                                                                int32_t placeholder_id, int64_t if_wider_than,
                                                                int64_t or_taller_than, int64_t downscaled_min_width,
                                                                int64_t downscaled_min_height);

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
                                   flow_interpolation_filter downscale_filter, flow_interpolation_filter upscale_filter);

PUB int32_t flow_node_create_primitive_flip_vertical(flow_c * c, struct flow_graph ** g, int32_t prev_node);
PUB int32_t flow_node_create_primitive_flip_horizontal(flow_c * c, struct flow_graph ** g, int32_t prev_node);
PUB int32_t flow_node_create_clone(flow_c * c, struct flow_graph ** g, int32_t prev_node);
PUB int32_t flow_node_create_expand_canvas(flow_c * c, struct flow_graph ** g, int32_t prev_node, uint32_t left,
                                           uint32_t top, uint32_t right, uint32_t bottom, uint32_t canvas_color_srgb);
PUB int32_t flow_node_create_fill_rect(flow_c * c, struct flow_graph ** g, int32_t prev_node, uint32_t x1, uint32_t y1,
                                       uint32_t x2, uint32_t y2, uint32_t color_srgb);
PUB int32_t flow_node_create_transpose(flow_c * c, struct flow_graph ** g, int32_t prev_node);

PUB int32_t flow_node_create_rotate_90(flow_c * c, struct flow_graph ** g, int32_t prev_node);

PUB int32_t flow_node_create_rotate_180(flow_c * c, struct flow_graph ** g, int32_t prev_node);

PUB int32_t flow_node_create_rotate_270(flow_c * c, struct flow_graph ** g, int32_t prev_node);

PUB int32_t
    flow_node_create_encoder_placeholder(flow_c * c, struct flow_graph ** g, int32_t prev_node, int32_t output_slot_id);

PUB int32_t flow_node_create_encoder(flow_c * c, struct flow_graph ** g, int32_t prev_node, int32_t placeholder_id,
                                     int64_t desired_encoder_id);

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

PUB int32_t flow_edge_create(flow_c * c, struct flow_graph ** g, int32_t from, int32_t to, flow_edgetype type);

PUB int32_t flow_node_create_render1d(flow_c * c, struct flow_graph ** g, int32_t prev_node, bool transpose_on_write,
                                      int32_t scale_to_width, flow_working_floatspace scale_and_filter_in_colorspace,
                                      float sharpen_percent, struct flow_scanlines_filter * filter_list,
                                      flow_interpolation_filter interpolation_filter);

PUB struct flow_job * flow_job_create(flow_c * c);
PUB bool flow_job_destroy(flow_c * c, struct flow_job * job);
PUB bool flow_job_configure_recording(flow_c * c, struct flow_job * job, bool record_graph_versions,
                                      bool record_frame_images, bool render_last_graph, bool render_graph_versions,
                                      bool render_animated_graph);

PUB bool flow_job_decoder_switch_frame(flow_c * c, struct flow_job * job, int32_t by_placeholder_id,
                                       int64_t frame_index);

PUB bool flow_graph_validate(flow_c * c, struct flow_graph * g);

PUB int32_t flow_node_create_generic(flow_c * c, struct flow_graph ** graph_ref, int32_t prev_node, flow_ntype type);

PUB uint32_t flow_pixel_format_bytes_per_pixel(flow_pixel_format format);

struct flow_decoder_info {
    int64_t codec_id;
    const char * preferred_mime_type;
    const char * preferred_extension;
    size_t frame_count;
    int64_t current_frame_index;
    int32_t frame0_width;
    int32_t frame0_height;
    flow_pixel_format frame0_post_decode_format;
    // const char * format_subtype;
    // bool is_srgb;
};

PUB bool flow_job_get_decoder_info(flow_c * c, struct flow_job * job, int32_t by_placeholder_id,
                                   struct flow_decoder_info * info);

PUB bool flow_bitmap_bgra_write_png(flow_c * c, struct flow_job * job, struct flow_bitmap_bgra * frame,
                                    struct flow_io * io);

#undef PUB

#ifdef __cplusplus
}
#endif
