#include "catch.hpp"
#include "helpers_visual.h"
#include "rewrite_in_rust/rewrite_in_rust.h"

// Uncomment to store new checksums. To replace them, you'll have to hand-edit visuals/checksums.list yourself and
// delete the old entry.
// Don't screw up the newlines.
//#define FLOW_STORE_CHECKSUMS

#ifdef FLOW_STORE_CHECKSUMS
bool store_checksums = true;
#else
bool store_checksums = false;
#endif



TEST_CASE("Test blurring", "")
{

    flow_c * c = flow_context_create();
    size_t bytes_count = 0;
    uint8_t * bytes = get_bytes_cached(
        c, &bytes_count,
        "http://s3-us-west-2.amazonaws.com/imageflow-resources/reference_image_originals/vgl_6548_0026.jpg", __FILE__);

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
    visual_compare_two(c, reference, b, "Blur", &dssim, true, true, __FILE__, __func__, __LINE__, __FILE__);

    fprintf(stdout, " DSSIM=%.010f\n", dssim);

    REQUIRE(dssim > 0);

    flow_context_destroy(c);
}
