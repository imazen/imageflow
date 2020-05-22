#include "imageflow_private.h"

struct flow_heap_object_record * flow_objtracking_get_record_by_ptr(flow_c * context, void * ptr);

int64_t flow_objtracking_get_record_id_by_ptr(flow_c * context, void * ptr);

bool flow_objtracking_partial_destroy_by_record(flow_c * context, struct flow_heap_object_record * record,
                                                const char * file, int line);

static bool flow_objtracking_expand_record_array(flow_c * context, struct flow_objtracking_info * tracking)
{
    struct flow_heap * underlying_heap = &context->underlying_heap;
    size_t growth_factor = 2;
    size_t growth_divisor = 1;

    size_t new_size = (tracking->total_slots * growth_factor) / growth_divisor + 1;
    if (new_size < tracking->total_slots)
        new_size = tracking->total_slots;
    if (new_size < 64)
        new_size = 64;

    struct flow_heap_object_record * allocs = (struct flow_heap_object_record *)underlying_heap->_calloc(
        context, underlying_heap, new_size, sizeof(struct flow_heap_object_record), __FILE__, __LINE__);
    if (allocs == NULL) {
        FLOW_error(context, flow_status_Out_of_memory);
        return false;
    }

    struct flow_heap_object_record * old = tracking->allocs;
    if (old != NULL) {
        memcpy(allocs, old, tracking->total_slots * sizeof(struct flow_heap_object_record));
    }

    tracking->allocs = allocs;
    tracking->total_slots = new_size;
    if (old != NULL) {
        underlying_heap->_free(context, underlying_heap, old, __FILE__, __LINE__);
    }
    return true;
}

static void flow_objtracking_stats_update(flow_c * c, size_t allocs, size_t frees, size_t alloc_bytes,
                                          size_t free_bytes)
{
    struct flow_objtracking_info * tracking = &c->object_tracking;
    tracking->allocations_gross += allocs;
    tracking->allocations_net += allocs;
    tracking->allocations_net += frees;
    if (tracking->allocations_net_peak < tracking->allocations_net) {
        tracking->allocations_net_peak = tracking->allocations_net;
    }
    tracking->bytes_allocated_gross += alloc_bytes;
    tracking->bytes_allocated_net += alloc_bytes;
    tracking->bytes_allocated_net -= free_bytes;

    if (tracking->bytes_allocated_net_peak < tracking->bytes_allocated_net) {
        tracking->bytes_allocated_net_peak = tracking->bytes_allocated_net;
    }
    tracking->bytes_freed += free_bytes;
}

static bool flow_objtracking_add(flow_c * context, void * ptr, size_t byte_count, flow_destructor_function destructor,
                                 void * owner, const char * file, int line)
{
    struct flow_objtracking_info * tracking = &context->object_tracking;

    // Expand tracking list
    if (tracking->next_free_slot == tracking->total_slots) {
        if (!flow_objtracking_expand_record_array(context, tracking)) {
            FLOW_error_return(context);
        }
    }
    // Use the next slot
    struct flow_heap_object_record * next = &tracking->allocs[tracking->next_free_slot];
    if (next->ptr != NULL) {
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false;
    }
    next->allocated_by = file;
    next->allocated_by_line = line;
    next->bytes = byte_count;
    next->ptr = ptr;
    next->owner = owner;
    next->destructor_called = false;
    next->destructor = destructor;
    next->is_owner = false;

    // Update stats
    flow_objtracking_stats_update(context, 1, 0, byte_count, 0);

    // Mark the owner so the destructor is called.
    if (owner == NULL || owner == context) {
        // Do nothing
    } else {
        struct flow_heap_object_record * owner_record = flow_objtracking_get_record_by_ptr(context, owner);
        owner_record->is_owner = true;
    }

    // Loop to find the next slot
    for (size_t i = tracking->next_free_slot + 1; i < tracking->total_slots; i++) {
        if (tracking->allocs[i].ptr == NULL) {
            tracking->next_free_slot = i;
            return true;
        }
    }
    tracking->next_free_slot = tracking->total_slots;

    return true;
}

static size_t Context_size_of_context(flow_c * context)
{
    return context->object_tracking.total_slots * sizeof(struct flow_heap_object_record)
           + context->log.capacity * sizeof(struct flow_profiling_entry) + sizeof(struct flow_context);
}

void flow_context_print_memory_info(flow_c * context)
{
    size_t meta_bytes = Context_size_of_context(context);
    fprintf(stderr,
            "flow_context %p is using %zu bytes for metadata, %zu bytes for %zu allocations (total bytes %zu)\n",
            (void *)context, meta_bytes, context->object_tracking.bytes_allocated_net,
            context->object_tracking.allocations_net, context->object_tracking.bytes_allocated_net + meta_bytes);
    fprintf(stderr, "flow_context %p peak usage %zu bytes total, %zu allocations. %zu bytes from %zu allocations freed "
                    "explicitly\n",
            (void *)context, meta_bytes + context->object_tracking.bytes_allocated_net_peak,
            context->object_tracking.allocations_net_peak, context->object_tracking.bytes_freed,
            context->object_tracking.allocations_gross - context->object_tracking.allocations_net);
}

int64_t flow_objtracking_get_record_id_by_ptr(flow_c * context, void * ptr)
{
    struct flow_objtracking_info * tracking = &context->object_tracking;
    for (int64_t i = tracking->total_slots - 1; i >= 0; i--) {
        if (tracking->allocs[i].ptr == ptr) {
            return i;
        }
    }
    return -1;
}

// Searches record list for the given pointer. Returns NULL if not found.
struct flow_heap_object_record * flow_objtracking_get_record_by_ptr(flow_c * context, void * ptr)
{
    int64_t id = flow_objtracking_get_record_id_by_ptr(context, ptr);
    return id < 0 ? NULL : &context->object_tracking.allocs[id];
}

static void flow_objtracking_record_update(flow_c * context, struct flow_heap_object_record * record, void * new_ptr,
                                           size_t new_size, const char * file, int line)
{
    // Part a: update statistics
    flow_objtracking_stats_update(context, 1, 1, new_size, record->bytes);

    // part b: update record (we leave the owner and destructor as-is)
    record->ptr = new_ptr;
    record->bytes = new_size;
    record->allocated_by = file;
    record->allocated_by_line = line;
}

void * flow_context_calloc(flow_c * context, size_t instance_count, size_t instance_size,
                           flow_destructor_function destructor, void * owner, const char * file, int line)
{
    struct flow_heap * heap = &context->underlying_heap;

    void * ptr = heap->_calloc(context, heap, instance_count, instance_size, file, line);
    if (ptr == NULL)
        return NULL;
    if (!flow_objtracking_add(context, ptr, instance_count * instance_size, destructor, owner, file, line)) {
        heap->_free(context, heap, ptr, file, line);
        return NULL;
    }
    return ptr;
}

void * flow_context_malloc(flow_c * context, size_t byte_count, flow_destructor_function destructor, void * owner,
                           const char * file, int line)
{
    struct flow_heap * heap = &context->underlying_heap;
    void * ptr = context->underlying_heap._malloc(context, heap, byte_count, file, line);
    if (ptr == NULL)
        return NULL;
    if (!flow_objtracking_add(context, ptr, byte_count, destructor, owner, file, line)) {
        context->underlying_heap._free(context, heap, ptr, file, line);
        return NULL;
    }
    return ptr;
}

void * flow_context_realloc(flow_c * context, void * old_pointer, size_t new_byte_count, const char * file, int line)
{
    // Revert to malloc if old_pointer == NULL. We'll assume no destructor, and context as the default.
    if (old_pointer == NULL) {
        return flow_context_malloc(context, new_byte_count, NULL, context, file, line);
    }
    // Find the original record first, otherwise we set up a catch-22
    // If we call realloc before we find the record, but can't find the record
    // The old pointer has been freed and new memory allocated so we have no way of
    // indicating the error to the caller. We would have to silently ignore it
    int64_t record_id = flow_objtracking_get_record_id_by_ptr(context, old_pointer);
    if (record_id < 0) {
        FLOW_error_msg(context, flow_status_Invalid_argument,
                       "No record of the original pointer found - cannot realloc what we didn't alloc");
        return NULL;
    }

    void * ptr = context->underlying_heap._realloc(context, &context->underlying_heap, old_pointer, new_byte_count,
                                                   file, line);
    if (ptr == NULL) {
        // NOTHING HAS CHANGED - ORIGINAL MEMORY STILL VALID. OOM appropriate, as per default behavior.
        return NULL;
    }
    flow_objtracking_record_update(context, &context->object_tracking.allocs[record_id], ptr, new_byte_count, file,
                                   line);
    return ptr;
}

static bool flow_objtracking_call_destructor(flow_c * context, struct flow_heap_object_record * record)
{
    if (record->destructor != NULL && record->ptr != NULL && !record->destructor_called) {
        record->destructor_called = true;
        if (!record->destructor(context, record->ptr)) {
            if (!flow_context_has_error(context)) {
                // Raise the error if the destructor was too lazy, but returned false
                FLOW_error_msg(context, flow_status_Other_error, "Destructor returned false, indicating failure");
                flow_context_add_to_callstack(context, record->allocated_by, record->allocated_by_line,
                                              "MEMORY ALLOCATED BY");
            } else {
                flow_context_add_to_callstack(context, record->allocated_by, record->allocated_by_line,
                                              "MEMORY ALLOCATED BY");
                FLOW_add_to_callstack(context);
            }
            return false;
        }
    }
    return true;
}

static bool flow_call_destructors_recursive(flow_c * context, void * owner, const char * file, int line)
{
    struct flow_heap_object_record * records = &context->object_tracking.allocs[0];
    bool success = true;
    for (size_t i = 0; i < context->object_tracking.total_slots; i++) {
        if (records[i].ptr != NULL && records[i].owner == owner) {
            struct flow_heap_object_record * record = &records[i];

            // Step 1. Call child destructors recursively
            if (record->is_owner) {
                if (!flow_call_destructors_recursive(context, record->ptr, file, line)) {
                    FLOW_add_to_callstack(context);
                    success = false;
                }
            }
            // Step 2. Call destructor
            if (!flow_objtracking_call_destructor(context, record)) {
                FLOW_add_to_callstack(context);
                success = false;
            }
        }
    }
    return success;
}

bool flow_objtracking_partial_destroy_by_record(flow_c * context, struct flow_heap_object_record * record,
                                                const char * file, int line)
{
    if (record->ptr == NULL) {
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false; // WTF? You shouldn't call this method on an empty record
    }
    bool success = true;
    struct flow_heap * heap = &context->underlying_heap;

    // Step 1. Call child destructors (depth first)
    if (record->is_owner && !flow_call_destructors_recursive(context, record->ptr, file, line)) {
        FLOW_add_to_callstack(context);
        success = false;
    }

    // Step 1. Call destructor
    if (!flow_objtracking_call_destructor(context, record)) {
        FLOW_add_to_callstack(context);
        success = false;
    }

    // Step 2. Destroy owned objects recursively
    if (record->is_owner) {
        if (!flow_destroy_by_owner(context, record->ptr, file, line)) {
            FLOW_add_to_callstack(context);
            success = false;
        }
    }

    // Step 3. Free bytes
    heap->_free(context, heap, record->ptr, file, line);

    // Step 4, update stats
    flow_objtracking_stats_update(context, 0, 1, 0, record->bytes);

    // Step 5. Clear record
    record->allocated_by = NULL;
    record->allocated_by_line = 0;
    record->bytes = 0;
    record->ptr = NULL;
    record->destructor = NULL;
    record->owner = NULL;

    return success;
}

bool flow_destroy_by_owner(flow_c * context, void * owner, const char * file, int line)
{
    struct flow_heap_object_record * records = &context->object_tracking.allocs[0];
    bool success = true;
    for (size_t i = 0; i < context->object_tracking.total_slots; i++) {
        if (records[i].ptr != NULL && records[i].owner == owner) {
            if (!flow_objtracking_partial_destroy_by_record(context, &records[i], file, line)) {
                success = false;
            }
            if (context->object_tracking.next_free_slot > i) {
                context->object_tracking.next_free_slot = i;
            }
        }
    }
    return success;
}

bool flow_set_destructor(flow_c * c, void * thing, flow_destructor_function destructor)
{
    if (thing == NULL) {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }
    struct flow_heap_object_record * records = &c->object_tracking.allocs[0];
    for (size_t i = 0; i < c->object_tracking.total_slots; i++) {
        if (records[i].ptr == thing) {
            records[i].destructor = destructor;
            return true;
        }
    }
    FLOW_error(c, flow_status_Item_does_not_exist);
    return false;
}

// Thing will only be automatically destroyed and freed at the time that owner is destroyed and freed
bool flow_set_owner(flow_c * c, void * thing, void * owner)
{
    if (thing == NULL) {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }
    struct flow_heap_object_record * records = &c->object_tracking.allocs[0];
    for (size_t i = 0; i < c->object_tracking.total_slots; i++) {
        if (records[i].ptr == thing) {
            records[i].owner = owner;
            return true;
        }
    }
    FLOW_error(c, flow_status_Item_does_not_exist);
    return false;
}

bool flow_destroy(flow_c * context, void * pointer, const char * file, int line)
{
    if (pointer == NULL)
        return true;

    // Different code path for the context itself...
    if (context == pointer) {
        flow_context_destroy(context);
        return true;
    }

    int64_t record_id = flow_objtracking_get_record_id_by_ptr(context, pointer);
    if (record_id < 0) {
        FLOW_error_msg(context, flow_status_Invalid_argument,
                       "You are trying to destroy an item that the context has no record of.");
        return false;
    }
    bool success
        = flow_objtracking_partial_destroy_by_record(context, &context->object_tracking.allocs[record_id], file, line);
    if (context->object_tracking.next_free_slot > (size_t)record_id) {
        context->object_tracking.next_free_slot = record_id;
    }
    return success;
}

void flow_deprecated_free(flow_c * context, void * pointer, const char * file, int line)
{
    if (pointer == NULL)
        return;
    int64_t record_id = flow_objtracking_get_record_id_by_ptr(context, pointer);
    if (record_id < 0) {
        FLOW_error_msg(context, flow_status_Invalid_argument,
                       "You are trying to destroy an item that the context has no record of.");
        exit(404);
    }
    if (context->object_tracking.allocs[record_id].is_owner
        || context->object_tracking.allocs[record_id].destructor != NULL) {
        FLOW_error_msg(context, flow_status_Invalid_argument, "FLOW_free is deprecated; use FLOW_destroy instead - "
                                                              "this item has child objects or a destructor and cannot "
                                                              "be used with FLOW_free");
        exit(404);
    }
    if (!flow_objtracking_partial_destroy_by_record(context, &context->object_tracking.allocs[record_id], file, line)) {
        FLOW_add_to_callstack(context);
        exit(405);
    }
    if (context->object_tracking.next_free_slot > (size_t)record_id) {
        context->object_tracking.next_free_slot = record_id;
    }
}

/***********************************************************/

// Simple underlying heap system

void * flow_heap_get_private_state(struct flow_heap * heap)
{
    if (heap == NULL)
        return NULL;
    return heap->_private_state;
}

bool flow_heap_set_private_state(struct flow_heap * heap, void * private_state)
{
    if (heap == NULL)
        return false;
    heap->_private_state = private_state;
    return true;
}

bool flow_heap_set_custom(flow_c * context, flow_heap_calloc_function calloc, flow_heap_malloc_function malloc,
                          flow_heap_realloc_function realloc, flow_heap_free_function free,
                          flow_heap_terminate_function terminate, void * initial_private_state)
{
    struct flow_heap * heap = &context->underlying_heap;
    heap->_calloc = calloc;
    heap->_malloc = malloc;
    heap->_realloc = realloc;
    heap->_free = free;
    heap->_context_terminate = terminate;
    heap->_private_state = initial_private_state;
    return true;
}

static void * f_default_calloc(struct flow_context * context, struct flow_heap * heap, size_t count,
                               size_t element_size, const char * file, int line)
{ 
 #ifdef _MSC_VER   
    // there is no _aligned_calloc so we ensure alignment via _aligned_malloc
    const size_t total =  count * element_size;
    if (total / element_size != count) {
        FLOW_error(context, flow_status_Invalid_argument);
        return NULL;
     }
    void *const ptr = _aligned_malloc(total, 16);
    if (ptr) memset(ptr, 0, total);
    return ptr;
 #else
    return calloc(count, element_size);
 #endif // _MSC_VER 
}

static void * f_default_malloc(struct flow_context * context, struct flow_heap * heap, size_t byte_count,
                               const char * file, int line)
{
#ifdef _MSC_VER
    return _aligned_malloc(byte_count, 16);
#else
    return malloc(byte_count); 
#endif // _MSC_VER
}

static void * f_default_realloc(struct flow_context * context, struct flow_heap * heap, void * old_pointer,
                                size_t new_byte_count, const char * file, int line)
{
#ifdef _MSC_VER
    return _aligned_realloc(old_pointer, new_byte_count, 16);
#else
    return realloc(old_pointer, new_byte_count);
#endif // _MSC_VER
}

static void f_default_free(struct flow_context * context, struct flow_heap * heap, void * pointer, const char * file,
                           int line)
{
    // fprintf(stdout, "Freeing %p\n", pointer);
#ifdef _MSC_VER    
    _aligned_free(pointer);
#else
    free(pointer);
#endif // _MSC_VER
}

bool flow_heap_set_default(flow_c * context)
{
    struct flow_heap * heap = &context->underlying_heap;
    heap->_calloc = f_default_calloc;
    heap->_malloc = f_default_malloc;
    heap->_free = f_default_free;
    heap->_realloc = f_default_realloc;
    heap->_context_terminate = NULL;
    heap->_private_state = NULL;
    return true;
}

void flow_context_objtracking_initialize(struct flow_objtracking_info * heap_tracking)
{
    heap_tracking->total_slots = 0;
    heap_tracking->next_free_slot = 0;
    heap_tracking->allocations_gross = 0;
    heap_tracking->allocations_net = 0;
    heap_tracking->allocs = NULL;
    heap_tracking->bytes_allocated_gross = 0;
    heap_tracking->bytes_allocated_net = 0;
    heap_tracking->allocations_net_peak = 0;
    heap_tracking->bytes_allocated_net_peak = 0;
    heap_tracking->bytes_freed = 0;
}

void flow_context_objtracking_terminate(flow_c * context)
{
    if (context->object_tracking.allocs != NULL) {
        context->underlying_heap._free(context, &context->underlying_heap, context->object_tracking.allocs, __FILE__,
                                       __LINE__);
    }
    flow_context_objtracking_initialize(&context->object_tracking);
}
