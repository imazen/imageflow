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

#if defined(__GNUC__)
#define FLOW_HINT_HOT __attribute__((hot))
#define FLOW_HINT_PURE __attribute__((pure))
#else
#define FLOW_HINT_HOT
#define FLOW_HINT_PURE
#endif

#if defined(__GNUC__) && !defined(__clang__)
#define FLOW_HINT_UNSAFE_MATH_OPTIMIZATIONS __attribute__((optimize("-funsafe-math-optimizations")))
#else
#define FLOW_HINT_UNSAFE_MATH_OPTIMIZATIONS
#endif

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
    // The number of floats between (0,0) and (0,1)
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
    struct flow_error_callstack_line callstack[8];
    int callstack_count;
    int callstack_capacity;
    bool locked;
    bool status_included_in_message;
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

struct flow_colorcontext_info {
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
    bool destructor_called;
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
    struct flow_context_codec_set * codec_set;
    struct flow_heap underlying_heap;
    struct flow_objtracking_info object_tracking;
    struct flow_profiling_log log;
    struct flow_error_info error;
};

typedef struct flow_context flow_c;
#include "color.h"

PUB bool write_frame_to_disk(flow_c * c, const char * path, struct flow_bitmap_bgra * b);

struct flow_nodeinfo_scale2d_render_to_canvas1d;


struct flow_nodeinfo_scale2d_render_to_canvas1d {
    // There will need to be consistency checks against the createcanvas node

    // struct flow_interpolation_details * interpolationDetails;
    uint32_t x;
    uint32_t y;

    uint32_t w;
    uint32_t h;
    float sharpen_percent_goal;
    flow_interpolation_filter interpolation_filter;

    flow_working_floatspace scale_in_colorspace;
};

PUB bool flow_node_execute_scale2d_render1d(
    flow_c * c, struct flow_bitmap_bgra * input, struct flow_bitmap_bgra * canvas,
    struct flow_nodeinfo_scale2d_render_to_canvas1d * info) FLOW_HINT_HOT FLOW_HINT_UNSAFE_MATH_OPTIMIZATIONS;

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

PUB bool flow_bitmap_float_convert_srgb_to_linear(flow_c * c, struct flow_colorcontext_info * colorcontext,
                                                  struct flow_bitmap_bgra * src, uint32_t from_row,
                                                  struct flow_bitmap_float * dest, uint32_t dest_row,
                                                  uint32_t row_count);

PUB uint32_t flow_bitmap_float_approx_gaussian_calculate_d(float sigma, uint32_t bitmap_width);

PUB uint32_t flow_bitmap_float_approx_gaussian_buffer_element_count_required(float sigma, uint32_t bitmap_width);

PUB bool flow_bitmap_float_approx_gaussian_blur_rows(flow_c * c, struct flow_bitmap_float * image, float sigma,
                                                     float * buffer, size_t buffer_element_count, uint32_t from_row,
                                                     int row_count);
PUB bool flow_bitmap_float_composite_linear_over_srgb(flow_c * c, struct flow_colorcontext_info * colorcontext,
                                                      struct flow_bitmap_float * src, uint32_t from_row,
                                                      struct flow_bitmap_bgra * dest, uint32_t dest_row,
                                                      uint32_t row_count, bool transpose);

PUB bool flow_bitmap_float_demultiply_alpha(flow_c * c, struct flow_bitmap_float * src, const uint32_t from_row,
                                            const uint32_t row_count);

PUB bool flow_bitmap_float_copy_linear_over_srgb(flow_c * c, struct flow_colorcontext_info * colorcontext,
                                                 struct flow_bitmap_float * src, const uint32_t from_row,
                                                 struct flow_bitmap_bgra * dest, const uint32_t dest_row,
                                                 const uint32_t row_count, const uint32_t from_col,
                                                 const uint32_t col_count, const bool transpose);

PUB bool flow_bitmap_bgra_fill_rect(flow_c * c, struct flow_bitmap_bgra * b, uint32_t x1, uint32_t y1, uint32_t x2,
                                    uint32_t y2, uint32_t color_srgb_argb);


PUB void flow_scale_spatial_srgb_7x7(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_srgb_6x6(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_srgb_5x5(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_srgb_4x4(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_srgb_3x3(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_srgb_2x2(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_srgb_1x1(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_7x7(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_6x6(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_5x5(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_4x4(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_3x3(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_2x2(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_1x1(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);
// https://github.com/imazen/freeimage/blob/master/Source/FreeImage/FreeImageIO.cpp
// https://github.com/imazen/freeimage/blob/master/Source/FreeImage/PluginJPEG.cpp

PUB bool flow_profile_is_srgb(unsigned char * profile, size_t profile_len);
// shutdown
// nature - memory, FILE *,

typedef enum flow_scale_flags {
    flow_scale_flags_none = 0,
    flow_scale_flags_use_scale2d = 1,

} flow_scale_flags;

struct flow_decoder_downscale_hints {

    int64_t downscale_if_wider_than;
    int64_t or_if_taller_than;
    int64_t downscaled_min_width;
    int64_t downscaled_min_height;
    bool scale_luma_spatially;
    bool gamma_correct_for_srgb_during_spatial_luma_scaling;
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
    int64_t optional_file_length; // Whoever sets up this structure can populate this value - or set it to -1 - as they
    // wish. useful for resource estimation.
};

struct flow_codec_instance {
    int32_t io_id;
    int64_t codec_id;
    void * codec_state;
    struct flow_io * io;
    FLOW_DIRECTION direction;
};

PUB int32_t flow_codecs_jpg_decoder_get_exif(flow_c * c, struct flow_codec_instance * codec_instance);

PUB bool flow_bitmap_bgra_save_png(flow_c * c, struct flow_bitmap_bgra * b, const char * path);
PUB uint8_t ** flow_bitmap_create_row_pointers(flow_c * c, void * buffer, size_t buffer_size, size_t stride,
                                               size_t height);

PUB bool flow_codec_decoder_set_downscale_hints(flow_c * c, struct flow_codec_instance * codec,
                                                struct flow_decoder_downscale_hints * hints,
                                                bool crash_if_not_implemented);
PUB struct flow_bitmap_bgra * flow_codec_execute_read_frame(flow_c * c, struct flow_codec_instance * codec, struct flow_decoder_color_info * info);

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

struct flow_sanity_check {
    uint32_t sizeof_bool;
    uint32_t sizeof_int;
    uint32_t sizeof_size_t;
};

PUB void flow_sanity_check(struct flow_sanity_check * info);

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
