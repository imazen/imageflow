#include "catch.hpp"
#include "imageflow_private.h"

extern "C" void keep3() {}
const int FLOW_MAX_BYTES_PP = 16;

static std::ostream & operator<<(std::ostream & out, const struct flow_bitmap_float & bitmap_float)
{
    return out << "flow_bitmap_float: w:" << bitmap_float.w << " h: " << bitmap_float.h
               << " channels:" << bitmap_float.channels << '\n';
}

class Fixture {
public:
    size_t last_attempted_allocation_size;
    bool always_return_null;
    size_t allocation_failure_size_threshold;
    size_t allocation_failure_size;
    int allow_successful_allocs;
    int alloc_count;
    int total_successful_allocs;

    static void * _calloc(flow_c * context, struct flow_heap * heap, size_t count, size_t element_size,
                          const char * file, int line)
    {
        return ((Fixture *)context->underlying_heap._private_state)->calloc(count, element_size);
    }
    static void * _malloc(flow_c * context, struct flow_heap * heap, size_t byte_count, const char * file, int line)
    {
        return ((Fixture *)context->underlying_heap._private_state)->malloc(byte_count);
    }
    static void _free(flow_c * context, struct flow_heap * heap, void * pointer, const char * file, int line)
    {
        free(pointer);
    }

    void initialize_heap(flow_c * context)
    {
        context->underlying_heap._private_state = this;
        context->underlying_heap._calloc = _calloc;
        context->underlying_heap._malloc = _malloc;
        context->underlying_heap._free = _free;
        context->underlying_heap._context_terminate = NULL;
    }
    Fixture()
    {
        always_return_null = false;
        allocation_failure_size_threshold = INT_MAX / 4;
        allocation_failure_size = allocation_failure_size_threshold;
        allow_successful_allocs = INT_MAX;
        last_attempted_allocation_size = -1;
        alloc_count = 0;
        total_successful_allocs = 0;
    }

    bool is_alloc_allowed(size_t byte_count)
    {
        last_attempted_allocation_size = byte_count;
        if (always_return_null) {
            return false;
        }
        if (allocation_failure_size_threshold < last_attempted_allocation_size) {
            return false;
        }
        if (allocation_failure_size == last_attempted_allocation_size) {
            return false;
        }
        if (alloc_count >= allow_successful_allocs) {
            return false;
        }
        alloc_count++;
        total_successful_allocs++;
        return true;
    }
    void * malloc(size_t byte_count)
    {
        if (is_alloc_allowed(byte_count)) {
            return ::malloc(byte_count);
        }
        return NULL;
    }

    void * calloc(size_t instances, size_t size_of_instance)
    {
        if (is_alloc_allowed(instances * size_of_instance)) {
            return ::calloc(instances, size_of_instance);
        }
        return NULL;
    }

    void always_fail_allocation() { always_return_null = true; }

    void fail_allocation_of(size_t byte_count) { allocation_failure_size = byte_count; }

    void fail_allocation_if_size_larger_than(size_t byte_count) { allocation_failure_size_threshold = byte_count; }

    void fail_alloc_after(int times)
    {
        alloc_count = 0;
        allow_successful_allocs = times;
    }
};

TEST_CASE_METHOD(Fixture, "Creating flow_bitmap_bgra", "[error_handling]")
{
    flow_c context;
    flow_context_initialize(&context);
    initialize_heap(&context);

    struct flow_bitmap_bgra * source = NULL;
    // Create something so object_tracking is initialized
    source = flow_bitmap_bgra_create(&context, 1, 1, true, flow_bgra32);
    SECTION("Creating a 1x1 bitmap is valid")
    {
        source = flow_bitmap_bgra_create(&context, 1, 1, true, flow_bgra32);
        REQUIRE_FALSE(source == NULL);
        REQUIRE_FALSE(flow_context_has_error(&context));
    }
    SECTION("A 0x0 bitmap is invalid")
    {
        source = flow_bitmap_bgra_create(&context, 0, 0, true, flow_bgra32);
        REQUIRE(source == NULL);
        REQUIRE(flow_context_has_error(&context));
        REQUIRE(flow_context_error_reason(&context) == flow_status_Invalid_dimensions);
        // REQUIRE(flow_context_error_message(&context));
    }
    SECTION("A gargantuan bitmap is also invalid")
    {
        source = flow_bitmap_bgra_create(&context, 1, INT_MAX, true, flow_bgra32);
        REQUIRE(source == NULL);
        REQUIRE(flow_context_has_error(&context));
        REQUIRE(flow_context_error_reason(&context) == flow_status_Invalid_dimensions);
    }

    SECTION("A bitmap that exhausts memory is invalid too")
    {
        always_fail_allocation();
        source = flow_bitmap_bgra_create(&context, 1, 1, true, flow_bgra32);
        REQUIRE(source == NULL);
        REQUIRE(flow_context_has_error(&context));
        REQUIRE(flow_context_error_reason(&context) == flow_status_Out_of_memory);
    }
    SECTION("Exhausting memory in the pixel allocation is also handled")
    {
        fail_allocation_if_size_larger_than(sizeof(struct flow_bitmap_bgra));
        source = flow_bitmap_bgra_create(&context, 640, 480, true, flow_bgra32);
        REQUIRE(source == NULL);
        REQUIRE(last_attempted_allocation_size == 640 * 480 * 4); // the failed allocation tried to allocate the pixels
        REQUIRE(flow_context_has_error(&context));
        REQUIRE(flow_context_error_reason(&context) == flow_status_Out_of_memory);
    }
    REQUIRE(flow_context_begin_terminate(&context) == true);
    flow_context_end_terminate(&context);
}

TEST_CASE("flow_context", "[error_handling]")
{
    flow_c context;
    flow_context_initialize(&context);

    SECTION("flow_context_error_message should be safe when no error has ocurred yet")
    {
        char error_msg[1024];
        flow_context_error_message(&context, error_msg, sizeof error_msg);
        REQUIRE(std::string("No error") == error_msg);

        REQUIRE(flow_context_begin_terminate(&context) == true);
        flow_context_end_terminate(&context);
    }
}

TEST_CASE("Argument checking for convert_sgrp_to_linear", "[error_handling]")
{
    flow_c context;
    flow_context_initialize(&context);
    struct flow_bitmap_bgra * src = flow_bitmap_bgra_create(&context, 2, 3, true, flow_bgra32);
    char error_msg[1024];
    flow_context_error_message(&context, error_msg, sizeof error_msg);
    CAPTURE(error_msg);
    REQUIRE(src != NULL);
    struct flow_bitmap_float * dest = flow_bitmap_float_create(&context, 1, 1, 4, false);
    flow_colorcontext_info colorcontext;
    flow_colorcontext_init(&context, &colorcontext, flow_working_floatspace_linear, 0, 0, 0);
    flow_bitmap_float_convert_srgb_to_linear(&context, &colorcontext, src, 3, dest, 0, 0);
    flow_bitmap_bgra_destroy(&context, src);
    CAPTURE(*dest);
    REQUIRE(dest->float_count == 16); // 1x1x4 channels
    flow_bitmap_float_destroy(&context, dest);
    REQUIRE(flow_context_begin_terminate(&context) == true);
    flow_context_end_terminate(&context);
}

TEST_CASE("Test stacktrace serialization", "[error_handling]")
{
    flow_c context;
    flow_context_initialize(&context);
    FLOW_error(&context, flow_status_Out_of_memory);
    FLOW_add_to_callstack(&context);
    FLOW_add_to_callstack(&context);
    FLOW_add_to_callstack(&context);
    FLOW_add_to_callstack(&context);
    FLOW_add_to_callstack(&context);
    FLOW_add_to_callstack(&context);
    FLOW_add_to_callstack(&context);
    FLOW_add_to_callstack(&context);
    FLOW_add_to_callstack(&context);
    FLOW_add_to_callstack(&context);
    FLOW_add_to_callstack(&context);

    int stacktrace_buffer_size = 8 * 32;

    char * stacktrace = (char *)malloc(stacktrace_buffer_size);
    flow_context_stacktrace(&context, stacktrace, stacktrace_buffer_size, false);

    CAPTURE(stacktrace_buffer_size);
    CAPTURE(stacktrace);

    free(stacktrace);

    REQUIRE(flow_context_begin_terminate(&context) == true);
    flow_context_end_terminate(&context);
}
