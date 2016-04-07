#include "catch.hpp"
#include "helpers_visual.h"

// Uncomment to store new checksums. To replace them, you'll have to hand-edit visuals/checksums.list yourself and
// delete the old entry.
// Don't screw up the newlines.
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

    REQUIRE(visual_compare(c, b, "FillRect", store_checksums, 0, __FILE__, __func__, __LINE__) == true);
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
    // flow_node_set_decoder_downscale_hint(c, g, last, 400, 300, 400, 300, false, 0);
    last = flow_node_create_scale(c, &g, last, 400, 300, (flow_interpolation_filter_Robidoux),
                                  (flow_interpolation_filter_Robidoux));
    last = flow_node_create_bitmap_bgra_reference(c, &g, last, &b);
    ERR(c);
    if (!flow_job_execute(c, job, &g)) {
        ERR(c);
    }

    REQUIRE(visual_compare(c, b, "ScaleThePotato", store_checksums, 50, __FILE__, __func__, __LINE__) == true);
    ERR(c);
    flow_context_destroy(c);
}

TEST_CASE("Test spatial IDCT downscale in linear light", "")
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
    long new_w = (800 * 4 + 8 - 1L) / 8L;
    long new_h = (600 * 4 + 8 - 1L) / 8L;

    if (!flow_job_decoder_set_downscale_hints_by_placeholder_id(c, job, input_placeholder, new_w, new_h, new_w, new_h,
                                                                true, true)) {
        ERR(c);
    }

    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);
    struct flow_bitmap_bgra * b;
    int32_t last;

    last = flow_node_create_decoder(c, &g, -1, input_placeholder);

    last = flow_node_create_bitmap_bgra_reference(c, &g, last, &b);
    ERR(c);
    if (!flow_job_execute(c, job, &g)) {
        ERR(c);
    }

    bool match = visual_compare(c, b, "ScaleIDCTFastvsSlow", store_checksums, 100, __FILE__, __func__, __LINE__);
    REQUIRE(match == true);
    ERR(c);
    flow_context_destroy(c);
}

TEST_CASE("Test spatial IDCT downscale without gamma correction", "")
{
    flow_c * c = flow_context_create();
    size_t bytes_count = 0;
    uint8_t * bytes = get_bytes_cached(c, &bytes_count, "http://s3.amazonaws.com/resizer-images/u1.jpg");
    REQUIRE(djb2_buffer(bytes, bytes_count) == 0x41acd8388399c2cb); // Test the checksum. I/O can be flaky

    struct flow_job * job = flow_job_create(c);
    ERR(c);
    int32_t input_placeholder = 0;
    struct flow_io * input = flow_io_create_from_memory(c, flow_io_mode_read_seekable, bytes, bytes_count, job, NULL);
    flow_job_add_io(c, job, input, input_placeholder, FLOW_INPUT);
    int original_width, original_height;
    if (!get_image_dimensions(c, bytes, bytes_count, &original_width, &original_height))
        ERR(c);
    long new_w = (original_width * 6 + 8 - 1L) / 8L;
    long new_h = (original_height * 6 + 8 - 1L) / 8L;
    if (!flow_job_decoder_set_downscale_hints_by_placeholder_id(c, job, input_placeholder, new_w, new_h, new_w, new_h,
                                                                true, false)) {
        ERR(c);
    }

    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);
    struct flow_bitmap_bgra * b;
    int32_t decode_node = flow_node_create_decoder(c, &g, -1, input_placeholder);

    flow_node_create_bitmap_bgra_reference(c, &g, decode_node, &b);
    ERR(c);
    if (!flow_job_execute(c, job, &g)) {
        ERR(c);
    }
    fprintf(stdout, "Execution time for srgb decoding (ms): %d \n",
            (int)(g->nodes[decode_node].ticks_elapsed * 1000 / flow_get_profiler_ticks_per_second()));
    fflush(stdout);

    bool match = visual_compare(c, b, "ScaleIDCT_approx_gamma", store_checksums, 100, __FILE__, __func__, __LINE__);
    REQUIRE(match == true);
    ERR(c);
    flow_context_destroy(c);
}

TEST_CASE("Test blurring", "")
{

    flow_c * c = flow_context_create();
    size_t bytes_count = 0;
    uint8_t * bytes = get_bytes_cached(
        c, &bytes_count,
        "http://s3-us-west-2.amazonaws.com/imageflow-resources/reference_image_originals/vgl_6548_0026.jpg");

    struct flow_job * job = flow_job_create(c);

    int32_t input_placeholder = 0;
    struct flow_io * input = flow_io_create_from_memory(c, flow_io_mode_read_seekable, bytes, bytes_count, job, NULL);
    if (input == NULL) {
        ERR(c);
    }
    if (!flow_job_add_io(c, job, input, input_placeholder, FLOW_INPUT)) {
        ERR(c);
    }

    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    if (g == NULL) {
        ERR(c);
    }
    struct flow_bitmap_bgra * b = NULL;
    struct flow_bitmap_bgra * reference = NULL;
    int32_t last;

    last = flow_node_create_decoder(c, &g, -1, input_placeholder);
    last = flow_node_create_bitmap_bgra_reference(c, &g, last, &reference);
    last = flow_node_create_clone(c, &g, last);
    last = flow_node_create_bitmap_bgra_reference(c, &g, last, &b);
    if (flow_context_has_error(c)) {
        ERR(c);
    }
    if (!flow_job_execute(c, job, &g)) {
        ERR(c);
    }

    if (!flow_bitmap_bgra_sharpen_block_edges(c, b, 1, -30)) {
        ERR(c);
    }
    double dssim;
    visual_compare_two(c, reference, b, "Blur", &dssim, true, true, __FILE__, __func__, __LINE__);

    fprintf(stdout, " DSSIM=%.010f\n", dssim);

    REQUIRE(dssim > 0);

    flow_context_destroy(c);
}
