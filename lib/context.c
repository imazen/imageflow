#include "imageflow_private.h"
#include <stdio.h>
#include <string.h>
#include <stdarg.h>

int flow_vsnprintf(char* s, size_t n, const char* fmt, va_list v)
{
    if (n == 0) {
        return -1; //because MSFT _vsn_printf_s will crash if you pass it zero for buffer size.
    }
    int res;
#ifdef _WIN32
    // Could use "_vsnprintf_s(s, n, _TRUNCATE, fmt, v)" ?
    res = _vsnprintf_s(s, n, _TRUNCATE, fmt, v);
#else
    res = vsnprintf(s, n, fmt, v);
#endif
    if (n)
        s[n - 1] = 0;
    // Unix returns length output would require, Windows returns negative when truncated.
    return (res >= (int)n || res < 0) ? -1 : res;
}

int flow_snprintf(char* s, size_t n, const char* fmt, ...)
{
    int res;
    va_list v;
    va_start(v, fmt);
    res = flow_vsnprintf(s, n, fmt, v);
    va_end(v);
    return res;
}

int flow_context_error_reason(flow_context* context) { return context->error.reason; }

void flow_context_raise_error(flow_context* context, flow_status_code code, char* message, const char* file, int line,
                              const char* function_name)
{
    char* buffer = flow_context_set_error_get_message_buffer(context, code, file, line, function_name);
    if (message != NULL) {
        flow_snprintf(buffer, FLOW_ERROR_MESSAGE_SIZE, "%s", message);
    }
}

char* flow_context_set_error_get_message_buffer(flow_context* context, flow_status_code code, const char* file,
                                                int line, const char* function_name)
{
    // We can't return an invalid buffer, even if an error has already been logged.
    static char throwaway_buffer[FLOW_ERROR_MESSAGE_SIZE + 1];
    if (context->error.reason != flow_status_No_Error) {
        // The last error wasn't cleared, lock it down. We prefer the original error.
        context->error.locked = true;
        return &throwaway_buffer[0];
    }
    context->error.reason = code;
    flow_context_add_to_callstack(context, file, line, function_name);
    return &context->error.message[0];
}

void flow_context_add_to_callstack(flow_context* context, const char* file, int line, const char* function_name)
{
    if (context->error.callstack_count < context->error.callstack_capacity && !context->error.locked) {
        context->error.callstack[context->error.callstack_count].file = file;
        context->error.callstack[context->error.callstack_count].line = line;
        context->error.callstack[context->error.callstack_count].function_name = function_name;
        context->error.callstack_count++;
    }
}

static size_t Context_size_of_context(flow_context* context)
{
    return context->heap_tracking.total_slots * sizeof(struct flow_heap_allocation)
           + context->log.capacity * sizeof(flow_profiling_entry) + sizeof(struct flow_context_struct);
}

void flow_context_clear_error(flow_context* context)
{
    context->error.callstack_count = 0;
    context->error.callstack[0].file = NULL;
    context->error.callstack[0].line = -1;
    context->error.callstack[0].function_name = NULL;
    context->error.reason = flow_status_No_Error;
    context->error.locked = false;
    context->error.message[0] = 0;
}

bool flow_context_has_error(flow_context* context) { return context->error.reason != flow_status_No_Error; }

static const char* status_code_to_string(flow_status_code code)
{
    // Code is an unsigned enum, and cannot be negative. We just check the upper bounds
    if (code >= flow_status_First_user_defined_error && code <= flow_status_Last_user_defined_error) {
        return "User defined error";
    }
    if (code >= flow_status____Last_library_error) {
        return "Unknown status code";
    }
    return flow_status_code_strings[code];
}

bool flow_context_print_and_exit_if_err(flow_context* c)
{
    if (flow_context_has_error(c)) {
        flow_context_print_error_to(c, stderr);
        return true;
    }
    return false;
}

void flow_context_print_error_to(flow_context* c, FILE* stream)
{
    char buffer[FLOW_ERROR_MESSAGE_SIZE + 2048];
    flow_context_error_message(c, buffer, sizeof(buffer));
    fprintf(stream, "%s", buffer);
}
int32_t flow_context_error_and_stacktrace(flow_context* context, char* buffer, size_t buffer_size, bool full_file_path)
{
    size_t original_buffer_size = buffer_size;
    int chars_written = flow_context_error_message(context, buffer, buffer_size);
    if (chars_written < 0) {
        return -1; // we ran out of space
    } else {
        buffer = buffer + chars_written;
        buffer_size -= chars_written;
    }

    if (context->error.callstack_count > 0) {
        chars_written = flow_snprintf(buffer, buffer_size, "\n");
        if (chars_written < 0) {
            return -1; // we ran out of space
        } else {
            buffer = buffer + chars_written;
            buffer_size -= chars_written;
        }
    }

    chars_written = flow_context_stacktrace(context, buffer, buffer_size, full_file_path);
    if (chars_written < 0) {
        return -1; // we ran out of space
    } else {
        buffer = buffer + chars_written;
        buffer_size -= chars_written;
    }

    return original_buffer_size - buffer_size;
}

int32_t flow_context_error_message(flow_context* context, char* buffer, size_t buffer_size)
{
    int chars_written = 0;
    if (context->error.message[0] == 0) {
        chars_written = flow_snprintf(buffer, buffer_size, "%s", status_code_to_string(context->error.reason));
    } else {
        chars_written = flow_snprintf(buffer, buffer_size, "%s : %s", status_code_to_string(context->error.reason),
                                      context->error.message);
    }
    if (chars_written < 0) {
        return -1; // we ran out of space
    }
    return chars_written;
}

int32_t flow_context_stacktrace(flow_context* context, char* buffer, size_t buffer_size, bool full_file_path)
{

    // Test with function_name = NULL

    size_t remaining_space = buffer_size; // For null character
    char* line = buffer;
    for (int i = 0; i < context->error.callstack_count; i++) {

        // Trim the directory
        const char* file = context->error.callstack[i].file;
        if (file == NULL) {
            file = "(unknown)";
        } else {
            const char* lastslash = (const char*)umax64((uint64_t)strrchr(file, '\\'), (uint64_t)strrchr(file, '/'));
            if (!full_file_path) {
                file = (const char*)umax64((uint64_t)lastslash + 1, (uint64_t)file);
            }
        }
        const char* func_name = context->error.callstack[i].function_name == NULL
                                    ? "(unknown)"
                                    : context->error.callstack[i].function_name;

        int32_t used = flow_snprintf(line, remaining_space, "%s:%d: in function %s\n", file,
                                     context->error.callstack[i].line, func_name);
        if (used < 0) {
            return -1;
        } else {
            remaining_space -= used;
            line += used;
        }
    }
    return buffer_size - remaining_space;
}

static bool expand_heap_tracking(flow_context* context)
{
    size_t growth_factor = 2;
    size_t growth_divisor = 1;

    size_t new_size = (context->heap_tracking.total_slots * growth_factor) / growth_divisor + 1;
    if (new_size < context->heap_tracking.total_slots)
        new_size = context->heap_tracking.total_slots;
    if (new_size < 64)
        new_size = 64;

    struct flow_heap_allocation* allocs = (struct flow_heap_allocation*)context->heap._calloc(
        context, new_size, sizeof(struct flow_heap_allocation), __FILE__, __LINE__);
    if (allocs == NULL) {
        FLOW_error(context, flow_status_Out_of_memory);
        return false;
    }

    struct flow_heap_allocation* old = context->heap_tracking.allocs;
    if (old != NULL) {
        memcpy(allocs, old, context->heap_tracking.total_slots * sizeof(struct flow_heap_allocation));
    }

    context->heap_tracking.allocs = allocs;
    context->heap_tracking.total_slots = new_size;
    if (old != NULL) {
        context->heap._free(context, old, __FILE__, __LINE__);
    }
    return true;
}

static bool Context_memory_track(flow_context* context, void* ptr, size_t byte_count, const char* file, int line)
{
    if (context->heap_tracking.next_free_slot == context->heap_tracking.total_slots) {
        if (!expand_heap_tracking(context)) {
            FLOW_error_return(context);
        }
    }
    struct flow_heap_allocation* next = &context->heap_tracking.allocs[context->heap_tracking.next_free_slot];
    if (next->ptr != NULL) {
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false;
    }
    next->allocated_by = file;
    next->allocated_by_line = line;
    next->bytes = byte_count;
    next->ptr = ptr;
    context->heap_tracking.allocations_gross++;
    context->heap_tracking.allocations_net++;
    if (context->heap_tracking.allocations_net_peak < context->heap_tracking.allocations_net) {
        context->heap_tracking.allocations_net_peak = context->heap_tracking.allocations_net;
    }
    context->heap_tracking.bytes_allocated_gross += byte_count;
    context->heap_tracking.bytes_allocated_net += byte_count;

    if (context->heap_tracking.bytes_allocated_net_peak < context->heap_tracking.bytes_allocated_net) {
        context->heap_tracking.bytes_allocated_net_peak = context->heap_tracking.bytes_allocated_net;
    }

    for (size_t i = context->heap_tracking.next_free_slot + 1; i < context->heap_tracking.total_slots; i++) {
        if (context->heap_tracking.allocs[i].ptr == NULL) {
            context->heap_tracking.next_free_slot = i;
            return true;
        }
    }
    context->heap_tracking.next_free_slot = context->heap_tracking.total_slots;
    return true;
}
static void Context_memory_untrack(flow_context* context, void* ptr, const char* file, int line)
{
    for (int64_t i = context->heap_tracking.total_slots - 1; i >= 0; i--) {
        if (context->heap_tracking.allocs[i].ptr == ptr) {
            struct flow_heap_allocation* alloc = &context->heap_tracking.allocs[i];

            context->heap_tracking.allocations_net--;
            context->heap_tracking.bytes_allocated_net -= alloc->bytes;
            context->heap_tracking.bytes_freed += alloc->bytes;
            alloc->ptr = NULL;
            alloc->bytes = 0;
            alloc->allocated_by = NULL;
            alloc->allocated_by_line = 0;

            // Only seek backwards, so we always point to the first.
            if ((int64_t)context->heap_tracking.next_free_slot > i) {
                context->heap_tracking.next_free_slot = (int64_t)i;
            }

            return;
        }
    }
// TODO: failed to untrack?? warning??
#ifdef DEBUG
    fprintf(stderr, "%s:%d Failed to untrack memory allocated at %zu bytes\n", file, line, ptr);
#endif
}

void flow_context_print_memory_info(flow_context* context)
{
    size_t meta_bytes = Context_size_of_context(context);
    fprintf(stderr,
            "flow_context %p is using %zu bytes for metadata, %zu bytes for %zu allocations (total bytes %zu)\n",
            (void*)context, meta_bytes, context->heap_tracking.bytes_allocated_net,
            context->heap_tracking.allocations_net, context->heap_tracking.bytes_allocated_net + meta_bytes);
    fprintf(stderr, "flow_context %p peak usage %zu bytes total, %zu allocations. %zu bytes from %zu allocations freed "
                    "explicitly\n",
            (void*)context, meta_bytes + context->heap_tracking.bytes_allocated_net_peak,
            context->heap_tracking.allocations_net_peak, context->heap_tracking.bytes_freed,
            context->heap_tracking.allocations_gross - context->heap_tracking.allocations_net);
}
void flow_context_free_all_allocations(flow_context* context)
{

    //    fprintf(stderr, "flow_context_free_all_allocations:\n");
    //    flow_context_print_memory_info(context);
    for (size_t i = 0; i < context->heap_tracking.total_slots; i++) {
        if (context->heap_tracking.allocs[i].ptr != NULL) {

            struct flow_heap_allocation* alloc = &context->heap_tracking.allocs[i];

            // Uncomment to debug double-frees
            // fprintf(stderr, "Freeing %zu bytes at %p, allocated at %s:%u\n",alloc->bytes,  alloc->ptr,
            // alloc->allocated_by, alloc->allocated_by_line);

            context->heap._free(context, context->heap_tracking.allocs[i].ptr, __FILE__, __LINE__);

            context->heap_tracking.allocations_net--;
            context->heap_tracking.bytes_allocated_net -= alloc->bytes;
            alloc->ptr = NULL;
            alloc->bytes = 0;
            alloc->allocated_by = NULL;
            alloc->allocated_by_line = 0;

            // Only seek backwards, so we always point to the first.
            if (context->heap_tracking.next_free_slot > i) {
                context->heap_tracking.next_free_slot = i;
            }
        }
    }
    if (context->heap_tracking.allocations_net != 0 || context->heap_tracking.bytes_allocated_net != 0) {
        fprintf(stderr, "Failed to deallocate %zu allocations (%zu bytes)", context->heap_tracking.allocations_net,
                context->heap_tracking.bytes_allocated_net);
    }
}

void* flow_context_calloc(flow_context* context, size_t instance_count, size_t instance_size, const char* file,
                          int line)
{
#ifdef DEBUG
    fprintf(stderr, "%s:%d calloc of %zu * %zu bytes\n", file, line, instance_count, instance_size);
#endif

    void* ptr = context->heap._calloc(context, instance_count, instance_size, file, line);
    if (ptr == NULL)
        return NULL;
    if (!Context_memory_track(context, ptr, instance_count * instance_size, file, line)) {
        context->heap._free(context, ptr, file, line);
        return NULL;
    }
    return ptr;
}

void* flow_context_malloc(flow_context* context, size_t byte_count, const char* file, int line)
{
#ifdef DEBUG
    fprintf(stderr, "%s:%d malloc of %zu bytes\n", file, line, byte_count);
#endif
    void* ptr = context->heap._malloc(context, byte_count, file, line);
    if (ptr == NULL)
        return NULL;
    if (!Context_memory_track(context, ptr, byte_count, file, line)) {
        context->heap._free(context, ptr, file, line);
        return NULL;
    }
    return ptr;
}

void* flow_context_realloc(flow_context* context, void* old_pointer, size_t new_byte_count, const char* file, int line)
{
#ifdef DEBUG
    fprintf(stderr, "%s:%d realloc of %zu bytes\n", file, line, byte_count);
#endif
    void* ptr = context->heap._realloc(context, old_pointer, new_byte_count, file, line);
    if (ptr == NULL)
        return NULL;
    Context_memory_untrack(context, old_pointer, __FILE__, __LINE__);
    if (!Context_memory_track(context, ptr, new_byte_count, file, line)) {
        context->heap._free(context, ptr, file, line);
        return NULL;
    }
    return ptr;
}

void flow_context_free(flow_context* context, void* pointer, const char* file, int line)
{
    if (pointer == NULL)
        return;
    Context_memory_untrack(context, pointer, file, line);
    context->heap._free(context, pointer, file, line);
}

void flow_context_free_static_caches(void) {}

static void* DefaultHeapManager_calloc(struct flow_context_struct* context, size_t count, size_t element_size,
                                       const char* file, int line)
{
    return calloc(count, element_size);
}
static void* DefaultHeapManager_malloc(struct flow_context_struct* context, size_t byte_count, const char* file,
                                       int line)
{
    return malloc(byte_count);
}
static void* DefaultHeapManager_realloc(struct flow_context_struct* context, void* old_pointer, size_t new_byte_count,
                                        const char* file, int line)
{
    return realloc(old_pointer, new_byte_count);
}
static void DefaultHeapManager_free(struct flow_context_struct* context, void* pointer, const char* file, int line)
{
    free(pointer);
}

void flow_default_heap_manager_initialize(flow_heap_manager* manager)
{
    manager->_calloc = DefaultHeapManager_calloc;
    manager->_malloc = DefaultHeapManager_malloc;
    manager->_free = DefaultHeapManager_free;
    manager->_realloc = DefaultHeapManager_realloc;
    manager->_context_terminate = NULL;
}

static void Context_heap_tracking_initialize(flow_context* context)
{

    context->heap_tracking.total_slots = 0;
    context->heap_tracking.next_free_slot = 0;
    context->heap_tracking.allocations_gross = 0;
    context->heap_tracking.allocations_net = 0;
    context->heap_tracking.allocs = NULL;
    context->heap_tracking.bytes_allocated_gross = 0;
    context->heap_tracking.bytes_allocated_net = 0;
    context->heap_tracking.allocations_net_peak = 0;
    context->heap_tracking.bytes_allocated_net_peak = 0;
    context->heap_tracking.bytes_freed = 0;
}

void flow_context_initialize(flow_context* context)
{
    context->log.log = NULL;
    context->log.capacity = 0;
    context->log.count = 0;
    context->error.callstack_capacity = 8;
    context->error.callstack_count = 0;
    context->error.callstack[0].file = NULL;
    context->error.callstack[0].line = -1;
    context->error.callstack[0].function_name = NULL;
    // memset(context->error.callstack, 0, sizeof context->error.callstack);
    context->error.reason = flow_status_No_Error;
    context->error.message[0] = 0;
    context->error.locked = false;
    flow_default_heap_manager_initialize(&context->heap);
    Context_heap_tracking_initialize(context);
    flow_context_set_floatspace(context, flow_working_floatspace_as_is, 0.0f, 0.0f, 0.0f);
}

static void Context_heap_tracking_terminate(flow_context* context)
{

    if (context->heap_tracking.allocs != NULL) {
        context->heap._free(context, context->heap_tracking.allocs, __FILE__, __LINE__);
    }
    Context_heap_tracking_initialize(context);
}

flow_context* flow_context_create(void)
{
    flow_context* c = (flow_context*)malloc(sizeof(flow_context));
    if (c != NULL) {
        flow_context_initialize(c);
    }
    return c;
}

void flow_context_terminate(flow_context* context)
{
    if (context != NULL) {
        if (context->heap._context_terminate != NULL) {
            context->heap._context_terminate(context);
        } else {
            flow_context_free_all_allocations(context);
        }
        context->log.log = NULL; // We allocated .log with FLOW_malloc. It's freed with everything else
        Context_heap_tracking_terminate(context);
    }
}
void flow_context_destroy(flow_context* context)
{
    flow_context_terminate(context);
    free(context);
}

bool flow_context_enable_profiling(flow_context* context, uint32_t default_capacity)
{
    if (context->log.log == NULL) {
        context->log.log = (flow_profiling_entry*)FLOW_malloc(context, sizeof(flow_profiling_entry) * default_capacity);
        if (context->log.log == NULL) {
            FLOW_error(context, flow_status_Out_of_memory);
            return false;
        } else {
            context->log.capacity = default_capacity;
            context->log.count = 0;
        }
        context->log.ticks_per_second = flow_get_profiler_ticks_per_second();

    } else {
        // TODO: grow and copy array
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false;
    }
    return true;
}

void flow_context_profiler_start(flow_context* context, const char* name, bool allow_recursion)
{
    if (context->log.log == NULL)
        return;
    flow_profiling_entry* current = &(context->log.log[context->log.count]);
    context->log.count++;
    if (context->log.count >= context->log.capacity)
        return;

    current->time = flow_get_high_precision_ticks();
    current->name = name;
    current->flags = allow_recursion ? flow_profiling_entry_start_allow_recursion : flow_profiling_entry_start;
}

void flow_context_profiler_stop(flow_context* context, const char* name, bool assert_started, bool stop_children)
{
    if (context->log.log == NULL)
        return;
    flow_profiling_entry* current = &(context->log.log[context->log.count]);
    context->log.count++;
    if (context->log.count >= context->log.capacity)
        return;

    current->time = flow_get_high_precision_ticks();
    current->name = name;
    current->flags = assert_started ? flow_profiling_entry_stop_assert_started : flow_profiling_entry_stop;
    if (stop_children) {
        current->flags = (flow_profiling_entry_flags)(current->flags | flow_profiling_entry_stop_children);
    }
}

flow_profiling_log* flow_context_get_profiler_log(flow_context* context) { return &context->log; }
