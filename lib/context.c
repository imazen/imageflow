#include "fastscaling_private.h"
#include <stdio.h>
#include <string.h>
#include <nathanaeljones/imageflow/fastscaling.h>

#ifdef _MSC_VER
#pragma unmanaged
#endif


int Context_error_reason(Context * context)
{
    return context->error.reason;
}

void Context_set_last_error(Context * context, StatusCode code, const char * file, int line)
{
    if (context->error.reason != No_Error){
        //The last error wasn't cleared, lock it down. We prefer the original error.
        context->error.locked = true;
        return;
    }
    context->error.reason = code;
    Context_add_to_callstack(context, file,line);
#ifdef DEBUG
    char buffer[1024];
    fprintf(stderr, "%s:%d Context_set_last_error the error registered was: %s\n", file, line, Context_error_message(context, buffer, sizeof(buffer)));
#endif
}

void Context_add_to_callstack(Context * context, const char * file, int line)
{
    if (context->error.callstack_count < context->error.callstack_capacity && !context->error.locked) {
        context->error.callstack[context->error.callstack_count].file = file;
        context->error.callstack[context->error.callstack_count].line = line;
        context->error.callstack_count++;
    }
}

static size_t Context_size_of_context(Context * context){
    return context->heap_tracking.total_slots * sizeof(struct HeapAllocation) + context->log.capacity * sizeof(ProfilingEntry) + sizeof(struct ContextStruct);
}

void Context_clear_error(Context * context){
    context->error.callstack_count = 0;
    context->error.callstack[0].file = NULL;
    context->error.callstack[0].line = -1;
    context->error.reason = No_Error;
    context->error.locked = false;
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

bool Context_print_and_exit_if_err(Context * c){
    if (Context_has_error(c)){
        Context_print_error_to(c, stderr);
        return true;
    }
    return false;
}

void Context_print_error_to(  Context * c, FILE * stream){
    char buffer[1024];
    fprintf(stream, "Error code %d: %s\n", c->error.reason, status_code_to_string(c->error.reason));
    fprintf(stream, "%s\n", Context_stacktrace(c, buffer, sizeof(buffer)));
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
        const char * lastslash = (const char *)umax64((uint64_t)strrchr (file, '\\'), (uint64_t)strrchr (file, '/'));
        file = (const char *)umax64((uint64_t)lastslash + 1, (uint64_t)file);

        uint32_t used = snprintf (line, remaining_space, "%s:%d:\n", file , context->error.callstack[i].line);
        if (used > 0 && used < remaining_space){
            remaining_space -= used;
            line += used;
        }else{
            return buffer;
        }
    }
    return buffer;
}

static bool expand_heap_tracking(Context * context){
    size_t growth_factor = 2;
    size_t growth_divisor = 1;

    size_t new_size = (context->heap_tracking.total_slots * growth_factor) / growth_divisor + 1;
    if (new_size < context->heap_tracking.total_slots) new_size = context->heap_tracking.total_slots;
    if (new_size < 64) new_size = 64;

    struct HeapAllocation * allocs = (struct HeapAllocation *)context->heap._calloc(context, new_size, sizeof(struct HeapAllocation),__FILE__, __LINE__);
    if (allocs == NULL){
        CONTEXT_error(context, Out_of_memory);
        return false;
    }


    struct HeapAllocation * old = context->heap_tracking.allocs;
    if (old != NULL) {
        memcpy(allocs, old,
               context->heap_tracking.total_slots * sizeof(struct HeapAllocation));
    }

    context->heap_tracking.allocs = allocs;
    context->heap_tracking.total_slots = new_size;
    if (old != NULL) {
        context->heap._free(context, old, __FILE__, __LINE__);
    }
    return true;
}

static bool Context_memory_track(Context * context, void * ptr, size_t byte_count, const char * file, int line){
    if (context->heap_tracking.next_free_slot == context->heap_tracking.total_slots){
        if (!expand_heap_tracking(context)){
            CONTEXT_error_return(context);
        }
    }
    struct HeapAllocation * next = &context->heap_tracking.allocs[context->heap_tracking.next_free_slot];
    if (next->ptr != NULL){
        CONTEXT_error(context, Invalid_internal_state);
        return false;
    }
    next->allocated_by = file;
    next->allocated_by_line = line;
    next->bytes = byte_count;
    next->ptr = ptr;
    context->heap_tracking.allocations_gross++;
    context->heap_tracking.allocations_net++;
    if (context->heap_tracking.allocations_net_peak < context->heap_tracking.allocations_net){
        context->heap_tracking.allocations_net_peak = context->heap_tracking.allocations_net;
    }
    context->heap_tracking.bytes_allocated_gross += byte_count;
    context->heap_tracking.bytes_allocated_net += byte_count;

    if (context->heap_tracking.bytes_allocated_net_peak < context->heap_tracking.bytes_allocated_net){
        context->heap_tracking.bytes_allocated_net_peak = context->heap_tracking.bytes_allocated_net;
    }

    for (size_t i = context->heap_tracking.next_free_slot + 1; i < context->heap_tracking.total_slots; i++){
        if (context->heap_tracking.allocs[i].ptr == NULL){
            context->heap_tracking.next_free_slot = i;
            return true;
        }
    }
    context->heap_tracking.next_free_slot = context->heap_tracking.total_slots;
    return true;
}
static void Context_memory_untrack(Context * context, void * ptr, const char * file, int line){
    for (int64_t i = context->heap_tracking.total_slots - 1; i >= 0; i--){
        if (context->heap_tracking.allocs[i].ptr == ptr){
            struct HeapAllocation * alloc = &context->heap_tracking.allocs[i];

            context->heap_tracking.allocations_net--;
            context->heap_tracking.bytes_allocated_net -= alloc->bytes;
            context->heap_tracking.bytes_freed += alloc->bytes;
            alloc->ptr = NULL;
            alloc->bytes = 0;
            alloc->allocated_by = NULL;
            alloc->allocated_by_line = 0;

            //Only seek backwards, so we always point to the first.
            if ((int64_t)context->heap_tracking.next_free_slot > i){
                context->heap_tracking.next_free_slot = (int64_t)i;
            }

            return;
        }
    }
    //TODO: failed to untrack?? warning??
#ifdef DEBUG
    fprintf(stderr, "%s:%d Failed to untrack memory allocated at %zu bytes\n", file, line, ptr);
#endif


}

 void Context_print_memory_info(Context * context){
    size_t meta_bytes = Context_size_of_context(context);
    fprintf(stderr, "Context %p is using %zu bytes for metadata, %zu bytes for %zu allocations (total bytes %zu)\n", (void *)context, meta_bytes, context->heap_tracking.bytes_allocated_net, context->heap_tracking.allocations_net, context->heap_tracking.bytes_allocated_net + meta_bytes);
    fprintf(stderr, "Context %p peak usage %zu bytes total, %zu allocations. %zu bytes from %zu allocations freed explicitly\n", (void *)context, meta_bytes + context->heap_tracking.bytes_allocated_net_peak, context->heap_tracking.allocations_net_peak, context->heap_tracking.bytes_freed, context->heap_tracking.allocations_gross - context->heap_tracking.allocations_net);

}
void Context_free_allocated_memory(Context * context){

//    fprintf(stderr, "Context_free_allocated_memory:\n");
//    Context_print_memory_info(context);
    for (size_t i = 0; i < context->heap_tracking.total_slots; i++){
        if (context->heap_tracking.allocs[i].ptr != NULL){


            context->heap._free(context, context->heap_tracking.allocs[i].ptr, __FILE__, __LINE__);


            struct HeapAllocation * alloc = &context->heap_tracking.allocs[i];
            //fprintf(stderr, "Freed %zu bytes at %p, allocated at %s:%u\n",alloc->bytes,  alloc->ptr, alloc->allocated_by, alloc->allocated_by_line);

            context->heap_tracking.allocations_net--;
            context->heap_tracking.bytes_allocated_net -= alloc->bytes;
            alloc->ptr = NULL;
            alloc->bytes = 0;
            alloc->allocated_by = NULL;
            alloc->allocated_by_line = 0;

            //Only seek backwards, so we always point to the first.
            if (context->heap_tracking.next_free_slot > i){
                context->heap_tracking.next_free_slot = i;
            }
        }
    }
    if (context->heap_tracking.allocations_net != 0 ||
            context->heap_tracking.bytes_allocated_net != 0){
        fprintf(stderr, "Failed to deallocated %zu bytes", context->heap_tracking.bytes_allocated_net);

    }

}

void * Context_calloc(Context * context, size_t instance_count, size_t instance_size, const char * file, int line)
{
#ifdef DEBUG
    fprintf(stderr, "%s:%d calloc of %zu * %zu bytes\n", file, line, instance_count, instance_size);
#endif

    void * ptr = context->heap._calloc(context, instance_count, instance_size, file, line);
    if (ptr == NULL) return NULL;
    if (!Context_memory_track(context, ptr, instance_count * instance_size, file, line)){
        context->heap._free(context, ptr, file, line);
        return NULL;
    }
    return ptr;
}

void * Context_malloc(Context * context, size_t byte_count, const char * file, int line)
{
#ifdef DEBUG
    fprintf(stderr, "%s:%d malloc of %zu bytes\n", file, line, byte_count);
#endif
    void * ptr = context->heap._malloc(context, byte_count, file, line);
    if (ptr == NULL) return NULL;
    if (!Context_memory_track(context, ptr, byte_count, file, line)){
        context->heap._free(context, ptr, file, line);
        return NULL;
    }
    return ptr;
}


void * Context_realloc(Context * context, void * old_pointer, size_t new_byte_count, const char * file, int line)
{
#ifdef DEBUG
    fprintf(stderr, "%s:%d realloc of %zu bytes\n", file, line, byte_count);
#endif
    void * ptr = context->heap._realloc(context, old_pointer, new_byte_count, file, line);
    if (ptr == NULL) return NULL;
    Context_memory_untrack(context, old_pointer, __FILE__, __LINE__);
    if (!Context_memory_track(context, ptr, new_byte_count, file, line)){
        context->heap._free(context, ptr, file, line);
        return NULL;
    }
    return ptr;
}

void Context_free(Context * context, void * pointer, const char * file, int line)
{
    if (pointer == NULL) return;
    Context_memory_untrack(context, pointer, file, line);
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
static void * DefaultHeapManager_realloc(struct ContextStruct * context, void * old_pointer, size_t new_byte_count, const char * file, int line)
{
    return realloc(old_pointer, new_byte_count);
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
    manager->_realloc = DefaultHeapManager_realloc;
    manager->_context_terminate = NULL;
}


static void Context_heap_tracking_initialize(Context * context){

    context->heap_tracking.total_slots = 0;
    context->heap_tracking.next_free_slot = 0;
    context->heap_tracking.allocations_gross = 0;
    context->heap_tracking.allocations_net  =0;
    context->heap_tracking.allocs = NULL;
    context->heap_tracking.bytes_allocated_gross = 0;
    context->heap_tracking.bytes_allocated_net = 0;
    context->heap_tracking.allocations_net_peak = 0;
    context->heap_tracking.bytes_allocated_net_peak = 0;
    context->heap_tracking.bytes_freed = 0;
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
    context->error.locked = false;
    DefaultHeapManager_initialize(&context->heap);
    Context_heap_tracking_initialize(context);
    Context_set_floatspace (context, Floatspace_as_is, 0.0f, 0.0f, 0.0f);
}

static void Context_heap_tracking_terminate(Context *context){

    if (context->heap_tracking.allocs != NULL){
        context->heap._free(context, context->heap_tracking.allocs, __FILE__, __LINE__);
    }
    Context_heap_tracking_initialize(context);
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
        }else{
            Context_free_allocated_memory(context);
        }
        Context_heap_tracking_terminate(context);

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

