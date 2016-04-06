#include "catch.hpp"
#include "helpers_visual.h"

//#define GENERATE_CODE_LITERALS
//#define SEARCH_FOR_BEST_LOOKUP_PARAMS
//#define SEARCH_FOR_BEST_BLOCK_BLURRING

#define REVERSE_LUT_SIZE (256 * 15)

#ifdef SEARCH_FOR_BEST_LOOKUP_PARAMS
TEST_CASE("Search for best lookup table", "")
{
    // These 3 results were generated with test_count = 10000
    // Average LUT loss in linear space (net) = 0.0007372895 - (abs) = 0.0013200855  (for a=0.50000, b=256.00000)
    // Average LUT loss in linear space (net) = -0.0019677513 - (abs) = 0.0019677513  (for a=0.50000, b=255.00000)
    // Average LUT loss in linear space (net) = -0.0000027183 - (abs) = 0.0009802943  (for a=0.00000, b=255.00000)
    float lut[256];
    float a = 0;
    float b = 255.0f;
    for (int index = 0; index < 256; index++) {
        lut[index] = srgb_to_linear(((float)index + a) / b);
    }
    int test_count = 1000;
    double lut_loss = 0;
    double lut_net_loss = 0;
    for (int i = 0; i < test_count; i++) {
        float srgb = (float)i / (float)(test_count - 1);
        uint8_t srgb_byte = uchar_clamp_ff(srgb * 255.0f);
        float linear = srgb_to_linear(srgb);
        lut_loss += fabs(linear - lut[srgb_byte]);
        lut_net_loss += (linear - lut[srgb_byte]);
    }
    lut_loss = lut_loss / (float)test_count;
    lut_net_loss = lut_net_loss / (float)test_count;
    fprintf(stdout, "Average LUT loss in linear space (net) = %.010f - (abs) = %.010f  (for a=%.5f, b=%.5f)\n",
            lut_net_loss, lut_loss, a, b);

    fflush(stdout);
}
TEST_CASE("Search for best reverse lookup table", "")
{

    // When LUT is index/ 255 and reverse_lut is (index + a) / (reverse_lut_size -b), and lookup uses (linear *
    // (reverse_lut_size -1)

    // Over 4096 samples: lowest absolute linear error (0.0010030786506831646), a=0.5097656250000000000,
    // b=1.0285155773162841797
    // Over 4096 samples: lowest net linear error (0.0000229428005695809), a=0.5097656250000000000,
    // b=1.0285155773162841797

    //  Over 16384 samples: lowest absolute linear error (0.0010040432680398226), a=0.5000000000000000000,
    //  b=0.9992187619209289551
    //  Over 16384 samples: lowest net linear error (0.0000237091426242841), a=0.5000000000000000000,
    //  b=0.9992187619209289551

    // Conclusion: a=5.0, b=1.0 are best
    int test_count = 16384;
    float step_size = (1.0 / 512.0f);
    float min_a = 0.5;
    float max_a = 0.5 + step_size;
    float min_b = 1;
    float max_b = 1 + step_size;
    float lut[256];
    for (int index = 0; index < 256; index++) {
        lut[index] = srgb_to_linear(((float)index) / 255.0f);
    }
    uint8_t reverse_lut[REVERSE_LUT_SIZE];
    for (uint32_t reverse_lut_size = REVERSE_LUT_SIZE; reverse_lut_size >= 2048; reverse_lut_size -= 256) {
        float best_abs_a = -10, best_abs_b = -10, best_net_a = -10, best_net_b = -10, best_net_loss = 3000,
              best_abs_loss = 3000;
        float best_max_a = -10, best_max_b = -10, best_max_error = 3000;
        for (float a = min_a; a <= max_a; a += step_size) {
            for (float b = min_b; b <= max_b; b += step_size) {
                // Build LUT with constants a and b
                // uchar_clamp_ff adds 0.5 to float for fast rounding; we could fix that and experiment with
                // pre-rounding
                for (uint32_t index = 0; index < reverse_lut_size; index++) {
                    reverse_lut[index]
                        = uchar_clamp_ff(linear_to_srgb(((float)index + a) / (float)(reverse_lut_size - b)));
                }

                float net_loss = 0;
                float abs_loss = 0;
                float max_abs_loss = 0;
                for (int i = 0; i < test_count; i++) {
                    float srgb = (float)i / (float)(test_count - 1);
                    uint8_t srgb_byte = uchar_clamp_ff(srgb * 255.0f);
                    float linear = srgb_to_linear(srgb);
                    uint32_t reverse_lut_index = (uint32_t)(linear * (reverse_lut_size - 1));
                    int roundtrip = reverse_lut[reverse_lut_index > reverse_lut_size - 1 ? reverse_lut_size - 1
                                                                                         : reverse_lut_index];

                    // float normal_loss = fabs(srgb - linear_to_srgb(lut[srgb_byte]) / 255.0f);
                    float current_loss = fabs(srgb - linear_to_srgb(lut[roundtrip]) / 255.0f);
                    net_loss += srgb - linear_to_srgb(lut[roundtrip]) / 255.0f;
                    abs_loss += current_loss;
                    if (max_abs_loss < current_loss)
                        max_abs_loss = current_loss;
                }
                net_loss = net_loss / (float)test_count;
                abs_loss = abs_loss / (float)test_count;

                if (best_abs_loss > abs_loss) {
                    best_abs_a = a;
                    best_abs_b = b;
                    best_abs_loss = abs_loss;
                }
                if (fabs(best_net_loss) > fabs(net_loss)) {
                    best_net_a = a;
                    best_net_b = b;
                    best_net_loss = net_loss;
                }
                if (best_max_error > max_abs_loss) {
                    best_max_a = a;
                    best_max_b = b;
                    best_max_error = max_abs_loss;
                }
            }
        }
        fprintf(stdout, "\nSearching for best parmas for reverse lookup table of size %d\n", reverse_lut_size);
        fprintf(stdout, "Searching for %0.5f <= a <= %.5f, %.5f <= b <= %.5f with step size %.5f\n", min_a, max_a,
                min_b, max_b, step_size);
        fprintf(stdout, "Over %d samples: lowest avg absolute error (%0.019f), a=%.019f, b=%.019f\n", test_count,
                best_abs_loss, best_abs_a, best_abs_b);
        fprintf(stdout, "Over %d samples: lowest avg net error (%0.019f), a=%.019f, b=%.019f\n", test_count,
                best_net_loss, best_net_a, best_net_b);
        fprintf(stdout, "Over %d samples: lowest maximum error (%0.019f), a=%.019f, b=%.019f\n", test_count,
                best_max_error, best_max_a, best_max_b);
        if (best_max_error > (0.99 / 255.0f)) {
            fprintf(stdout, "WARNING - accuracy may be worse than 8-bit per channel sRGB!\n\n");
        }
    }
}
#endif

#ifdef GENERATE_CODE_LITERALS

#define FLOW_FLOATS_PER_LINE 8
#define FLOW_bytes_PER_LINE 32

TEST_CASE("Export float LUT", "")
{
    uint8_t reverse_lut[REVERSE_LUT_SIZE];
    fprintf(stdout, "static const uint8_t lut_linear_to_srgb[%d] = {\n", REVERSE_LUT_SIZE);
    for (int a = 0; a < REVERSE_LUT_SIZE / FLOW_bytes_PER_LINE; a++) {
        fprintf(stdout, "    ");
        for (int b = 0; b < FLOW_bytes_PER_LINE; b++) {
            int index = a * FLOW_bytes_PER_LINE + b;
            uint8_t v = uchar_clamp_ff(linear_to_srgb(((float)index + 0.5) / (float)(REVERSE_LUT_SIZE - 1)));
            reverse_lut[index] = v;
            fprintf(stdout, "%d, ", v);
        }
        fprintf(stdout, "\n");
    }
    fprintf(stdout, "};\n");

    float lut[256];
    fprintf(stdout, "static const float lut_srgb_to_linear[256] = {\n");
    for (int a = 0; a < 256 / FLOW_FLOATS_PER_LINE; a++) {
        fprintf(stdout, "    ");
        for (int b = 0; b < FLOW_FLOATS_PER_LINE; b++) {
            uint8_t index = (uint8_t)(a * FLOW_FLOATS_PER_LINE + b);
            lut[index] = srgb_to_linear((float)index / 255.0f);
            uint32_t reverse_lut_index = (uint32_t)(lut[index] * (REVERSE_LUT_SIZE - 1));
            int roundtrip
                = reverse_lut[reverse_lut_index > REVERSE_LUT_SIZE - 1 ? REVERSE_LUT_SIZE - 1 : reverse_lut_index];
            if (roundtrip != index) {
                fprintf(stderr, "/* Failed to round-trip byte %d  linear = %.010f, but round-tripped to %d */\n", index,
                        lut[index], roundtrip);
            }
            fprintf(stdout, "%.019f, ", lut[index]);
        }
        fprintf(stdout, "\n");
    }
    fprintf(stdout, "};\n");
    fflush(stdout);
    fflush(stderr);
}

TEST_CASE("Export weights", "")
{
    flow_c * c = flow_context_create();

    struct flow_interpolation_details * details
        = flow_interpolation_details_create_from(c, flow_interpolation_filter_Robidoux);
    if (details == NULL) {
        ERR(c);
    }

    for (int size = 7; size > 0; size--) {
        fprintf(stdout, "static const float jpeg_scale_to_%d_x_%d_weights[%d][8] = {\n", size, size, size);
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
#endif

#ifdef SEARCH_FOR_BEST_BLOCK_BLURRING

// Major flaws:
// (a) the test image set does not statistically represent average jpegs. There are only 2 high-res photos, and 2
// low-res photos
// (b) DSSIM values are useless for different images. We have no good way to average them.
// Currently, we throw in one 'bad' sharpen value, and then use a relative range for each photo, then compare the
// relative ranges.
// This is not good math.

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

//#define TEST_IMAGE_FIRST 0
//#define TEST_IMAGE_LAST  (TEST_IMAGE_COUNT -1)

// Override this to only test with 1 image
#define TEST_IMAGE_FIRST 3
#define TEST_IMAGE_LAST TEST_IMAGE_FIRST

// for u6.jpg
// Least bad configuration (6) for 7/8: (worst dssim 0.0033935200, rank 0.000) - sharpen=-14.00
// Least bad configuration (6) for 3/8: (worst dssim 0.0051482800, rank 0.000) - sharpen=-14.00
// Least bad configuration (5) for 2/8: (worst dssim 0.0047244700, rank 0.000) - sharpen=-15.00
// Least bad configuration (5) for 1/8: (worst dssim 0.0040946400, rank 0.000) - sharpen=-15.00
// Least bad configuration (4) for 4/8: (worst dssim 0.0014033400, rank 0.000) - sharpen=-7.00
// Least bad configuration (5) for 5/8: (worst dssim 0.0011648900, rank 0.000) - sharpen=-6.00
// Least bad configuration (7) for 6/8: (worst dssim 0.0017093100, rank 0.000) - sharpen=-4.00

// We are using an 'ideal' scaling of the full image as a control

struct config_result {
    float blur;
    float sharpen;
    flow_interpolation_filter filter;
    double dssim[TEST_IMAGE_COUNT];
    const char * names[TEST_IMAGE_COUNT];
};

bool scale_down(flow_c * c, uint8_t * bytes, size_t bytes_count, bool scale_luma_spatially,
                bool gamma_correct_for_srgb_during_spatial_luma_scaling, int target_block_size, int block_scale_to_x,
                int block_scale_to_y, int scale_to_x, int scale_to_y, flow_interpolation_filter precise_filter,
                float post_sharpen, float blur, flow_bitmap_bgra ** ref);

TEST_CASE("Exhasutive search for best block downscaling params", "")
{
    //    flow_interpolation_filter filters[] = { flow_interpolation_filter_Robidoux };
    //    float blurs[] = { 1. / 1.1685777620836932 };
    // float sharpens[] = { 0, -25, -20, -18,-16, -15, -14, -12 -10 -8, -5, 50 };
    float sharpens[] = { 0, -15, -14, -12 - 10 - 8, -7, -6, -5, -4, -3, -2, 50 };
    int target_sizes[] = { 1, 2, 3, 4, 5, 6, 7 };
#define target_sizes_count (sizeof(target_sizes) / sizeof(int))
#define sharpens_count (sizeof(sharpens) / sizeof(float))
#define blurs_count 1 //(sizeof(blurs) / sizeof(float))
#define filters_count 1 //(sizeof(filters) / sizeof(flow_interpolation_filter))

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

        for (size_t test_image_index = TEST_IMAGE_FIRST; test_image_index <= TEST_IMAGE_LAST; test_image_index++) {

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
            if (!scale_down(c, bytes, bytes_count, 0, false, scale_to, 0, 0, new_w, new_h,
                            flow_interpolation_filter_Robidoux, 0, 0, &reference_bitmap)) {
                ERR(c);
            }

            size_t filter_ix = 1;
            // for (size_t filter_ix = 0; filter_ix < filters_count; filter_ix++) {
            // for (size_t blur_ix = 0; blur_ix < blurs_count; blur_ix++) {
            for (size_t sharpen_ix = 0; sharpen_ix < sharpens_count; sharpen_ix++) {
                struct config_result * config = &config_results[config_index];
                config_index++;
                // config->blur = blurs[blur_ix];
                config->sharpen = sharpens[sharpen_ix];
                // config->filter = filters[filter_ix];

                flow_c * inner_context = flow_context_create();
                struct flow_bitmap_bgra * experiment_bitmap;
                // fprintf(stdout, "f%i sharp %.04f blur %0.4f: ", (int)config->filter, config->sharpen / 100.f,
                // config->blur);
                fprintf(stdout, "sharpen= %0.19f: ", config->sharpen);

                if (!scale_down(inner_context, bytes, bytes_count, 2, false, scale_to, new_w, new_h, new_w, new_h,
                                flow_interpolation_filter_Robidoux, config->sharpen, config->blur,
                                &experiment_bitmap)) {
                    ERR(c);
                }
                double dssim;
                visual_compare_two(inner_context, reference_bitmap, experiment_bitmap,
                                   "Compare ideal downscaling vs downscaling in decoder", &dssim, true, false, __FILE__,
                                   __func__, __LINE__);

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
            //}
            //}

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
            for (size_t i = TEST_IMAGE_FIRST; i <= TEST_IMAGE_LAST; i++) {
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
        fprintf(stdout,
                "\n\n\nLeast bad configuration (%d) for %d/8: (worst dssim %.010f, rank %.03f) - " // - f%d blur=%.2f "
                "sharpen=%.2f \n\n\n",
                (int)least_bad_ix, scale_to, least_bad_for_target,
                least_bad_relative, // least_bad.filter, least_bad.blur,
                least_bad.sharpen);

        fprintf(stdout, "Configuration            , ");
        for (size_t i = TEST_IMAGE_FIRST; i <= TEST_IMAGE_LAST; i++) {
            fprintf(stdout, "%s, ", test_image_names[i]);
        }
        fprintf(stdout, "\n");
        for (size_t config_ix = 0; config_ix < config_result_count; config_ix++) {
            struct config_result r = config_results[config_ix];
            // fprintf(stdout, "f%d blur=%.2f sharpen=%.2f, ", r.filter, r.blur, r.sharpen);
            fprintf(stdout, "sharpen=%.2f, ", r.sharpen);
            for (size_t i = TEST_IMAGE_FIRST; i <= TEST_IMAGE_LAST; i++) {
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

bool scale_down(flow_c * c, uint8_t * bytes, size_t bytes_count, bool scale_luma_spatially,
                bool gamma_correct_for_srgb_during_spatial_luma_scaling, int target_block_size, int block_scale_to_x,
                int block_scale_to_y, int scale_to_x, int scale_to_y, flow_interpolation_filter precise_filter,
                float post_sharpen, float blur, flow_bitmap_bgra ** ref)
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
        if (!flow_job_decoder_set_downscale_hints_by_placeholder_id(
                c, job, input_placeholder, block_scale_to_x, block_scale_to_y, block_scale_to_x, block_scale_to_y,
                scale_luma_spatially, gamma_correct_for_srgb_during_spatial_luma_scaling)) {
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

#endif
