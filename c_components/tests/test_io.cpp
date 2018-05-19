#include "catch.hpp"
#include "imageflow_private.h"
#include "helpers.h"

extern "C" void keep5() {}

TEST_CASE("Test memory io", "")
{
    flow_c * c = flow_context_create();
    uint8_t buf[] = { 3, 25, 1, 2, 3, 4, 5 };
    struct flow_io * mem
        = flow_io_create_from_memory(c, flow_io_mode_read_write_seekable, &buf[0], sizeof(buf), c, NULL);

    uint8_t buf2[] = { 0, 0 };
    REQUIRE(mem->read_func(c, mem, &buf2[0], sizeof(buf2)) == 2);

    REQUIRE(buf2[0] == buf[0]);
    REQUIRE(buf2[1] == buf[1]);

    REQUIRE(mem->read_func(c, mem, &buf2[0], sizeof(buf2)) == 2);

    REQUIRE(buf2[0] == 1);
    REQUIRE(buf2[1] == 2);

    REQUIRE(mem->seek_function(c, mem, 0) == true);
    REQUIRE(mem->read_func(c, mem, &buf2[0], sizeof(buf2)) == 2);

    REQUIRE(buf2[0] == buf[0]);
    REQUIRE(buf2[1] == buf[1]);

    ERR(c);
    flow_context_destroy(c);
}

TEST_CASE("Test file read", "")
{
    flow_c * c = flow_context_create();
    uint8_t buf[] = { 3, 25, 1, 2, 3, 4, 5 };
    write_all_byte("test_io_file.txt", (char *)&buf[0], sizeof(buf));

    struct flow_io * mem = flow_io_create_for_file(c, flow_io_mode_read_write_seekable, "test_io_file.txt", c);

    uint8_t buf2[] = { 0, 0 };
    REQUIRE(mem->read_func(c, mem, &buf2[0], sizeof(buf2)) == 2);

    REQUIRE(buf2[0] == buf[0]);
    REQUIRE(buf2[1] == buf[1]);

    REQUIRE(mem->read_func(c, mem, &buf2[0], sizeof(buf2)) == 2);

    REQUIRE(buf2[0] == 1);
    REQUIRE(buf2[1] == 2);

    REQUIRE(mem->seek_function(c, mem, 0) == true);
    REQUIRE(mem->read_func(c, mem, &buf2[0], sizeof(buf2)) == 2);

    REQUIRE(buf2[0] == buf[0]);
    REQUIRE(buf2[1] == buf[1]);

    ERR(c);
    flow_context_destroy(c);
    unlink("test_io_file.txt");
}
