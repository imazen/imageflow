#include "catch.hpp"
#include "helpers_visual.h"
#include "jpeglib.h"
// Uncomment to store new checksums. To replace them, you'll have to hand-edit visuals/checksums.list yourself and delete the old entry.
// Don't screw up the newlines.
//#define FLOW_STORE_CHECKSUMS

#ifdef FLOW_STORE_CHECKSUMS
bool store_checksums = true;
#else
bool store_checksums = false;
#endif


//#define GENERATE_CODE_LITERALS

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
    //flow_node_set_decoder_downscale_hint(c, g, last, 400, 300, 400, 300, false, 0);
    last = flow_node_create_scale(c, &g, last, 400, 300, (flow_interpolation_filter_Robidoux),
                                  (flow_interpolation_filter_Robidoux));
    last = flow_node_create_bitmap_bgra_reference(c, &g, last, &b);
    ERR(c);
    if (!flow_job_execute(c, job, &g)) {
        ERR(c);
    }

    REQUIRE(visual_compare(c, b, "ScaleThePotato", store_checksums, __FILE__, __func__, __LINE__) == true);
    ERR(c);
    flow_context_destroy(c);
}



bool get_image_dimensions(flow_c * c, uint8_t * bytes, size_t bytes_count, int32_t * width, int32_t * height)
{
    struct flow_job * job = flow_job_create(c);


    struct flow_io * input = flow_io_create_from_memory(c, flow_io_mode_read_seekable, bytes, bytes_count, job, NULL);
    flow_job_add_io(c, job, input, 0, FLOW_INPUT);
    struct flow_decoder_info info;
    flow_job_get_decoder_info(c, job, 0, &info);
    *width = info.frame0_width;
    *height = info.frame0_height;
    flow_job_destroy(c, job);
    return true;
}
//
//extern "C" {
//extern float * weights_by_target[7];
//}
//
//bool set_scale_weights(flow_c * c, flow_interpolation_filter filter, float blur)
//{
//    struct flow_interpolation_details * details
//        = flow_interpolation_details_create_from(c, flow_interpolation_filter_Robidoux);
//    if (details == NULL) {
//        FLOW_add_to_callstack(c);
//        return false;
//    }
//    for (int size = 1; size < 8; size++) {
//        struct flow_interpolation_line_contributions * contrib
//            = flow_interpolation_line_contributions_create(c, size, 8, details);
//        if (contrib == NULL) {
//            FLOW_add_to_callstack(c);
//            return false;
//        }
//        for (int i = 0; i < size; i++) {
//            float * eight = weights_by_target[size - 1] + i * 8;
//            memset(eight, 0, sizeof(float) * 8);
//            for (int input_ix = contrib->ContribRow[i].Left; input_ix <= contrib->ContribRow[i].Right; input_ix++) {
//                eight[input_ix] = contrib->ContribRow[i].Weights[input_ix - contrib->ContribRow[i].Left];
//            }
//        }
//        flow_interpolation_line_contributions_destroy(c, contrib);
//    }
//
//    flow_interpolation_details_destroy(c, details);
//    return true;
//}

bool scale_down(flow_c * c, uint8_t * bytes, size_t bytes_count, int idct_downscale_fn, int target_block_size,
                int block_scale_to_x, int block_scale_to_y, int scale_to_x, int scale_to_y,
                flow_interpolation_filter precise_filter, flow_interpolation_filter block_filter, float post_sharpen,
                float blur, flow_bitmap_bgra ** ref)
{
    struct flow_job * job = flow_job_create(c);

    int32_t input_placeholder = 0;
    struct flow_io * input = flow_io_create_from_memory(c, flow_io_mode_read_seekable, bytes, bytes_count, job, NULL);
    if (input == NULL) {
        FLOW_add_to_callstack(c);
        return false;
    }
    if (!flow_job_add_io(c, job, input, input_placeholder, FLOW_INPUT)) {
        FLOW_add_to_callstack(c);
        return false;
    }

    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    if (g == NULL) {
        FLOW_add_to_callstack(c);
        return false;
    }
    struct flow_bitmap_bgra * b;
    int32_t last;

    last = flow_node_create_decoder(c, &g, -1, input_placeholder);

    if (block_scale_to_x > 0) {
        if (!flow_job_decoder_set_downscale_hints_by_placeholder_id(c, job, input_placeholder, block_scale_to_x,
                                                                    block_scale_to_y, block_scale_to_x,
                                                                    block_scale_to_y, idct_downscale_fn != 0, 2.4f)) {
            FLOW_add_to_callstack(c);
            return false;
        }
    }

    if (scale_to_x != block_scale_to_x || scale_to_y != block_scale_to_y) {
        last = flow_node_create_scale(c, &g, last, scale_to_x, scale_to_y, precise_filter, precise_filter);
    }
    last = flow_node_create_bitmap_bgra_reference(c, &g, last, ref);

    if (flow_context_has_error(c)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    if (!flow_job_execute(c, job, &g)) {
        FLOW_add_to_callstack(c);
        return false;
    }

    // Let the bitmap last longer than the context or job
    if (!flow_set_owner(c, *ref, NULL)) {
        FLOW_add_to_callstack(c);
        return false;
    }

    if (!flow_bitmap_bgra_sharpen_block_edges(c, *ref, target_block_size, post_sharpen)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    if (!flow_job_destroy(c, job)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    return true;
}

TEST_CASE("Test faster block downscale method", "")
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

    if (!flow_job_decoder_set_downscale_hints_by_placeholder_id(c, job, input_placeholder, new_w, new_h, new_w, new_h, true, 2.4f)) {
        ERR(c);
    }
    // DSSIM 0.00002442000000000000 with 2.1f vs true srgb
    // DSSIM 0.00001457000000000000  with 2.2f
    // DSSIM 0.00002413000000000000 with 2.3f
    // DSSIM 0.00004514000000000000  with 2.39f
    // DSSIM 0.00128068999999999991 with 1.0f
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

    bool match = visual_compare(c, b, "ScaleIDCTFastvsSlow", store_checksums, __FILE__, __func__, __LINE__);
    REQUIRE(match == true);
    ERR(c);
    flow_context_destroy(c);
}

TEST_CASE("Test gamma 2.2 gamma approximations downscale method", "")
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
    if (!get_image_dimensions(c, bytes,bytes_count, &original_width, &original_height)) ERR(c);
    long new_w = (original_width * 1 + 8 - 1L) / 8L;
    long new_h = (original_height * 1 + 8 - 1L) / 8L;
    if (!flow_job_decoder_set_downscale_hints_by_placeholder_id(c, job, input_placeholder, new_w, new_h, new_w, new_h, true, 1.5f)) {
        ERR(c);
    }
    // Execution time for gamma 2.2 decoding (ticks): 1791750 (pow)
    // Execution time for gamma 2.2 decoding (ticks): 1428526 (fastpow) 75 -> 42
    //Execution time for gamma 2.2 decoding (ticks): 638640
    // Execution time for gamma 2.2 decoding (ticks): 1291405 (fasterpow - bad) 75 -> 25
    // Execution time for gamma 1.0 decoding (ticks): 1044632
    // Execution time for gamma 1.0 decoding (ticks): 501,865
    // Execution time for gamma 2.2 decoding LUT in two directions (ticks): 1565, 698
    // Execution time for gamma 2.2 decoding LUT in two directions (ticks): 762,585

    //Execution time when we add no code, let it decode to 8x8, then ignore
    //Execution time for gamma 2.2 decoding (ticks): 375254
    //Execution time for gamma 2.2 decoding (ms): 374  (7/8)
    //Execution time for gamma 2.2 decoding (ms): 125  (1/8)

    //Execution time for gamma 2.2 decoding (ms): 196  using lookup table. 1/8
    //Execution time for gamma 2.2 decoding (ms): 513 7/8
    // Execution time for gamma 2.2 decoding (ms): 490 7/8, no gamma decoding
    //Execution time for gamma 2.2 decoding (ms): 452 6/8

    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);
    struct flow_bitmap_bgra * b;
    int32_t decode_node = flow_node_create_decoder(c, &g, -1, input_placeholder);

    flow_node_create_bitmap_bgra_reference(c, &g, decode_node, &b);
    ERR(c);
    if (!flow_job_execute(c, job, &g)) {
        ERR(c);
    }
    fprintf(stdout, "Execution time for gamma 2.2 decoding (ms): %d \n", (int)(g->nodes[decode_node].ticks_elapsed * 1000/ flow_get_profiler_ticks_per_second()));
    fflush(stdout);
    exit(0);

    bool match = visual_compare(c, b, "ScaleIDCT_approx_gamma", store_checksums, __FILE__, __func__, __LINE__);
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

TEST_CASE("Test 8->4 downscaling contrib windows", "")
{
    flow_c * c = flow_context_create();
    struct flow_interpolation_details * details = flow_interpolation_details_create_bicubic_custom(
        c, 2, 1. / 1.1685777620836932, 0.37821575509399867, 0.31089212245300067);

    struct flow_interpolation_line_contributions * contrib
        = flow_interpolation_line_contributions_create(c, 4, 8, details);

    REQUIRE(contrib->ContribRow[0].Weights[0] == Approx(0.45534f));
    REQUIRE(contrib->ContribRow[3].Weights[contrib->ContribRow[3].Right - contrib->ContribRow[3].Left - 1]
            == Approx(0.45534f));
    flow_context_destroy(c);
}

#ifdef GENERATE_CODE_LITERALS
TEST_CASE("Export weights", "")
{
    flow_c * c = flow_context_create();

    struct flow_interpolation_details * details
        = flow_interpolation_details_create_from(c, flow_interpolation_filter_Robidoux);
    if (details == NULL) {
        ERR(c);
    }

    for (int size = 7; size > 0; size--) {
        fprintf(stdout, "const float jpeg_scale_to_%d_x_%d_weights[%d][8] = {\n", size, size, size);
        struct flow_interpolation_line_contributions * contrib
            = flow_interpolation_line_contributions_create(c, size, 8, details);
        for (int i = 0; i < size; i++) {
            float eight[] = { 0, 0, 0, 0, 0, 0, 0, 0 };

            for (int input_ix = contrib->ContribRow[i].Left; input_ix <= contrib->ContribRow[i].Right; input_ix++) {
                eight[input_ix] = contrib->ContribRow[i].Weights[input_ix - contrib->ContribRow[i].Left];
            }

            fprintf(stdout, "    { %.019f, %.019f, %.019f, %.019f, %.019f, %.019f, %.019f, %.019f },\n", eight[0],
                    eight[1], eight[2], eight[3], eight[4], eight[5], eight[6], eight[7]);
        }
        fprintf(stdout, "};\n");
    }
    flow_context_destroy(c);
}

TEST_CASE("Export LUT", "")
{
    flow_c * c = flow_context_create();

    struct flow_interpolation_details * details
        = flow_interpolation_details_create_from(c, flow_interpolation_filter_Robidoux);
    if (details == NULL) {
        ERR(c);
    }

    fprintf(stdout, "const float lut_srgb_to_linear[256] = {\n");
    for (int a = 0; a < 32; a++) {
        fprintf(stdout, "    ");
        for (int b = 0; b < 8; b++) {
            fprintf(stdout, "%.019f, ", flow_context_byte_to_floatspace(c, (uint8_t)(a * 8 + b)));
        }
        fprintf(stdout, "\n");
    }
    fprintf(stdout, "};\n");
    flow_context_destroy(c);
}
#endif

static const char * const test_images[] = {

    "http://s3-us-west-2.amazonaws.com/imageflow-resources/reference_image_originals/vgl_6548_0026.jpg",
    "http://s3-us-west-2.amazonaws.com/imageflow-resources/reference_image_originals/vgl_6434_0018.jpg",
    "http://s3-us-west-2.amazonaws.com/imageflow-resources/reference_image_originals/vgl_5674_0098.jpg",
    "http://s3.amazonaws.com/resizer-images/u6.jpg",
    "http://s3.amazonaws.com/resizer-images/u1.jpg",
    "http://s3-us-west-2.amazonaws.com/imageflow-resources/reference_image_originals/artificial.jpg",
    "http://www.rollthepotato.net/~john/kevill/test_800x600.jpg",
    "http://s3-us-west-2.amazonaws.com/imageflow-resources/reference_image_originals/nightshot_iso_100.jpg",
};
static const char * const test_image_names[] = {

    "vgl_6548_0026.jpg",      "vgl_6434_0018.jpg", "vgl_5674_0098.jpg",       "u6.jpg (from unsplash)",
    "u1.jpg (from unsplash)", "artificial.jpg",    "kevill/test_800x600.jpg", "nightshot_iso_100.jpg",
};
static const unsigned long test_image_checksums[] = {

    12408886241370335986UL, 4555980965349232399UL, 16859055904024046582UL, 4586057909633522523UL,
    4732395045697209035UL,  0x4bc30144f62925c1,    0x8ff8ec7a8539a2d5,     6083832193877068235L,

};
#define TEST_IMAGE_COUNT (sizeof(test_image_checksums) / sizeof(unsigned long))
// We are using an 'ideal' scaling of the full image as a control
// Under srgb decoding (libjpeg-turbo as-is ISLOW downsampling), DSSIM=0.003160
// Under linear light decoder box downsampling (vs linear light true resampling), DSSIM=0.002947
// Using the flow_bitmap_float scaling in two dierctions, DSSIM=0.000678

// Compared against robidoux
// All filters are equal for 1/8th 0.00632
// Lowest DSSIM for 2/8 was 0.00150 for block interpolation filter 2
// Lowest DSSIM for 2/8 was 0.001493900 for block interpolation filter 14, sharpen 0.000000, blur 0.850000
// Lowest DSSIM for 3/8 was 0.00078 for block interpolation filter 14
// Lowest DSSIM for 4/8 was 0.00065 for block interpolation filter 3
// Lowest DSSIM for 5/8 was 0.00027 for block interpolation filter 2
// Lowest DSSIM for 6/8 was 0.00018 for block interpolation filter 2
// Lowest DSSIM for 7/8 was 0.00007 for block interpolation filter 2
//
// image used                    , 1/8 - DSSIM, 1/8 - Params,  2/8 - DSSIM, 2/8 - Params, 3/8 - DSSIM, 3/8 - Params, 4/8
// - DSSIM, 4/8 - Params, 5/8 - DSSIM, 5/8 - Params, 6/8 - DSSIM, 6/8 - Params, 7/8 - DSSIM, 7/8 - Params,
// u6.jpg (from unsplash)        , 0.0046863400, f2 b0.00 s0.00, 0.0029699400, f3 b0.85 s0.00, 0.0040962400, f2 b0.90
// s0.00, 0.0004396400, f2 b0.00 s0.00, 0.0006252100, f2 b0.00 s0.00, 0.0016532400, f2 b0.00 s0.00, 0.0031835700, f2
// b0.90 s0.00,
// u1.jpg (from unsplash)        , 0.0114932700, f2 b0.00 s0.00, 0.0018476900, f2 b0.85 s0.00, 0.0031388100, f2 b0.00
// s0.00, 0.0006262900, f2 b0.85 s0.00, 0.0021540200, f2 b0.00 s0.00, 0.0002931700, f2 b0.85 s0.00, 0.0017235700, f2
// b0.00 s0.00,
// artificial.jpg                , 0.0023080300, f2 b0.00 s0.00, 0.0003323700, f3 b0.85 s0.00, 0.0001204200, f2 b0.85
// s0.00, 0.0001255500, f14 b0.85 s0.00, 0.0000715400, f2 b0.00 s0.00, 0.0000685200, f2 b0.85 s0.00, 0.0000570500, f2
// b0.85 s0.00,
// kevill/test_800x600.jpg       , 0.0063219000, f2 b0.00 s0.00, 0.0014939000, f14 b0.85 s0.00, 0.0007776700, f14 b0.00
// s0.00, 0.0006515500, f3 b0.90 s0.00, 0.0002729900, f2 b0.85 s0.00, 0.0001801800, f2 b0.00 s0.00, 0.0000702500, f2
// b0.00 s0.00,
// nightshot_iso_100.jpg         , 0.0013806700, f2 b0.00 s0.00, 0.0003930900, f3 b0.80 s0.00, 0.0002217200, f14 b0.85
// s0.00, 0.0002291900, f2 b0.80 s0.00, 0.0002127800, f2 b0.85 s0.00, 0.0002054500, f2 b0.80 s0.00, 0.0001715300, f14
// b0.80 s0.00,
// vgl_6548_0026.jpg             , 0.0396496800, f2 b0.00 s0.00, 0.0014971800, f2 b0.90 s0.00, 0.0009725300, f2 b0.85
// s0.00, 0.0007197700, f2 b0.90 s0.00, 0.0006977400, f2 b0.00 s0.00, 0.0004041700, f2 b0.00 s0.00, 0.0001562700, f14
// b0.90 s0.00,
// vgl_6434_0018.jpg             , 0.0045688400, f2 b0.00 s0.00, 0.0004874900, f3 b0.85 s0.00, 0.0002206900, f2 b0.85
// s0.00, 0.0002524000, f14 b0.85 s0.00, 0.0002299500, f2 b0.85 s0.00, 0.0002411100, f2 b0.85 s0.00, 0.0002378800, f2
// b0.00 s0.00,
// vgl_5674_0098.jpg             , 0.0258202300, f2 b0.00 s0.00, 0.0017091900, f2 b0.00 s0.00, 0.0009284800, f14 b0.90
// s0.00, 0.0008620000, f14 b0.00 s0.00, 0.0005750100, f2 b0.00 s0.00, 0.0004875900, f14 b0.00 s0.00, 0.0003509500, f2
// b0.00 s0.00,

// For 5/8 and 6/8 and 7/8 - stick to f2, but search space between blur 0.85 and 1
// For 2/8, search f2 and f3 between the 0.80 space and the 1.0 space
// For 1/8 use 2, but try various levels of post-sharpening
// For 3/8 and 4/8, stick with f2 repeat search space with full logging
// struct downscale_test_result{
//    double best_dssim[7];
//    flow_interpolation_filter best_filter[7];
//    float best_sharpen[7];
//    float best_blur[7];
//};
// Least bad configuration (0) for 3/8: (worst dssim 0.0041063200, rank 0.001) - f2 blur=0.86 sharp=0.00

// Least bad configuration (128) for 4/8: (worst dssim 0.0008814600, rank 0.008) - f2 blur=0.90 sharp=1.00
// Least bad configuration (10) for 5/8: (worst dssim 0.0021616700, rank 0.003) - f2 blur=0.86 sharp=3.00
// Least bad configuration (10) for 6/8: (worst dssim 0.0016544200, rank 0.009) - f2 blur=0.86 sharp=3.00
// Least bad configuration (9) for 7/8: (worst dssim 0.0032112500, rank 0.010) - f2 blur=0.86 sharp=1.00
// Least bad configuration (7) for 1/8: (worst dssim 0.0397230300, rank 0.009) - f2 blur=0.86 sharp=0.50
// Least bad configuration (0) for 2/8: (worst dssim 0.0029780000, rank 0.007) - f2 blur=0.86 sharp=0.00
struct config_result {
    float blur;
    float sharpen;
    flow_interpolation_filter filter;
    double dssim[TEST_IMAGE_COUNT];
    const char * names[TEST_IMAGE_COUNT];
};

// Testing blurring for the first time
// Least bad configuration (2) for 3/8: (worst dssim 0.0041007100, rank 0.795) - f2 blur=0.86 sharp=-2.00
//
//
// Configuration            , vgl_6548_0026.jpg, vgl_6434_0018.jpg, vgl_5674_0098.jpg, u6.jpg (from unsplash), u1.jpg
// (from unsplash), artificial.jpg, kevill/test_800x600.jpg, nightshot_iso_100.jpg,
// f2 blur=0.86 sharp=0.00, 0.0009735000000000000, 0.0002212100000000000, 0.0009313300000000000, 0.0041063200000000001,
// 0.0031388100000000001, 0.0001205000000000000, 0.0007792900000000000, 0.0002229100000000000,
// f2 blur=0.86 sharp=-3.00, 0.0009771700000000001, 0.0002168100000000000, 0.0009347900000000000, 0.0040963500000000003,
// 0.0031265300000000002, 0.0001211600000000000, 0.0007812200000000000, 0.0002235200000000000,
// f2 blur=0.86 sharp=-2.00, 0.0009749200000000000, 0.0002180800000000000, 0.0009340800000000000, 0.0041007099999999996,
// 0.0031302100000000000, 0.0001205200000000000, 0.0007800100000000000, 0.0002230100000000000,
// f2 blur=0.86 sharp=-1.00, 0.0009756000000000000, 0.0002193200000000000, 0.0009326600000000000, 0.0041058700000000002,
// 0.0031356299999999999, 0.0001204800000000000, 0.0007791600000000000, 0.0002229800000000000,
//

//
// Least bad configuration (2) for 3/8: (worst dssim 0.0235432600, rank 0.671) - f2 blur=0.86 sharp=-2.00
//
//
// Configuration            , vgl_6548_0026.jpg, vgl_6434_0018.jpg, vgl_5674_0098.jpg, u6.jpg (from unsplash), u1.jpg
// (from unsplash), artificial.jpg, kevill/test_800x600.jpg, nightshot_iso_100.jpg,
// f2 blur=0.86 sharp=0.00, 0.0166718600000000002, 0.0033206799999999999, 0.0124358200000000002, 0.0115724000000000000,
// 0.0098416300000000005, 0.0047794100000000004, 0.0235566400000000002, 0.0043571499999999997,
// f2 blur=0.86 sharp=-3.00, 0.0166349200000000010, 0.0033191800000000001, 0.0124035499999999993, 0.0115649700000000008,
// 0.0098405100000000002, 0.0047848200000000004, 0.0235159199999999992, 0.0043641699999999997,
// f2 blur=0.86 sharp=-2.00, 0.0166491500000000014, 0.0033189700000000001, 0.0124166899999999993, 0.0115661800000000006,
// 0.0098403499999999994, 0.0047812300000000000, 0.0235432599999999999, 0.0043600799999999997,
// f2 blur=0.86 sharp=-1.00, 0.0166597799999999990, 0.0033202399999999999, 0.0124354099999999992, 0.0115713799999999992,
// 0.0098407800000000004, 0.0047792099999999999, 0.0235566400000000002, 0.0043571699999999996,
//
//
// Least bad configuration (4) for 1/8: (worst dssim 0.0329705100, rank 0.049) - f2 blur=0.86 sharp=-20.00
//
//
// Configuration            , vgl_6548_0026.jpg, vgl_6434_0018.jpg, vgl_5674_0098.jpg, u6.jpg (from unsplash), u1.jpg
// (from unsplash), artificial.jpg, kevill/test_800x600.jpg, nightshot_iso_100.jpg,
// f2 blur=0.86 sharp=0.00, 0.0396496799999999996, 0.0045688400000000002, 0.0258202299999999996, 0.0046863399999999998,
// 0.0114932699999999999, 0.0023080300000000000, 0.0063219000000000001, 0.0013806700000000001,
// f2 blur=0.86 sharp=-30.00, 0.0310881699999999983, 0.0060444899999999996, 0.0267351200000000011,
// 0.0044244000000000002, 0.0106874499999999994, 0.0023641000000000001, 0.0052676499999999996, 0.0018543100000000001,
// f2 blur=0.86 sharp=-40.00, 0.0298468100000000014, 0.0067757399999999997, 0.0275375599999999991,
// 0.0049290000000000002, 0.0109019300000000006, 0.0026247900000000001, 0.0059822699999999996, 0.0022172400000000001,
// f2 blur=0.86 sharp=-25.00, 0.0319104799999999980, 0.0056566799999999999, 0.0264039299999999991,
// 0.0042365800000000002, 0.0106188900000000006, 0.0022385800000000000, 0.0049987100000000000, 0.0016944099999999999,
// f2 blur=0.86 sharp=-20.00, 0.0329705100000000015, 0.0053769400000000002, 0.0261458000000000003,
// 0.0041285799999999998, 0.0106497899999999993, 0.0021847400000000001, 0.0048988399999999998, 0.0015697800000000000,
// f2 blur=0.86 sharp=-18.00, 0.0334803200000000009, 0.0052774500000000004, 0.0261108100000000017,
// 0.0041123799999999997, 0.0106793200000000008, 0.0021829900000000001, 0.0049296799999999997, 0.0015307200000000000,
// f2 blur=0.86 sharp=-15.00, 0.0342198800000000011, 0.0051011399999999997, 0.0260294299999999992,
// 0.0040945799999999996, 0.0107228400000000008, 0.0021567700000000001, 0.0049479700000000003, 0.0014633500000000000,
// f2 blur=0.86 sharp=-10.00, 0.0357080300000000017, 0.0048456200000000001, 0.0258460099999999991,
// 0.0041590200000000003, 0.0108743599999999996, 0.0021569800000000002, 0.0051517400000000001, 0.0013918600000000000,
// f2 blur=0.86 sharp=50.00, 0.0933786699999999970, 0.0219094599999999987, 0.0424101799999999987, 0.0301197900000000005,
// 0.0272108900000000015, 0.0137332900000000004, 0.0568919300000000003, 0.0161208099999999993,

//
// Least bad configuration (7) for 4/8: (worst dssim 0.0008707100, rank 0.008) - f2 blur=0.86 sharp=-10.00
//
//
// Configuration            , vgl_6548_0026.jpg, vgl_6434_0018.jpg, vgl_5674_0098.jpg, u6.jpg (from unsplash), u1.jpg
// (from unsplash), artificial.jpg, kevill/test_800x600.jpg, nightshot_iso_100.jpg,
// f2 blur=0.86 sharp=0.00, 0.0007991200000000000, 0.0002529200000000000, 0.0008683000000000000, 0.0004396400000000000,
// 0.0006265100000000000, 0.0001260600000000000, 0.0006545500000000001, 0.0002315900000000000,
// f2 blur=0.86 sharp=-30.00, 0.0008332000000000000, 0.0002376600000000000, 0.0009229300000000000,
// 0.0004990400000000000, 0.0006527600000000000, 0.0001551500000000000, 0.0007387300000000001, 0.0002503300000000000,
// f2 blur=0.86 sharp=-40.00, 0.0008566700000000000, 0.0002472100000000000, 0.0009577800000000000,
// 0.0005356400000000000, 0.0006770800000000000, 0.0001704400000000000, 0.0007920900000000000, 0.0002616700000000000,
// f2 blur=0.86 sharp=-25.00, 0.0008218300000000000, 0.0002348500000000000, 0.0009079900000000000,
// 0.0004840400000000000, 0.0006434200000000000, 0.0001483800000000000, 0.0007151300000000000, 0.0002475800000000000,
// f2 blur=0.86 sharp=-20.00, 0.0008121100000000000, 0.0002328200000000000, 0.0008912800000000000,
// 0.0004687400000000000, 0.0006342800000000000, 0.0001412500000000000, 0.0006946600000000000, 0.0002426800000000000,
// f2 blur=0.86 sharp=-18.00, 0.0008084200000000000, 0.0002318400000000000, 0.0008857900000000000,
// 0.0004627900000000000, 0.0006307200000000000, 0.0001390800000000000, 0.0006846200000000000, 0.0002395800000000000,
// f2 blur=0.86 sharp=-15.00, 0.0008044699999999999, 0.0002326200000000000, 0.0008796100000000000,
// 0.0004553900000000000, 0.0006266800000000000, 0.0001354800000000000, 0.0006745300000000000, 0.0002374200000000000,
// f2 blur=0.86 sharp=-10.00, 0.0007990900000000000, 0.0002351800000000000, 0.0008707100000000000,
// 0.0004456500000000000, 0.0006225800000000000, 0.0001304800000000000, 0.0006617200000000000, 0.0002342200000000000,
// f2 blur=0.86 sharp=50.00, 0.0024026700000000000, 0.0016387299999999999, 0.0025417600000000001, 0.0016971300000000000,
// 0.0017169600000000000, 0.0007030199999999999, 0.0025111100000000000, 0.0008529399999999999,
// f2 blur=0.90 sharp=0.00, 0.0007991200000000000, 0.0002529200000000000, 0.0008683000000000000, 0.0004396400000000000,
// 0.0006265100000000000, 0.0001260600000000000, 0.0006545500000000001, 0.0002315900000000000,
// f2 blur=0.90 sharp=-30.00, 0.0008332000000000000, 0.0002376600000000000, 0.0009229300000000000,
// 0.0004990400000000000, 0.0006527600000000000, 0.0001551500000000000, 0.0007387300000000001, 0.0002503300000000000,
// f2 blur=0.90 sharp=-40.00, 0.0008566700000000000, 0.0002472100000000000, 0.0009577800000000000,
// 0.0005356400000000000, 0.0006770800000000000, 0.0001704400000000000, 0.0007920900000000000, 0.0002616700000000000,
// f2 blur=0.90 sharp=-25.00, 0.0008218300000000000, 0.0002348500000000000, 0.0009079900000000000,
// 0.0004840400000000000, 0.0006434200000000000, 0.0001483800000000000, 0.0007151300000000000, 0.0002475800000000000,
// f2 blur=0.90 sharp=-20.00, 0.0008121100000000000, 0.0002328200000000000, 0.0008912800000000000,
// 0.0004687400000000000, 0.0006342800000000000, 0.0001412500000000000, 0.0006946600000000000, 0.0002426800000000000,
// f2 blur=0.90 sharp=-18.00, 0.0008084200000000000, 0.0002318400000000000, 0.0008857900000000000,
// 0.0004627900000000000, 0.0006307200000000000, 0.0001390800000000000, 0.0006846200000000000, 0.0002395800000000000,
// f2 blur=0.90 sharp=-15.00, 0.0008044699999999999, 0.0002326200000000000, 0.0008796100000000000,
// 0.0004553900000000000, 0.0006266800000000000, 0.0001354800000000000, 0.0006745300000000000, 0.0002374200000000000,
// f2 blur=0.90 sharp=-10.00, 0.0007990900000000000, 0.0002351800000000000, 0.0008707100000000000,
// 0.0004456500000000000, 0.0006225800000000000, 0.0001304800000000000, 0.0006617200000000000, 0.0002342200000000000,
// f2 blur=0.90 sharp=50.00, 0.0024026700000000000, 0.0016387299999999999, 0.0025417600000000001, 0.0016971300000000000,
// 0.0017169600000000000, 0.0007030199999999999, 0.0025111100000000000, 0.0008529399999999999,
// f2 blur=0.95 sharp=0.00, 0.0007991200000000000, 0.0002529200000000000, 0.0008683000000000000, 0.0004396400000000000,
// 0.0006265100000000000, 0.0001260600000000000, 0.0006545500000000001, 0.0002315900000000000,
// f2 blur=0.95 sharp=-30.00, 0.0008332000000000000, 0.0002376600000000000, 0.0009229300000000000,
// 0.0004990400000000000, 0.0006527600000000000, 0.0001551500000000000, 0.0007387300000000001, 0.0002503300000000000,
// f2 blur=0.95 sharp=-40.00, 0.0008566700000000000, 0.0002472100000000000, 0.0009577800000000000,
// 0.0005356400000000000, 0.0006770800000000000, 0.0001704400000000000, 0.0007920900000000000, 0.0002616700000000000,
// f2 blur=0.95 sharp=-25.00, 0.0008218300000000000, 0.0002348500000000000, 0.0009079900000000000,
// 0.0004840400000000000, 0.0006434200000000000, 0.0001483800000000000, 0.0007151300000000000, 0.0002475800000000000,
// f2 blur=0.95 sharp=-20.00, 0.0008121100000000000, 0.0002328200000000000, 0.0008912800000000000,
// 0.0004687400000000000, 0.0006342800000000000, 0.0001412500000000000, 0.0006946600000000000, 0.0002426800000000000,
// f2 blur=0.95 sharp=-18.00, 0.0008084200000000000, 0.0002318400000000000, 0.0008857900000000000,
// 0.0004627900000000000, 0.0006307200000000000, 0.0001390800000000000, 0.0006846200000000000, 0.0002395800000000000,
// f2 blur=0.95 sharp=-15.00, 0.0008044699999999999, 0.0002326200000000000, 0.0008796100000000000,
// 0.0004553900000000000, 0.0006266800000000000, 0.0001354800000000000, 0.0006745300000000000, 0.0002374200000000000,
// f2 blur=0.95 sharp=-10.00, 0.0007990900000000000, 0.0002351800000000000, 0.0008707100000000000,
// 0.0004456500000000000, 0.0006225800000000000, 0.0001304800000000000, 0.0006617200000000000, 0.0002342200000000000,
// f2 blur=0.95 sharp=50.00, 0.0024026700000000000, 0.0016387299999999999, 0.0025417600000000001, 0.0016971300000000000,
// 0.0017169600000000000, 0.0007030199999999999, 0.0025111100000000000, 0.0008529399999999999,
// f2 blur=0.80 sharp=0.00, 0.0007991200000000000, 0.0002529200000000000, 0.0008683000000000000, 0.0004396400000000000,
// 0.0006265100000000000, 0.0001260600000000000, 0.0006545500000000001, 0.0002315900000000000,
// f2 blur=0.80 sharp=-30.00, 0.0008332000000000000, 0.0002376600000000000, 0.0009229300000000000,
// 0.0004990400000000000, 0.0006527600000000000, 0.0001551500000000000, 0.0007387300000000001, 0.0002503300000000000,
// f2 blur=0.80 sharp=-40.00, 0.0008566700000000000, 0.0002472100000000000, 0.0009577800000000000,
// 0.0005356400000000000, 0.0006770800000000000, 0.0001704400000000000, 0.0007920900000000000, 0.0002616700000000000,
// f2 blur=0.80 sharp=-25.00, 0.0008218300000000000, 0.0002348500000000000, 0.0009079900000000000,
// 0.0004840400000000000, 0.0006434200000000000, 0.0001483800000000000, 0.0007151300000000000, 0.0002475800000000000,
// f2 blur=0.80 sharp=-20.00, 0.0008121100000000000, 0.0002328200000000000, 0.0008912800000000000,
// 0.0004687400000000000, 0.0006342800000000000, 0.0001412500000000000, 0.0006946600000000000, 0.0002426800000000000,
// f2 blur=0.80 sharp=-18.00, 0.0008084200000000000, 0.0002318400000000000, 0.0008857900000000000,
// 0.0004627900000000000, 0.0006307200000000000, 0.0001390800000000000, 0.0006846200000000000, 0.0002395800000000000,
// f2 blur=0.80 sharp=-15.00, 0.0008044699999999999, 0.0002326200000000000, 0.0008796100000000000,
// 0.0004553900000000000, 0.0006266800000000000, 0.0001354800000000000, 0.0006745300000000000, 0.0002374200000000000,
// f2 blur=0.80 sharp=-10.00, 0.0007990900000000000, 0.0002351800000000000, 0.0008707100000000000,
// 0.0004456500000000000, 0.0006225800000000000, 0.0001304800000000000, 0.0006617200000000000, 0.0002342200000000000,
// f2 blur=0.80 sharp=50.00, 0.0024026700000000000, 0.0016387299999999999, 0.0025417600000000001, 0.0016971300000000000,
// 0.0017169600000000000, 0.0007030199999999999, 0.0025111100000000000, 0.0008529399999999999,

TEST_CASE("Exhasutive search for best downscaling kernel", "")
{
    return;
    flow_interpolation_filter filters[] = { flow_interpolation_filter_Robidoux };
    float blurs[] = { 1. / 1.1685777620836932 };
    float sharpens[] = { 0, -30, -40, -25, -20, -18, -15, -10, 50 };
    int target_sizes[] = { 2, 3, 5, 6, 7 };
#define target_sizes_count (sizeof(target_sizes) / sizeof(int))
#define sharpens_count (sizeof(sharpens) / sizeof(float))
#define blurs_count (sizeof(blurs) / sizeof(float))
#define filters_count (sizeof(filters) / sizeof(flow_interpolation_filter))

    for (size_t target_size_ix = 0; target_size_ix < target_sizes_count; target_size_ix++) {
        int scale_to = target_sizes[target_size_ix];

        fprintf(stdout, "Searching for best candidate for %d/8 filter\n", scale_to);
        struct config_result config_results[sharpens_count * blurs_count * filters_count];
#define config_result_count (sizeof(config_results) / sizeof(struct config_result))

        double worst_dssims[TEST_IMAGE_COUNT];
        memset(&worst_dssims[0], 0, sizeof(worst_dssims));
        double best_dssims[TEST_IMAGE_COUNT];
        memset(&best_dssims[0], 0, sizeof(best_dssims));
        memset(&config_results[0], 0, sizeof(config_results));

        for (size_t test_image_index = 0; test_image_index < TEST_IMAGE_COUNT; test_image_index++) {

            int config_index = 0;
            fprintf(stdout, "Testing with %s\n\n", test_images[test_image_index]);
            flow_c * c = flow_context_create();
            size_t bytes_count = 0;
            uint8_t * bytes = get_bytes_cached(c, &bytes_count, test_images[test_image_index]);
            unsigned long input_checksum = djb2_buffer(bytes, bytes_count);
            REQUIRE(input_checksum == test_image_checksums[test_image_index]); // Test the checksum. I/O can be flaky
            int original_width, original_height;
            REQUIRE(get_image_dimensions(c, bytes, bytes_count, &original_width, &original_height) == true);

            long new_w = (original_width * scale_to + 8 - 1L) / 8L;
            long new_h = (original_height * scale_to + 8 - 1L) / 8L;
            fprintf(stdout, "Testing downscaling to %d/8: %dx%d -> %ldx%ld\n", scale_to, original_width,
                    original_height, new_w, new_h);

            double best_dssim = 1;

            struct flow_bitmap_bgra * reference_bitmap;
            if (!scale_down(c, bytes, bytes_count, 0, scale_to, 0, 0, new_w, new_h, flow_interpolation_filter_Robidoux,
                            flow_interpolation_filter_Robidoux, 0, 0, &reference_bitmap)) {
                ERR(c);
            }

            size_t filter_ix = 1;
            for (size_t filter_ix = 0; filter_ix < filters_count; filter_ix++) {
                for (size_t blur_ix = 0; blur_ix < blurs_count; blur_ix++) {
                    for (size_t sharpen_ix = 0; sharpen_ix < sharpens_count; sharpen_ix++) {
                        struct config_result * config = &config_results[config_index];
                        config_index++;
                        config->blur = blurs[blur_ix];
                        config->sharpen = sharpens[sharpen_ix];
                        config->filter = filters[filter_ix];

                        flow_c * inner_context = flow_context_create();
                        struct flow_bitmap_bgra * experiment_bitmap;
                        fprintf(stdout, "f%i sharp %.04f blur %0.4f: ", (int)config->filter, config->sharpen / 100.f,
                                config->blur);

                        if (!scale_down(inner_context, bytes, bytes_count, 2, scale_to, new_w, new_h, new_w, new_h,
                                        flow_interpolation_filter_Robidoux, config->filter, config->sharpen,
                                        config->blur, &experiment_bitmap)) {
                            ERR(c);
                        }
                        double dssim;
                        visual_compare_two(inner_context, reference_bitmap, experiment_bitmap,
                                           "Compare ideal downscaling vs downscaling in decoder", &dssim, true, false,
                                           __FILE__, __func__, __LINE__);

                        fprintf(stdout, " DSSIM=%.010f\n", dssim);

                        if (dssim > worst_dssims[test_image_index])
                            worst_dssims[test_image_index] = dssim;

                        if (best_dssims[test_image_index] == 0 || best_dssims[test_image_index] > dssim) {
                            best_dssims[test_image_index] = dssim;
                        }
                        config->dssim[test_image_index] = dssim;
                        config->names[test_image_index] = test_image_names[test_image_index];

                        ERR(inner_context);
                        flow_bitmap_bgra_destroy(inner_context, experiment_bitmap);
                        flow_context_destroy(inner_context);
                        inner_context = NULL;
                    }
                }
            }

            flow_context_destroy(c);
        }

        size_t peak_ix, least_bad_ix = 0;
        double peak_for_target = 1, least_bad_for_target = 1;
        double least_bad_relative = 2;
        for (size_t config_ix = 0; config_ix < config_result_count; config_ix++) {
            double min_rel = 1;
            double max_rel = 0;
            double min = 1;
            double max = 0;
            for (size_t i = 0; i < TEST_IMAGE_COUNT; i++) {
                double dssim = config_results[config_ix].dssim[i];

                double dssim_relative = 0.0001;
                if (worst_dssims[i] > best_dssims[i]) {
                    dssim_relative = (dssim - best_dssims[i]) / (worst_dssims[i] - best_dssims[i]);
                }
                if (dssim_relative < min_rel)
                    min_rel = dssim_relative;
                if (dssim_relative > max_rel)
                    max_rel = dssim_relative;
                if (dssim < min)
                    min = dssim;
                if (dssim > max)
                    max = dssim;
            }
            if (least_bad_relative > max_rel) {
                least_bad_relative = max_rel;
                least_bad_for_target = max;
                least_bad_ix = config_ix;
            }
            if (peak_ix > min) {
                peak_ix = min;
                peak_ix = config_ix;
            }
        }
        struct config_result least_bad = config_results[least_bad_ix];
        fprintf(stdout, "\n\n\nLeast bad configuration (%d) for %d/8: (worst dssim %.010f, rank %.03f) - f%d blur=%.2f "
                        "sharp=%.2f \n\n\n",
                (int)least_bad_ix, scale_to, least_bad_for_target, least_bad_relative, least_bad.filter, least_bad.blur,
                least_bad.sharpen);

        fprintf(stdout, "Configuration            , ");
        for (size_t i = 0; i < TEST_IMAGE_COUNT; i++) {
            fprintf(stdout, "%s, ", test_image_names[i]);
        }
        fprintf(stdout, "\n");
        for (size_t config_ix = 0; config_ix < config_result_count; config_ix++) {
            struct config_result r = config_results[config_ix];
            fprintf(stdout, "f%d blur=%.2f sharp=%.2f, ", r.filter, r.blur, r.sharpen);
            for (size_t i = 0; i < TEST_IMAGE_COUNT; i++) {
                fprintf(stdout, "%.019f, ", r.dssim[i]);
            }
            fprintf(stdout, "\n");
        }
        fprintf(stdout, "\n\n\n\n");
        fflush(stdout);
    }

    fprintf(stdout, "\n\n...done\n");
    sleep(1);
}
