#pragma once

#include "imageflow.h"

#ifdef __cplusplus
extern "C" {
#endif

#define PUB FLOW_EXPORT

struct flow_heap;
struct flow_codec_instance;
struct flow_job;
struct flow_bitmap_float;
struct flow_interpolation_details;
struct flow_interpolation_pixel_contributions;
struct flow_interpolation_line_contributions;
struct flow_profiling_log;
struct flow_profiling_entry;
struct flow_convolution_kernel;
struct flow_Renderer;

////////////////////////////////////////////
//  Portable snprintf
PUB int flow_snprintf(char * s, size_t n, const char * fmt, ...);
PUB int flow_vsnprintf(char * s, size_t n, const char * fmt, va_list v);

////////////////////////////////////////////
// You can control the underlying heap if you want

typedef void * (*flow_heap_calloc_function)(struct flow_context * context, struct flow_heap * heap, size_t count,
                                            size_t element_size, const char * file, int line);
typedef void * (*flow_heap_malloc_function)(struct flow_context * context, struct flow_heap * heap, size_t byte_count,
                                            const char * file, int line);

typedef void * (*flow_heap_realloc_function)(struct flow_context * context, struct flow_heap * heap, void * old_pointer,
                                             size_t new_byte_count, const char * file, int line);
typedef void (*flow_heap_free_function)(struct flow_context * context, struct flow_heap * heap, void * pointer,
                                        const char * file, int line);
typedef void (*flow_heap_terminate_function)(struct flow_context * context, struct flow_heap * heap);

PUB void * flow_heap_get_private_state(struct flow_heap * heap);
PUB bool flow_heap_set_private_state(struct flow_heap * heap, void * private_state);

PUB bool flow_heap_set_default(flow_c * c);
PUB bool flow_heap_set_custom(flow_c * c, flow_heap_calloc_function calloc, flow_heap_malloc_function malloc,
                              flow_heap_realloc_function realloc, flow_heap_free_function free,
                              flow_heap_terminate_function terminate, void * initial_private_state);

PUB bool flow_set_destructor(flow_c * c, void * thing, flow_destructor_function destructor);

// Thing will only be automatically destroyed and freed at the time that owner is destroyed and freed
PUB bool flow_set_owner(flow_c * c, void * thing, void * owner);

////////////////////////////////////////////
// use imageflow memory management

PUB void * flow_context_calloc(flow_c * c, size_t instance_count, size_t instance_size,
                               flow_destructor_function destructor, void * owner, const char * file, int line);
PUB void * flow_context_malloc(flow_c * c, size_t byte_count, flow_destructor_function destructor, void * owner,
                               const char * file, int line);
PUB void * flow_context_realloc(flow_c * c, void * old_pointer, size_t new_byte_count, const char * file, int line);
PUB void flow_deprecated_free(flow_c * c, void * pointer, const char * file, int line);
PUB bool flow_destroy_by_owner(flow_c * c, void * owner, const char * file, int line);
PUB bool flow_destroy(flow_c * c, void * pointer, const char * file, int line);

#define FLOW_calloc(context, instance_count, element_size)                                                             \
    flow_context_calloc(context, instance_count, element_size, NULL, context, __FILE__, __LINE__)
#define FLOW_calloc_array(context, instance_count, type_name)                                                          \
    (type_name *) flow_context_calloc(context, instance_count, sizeof(type_name), NULL, context, __FILE__, __LINE__)
#define FLOW_malloc(context, byte_count) flow_context_malloc(context, byte_count, NULL, context, __FILE__, __LINE__)

#define FLOW_calloc_owned(context, instance_count, element_size, owner)                                                \
    flow_context_calloc(context, instance_count, element_size, NULL, owner, __FILE__, __LINE__)
#define FLOW_calloc_array_owned(context, instance_count, type_name, owner)                                             \
    (type_name *) flow_context_calloc(context, instance_count, sizeof(type_name), NULL, owner, __FILE__, __LINE__)
#define FLOW_malloc_owned(context, byte_count, owner)                                                                  \
    flow_context_malloc(context, byte_count, NULL, owner, __FILE__, __LINE__)

#define FLOW_realloc(context, old_pointer, new_byte_count)                                                             \
    flow_context_realloc(context, old_pointer, new_byte_count, __FILE__, __LINE__)

#define FLOW_free(context, pointer) flow_deprecated_free(context, pointer, __FILE__, __LINE__)
#define FLOW_destroy(context, pointer) flow_destroy(context, pointer, __FILE__, __LINE__)

////////////////////////////////////////////
// use imageflow's error system
PUB void flow_context_raise_error(flow_c * c, flow_status_code code, char * message, const char * file, int line,
                                  const char * function_name);
PUB char * flow_context_set_error_get_message_buffer(flow_c * c, flow_status_code code, const char * file, int line,
                                                     const char * function_name);
PUB void flow_context_add_to_callstack(flow_c * c, const char * file, int line, const char * function_name);

#define FLOW_error(context, status_code)                                                                               \
    flow_context_set_error_get_message_buffer(context, status_code, __FILE__, __LINE__, __func__)
#define FLOW_error_msg(context, status_code, ...)                                                                      \
    flow_snprintf(flow_context_set_error_get_message_buffer(context, status_code, __FILE__, __LINE__, __func__),       \
                  FLOW_ERROR_MESSAGE_SIZE, __VA_ARGS__)

#define FLOW_add_to_callstack(context) flow_context_add_to_callstack(context, __FILE__, __LINE__, __func__)

#define FLOW_error_return(context)                                                                                     \
    flow_context_add_to_callstack(context, __FILE__, __LINE__, __func__);                                              \
    return false

////////////////////////////////////////////
// profiling (not widely used ATM)

typedef enum flow_profiling_entry_flags {
    flow_profiling_entry_start = 2,
    flow_profiling_entry_start_allow_recursion = 6,
    flow_profiling_entry_stop = 8,
    flow_profiling_entry_stop_assert_started = 24,
    flow_profiling_entry_stop_children = 56
} flow_profiling_entry_flags;

struct flow_profiling_entry {
    int64_t time;
    const char * name;
    flow_profiling_entry_flags flags;
};

struct flow_profiling_log {
    struct flow_profiling_entry * log;
    uint32_t count;
    uint32_t capacity;
    int64_t ticks_per_second;
};

PUB struct flow_profiling_log * flow_context_get_profiler_log(flow_c * c);

PUB bool flow_context_enable_profiling(flow_c * c, uint32_t default_capacity);

#define FLOW_ALLOW_PROFILING

#ifdef FLOW_ALLOW_PROFILING
#define flow_prof_start(context, name, allow_recursion) flow_context_profiler_start(context, name, allow_recursion);
#define flow_prof_stop(context, name, assert_started, stop_children)                                                   \
    flow_context_profiler_stop(context, name, assert_started, stop_children);
#else
#define flow_prof_start(context, name, allow_recursion)
#define flow_prof_stop(context, name, assert_started, stop_children)
#endif

PUB void flow_context_profiler_start(flow_c * c, const char * name, bool allow_recursion);
PUB void flow_context_profiler_stop(flow_c * c, const char * name, bool assert_started, bool stop_children);

////////////////////////////////////////////
// Make your own I/O systems
struct flow_io;

// Returns the number of read into the buffer. Failure to read 'count' bytes could mean EOF or failure. Check context
// status. Pass NULL to buffer if you want to skip 'count' many bytes, seeking ahead.
typedef int64_t (*flow_io_read_function)(flow_c * c, struct flow_io * io, uint8_t * buffer, size_t count);
// Returns the number of bytes written. If it doesn't equal 'count', there was an error. Check context status
typedef int64_t (*flow_io_write_function)(flow_c * c, struct flow_io * io, const uint8_t * buffer, size_t count);

// Returns negative on failure - check context for more detail. Returns the current position in the stream when
// successful
typedef int64_t (*flow_io_position_function)(flow_c * c, struct flow_io * io);

// Returns true if seek was successful.
typedef bool (*flow_io_seek_function)(flow_c * c, struct flow_io * io, int64_t position);

////////////////////////////////////////////
// Make your own codecs
struct flow_decoder_frame_info;

typedef bool (*codec_intialize)(flow_c * c, struct flow_job * job, struct flow_codec_instance * instance);

typedef bool (*codec_get_info_fn)(flow_c * c, struct flow_job * job, void * codec_state,
                                  struct flow_decoder_info * decoder_info_ref);
typedef bool (*codec_switch_frame_fn)(flow_c * c, struct flow_job * job, void * codec_state, size_t frame_index);

typedef bool (*codec_get_frame_info_fn)(flow_c * c, struct flow_job * job, void * codec_state,
                                        struct flow_decoder_frame_info * decoder_frame_info_ref);

typedef bool (*codec_set_downscale_hints_fn)(flow_c * c, struct flow_job * job, struct flow_codec_instance * codec,
                                             struct flow_decoder_downscale_hints * hints);

typedef bool (*codec_read_frame_fn)(flow_c * c, struct flow_job * job, void * codec_state,
                                    struct flow_bitmap_bgra * canvas);

typedef bool (*codec_write_frame_fn)(flow_c * c, struct flow_job * job, void * codec_state,
                                     struct flow_bitmap_bgra * frame);

typedef bool (*codec_stringify_fn)(flow_c * c, struct flow_job * job, void * codec_state, char * buffer,
                                   size_t buffer_size);

struct flow_codec_magic_bytes {
    size_t byte_count;
    const uint8_t * bytes;
};

struct flow_codec_definition {
    int64_t codec_id;
    codec_intialize initialize;
    codec_get_info_fn get_info;
    codec_get_frame_info_fn get_frame_info;
    codec_set_downscale_hints_fn set_downscale_hints;
    codec_switch_frame_fn switch_frame;
    codec_read_frame_fn read_frame;
    codec_write_frame_fn write_frame;
    codec_stringify_fn stringify;
    const char * name;
    const char * preferred_mime_type;
    const char * preferred_extension;
    struct flow_codec_magic_bytes * magic_byte_sets;
    size_t magic_byte_sets_count;
};

struct flow_context_codec_set {
    struct flow_codec_definition * codecs;
    size_t codecs_count;
};
PUB struct flow_context_codec_set * flow_context_get_default_codec_set(void);

////////////////////////////////////////////
// Deal with graphs

typedef bool (*flow_graph_visitor)(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref, int32_t id,
                                   bool * quit, bool * skip_outbound_paths, void * custom_data);

PUB bool flow_graph_walk(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref,
                         flow_graph_visitor node_visitor, flow_graph_visitor edge_visitor, void * custom_data);

PUB bool flow_node_delete(flow_c * c, struct flow_graph * g, int32_t node_id);

PUB bool flow_edge_delete(flow_c * c, struct flow_graph * g, int32_t edge_id);

PUB bool flow_edge_delete_all_connected_to_node(flow_c * c, struct flow_graph * g, int32_t node_id);

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

PUB void flow_graph_print_to(flow_c * c, struct flow_graph * g, FILE * stream);

////////////////////////////////////////////
// Deal with bitmaps

// non-indexed bitmap
struct flow_bitmap_bgra {

    // bitmap width in pixels
    uint32_t w;
    // bitmap height in pixels
    uint32_t h;
    // byte length of each row (may include any amount of padding)
    uint32_t stride;
    // pointer to pixel 0,0; should be of length > h * stride
    unsigned char * pixels;
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
};

PUB float flow_context_byte_to_floatspace(flow_c * c, uint8_t srgb_value);
PUB uint8_t flow_context_floatspace_to_byte(flow_c * c, float space_value);

PUB void flow_context_set_floatspace(flow_c * ctx, flow_working_floatspace space, float a, float b, float c);

typedef double (*flow_detailed_interpolation_method)(const struct flow_interpolation_details *, double);

struct flow_interpolation_details {
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
};

struct flow_convolution_kernel {
    float * kernel;
    uint32_t width;
    uint32_t radius;
    float threshold_min_change; // These change values are on a somewhat arbitrary scale between 0 and 4;
    float threshold_max_change;
    float * buffer;
};

struct flow_RenderDetails {
    // Interpolation and scaling details
    struct flow_interpolation_details * interpolation;
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
    struct flow_convolution_kernel * kernel_a;
    // A second convolution to apply
    struct flow_convolution_kernel * kernel_b;

    // If greater than 0, a percentage to sharpen the result along each axis;
    float sharpen_percent_goal;

    // If true, we should apply the color matrix
    bool apply_color_matrix;

    float color_matrix_data[25];
    float * color_matrix[5];

    // Transpose, flipx, flipy - combined, these give you all 90 interval rotations
    bool post_transpose;
    bool post_flip_x;
    bool post_flip_y;

    // Enables profiling
    bool enable_profiling;
};

PUB struct flow_bitmap_bgra * flow_bitmap_bgra_create(flow_c * c, int sx, int sy, bool zeroed,
                                                      flow_pixel_format format);
PUB struct flow_bitmap_bgra * flow_bitmap_bgra_create_header(flow_c * c, int sx, int sy);
PUB void flow_bitmap_bgra_destroy(flow_c * c, struct flow_bitmap_bgra * im);
PUB bool flow_bitmap_bgra_flip_horizontal(flow_c * c, struct flow_bitmap_bgra * b);
PUB bool flow_bitmap_bgra_compare(flow_c * c, struct flow_bitmap_bgra * a, struct flow_bitmap_bgra * b,
                                  bool * equal_out);

PUB struct flow_RenderDetails * flow_RenderDetails_create(flow_c * c);
PUB struct flow_RenderDetails * flow_RenderDetails_create_with(flow_c * c, flow_interpolation_filter filter);

PUB bool flow_RenderDetails_render(flow_c * c, struct flow_RenderDetails * details, struct flow_bitmap_bgra * source,
                                   struct flow_bitmap_bgra * canvas);
PUB bool flow_RenderDetails_render_in_place(flow_c * c, struct flow_RenderDetails * details,
                                            struct flow_bitmap_bgra * edit_in_place);
PUB void flow_RenderDetails_destroy(flow_c * c, struct flow_RenderDetails * d);

PUB bool flow_interpolation_filter_exists(flow_interpolation_filter filter);
PUB struct flow_interpolation_details * flow_interpolation_details_create(flow_c * c);
PUB struct flow_interpolation_details *
flow_interpolation_details_create_bicubic_custom(flow_c * c, double window, double blur, double B, double C);
PUB struct flow_interpolation_details *
flow_interpolation_details_create_custom(flow_c * c, double window, double blur,
                                         flow_detailed_interpolation_method filter);
PUB struct flow_interpolation_details * flow_interpolation_details_create_from(flow_c * c,
                                                                               flow_interpolation_filter filter);
PUB double flow_interpolation_details_percent_negative_weight(const struct flow_interpolation_details * details);
PUB void flow_interpolation_details_destroy(flow_c * c, struct flow_interpolation_details *);

struct flow_interpolation_pixel_contributions {
    float * Weights; /* Normalized weights of neighboring pixels */
    int Left; /* Bounds of source pixels window */
    int Right;
}; /* Contirbution information for a single pixel */

struct flow_interpolation_line_contributions {
    struct flow_interpolation_pixel_contributions * ContribRow; /* Row (or column) of contribution weights */
    uint32_t WindowSize; /* Filter window size (of affecting source pixels) */
    uint32_t LineLength; /* Length of line (no. or rows / cols) */
    double percent_negative; /* Estimates the sharpening effect actually applied*/
};

PUB struct flow_interpolation_line_contributions *
flow_interpolation_line_contributions_create(flow_c * c, const uint32_t output_line_size,
                                             const uint32_t input_line_size,
                                             const struct flow_interpolation_details * details);
PUB void flow_interpolation_line_contributions_destroy(flow_c * c, struct flow_interpolation_line_contributions * p);

PUB struct flow_convolution_kernel * flow_convolution_kernel_create(flow_c * c, uint32_t radius);
PUB void flow_convolution_kernel_destroy(flow_c * c, struct flow_convolution_kernel * kernel);

PUB struct flow_convolution_kernel * flow_convolution_kernel_create_guassian(flow_c * c, double stdDev,
                                                                             uint32_t radius);
// The only error these 2 could generate would be a null pointer. Should they have a context just for this?
PUB double flow_convolution_kernel_sum(struct flow_convolution_kernel * kernel);
PUB void flow_convolution_kernel_normalize(struct flow_convolution_kernel * kernel, float desiredSum);
PUB struct flow_convolution_kernel * flow_convolution_kernel_create_gaussian_normalized(flow_c * c, double stdDev,
                                                                                        uint32_t radius);
PUB struct flow_convolution_kernel * flow_convolution_kernel_create_guassian_sharpen(flow_c * c, double stdDev,
                                                                                     uint32_t radius);

PUB bool flow_bitmap_bgra_populate_histogram(flow_c * c, struct flow_bitmap_bgra * bmp, uint64_t * histograms,
                                             uint32_t histogram_size_per_channel, uint32_t histogram_count,
                                             uint64_t * pixels_sampled);
#undef PUB
#ifdef __cplusplus
}
#endif
