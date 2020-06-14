#include "imageflow_private.h"

int flow_vsnprintf(char * s, size_t n, const char * fmt, va_list v)
{
    if (n == 0) {
        return -1; // because MSFT _vsn_printf_s will crash if you pass it zero for buffer size.
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

int flow_snprintf(char * s, size_t n, const char * fmt, ...)
{
    int res;
    va_list v;
    va_start(v, fmt);
    res = flow_vsnprintf(s, n, fmt, v);
    va_end(v);
    return res;
}

int flow_context_error_reason(flow_c * context) { return context->error.reason; }

bool flow_context_raise_error(flow_c * context, flow_status_code code, char * message, const char * file, int line,
                              const char * function_name)
{
    char * buffer = flow_context_set_error_get_message_buffer(context, code, file, line, function_name);
    if (context->error.locked == true) {
        return false; // There was already another error reported, and uncleared.
    }
    if (message != NULL) {
        flow_snprintf(buffer, FLOW_ERROR_MESSAGE_SIZE, "%s", message);
    }
    return true;
}

char * flow_context_set_error_get_message_buffer(flow_c * context, flow_status_code code, const char * file, int line,
                                                 const char * function_name)
{
    // We can't return an invalid buffer, even if an error has already been logged.
    static char throwaway_buffer[FLOW_ERROR_MESSAGE_SIZE + 1];
    if (context->error.reason != flow_status_No_Error) {
        // The last error wasn't cleared, lock it down. We prefer the original error.
        context->error.locked = true;
        return &throwaway_buffer[0];
    }
    if (code == flow_status_No_Error) {
        context->error.reason = flow_status_Other_error;
    } else {
        context->error.reason = code;
    }
    flow_context_add_to_callstack(context, file, line, function_name);
    return &context->error.message[0];
}

// Returns true if the operation succeeded
// Does not add to call stack
bool flow_context_set_error_get_message_buffer_info(flow_c * context, flow_status_code code,
                                                    bool status_included_in_buffer, char ** buffer,
                                                    size_t * buffer_size)
{
    if (context->error.reason != flow_status_No_Error) {
        // The last error wasn't cleared, lock it down. We prefer the original error.
        context->error.locked = true;
        *buffer = 0;
        *buffer_size = 0;
        return false;
    } else {
        context->error.status_included_in_message = status_included_in_buffer;
        if (code == flow_status_No_Error) {
            context->error.reason = flow_status_Other_error;
        } else {
            context->error.reason = code;
        }
        *buffer = &context->error.message[0];
        *buffer_size = FLOW_ERROR_MESSAGE_SIZE;
        return true;
    }
}

bool flow_context_add_to_callstack(flow_c * context, const char * file, int line, const char * function_name)
{
    if (context->error.callstack_count < context->error.callstack_capacity && !context->error.locked
        && context->error.reason != flow_status_No_Error) {
        context->error.callstack[context->error.callstack_count].file = file;
        context->error.callstack[context->error.callstack_count].line = line;
        context->error.callstack[context->error.callstack_count].function_name = function_name;
        context->error.callstack_count++;
        context->error.status_included_in_message = false;
        return true;
    }
    return false;
}

void flow_context_clear_error(flow_c * context)
{
    context->error.callstack_count = 0;
    context->error.callstack[0].file = NULL;
    context->error.callstack[0].line = -1;
    context->error.callstack[0].function_name = NULL;
    context->error.reason = flow_status_No_Error;
    context->error.locked = false;
    context->error.status_included_in_message = false;
    context->error.message[0] = 0;
}

bool flow_context_has_error(flow_c * context) { return context->error.reason != flow_status_No_Error; }

static const char * status_code_to_string(flow_status_code code)
{
    // Code is an unsigned enum, and cannot be negative. We just check the upper bounds
    if (code >= flow_status_First_user_defined_error && code <= flow_status_Last_user_defined_error) {
        return "User defined error";
    }
    switch (code) {
        case 0:
            return "No error";
        case 10:
            return "Out Of Memory";
        case 20:
            return "I/O Error";
        case 30:
            return "Internal state invalid";
        case 31:
            return "Internal panic (please file a bug report)";
        case 40:
            return "Not implemented";
        case 50:
            return "Invalid argument";
        case 51:
            return "Null argument";
        case 52:
            return "Invalid dimensions";
        case 53:
            return "Pixel format unsupported by algorithm";
        case 54:
            return "Item does not exist";
        case 60:
            return "Image decoding failed";
        case 61:
            return "Image encoding failed";
        case 90:
            return "C Error Reporting Inconsistency";
        case 1024:
            return "Other error";
        default:
            if (code >= flow_status_First_rust_error && code < flow_status_Other_error) {
                return "Rust status code";
            } else {
                return "Unknown status code";
            }
    }
}

bool flow_context_print_and_exit_if_err(flow_c * c)
{
    if (flow_context_has_error(c)) {
        flow_context_print_error_to(c, stderr);
        return true;
    }
    return false;
}

void flow_context_print_error_to(flow_c * c, FILE * stream)
{
    char buffer[FLOW_ERROR_MESSAGE_SIZE + 2048];
    flow_context_error_and_stacktrace(c, buffer, sizeof(buffer), true);
    fprintf(stream, "%s", buffer);
    fflush(stream);
}
int64_t flow_context_error_and_stacktrace(flow_c * context, char * buffer, size_t buffer_size, bool full_file_path)
{
    if (buffer == NULL)
        return -1;

    size_t original_buffer_size = buffer_size;
    int64_t chars_written = flow_context_error_message(context, buffer, buffer_size);
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
bool flow_context_error_status_included_in_message(flow_c * context)
{
    return context->error.status_included_in_message;
}

int64_t flow_context_error_message(flow_c * context, char * buffer, size_t buffer_size)
{
    int chars_written = 0;
    const char * reason_str = status_code_to_string(context->error.reason);
    if (context->error.reason == flow_status_No_Error) {
        chars_written = flow_snprintf(buffer, buffer_size, "%s", reason_str);
    } else {
        if (context->error.status_included_in_message == true) {
            if (context->error.message[0] == 0) {
                // This branch shouldn't happen
                chars_written = flow_snprintf(buffer, buffer_size, "CError of Rust Error %d - message missing",
                                              (int)context->error.reason - 200);
            } else {
                chars_written = flow_snprintf(buffer, buffer_size, "%s", context->error.message);
            }
        } else {
            if (context->error.message[0] == 0) {
                chars_written = flow_snprintf(buffer, buffer_size, "CError %d: %s", context->error.reason, reason_str);
            } else {
                chars_written = flow_snprintf(buffer, buffer_size, "CError %d: %s : %s", context->error.reason,
                                              reason_str, context->error.message);
            }
        }
    }
    if (chars_written < 0) {
        return -1; // we ran out of space
    }
    return chars_written;
}

int64_t flow_context_stacktrace(flow_c * context, char * buffer, size_t buffer_size, bool full_file_path)
{

    // Test with function_name = NULL

    size_t remaining_space = buffer_size; // For null character
    char * line = buffer;
    for (int i = 0; i < context->error.callstack_count; i++) {

        // Trim the directory
        const char * file = context->error.callstack[i].file;
        if (file == NULL) {
            file = "(unknown)";
        } else {
            const char * lastslash = (const char *)umax64((uint64_t)strrchr(file, '\\'), (uint64_t)strrchr(file, '/'));
            if (!full_file_path) {
                file = (const char *)umax64((uint64_t)lastslash + 1, (uint64_t)file);
            }
        }
        const char * func_name = context->error.callstack[i].function_name == NULL
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


void flow_context_initialize(flow_c * context)
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
    flow_heap_set_default(context);
    flow_context_objtracking_initialize(&context->object_tracking);
}

flow_c * flow_context_create(void)
{
    flow_c * c = (flow_c *)malloc(sizeof(flow_c));
    if (c != NULL) {
        flow_context_initialize(c);
    }
    return c;
}

size_t flow_context_sizeof_context_struct() { return sizeof(struct flow_context); }

// One can call begin_terminate to do the error-possible things, yet later check the heap status, remaining allocations,
// and error count
// Later, you can call flow_context_destroy()
bool flow_context_begin_terminate(flow_c * context)
{
    if (context == NULL)
        return true;

    bool success = true;
    if (!flow_destroy_by_owner(context, context, __FILE__, __LINE__)) {
        FLOW_add_to_callstack(context);
        success = false;
    }
    context->log.log = NULL; // We allocated .log with FLOW_malloc. It's freed with everything else
    return success;
}

void flow_context_end_terminate(flow_c * context)
{
    if (context == NULL)
        return;

    flow_context_objtracking_terminate(context);

    if (context->underlying_heap._context_terminate != NULL) {
        context->underlying_heap._context_terminate(context, &context->underlying_heap);
    }
}

void flow_context_terminate(flow_c * context)
{
    flow_context_begin_terminate(context);
    flow_context_end_terminate(context);
}

void flow_context_destroy(flow_c * context)
{
    flow_context_terminate(context);
    free(context);
}

bool flow_context_enable_profiling(flow_c * context, uint32_t default_capacity)
{
    if (context->log.log == NULL) {
        context->log.log = (struct flow_profiling_entry *)FLOW_malloc(context, sizeof(struct flow_profiling_entry)
                                                                               * default_capacity);
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

void flow_context_profiler_start(flow_c * context, const char * name, bool allow_recursion)
{
    if (context->log.log == NULL)
        return;
    struct flow_profiling_entry * current = &(context->log.log[context->log.count]);
    context->log.count++;
    if (context->log.count >= context->log.capacity)
        return;

    current->time = flow_get_high_precision_ticks();
    current->name = name;
    current->flags = allow_recursion ? flow_profiling_entry_start_allow_recursion : flow_profiling_entry_start;
}

void flow_context_profiler_stop(flow_c * context, const char * name, bool assert_started, bool stop_children)
{
    if (context->log.log == NULL)
        return;
    struct flow_profiling_entry * current = &(context->log.log[context->log.count]);
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

struct flow_profiling_log * flow_context_get_profiler_log(flow_c * context) { return &context->log; }

void flow_sanity_check(struct flow_sanity_check * info)
{
    info->sizeof_bool = sizeof(bool);
    info->sizeof_int = sizeof(int);
    info->sizeof_size_t = sizeof(size_t);
}
