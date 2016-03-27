/*
 * Copyright (c) Imazen LLC.
 * No part of this project, including this file, may be copied, modified,
 * propagated, or distributed except as permitted in COPYRIGHT.txt.
 * Licensed under the GNU Affero General Public License, Version 3.0.
 * Commercial licenses available at http://imageresizing.net/
 */
#pragma once

#include "imageflow.h"
#include "math_functions.h"
#include "png.h"
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <stdarg.h>
#include <string.h>
#include <sys/stat.h>
#include <errno.h>

#ifdef __cplusplus
extern "C" {
#endif

#define PUB FLOW_EXPORT

//** START: future: imageflow_advanced.h */


struct flow_heap;
struct flow_codec_instance;
struct flow_job;
struct flow_bitmap_float;

//Portable snprintf
PUB int flow_snprintf(char* s, size_t n, const char* fmt, ...);
PUB int flow_vsnprintf(char* s, size_t n, const char* fmt, va_list v);


////////////////////////////////////////////
// You can control the underlying heap if you want

typedef void* (*flow_heap_calloc_function)(struct flow_ctx* context, struct flow_heap * heap, size_t count, size_t element_size,
                                           const char* file, int line);
typedef void* (*flow_heap_malloc_function)(struct flow_ctx* context, struct flow_heap * heap, size_t byte_count, const char* file,
                                           int line);

typedef void* (*flow_heap_realloc_function)(struct flow_ctx* context,struct flow_heap * heap,  void* old_pointer,
                                            size_t new_byte_count, const char* file, int line);
typedef void (*flow_heap_free_function)(struct flow_ctx* context, struct flow_heap * heap, void* pointer, const char* file,
                                        int line);
typedef void (*flow_heap_terminate_function)(struct flow_ctx* context, struct flow_heap * heap);

PUB void * flow_heap_get_private_state(struct flow_heap * heap);
PUB bool flow_heap_set_private_state(struct flow_heap * heap, void * private_state);

PUB bool flow_heap_set_default(flow_context * context);
PUB bool flow_heap_set_custom(flow_context *context, flow_heap_calloc_function calloc, flow_heap_malloc_function malloc,
                          flow_heap_realloc_function realloc, flow_heap_free_function free,
                          flow_heap_terminate_function terminate, void *initial_private_state);


//When you need to control 100% of heap operations, you can allocate
// flow_context_sizeof_context() bytes and initialize them with flow_context_initialize,
// then call flow_heap_set_custom. Use flow_context_terminate and your matching free() function instead of flow_context_destroy
PUB size_t flow_context_sizeof_context(void);
PUB void flow_context_initialize(flow_context* context);
//Want to ensure there were no memory leaks due to incorrect API use, and that all files flushed and close successfully?
//Call begin_terminate, then check on error status and memory stats.
PUB bool flow_context_begin_terminate(flow_context* context);
PUB void flow_context_end_terminate(flow_context* context);



PUB void* flow_context_calloc(flow_context* context, size_t instance_count, size_t instance_size, flow_destructor_function destructor,void * owner, const char* file, int line);
PUB void* flow_context_malloc(flow_context* context, size_t byte_count, flow_destructor_function destructor,void * owner, const char* file, int line);
PUB void* flow_context_realloc(flow_context* context, void* old_pointer, size_t new_byte_count, const char* file,
                               int line);
PUB void flow_deprecated_free(flow_context *context, void *pointer, const char *file, int line);
PUB bool flow_destroy_by_owner(flow_context *context, void *owner, const char *file, int line);
PUB bool flow_destroy(flow_context *context, void *pointer, const char *file, int line);



#define FLOW_calloc(context, instance_count, element_size)                                                             \
    flow_context_calloc(context, instance_count, element_size, NULL, context, __FILE__, __LINE__)
#define FLOW_calloc_array(context, instance_count, type_name)                                                          \
    (type_name*) flow_context_calloc(context, instance_count, sizeof(type_name), NULL, context, __FILE__, __LINE__)
#define FLOW_malloc(context, byte_count) flow_context_malloc(context, byte_count, NULL, context, __FILE__, __LINE__)

#define FLOW_calloc_owned(context, instance_count, element_size, owner)                                                             \
    flow_context_calloc(context, instance_count, element_size, NULL, owner, __FILE__, __LINE__)
#define FLOW_calloc_array_owned(context, instance_count, type_name, owner)                                                          \
    (type_name*) flow_context_calloc(context, instance_count, sizeof(type_name), NULL, owner, __FILE__, __LINE__)
#define FLOW_malloc_owned(context, byte_count, owner) flow_context_malloc(context, byte_count, NULL, owner, __FILE__, __LINE__)

#define FLOW_realloc(context, old_pointer, new_byte_count)                                                             \
    flow_context_realloc(context, old_pointer, new_byte_count, __FILE__, __LINE__)

#define FLOW_free(context, pointer) flow_deprecated_free(context, pointer, __FILE__, __LINE__)
#define FLOW_destroy(context, pointer) flow_destroy(context, pointer, __FILE__, __LINE__)


PUB void flow_context_raise_error(flow_context* context, flow_status_code code, char* message, const char* file,
                                  int line, const char* function_name);
PUB char* flow_context_set_error_get_message_buffer(flow_context* context, flow_status_code code, const char* file,
                                                    int line, const char* function_name);
PUB void flow_context_add_to_callstack(flow_context* context, const char* file, int line, const char* function_name);


#define FLOW_error(context, status_code)                                                                               \
    flow_context_set_error_get_message_buffer(context, status_code, __FILE__, __LINE__, __func__)
#define FLOW_error_msg(context, status_code, ...)                                                                      \
    flow_snprintf(flow_context_set_error_get_message_buffer(context, status_code, __FILE__, __LINE__, __func__),       \
                  FLOW_ERROR_MESSAGE_SIZE, __VA_ARGS__)

#define FLOW_add_to_callstack(context) flow_context_add_to_callstack(context, __FILE__, __LINE__, __func__)

#define FLOW_error_return(context)                                                                                     \
    flow_context_add_to_callstack(context, __FILE__, __LINE__, __func__);                                              \
    return false











PUB bool flow_context_enable_profiling(flow_context* context, uint32_t default_capacity);

#define FLOW_ALLOW_PROFILING

#ifdef FLOW_ALLOW_PROFILING
#define flow_prof_start(context, name, allow_recursion) flow_context_profiler_start(context, name, allow_recursion);
#define flow_prof_stop(context, name, assert_started, stop_children)                                                   \
    flow_context_profiler_stop(context, name, assert_started, stop_children);
#else
#define flow_prof_start(context, name, allow_recursion)
#define flow_prof_stop(context, name, assert_started, stop_children)
#endif

PUB void flow_context_profiler_start(flow_context* context, const char* name, bool allow_recursion);
PUB void flow_context_profiler_stop(flow_context* context, const char* name, bool assert_started, bool stop_children);



//** END: future: imageflow_advanced.h */

// floating-point bitmap, typically linear RGBA, premultiplied
typedef struct flow_bitmap_float {
    // buffer width in pixels
    uint32_t w;
    // buffer height in pixels
    uint32_t h;
    // The number of floats per pixel
    uint32_t channels;
    // The pixel data
    float* pixels;
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
} flow_bitmap_float;

/** flow_context: Heap Manager **/

struct flow_heap {
    flow_heap_calloc_function _calloc;
    flow_heap_malloc_function _malloc;
    flow_heap_realloc_function _realloc;
    flow_heap_free_function _free;
    flow_heap_terminate_function _context_terminate;
    void* _private_state;
};
struct flow_objtracking_info;
void flow_context_objtracking_initialize(struct flow_objtracking_info *heap_tracking);
void flow_context_objtracking_terminate(flow_context *context);

/** flow_context: flow_error_info **/

typedef struct flow_error_callstack_line {
    const char* file;
    int line;
    const char* function_name;
} flow_error_callstack_line;

#define FLOW_ERROR_MESSAGE_SIZE 1023

typedef struct flow_error_info {
    flow_status_code reason;
    flow_error_callstack_line callstack[14];
    int callstack_count;
    int callstack_capacity;
    bool locked;
    char message[FLOW_ERROR_MESSAGE_SIZE + 1];
} flow_error_info;

#ifdef EXPOSE_SIGMOID
/** flow_context: Colorspace */
typedef struct _SigmoidInfo {
    float constant;
    float x_coeff;
    float x_offset;
    float y_offset;
    float y_coeff;
} SigmoidInfo;

#endif

typedef struct flow_colorspace_info {
    float byte_to_float[256]; // Converts 0..255 -> 0..1, but knowing that 0.255 has sRGB gamma.
    flow_working_floatspace floatspace;
    bool apply_srgb;
    bool apply_gamma;
    float gamma;
    float gamma_inverse;
#ifdef EXPOSE_SIGMOID
    SigmoidInfo sigmoid;
    bool apply_sigmoid;
#endif

} flow_colorspace_info;

#define FLOW_USER_IS_OWNER
struct flow_heap_object_record {
    void* ptr;
    size_t bytes;
    void * owner;
    flow_destructor_function destructor;
    const char* allocated_by;
    int allocated_by_line;
    bool is_owner;
};
struct flow_objtracking_info {
    struct flow_heap_object_record* allocs;
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

typedef struct flow_ctx {
    flow_error_info error;
    struct flow_heap underlying_heap;
    flow_profiling_log log;
    flow_colorspace_info colorspace;
    struct flow_objtracking_info object_tracking;
} flow_context;

#include "color.h"


PUB bool flow_graph_walk_dependency_wise(flow_context* c, struct flow_job* job, struct flow_graph** graph_ref,
                                     flow_graph_visitor node_visitor, flow_graph_visitor edge_visitor,
                                     void* custom_data);



PUB flow_bitmap_float* flow_bitmap_float_create_header(flow_context* context, int sx, int sy, int channels);

PUB flow_bitmap_float* flow_bitmap_float_create(flow_context* context, int sx, int sy, int channels, bool zeroed);

PUB void flow_bitmap_float_destroy(flow_context* context, flow_bitmap_float* im);

PUB bool flow_bitmap_float_scale_rows(flow_context* context, flow_bitmap_float* from, uint32_t from_row,
                                      flow_bitmap_float* to, uint32_t to_row, uint32_t row_count,
                                      flow_interpolation_pixel_contributions* weights);
PUB bool flow_bitmap_float_convolve_rows(flow_context* context, flow_bitmap_float* buf, flow_convolution_kernel* kernel,
                                         uint32_t convolve_channels, uint32_t from_row, int row_count);

PUB bool flow_bitmap_float_sharpen_rows(flow_context* context, flow_bitmap_float* im, uint32_t start_row,
                                        uint32_t row_count, double pct);

PUB bool flow_bitmap_float_convert_srgb_to_linear(flow_context* context, flow_bitmap_bgra* src, uint32_t from_row,
                                                  flow_bitmap_float* dest, uint32_t dest_row, uint32_t row_count);

PUB uint32_t flow_bitmap_float_approx_gaussian_calculate_d(float sigma, uint32_t bitmap_width);

PUB uint32_t flow_bitmap_float_approx_gaussian_buffer_element_count_required(float sigma, uint32_t bitmap_width);

PUB bool flow_bitmap_float_approx_gaussian_blur_rows(flow_context* context, flow_bitmap_float* image, float sigma,
                                                     float* buffer, size_t buffer_element_count, uint32_t from_row,
                                                     int row_count);
PUB bool flow_bitmap_float_pivoting_composite_linear_over_srgb(flow_context* context, flow_bitmap_float* src,
                                                               uint32_t from_row, flow_bitmap_bgra* dest,
                                                               uint32_t dest_row, uint32_t row_count, bool transpose);

PUB bool flow_bitmap_float_flip_vertical(flow_context* context, flow_bitmap_bgra* b);

PUB bool flow_bitmap_float_demultiply_alpha(flow_context* context, flow_bitmap_float* src, const uint32_t from_row,
                                            const uint32_t row_count);

PUB bool flow_bitmap_float_copy_linear_over_srgb(flow_context* context, flow_bitmap_float* src, const uint32_t from_row,
                                                 flow_bitmap_bgra* dest, const uint32_t dest_row,
                                                 const uint32_t row_count, const uint32_t from_col,
                                                 const uint32_t col_count, const bool transpose);

PUB bool flow_halve(flow_context* context, const flow_bitmap_bgra* from, flow_bitmap_bgra* to, int divisor);

PUB bool flow_halve_in_place(flow_context* context, flow_bitmap_bgra* from, int divisor);


PUB void flow_utils_ensure_directory_exists(const char* dir_path);


struct flow_codec_definition;

// https://github.com/imazen/freeimage/blob/master/Source/FreeImage/FreeImageIO.cpp
// https://github.com/imazen/freeimage/blob/master/Source/FreeImage/PluginJPEG.cpp



    //shutdown
    //nature - memory, FILE *,


struct flow_codec_instance{
    int32_t graph_placeholder_id;
    size_t codec_id;
    void* codec_state;
    struct flow_io io;

};

struct flow_job_resource_item {
    struct flow_job_resource_item* next;
    int32_t id;
    int32_t graph_placeholder_id;
    FLOW_DIRECTION direction;
    flow_job_resource_type type;
    flow_codec_type codec_type;
    void* codec_state;
    void* data;
};

struct flow_job {
    int32_t debug_job_id;
    int32_t next_graph_version;
    int32_t next_resource_id;
    int32_t max_calc_flatten_execute_passes;
    struct flow_codec_instance * codecs;
    struct flow_job_resource_item* resources_head;
    struct flow_job_resource_item* resources_tail;
    bool record_graph_versions;
    bool record_frame_images;
    bool render_graph_versions;
    bool render_animated_graph;
    bool render_last_graph;
};


PUB bool flow_job_render_graph_to_png(flow_context* c, struct flow_job* job, struct flow_graph* g, int32_t graph_version);
PUB bool flow_job_notify_node_complete(flow_context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id);
PUB bool flow_job_initialize_input_resource(flow_context* c, struct flow_job* job, struct flow_job_resource_item* item);



#undef PUB

#ifndef _TIMERS_IMPLEMENTED
#define _TIMERS_IMPLEMENTED
#ifdef _WIN32
#define STRICT
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
