#include "catch.hpp"
#include "helpers_visual.h"
//#define FLOW_STORE_CHECKSUMS

#ifdef FLOW_STORE_CHECKSUMS
bool store_checksums = true;
#else
bool store_checksums = false;
#endif

TEST_CASE("Test fill_rect", "")
{
    flow_c * c = flow_context_create();
    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);
    struct flow_bitmap_bgra * b;
    int32_t last;

    last = flow_node_create_canvas(c, &g, -1, flow_bgra32, 400, 300, 0xFFFFFFFF);
    last = flow_node_create_fill_rect(c, &g, last, 0, 0, 50, 100, 0xFF0000FF);
    last = flow_node_create_bitmap_bgra_reference(c, &g, last, &b);
    struct flow_job * job = flow_job_create(c);
    ERR(c);
    if (!flow_job_execute(c, job, &g)) {
        ERR(c);
    }

    REQUIRE(visual_compare(c, b, "FillRect", store_checksums, __FILE__, __func__, __LINE__) == true);
    ERR(c);
    flow_context_destroy(c);
}

TEST_CASE("Test scale image", "")
{

    flow_c * c = flow_context_create();
    size_t bytes_count = 0;
    uint8_t * bytes = get_bytes_cached(c, &bytes_count, "http://www.rollthepotato.net/~john/kevill/test_800x600.jpg");
    REQUIRE(djb2_buffer(bytes, bytes_count) == 0x8ff8ec7a8539a2d5); // Test the checksum. I/O can be flaky

    struct flow_job * job = flow_job_create(c);
    ERR(c);
    int32_t input_placeholder = 0;
    struct flow_io * input = flow_io_create_from_memory(c, flow_io_mode_read_seekable, bytes, bytes_count, job, NULL);
    flow_job_add_io(c, job, input, input_placeholder, FLOW_INPUT);

    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);
    struct flow_bitmap_bgra * b;
    int32_t last;

    last = flow_node_create_decoder(c, &g, -1, input_placeholder);
    // flow_node_set_decoder_downscale_hint(c, g, last, 400,300,400,300);
    last = flow_node_create_scale(c, &g, last, 400, 300);
    last = flow_node_create_bitmap_bgra_reference(c, &g, last, &b);
    ERR(c);
    if (!flow_job_execute(c, job, &g)) {
        ERR(c);
    }

    REQUIRE(visual_compare(c, b, "ScaleThePotato", store_checksums, __FILE__, __func__, __LINE__) == true);
    ERR(c);
    flow_context_destroy(c);
}
