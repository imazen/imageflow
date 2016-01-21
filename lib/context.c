#include "fastscaling_private.h"
#include <stdio.h>
#include <string.h>

#ifdef _MSC_VER
#pragma unmanaged
#endif


int Context_error_reason(Context * context)
{
    return context->error.reason;
}

void Context_set_last_error(Context * context, StatusCode code, const char * file, int line)
{
    context->error.reason = code;
    Context_add_to_callstack(context, file,line);
#ifdef DEBUG
    char buffer[1024];
    fprintf(stderr, "%s:%d Context_set_last_error the error registered was: %s\n", file, line, Context_error_message(context, buffer, sizeof(buffer)));
#endif
}

void Context_add_to_callstack(Context * context, const char * file, int line)
{
    if (context->error.callstack_count < context->error.callstack_capacity) {
        context->error.callstack[context->error.callstack_count].file = file;
        context->error.callstack[context->error.callstack_count].line = line;
        context->error.callstack_count++;
    }
}


bool Context_has_error(Context * context)
{
    return context->error.reason != No_Error;
}

const char * TheStatus = "Status code lookup not implemented";
static const char * status_code_to_string(StatusCode code)
{
    return TheStatus;
}

const char * Context_error_message(Context * context, char * buffer, size_t buffer_size)
{
    snprintf(buffer, buffer_size, "Error in file: %s line: %d status_code: %d reason: %s", context->error.callstack[0].file, context->error.callstack[0].line, context->error.reason, status_code_to_string(context->error.reason));

    return buffer;
}

const char * Context_stacktrace (Context * context, char * buffer, size_t buffer_size)
{
    size_t remaining_space = buffer_size - 1; //For null character
    char * line = buffer;
    for (int i = 0; i < context->error.callstack_count; i++){

        //Trim the directory
        const char * file = context->error.callstack[i].file;
        const char * lastslash = (const char *)umax64((uint64_t)strchr (file, '\\'), (uint64_t)strchr (file, '/'));
        file = (const char *)umax64((uint64_t)lastslash + 1, (uint64_t)file);

        uint32_t used = snprintf (line, remaining_space, "%s: line %d\n", file , context->error.callstack[i].line);
        if (used > 0 && used < remaining_space){
            remaining_space -= used;
            line += used;
        }else{
            return buffer;
        }
    }
    return buffer;
}


void * Context_calloc(Context * context, size_t instance_count, size_t instance_size, const char * file, int line)
{
#ifdef DEBUG
    fprintf(stderr, "%s:%d calloc of %zu * %zu bytes\n", file, line, instance_count, instance_size);
#endif
    return context->heap._calloc(context, instance_count, instance_size, file, line);
}

void * Context_malloc(Context * context, size_t byte_count, const char * file, int line)
{
#ifdef DEBUG
    fprintf(stderr, "%s:%d malloc of %zu bytes\n", file, line, byte_count);
#endif
    return context->heap._malloc(context, byte_count, file, line);
}

void Context_free(Context * context, void * pointer, const char * file, int line)
{
    context->heap._free(context, pointer, file, line);
}

void Context_free_static_caches(void)
{

}

static void * DefaultHeapManager_calloc(struct ContextStruct * context, size_t count, size_t element_size, const char * file, int line)
{
    return calloc(count, element_size);
}
static void * DefaultHeapManager_malloc(struct ContextStruct * context, size_t byte_count, const char * file, int line)
{
    return malloc(byte_count);
}
static void  DefaultHeapManager_free(struct ContextStruct * context, void * pointer, const char * file, int line)
{
    free(pointer);
}

void DefaultHeapManager_initialize(HeapManager * manager)
{
    manager->_calloc = DefaultHeapManager_calloc;
    manager->_malloc = DefaultHeapManager_malloc;
    manager->_free = DefaultHeapManager_free;
    manager->_context_terminate = NULL;
}

void Context_initialize(Context * context)
{
    context->log.log = NULL;
    context->log.capacity = 0;
    context->log.count = 0;
    context->error.callstack_capacity = 8;
    context->error.callstack_count = 0;
    context->error.callstack[0].file = NULL;
    context->error.callstack[0].line = -1;
    //memset(context->error.callstack, 0, sizeof context->error.callstack);
    context->error.reason = No_Error;
    DefaultHeapManager_initialize(&context->heap);
    Context_set_floatspace (context, Floatspace_as_is, 0.0f, 0.0f, 0.0f);
}

Context * Context_create(void)
{
    Context * c = (Context *)malloc(sizeof(Context));
    if (c != NULL) {
        Context_initialize(c);
    }
    return c;
}

void Context_terminate(Context * context)
{
    if (context != NULL) {
        if (context->heap._context_terminate != NULL) {
            context->heap._context_terminate(context);
        }
        CONTEXT_free(context, context->log.log);
    }
}
void Context_destroy(Context * context)
{
    Context_terminate(context);
    free(context);
}

bool Context_enable_profiling(Context * context, uint32_t default_capacity)
{
    if (context->log.log == NULL) {
        context->log.log = (ProfilingEntry *)CONTEXT_malloc(context, sizeof(ProfilingEntry) * default_capacity);
        if (context->log.log == NULL) {
            CONTEXT_error(context, Out_of_memory);
            return false;
        } else {
            context->log.capacity = default_capacity;
            context->log.count = 0;
        }
        context->log.ticks_per_second = get_profiler_ticks_per_second();


    } else {
        //TODO: grow and copy array
        CONTEXT_error (context, Invalid_internal_state);
        return false;
    }
    return true;
}

void Context_profiler_start(Context * context, const char * name, bool allow_recursion)
{
    if (context->log.log == NULL) return;
    ProfilingEntry * current = &(context->log.log[context->log.count]);
    context->log.count++;
    if (context->log.count >= context->log.capacity) return;

    current->time =get_high_precision_ticks();
    current->name = name;
    current->flags = allow_recursion ? Profiling_start_allow_recursion : Profiling_start;
}

void Context_profiler_stop(Context * context, const char * name, bool assert_started, bool stop_children)
{
    if (context->log.log == NULL) return;
    ProfilingEntry * current = &(context->log.log[context->log.count]);
    context->log.count++;
    if (context->log.count >= context->log.capacity) return;

    current->time =get_high_precision_ticks();
    current->name = name;
    current->flags = assert_started ? Profiling_stop_assert_started : Profiling_stop;
    if (stop_children) {
        current->flags = (ProfilingEntryFlags)(current->flags | Profiling_stop_children);
    }
}


ProfilingLog * Context_get_profiler_log(Context * context)
{
    return &context->log;
}


/* Aligned allocations

#include <stdlib.h>
#include "malloc.h"

#define ir_malloc(size) _aligned_malloc(size, 32)
#define ir_free(ptr) _aligned_free(ptr)


_declspec(noalias) _declspec(restrict) inline void* _ir_aligned_calloc(size_t count, size_t elsize, size_t alignment){
    if (elsize == 0 || count >= SIZE_MAX / elsize) { return NULL; } // Watch out for overflow
    size_t size = count * elsize;
    void *memory = _aligned_malloc(size, alignment);
    if (memory != NULL) { memset(memory, 0, size); }
    return memory;
}

#define ir_calloc(count, element_size) _ir_aligned_calloc(count,element_size, 32)
#else
#define ir_malloc(size) malloc(size)
#define ir_free(ptr) free(ptr)
#define ir_calloc(count, element_size) calloc(count,element_size)
#endif
*/

