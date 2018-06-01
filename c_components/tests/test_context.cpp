#include "catch.hpp"
#include "imageflow_private.h"
#include "helpers.h"

extern "C" void keep1() {}

TEST_CASE("Verify .cpp and .c files are being compiled with compatible type sizes")
{
    struct flow_sanity_check info;
    flow_sanity_check(&info);
    REQUIRE(info.sizeof_bool == sizeof(bool));
    REQUIRE(info.sizeof_size_t == sizeof(size_t));
    REQUIRE(info.sizeof_int == sizeof(int));
}

TEST_CASE("Test failure works", "[.]")
{
    REQUIRE(false);
}

TEST_CASE("Test size of flow_context")
{
    printf("sizeof(flow_c) = %lu", sizeof(flow_c));
    REQUIRE(sizeof(flow_c) < 1500);
}

TEST_CASE("Test flow_snprintf with single-character buffer", "")
{
    char buf[] = { 3, 25 };
    REQUIRE(flow_snprintf(&buf[0], 1, "hello") == -1);
    REQUIRE(buf[0] == 0); // It should have written the null character
    REQUIRE(buf[1] == 25); // It should have left the last character untouched
}

TEST_CASE("Test flow_snprintf with zero-character  buffer_size", "")
{
    char buf[] = { 25 };
    REQUIRE(flow_snprintf(&buf[0], 0, "hello") == -1);
    REQUIRE(buf[0] == 25); // It shouldn't have written past the size
}

TEST_CASE("Test flow_snprintf with an insufficent  buffer_size", "")
{
    char buf[] = { 23, 24, 25, 26 };
    REQUIRE(flow_snprintf(&buf[0], 3, "hello") == -1);
    REQUIRE(buf[0] == 'h');
    REQUIRE(buf[1] == 'e');
    REQUIRE(buf[2] == 0); // It shouldn't have written past the size
    REQUIRE(buf[3] == 26);
}

TEST_CASE("Test flow_snprintf with a sufficient buffer", "")
{
    char buf[7];
    buf[6] = 25;
    REQUIRE(flow_snprintf(&buf[0], 6, "hello") == 5);

    REQUIRE(buf[5] == 0); // It should have written the null
    REQUIRE(buf[6] == 25); // It shouldn't have written past the size
}

TEST_CASE("Test context creation", "")
{
    flow_c * c = flow_context_create();
    ERR(c);
    flow_context_destroy(c);
}
using namespace Catch::Matchers;

TEST_CASE("Test error message printing", "")
{
    flow_c * c = flow_context_create();
    ERR(c);

    FLOW_error_msg(c, flow_status_Invalid_argument, "You passed a value outside [0,1]: %d", 3);
    char buf[4096];
    int64_t chars_written = flow_context_error_and_stacktrace(c, buf, 4096, false);
    REQUIRE(chars_written > 0);
    REQUIRE_THAT(buf,
                 StartsWith("CError 50: Invalid argument : You passed a value outside [0,1]: 3\ntest_context.cpp:"));

    flow_context_destroy(c);
}

TEST_CASE("Test error message printing with null files or functions in the stacktrace", "")
{
    flow_c * c = flow_context_create();
    ERR(c);

    REQUIRE(flow_context_raise_error(c, flow_status_Invalid_argument, NULL, NULL, 25, NULL) == true);

    char buf[4096];
    int64_t chars_written = flow_context_error_and_stacktrace(c, buf, 4096, false);
    REQUIRE(chars_written > 0);
    REQUIRE(buf == std::string("CError 50: Invalid argument\n(unknown):25: in function (unknown)\n"));

    flow_context_destroy(c);
}

static bool bad_destructor(flow_c * c, void * p) { return false; }
TEST_CASE("Test reporting of a failing destructor", "")
{
    flow_c * c = flow_context_create();
    REQUIRE_FALSE(c == NULL);
    ERR(c);

    void * data = flow_context_malloc(c, 20, bad_destructor, c, __FILE__, __LINE__);
    REQUIRE(data != NULL);
    ERR(c);
    // begin_terminate should trigger the destructor
    REQUIRE(flow_context_begin_terminate(c) == false);

    char buf[4096];
    int64_t chars_written = flow_context_error_and_stacktrace(c, buf, 4096, false);
    REQUIRE(chars_written > 0);
    REQUIRE_THAT(buf, StartsWith("CError 1024: Other error : Destructor returned false, indicating failure"));

    flow_context_destroy(c);
}

bool destructor_called = false;

static bool tattletale_destructor(flow_c * c, void * p)
{
    destructor_called = true;
    return true;
}

TEST_CASE("Test destructor", "")
{
    flow_c * c = flow_context_create();
    ERR(c);

    destructor_called = false;
    void * data = flow_context_malloc(c, 20, tattletale_destructor, c, __FILE__, __LINE__);

    // begin_terminate should trigger the destructor
    REQUIRE(flow_context_begin_terminate(c) == true);
    flow_context_destroy(c);
    REQUIRE(destructor_called == true);
    destructor_called = false;
}

TEST_CASE("Test ownership", "")
{
    flow_c * c = flow_context_create();
    ERR(c);

    void * container = FLOW_malloc(c, 10);
    void * data = flow_context_malloc(c, 20, tattletale_destructor, container, __FILE__, __LINE__);

    // Destroying container should destroy data as well
    destructor_called = false;
    FLOW_destroy(c, container);
    REQUIRE(destructor_called == true);
    destructor_called = false;

    REQUIRE(flow_context_begin_terminate(c) == true);
    flow_context_destroy(c);
}
