/*
 * Copyright (c) Imazen LLC.
 * No part of this project, including this file, may be copied, modified,
 * propagated, or distributed except as permitted in COPYRIGHT.txt.
 * Licensed under the GNU Affero General Public License, Version 3.0.
 * Commercial licenses available at http://imageresizing.net/
 */
#pragma once

#include "imageflow_advanced.h"
#include "math_functions.h"
#include "png.h"
#include <stdlib.h>
#include <stdarg.h>
#include <string.h>
#include <sys/stat.h>
#include <errno.h>

#define __STDC_FORMAT_MACROS
#include <inttypes.h>
#undef __STDC_FORMAT_MACROS

#ifdef __cplusplus
extern "C" {
#endif

#define PUB FLOW_EXPORT

// floating-point bitmap, typically linear RGBA, premultiplied
struct flow_bitmap_float {
    // buffer width in pixels
    uint32_t w;
    // buffer height in pixels
    uint32_t h;
    // The number of floats per pixel
    uint32_t channels;
    // The pixel data
    float * pixels;
    // If true, don't dispose the buffer with the struct
    bool pixels_borrowed;
    // The number of floats in the buffer
    uint32_t float_count;
    // The number of floats betwen (0,0) and (0,1)
    uint32_t float_stride;

    // If true, alpha has been premultiplied
    bool alpha_premultiplied;
    // If true, the alpha channel holds meaningful data
    bool alpha_meaningful;
};

/** flow_context: Heap Manager **/

struct flow_heap {
    flow_heap_calloc_function _calloc;
    flow_heap_malloc_function _malloc;
    flow_heap_realloc_function _realloc;
    flow_heap_free_function _free;
    flow_heap_terminate_function _context_terminate;
    void * _private_state;
};
struct flow_objtracking_info;
void flow_context_objtracking_initialize(struct flow_objtracking_info * heap_tracking);
void flow_context_objtracking_terminate(flow_c * c);

/** flow_context: struct flow_error_info **/

struct flow_error_callstack_line {
    const char * file;
    int line;
    const char * function_name;
};

#define FLOW_ERROR_MESSAGE_SIZE 1023

struct flow_error_info {
    flow_status_code reason;
    struct flow_error_callstack_line callstack[14];
    int callstack_count;
    int callstack_capacity;
    bool locked;
    char message[FLOW_ERROR_MESSAGE_SIZE + 1];
};

#ifdef EXPOSE_SIGMOID
/** flow_context: Colorspace */
struct flow_SigmoidInfo {
    float constant;
    float x_coeff;
    float x_offset;
    float y_offset;
    float y_coeff;
};

#endif

struct flow_colorspace_info {
    float byte_to_float[256]; // Converts 0..255 -> 0..1, but knowing that 0.255 has sRGB gamma.
    flow_working_floatspace floatspace;
    bool apply_srgb;
    bool apply_gamma;
    float gamma;
    float gamma_inverse;
#ifdef EXPOSE_SIGMOID
    struct flow_SigmoidInfo sigmoid;
    bool apply_sigmoid;
#endif
};

#define FLOW_USER_IS_OWNER
struct flow_heap_object_record {
    void * ptr;
    size_t bytes;
    void * owner;
    flow_destructor_function destructor;
    const char * allocated_by;
    int allocated_by_line;
    bool is_owner;
};
struct flow_objtracking_info {
    struct flow_heap_object_record * allocs;
    size_t next_free_slot;
    size_t total_slots;
    size_t bytes_allocated_net;
    size_t bytes_allocated_gross;
    size_t allocations_net;
    size_t allocations_gross;
    size_t bytes_freed;
    size_t allocations_net_peak;
    size_t bytes_allocated_net_peak;
};

/** flow_context: main structure **/

struct flow_context {
    struct flow_error_info error;
    struct flow_heap underlying_heap;
    struct flow_profiling_log log;
    struct flow_colorspace_info colorspace;
    struct flow_objtracking_info object_tracking;
    struct flow_context_codec_set * codec_set;
};

typedef struct flow_context flow_c;
#include "color.h"

PUB bool write_frame_to_disk(flow_c * c, const char * path, struct flow_bitmap_bgra * b);

PUB bool flow_node_execute_render_to_canvas_1d(flow_c * c, struct flow_job * job, struct flow_bitmap_bgra * input,
                                               struct flow_bitmap_bgra * canvas,
                                               struct flow_nodeinfo_render_to_canvas_1d * info);

PUB bool flow_job_populate_dimensions_where_certain(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref);
// For doing execution cost estimates, we force estimate, then flatten, then calculate cost
PUB bool flow_job_force_populate_dimensions(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref);
PUB bool flow_job_execute_where_certain(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref);
PUB bool flow_job_graph_fully_executed(flow_c * c, struct flow_job * job, struct flow_graph * g);

PUB bool flow_job_notify_graph_changed(flow_c * c, struct flow_job * job, struct flow_graph * g);
PUB bool flow_job_execute(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref);
PUB bool flow_graph_post_optimize_flatten(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref);

PUB bool flow_graph_optimize(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref);
PUB bool flow_graph_pre_optimize_flatten(flow_c * c, struct flow_graph ** graph_ref);
PUB int32_t flow_graph_get_edge_count(flow_c * c, struct flow_graph * g, int32_t node_id, bool filter_by_edge_type,
                                      flow_edgetype type, bool include_inbound, bool include_outbound);

PUB bool flow_node_post_optimize_flatten(flow_c * c, struct flow_graph ** graph_ref, int32_t node_id);

PUB bool flow_graph_walk_dependency_wise(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref,
                                         flow_graph_visitor node_visitor, flow_graph_visitor edge_visitor,
                                         void * custom_data);

PUB struct flow_bitmap_float * flow_bitmap_float_create_header(flow_c * c, int sx, int sy, int channels);

PUB struct flow_bitmap_float * flow_bitmap_float_create(flow_c * c, int sx, int sy, int channels, bool zeroed);

PUB void flow_bitmap_float_destroy(flow_c * c, struct flow_bitmap_float * im);

PUB bool flow_bitmap_float_scale_rows(flow_c * c, struct flow_bitmap_float * from, uint32_t from_row,
                                      struct flow_bitmap_float * to, uint32_t to_row, uint32_t row_count,
                                      struct flow_interpolation_pixel_contributions * weights);
PUB bool flow_bitmap_float_convolve_rows(flow_c * c, struct flow_bitmap_float * buf,
                                         struct flow_convolution_kernel * kernel, uint32_t convolve_channels,
                                         uint32_t from_row, int row_count);

PUB bool flow_bitmap_float_sharpen_rows(flow_c * c, struct flow_bitmap_float * im, uint32_t start_row,
                                        uint32_t row_count, double pct);

PUB bool flow_bitmap_float_convert_srgb_to_linear(flow_c * c, struct flow_bitmap_bgra * src, uint32_t from_row,
                                                  struct flow_bitmap_float * dest, uint32_t dest_row,
                                                  uint32_t row_count);

PUB uint32_t flow_bitmap_float_approx_gaussian_calculate_d(float sigma, uint32_t bitmap_width);

PUB uint32_t flow_bitmap_float_approx_gaussian_buffer_element_count_required(float sigma, uint32_t bitmap_width);

PUB bool flow_bitmap_float_approx_gaussian_blur_rows(flow_c * c, struct flow_bitmap_float * image, float sigma,
                                                     float * buffer, size_t buffer_element_count, uint32_t from_row,
                                                     int row_count);
PUB bool flow_bitmap_float_pivoting_composite_linear_over_srgb(flow_c * c, struct flow_bitmap_float * src,
                                                               uint32_t from_row, struct flow_bitmap_bgra * dest,
                                                               uint32_t dest_row, uint32_t row_count, bool transpose);

PUB bool flow_bitmap_float_flip_vertical(flow_c * c, struct flow_bitmap_bgra * b);

PUB bool flow_bitmap_float_demultiply_alpha(flow_c * c, struct flow_bitmap_float * src, const uint32_t from_row,
                                            const uint32_t row_count);

PUB bool flow_bitmap_float_copy_linear_over_srgb(flow_c * c, struct flow_bitmap_float * src, const uint32_t from_row,
                                                 struct flow_bitmap_bgra * dest, const uint32_t dest_row,
                                                 const uint32_t row_count, const uint32_t from_col,
                                                 const uint32_t col_count, const bool transpose);

PUB bool flow_halve(flow_c * c, const struct flow_bitmap_bgra * from, struct flow_bitmap_bgra * to, int divisor);

PUB bool flow_halve_in_place(flow_c * c, struct flow_bitmap_bgra * from, int divisor);

// https://github.com/imazen/freeimage/blob/master/Source/FreeImage/FreeImageIO.cpp
// https://github.com/imazen/freeimage/blob/master/Source/FreeImage/PluginJPEG.cpp

// shutdown
// nature - memory, FILE *,

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
};
struct flow_nodeinfo_bitmap_bgra_pointer {
    struct flow_bitmap_bgra ** ref;
};

struct flow_decoder_downscale_hints {

    int64_t downscale_if_wider_than;
    int64_t or_if_taller_than;
    int64_t downscaled_min_width;
    int64_t downscaled_min_height;
};
struct flow_nodeinfo_codec {
    int32_t placeholder_id;
    struct flow_codec_instance * codec;
    // For encoders
    int64_t desired_encoder_id;
    // For decdoers
    struct flow_decoder_downscale_hints downscale_hints;
};

struct flow_nodeinfo_render_to_canvas_1d {
    // There will need to be consistency checks against the createcanvas node

    flow_interpolation_filter interpolation_filter;
    // struct flow_interpolation_details * interpolationDetails;
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

    struct flow_scanlines_filter * filter_list;
};

// If you want to know what kind of I/O structure is inside user_data, compare the read_func/write_func function
// pointers. No need for another human-assigned set of custom structure identifiers.
struct flow_io {
    flow_c * context;
    flow_io_mode mode; // Call nothing, dereference nothing, if this is 0
    flow_io_read_function read_func; // Optional for write modes
    flow_io_write_function write_func; // Optional for read modes
    flow_io_position_function position_func; // Optional for sequential modes
    flow_io_seek_function seek_function; // Optional for sequential modes
    flow_destructor_function dispose_func; // Optional.
    void * user_data;
    uint64_t optional_file_length; // Whoever sets up this structure can populate this value - or set it to -1 - as they
    // wish. useful for resource estimation.
};

struct flow_codec_instance {
    int32_t graph_placeholder_id;
    int64_t codec_id;
    void * codec_state;
    struct flow_io * io;
    struct flow_codec_instance * next;
    FLOW_DIRECTION direction;
};

struct flow_job {
    int32_t debug_job_id;
    int32_t next_graph_version;
    int32_t max_calc_flatten_execute_passes;
    struct flow_codec_instance * codecs_head;
    struct flow_codec_instance * codecs_tail; // Makes appends simple. Deletes, not so much
    bool record_graph_versions;
    bool record_frame_images;
    bool render_graph_versions;
    bool render_animated_graph;
    bool render_last_graph;
};

PUB bool flow_job_render_graph_to_png(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t graph_version);
PUB bool flow_job_notify_node_complete(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id);

PUB bool flow_job_link_codecs(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref);

PUB bool flow_job_decoder_set_downscale_hints(flow_c * c, struct flow_job * job, struct flow_codec_instance * codec,
                                              struct flow_decoder_downscale_hints * hints,
                                              bool crash_if_not_implemented);
struct flow_scanlines_filter {
    flow_scanlines_filter_type type;
    struct flow_scanlines_filter * next;
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

#undef PUB

#ifndef _TIMERS_IMPLEMENTED
#define _TIMERS_IMPLEMENTED
#ifdef _WIN32
#ifndef STRICT
#define STRICT
#endif
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <winbase.h>
static inline int64_t flow_get_high_precision_ticks(void)
{
    LARGE_INTEGER val;
    QueryPerformanceCounter(&val);
    return val.QuadPart;
}
static inline int64_t flow_get_profiler_ticks_per_second(void)
{
    LARGE_INTEGER val;
    QueryPerformanceFrequency(&val);
    return val.QuadPart;
}
#else
#include <sys/time.h>
#if defined(_POSIX_VERSION)
#if defined(_POSIX_TIMERS) && (_POSIX_TIMERS > 0)
#if defined(CLOCK_MONOTONIC_PRECISE)
/* BSD. --------------------------------------------- */
#define PROFILER_CLOCK_ID CLOCK_MONOTONIC_PRECISE
#elif defined(CLOCK_MONOTONIC_RAW)
/* Linux. ------------------------------------------- */
#define PROFILER_CLOCK_ID CLOCK_MONOTONIC_RAW
#elif defined(CLOCK_HIGHRES)
/* Solaris. ----------------------------------------- */
#define PROFILER_CLOCK_ID CLOCK_HIGHRES
#elif defined(CLOCK_MONOTONIC)
/* AIX, BSD, Linux, POSIX, Solaris. ----------------- */
#define PROFILER_CLOCK_ID CLOCK_MONOTONIC
#elif defined(CLOCK_REALTIME)
/* AIX, BSD, HP-UX, Linux, POSIX. ------------------- */
#define PROFILER_CLOCK_ID CLOCK_REALTIME
#endif
#endif
#endif

static inline int64_t flow_get_high_precision_ticks(void)
{
#ifdef PROFILER_CLOCK_ID
    struct timespec ts;
    if (clock_gettime(PROFILER_CLOCK_ID, &ts) != 0) {
        return -1;
    }
    return ts.tv_sec * 1000000 + ts.tv_nsec;
#else
    struct timeval tm;
    if (gettimeofday(&tm, NULL) != 0) {
        return -1;
    }
    return tm.tv_sec * 1000000 + tm.tv_usec;
#endif
}

static inline int64_t flow_get_profiler_ticks_per_second(void)
{
#ifdef PROFILER_CLOCK_ID
    struct timespec ts;
    if (clock_getres(PROFILER_CLOCK_ID, &ts) != 0) {
        return -1;
    }
    return ts.tv_nsec;
#else
    return 1000000;
#endif
}

#endif
#endif

#ifdef __cplusplus
}
#endif
