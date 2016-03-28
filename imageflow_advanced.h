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

// Portable snprintf
PUB int flow_snprintf(char* s, size_t n, const char* fmt, ...);
PUB int flow_vsnprintf(char* s, size_t n, const char* fmt, va_list v);

////////////////////////////////////////////
// You can control the underlying heap if you want

typedef void* (*flow_heap_calloc_function)(struct flow_ctx* context, struct flow_heap* heap, size_t count,
                                           size_t element_size, const char* file, int line);
typedef void* (*flow_heap_malloc_function)(struct flow_ctx* context, struct flow_heap* heap, size_t byte_count,
                                           const char* file, int line);

typedef void* (*flow_heap_realloc_function)(struct flow_ctx* context, struct flow_heap* heap, void* old_pointer,
                                            size_t new_byte_count, const char* file, int line);
typedef void (*flow_heap_free_function)(struct flow_ctx* context, struct flow_heap* heap, void* pointer,
                                        const char* file, int line);
typedef void (*flow_heap_terminate_function)(struct flow_ctx* context, struct flow_heap* heap);

PUB void* flow_heap_get_private_state(struct flow_heap* heap);
PUB bool flow_heap_set_private_state(struct flow_heap* heap, void* private_state);

PUB bool flow_heap_set_default(flow_context* context);
PUB bool flow_heap_set_custom(flow_context* context, flow_heap_calloc_function calloc, flow_heap_malloc_function malloc,
                              flow_heap_realloc_function realloc, flow_heap_free_function free,
                              flow_heap_terminate_function terminate, void* initial_private_state);

PUB bool flow_set_destructor(flow_context* context, void* thing, flow_destructor_function* destructor);

// Thing will only be destroyed and freed at the time that owner is destroyed and freed
PUB bool flow_set_owner(flow_context* context, void* thing, void* owner);

PUB void* flow_context_calloc(flow_context* context, size_t instance_count, size_t instance_size,
                              flow_destructor_function destructor, void* owner, const char* file, int line);
PUB void* flow_context_malloc(flow_context* context, size_t byte_count, flow_destructor_function destructor,
                              void* owner, const char* file, int line);
PUB void* flow_context_realloc(flow_context* context, void* old_pointer, size_t new_byte_count, const char* file,
                               int line);
PUB void flow_deprecated_free(flow_context* context, void* pointer, const char* file, int line);
PUB bool flow_destroy_by_owner(flow_context* context, void* owner, const char* file, int line);
PUB bool flow_destroy(flow_context* context, void* pointer, const char* file, int line);

#define FLOW_calloc(context, instance_count, element_size)                                                             \
    flow_context_calloc(context, instance_count, element_size, NULL, context, __FILE__, __LINE__)
#define FLOW_calloc_array(context, instance_count, type_name)                                                          \
    (type_name*) flow_context_calloc(context, instance_count, sizeof(type_name), NULL, context, __FILE__, __LINE__)
#define FLOW_malloc(context, byte_count) flow_context_malloc(context, byte_count, NULL, context, __FILE__, __LINE__)

#define FLOW_calloc_owned(context, instance_count, element_size, owner)                                                \
    flow_context_calloc(context, instance_count, element_size, NULL, owner, __FILE__, __LINE__)
#define FLOW_calloc_array_owned(context, instance_count, type_name, owner)                                             \
    (type_name*) flow_context_calloc(context, instance_count, sizeof(type_name), NULL, owner, __FILE__, __LINE__)
#define FLOW_malloc_owned(context, byte_count, owner)                                                                  \
    flow_context_malloc(context, byte_count, NULL, owner, __FILE__, __LINE__)

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

typedef enum flow_profiling_entry_flags {
    flow_profiling_entry_start = 2,
    flow_profiling_entry_start_allow_recursion = 6,
    flow_profiling_entry_stop = 8,
    flow_profiling_entry_stop_assert_started = 24,
    flow_profiling_entry_stop_children = 56
} flow_profiling_entry_flags;

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

PUB flow_profiling_log* flow_context_get_profiler_log(flow_context* context);

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

struct flow_io;

// Returns the number of read into the buffer. Failure to read 'count' bytes could mean EOF or failure. Check context
// status. Pass NULL to buffer if you want to skip 'count' many bytes, seeking ahead.
typedef int64_t (*flow_io_read_function)(flow_context* c, struct flow_io* io, uint8_t* buffer, size_t count);
// Returns the number of bytes written. If it doesn't equal 'count', there was an error. Check context status
typedef int64_t (*flow_io_write_function)(flow_context* c, struct flow_io* io, uint8_t* buffer, size_t count);

// Returns negative on failure - check context for more detail. Returns the current position in the stream when
// successful
typedef int64_t (*flow_io_position_function)(flow_context* c, struct flow_io* io);

// Returns true if seek was successful.
typedef bool (*flow_io_seek_function)(flow_context* c, struct flow_io* io, int64_t position);

#undef PUB
#ifdef __cplusplus
}
#endif
