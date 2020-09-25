#include "catch.hpp" // If this fails, download https://github.com/catchorg/Catch2/releases/download/v1.11.0/catch.hpp
#include "helpers.h"
#include "test_weighting_helpers.h"

extern "C" void keep9() {}

TEST_CASE("Test contrib windows", "[fastscaling]")
{

    char msg[256];

    flow_c context;
    flow_context_initialize(&context);

    bool r = test_contrib_windows(&context, msg);

    if (!r)
        FAIL(msg);
    REQUIRE(r);
    REQUIRE(flow_context_begin_terminate(&context) == true);
    flow_context_end_terminate(&context);
}

TEST_CASE("Test Weighting", "[fastscaling]")
{

    char msg[256];

    flow_c context;
    flow_context_initialize(&context);

    // These have window = 1, and shouldnt' have negative values. They should also end at 1
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Hermite, msg, 0, 0, 0.99, 0.08, 1)
          == nullptr);
    // Also called a linear filter
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Triangle, msg, 0, 0, 0.99, 0.08, 1)
          == nullptr);
    // Box should only return a value from -0.5..0.5
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Box, msg, 0, 0, 0.51, 0.001, 0.51)
          == nullptr);

    // These should go negative between x=1 and x=2, but should end at x=2
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_CatmullRom, msg, 1, 2, 1, 0.08, 2)
          == nullptr);
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_CubicFast, msg, 1, 2, 1, 0.08, 2)
          == nullptr);
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Cubic, msg, 0, 0, 2.0, 0.08, 2)
          == nullptr);

    // BSpline is a smoothing filter, always positive
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_CubicBSpline, msg, 0, 0, 1.75,
                      0.08, 2) == nullptr);

    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Mitchell, msg, 8.0 / 7.0, 2.0, 1,
                      0.08, 2.0) == nullptr);

    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Robidoux, msg, 1.1685777620836932,
                      2, 1, 0.08, 2) == nullptr);
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_RobidouxSharp, msg,
                      1.105822933719019, 2, 1, 0.08, 2) == nullptr);

    // Sinc filters. These have second crossings.
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_RawLanczos2, msg, 1, 2, 1, 0.08, 2)
          == nullptr);
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_RawLanczos2Sharp, msg, 0.954, 1.86,
                      1, 0.08, 2) == nullptr);

    // These should be negative between x=1 and x=2, positive between 2 and 3, but should end at 3

    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_RawLanczos3, msg, 1, 2, 1, 0.1, 3)
          == nullptr);
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_RawLanczos3Sharp, msg, 0.98,
                      1.9625, 1, 0.1, 3) == nullptr);

    ///
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Lanczos2, msg, 1, 2, 1, 0.08, 2)
          == nullptr);

    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Lanczos2Sharp, msg, 0.954, 1.86, 1,
                      0.08, 2) == nullptr);

    // These should be negative between x=1 and x=2, positive between 2 and 3, but should end at 3

    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Lanczos, msg, 1, 2, 1, 0.1, 3)
          == nullptr);

    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_LanczosSharp, msg, 0.98, 1.9625, 1,
                      0.1, 2.943) == nullptr);

    REQUIRE(flow_context_begin_terminate(&context) == true);
    flow_context_end_terminate(&context);
}

TEST_CASE("Verify weights are symmetric and bounded", "[fastscaling]")
{
    flow_c context;
    flow_c * c = &context;
    flow_context_initialize(&context);

    // Loop through every filter type, and for each filter type, try a variety of scaling ratios.
    // For each scaling ratio, export a row where columns contain the weights for the input pixels
    // filter, 2, from, 200, to, 300, weights, src, 0, (0.00001, 0.00200, 1.2200), 1, ...
    int32_t filter_id = 1;
    int32_t scalings[] = { /*downscale to 1px*/ 1, 1, 2, 1, 3, 1, 4, 1,  5, 1, 6, 1, 7, 1, 17, 1,
        /*upscale from 2px*/ 2, 3, 2, 4, 2, 5, 2, 17,
        /*other*/ 11,           7, 7, 3,
        /* IDCT kernel sizes */ 8, 8, 8, 7, 8, 6, 8, 5, 8, 4, 8, 3, 8, 2, 8, 1 };

    flow_interpolation_filter first_filter = flow_interpolation_filter_RobidouxFast;
    //flow_interpolation_filter first_filter = flow_interpolation_filter_NCubicSharp;
    flow_interpolation_filter last_filter = flow_interpolation_filter_NCubicSharp;
    uint32_t scaling_ix;
    for (filter_id = (int32_t)first_filter; filter_id <= (int32_t)last_filter; filter_id++) {
        for (scaling_ix = 0; scaling_ix < sizeof(scalings) / sizeof(int32_t); scaling_ix += 2) {
            int32_t from_width = scalings[scaling_ix];
            int32_t to_width = scalings[scaling_ix + 1];
            flow_interpolation_filter filter = (flow_interpolation_filter)filter_id;

            struct flow_interpolation_details * details = flow_interpolation_details_create_from(&context, filter);

            ERR(c);

            struct flow_interpolation_line_contributions * lct
                = flow_interpolation_line_contributions_create(&context, to_width, from_width, details);

            CAPTURE(filter);
            CAPTURE(from_width);
            CAPTURE(to_width);
            if (flow_context_has_error(c)) {

                ERR(c);
            }

            for (uint32_t output_pixel = 0; output_pixel < lct->LineLength / 2; output_pixel++) {

                uint32_t opposite_output_pixel = lct->LineLength - 1 - output_pixel;
                CAPTURE(output_pixel);
                CAPTURE(opposite_output_pixel);
                struct flow_interpolation_pixel_contributions * current = &lct->ContribRow[output_pixel];

                struct flow_interpolation_pixel_contributions * opposite = &lct->ContribRow[opposite_output_pixel];

                // printf("%d[%d,%d] vs %d[%d,%d]\n", output_pixel, current->Left, current->Right,
                // opposite_output_pixel, opposite->Left, opposite->Right);
                CAPTURE(current->Left);
                CAPTURE(current->Right);
                CAPTURE(opposite->Left);
                CAPTURE(opposite->Right);
                REQUIRE(from_width - 1 - opposite->Right == current->Left); // "Outer bounds must be symmetrical."

                REQUIRE(from_width - 1 - current->Right == opposite->Left); // "Outer bounds must be symmetrical."

                for (int32_t ix = current->Left; ix <= current->Right; ix++) {

                    REQUIRE(fabs(current->Weights[ix - current->Left] - opposite->Weights[current->Right - ix])
                            < 0.00001);

                    REQUIRE(fabs(current->Weights[ix - current->Left]) < 5);
                }
            }

            FLOW_destroy(c, lct);
        }
    }
    REQUIRE(flow_context_begin_terminate(&context) == true);
    flow_context_end_terminate(&context);
}

TEST_CASE("Test output weights", "[fastscaling]")
{

    flow_c context;
    flow_c * c = &context;
    flow_context_initialize(&context);

    char filename[2048];
    if (!create_path_from_relative(&context, __FILE__, true, filename, 2048, "/visuals/weights.txt")) {
        ERR(c);
    }

    FILE * output;
    if ((output = fopen(filename, "w")) == NULL) {
        ERR(c);
    }

    fprintf(output, "filter, from_width, to_width, weights");

    // Loop through every filter type, and for each filter type, try a variety of scaling ratios.
    // For each scaling ratio, export a row where columns contain the weights for the input pixels
    // filter, 2, from, 200, to, 300, weights, src, 0, (0.00001, 0.00200, 1.2200), 1, ...
    int32_t filter_id = 1;
    int32_t scalings[] = { /*downscale to 1px*/ 1, 1, 2, 1, 3, 1, 4, 1,  5, 1, 6, 1, 7, 1, 17, 1,
                           /*upscale from 2px*/ 2, 3, 2, 4, 2, 5, 2, 17,
                           /*other*/ 11,           7, 7, 3,
                            /* IDCT kernel sizes */ 8, 8, 8, 7, 8, 6, 8, 5, 8, 4, 8, 3, 8, 2, 8, 1 };
    flow_interpolation_filter last_filter = flow_interpolation_filter_NCubicSharp;
    uint32_t scaling_ix;
    for (filter_id = 1; filter_id <= (int32_t)last_filter; filter_id++) {
        for (scaling_ix = 0; scaling_ix < sizeof(scalings) / sizeof(int32_t); scaling_ix += 2) {
            int32_t from_width = scalings[scaling_ix];
            int32_t to_width = scalings[scaling_ix + 1];
            flow_interpolation_filter filter = (flow_interpolation_filter)filter_id;

            struct flow_interpolation_details * details = flow_interpolation_details_create_from(&context, filter);

            ERR(c);

            struct flow_interpolation_line_contributions * lct
                = flow_interpolation_line_contributions_create(&context, to_width, from_width, details);

            if (flow_context_has_error(c)) {
                CAPTURE(filter);
                ERR(c);
            }

            fprintf(output, "\r\nfilter_%02d (%2dpx to %2dpx):", filter_id, from_width, to_width);

            for (uint32_t output_pixel = 0; output_pixel < lct->LineLength; output_pixel++) {
                struct flow_interpolation_pixel_contributions * current = &lct->ContribRow[output_pixel];

                fprintf(output, " x=%i from ", output_pixel);

                for (int32_t ix = current->Left; ix <= current->Right; ix++) {
                    float weight = current->Weights[ix - current->Left];
                    fprintf(output, (ix == current->Left) ? "(" : " ");
                    fprintf(output, "%.06f", weight);
                }
                fprintf(output, "),");
            }

            FLOW_destroy(&context, lct);
        }
    }

    fclose(output);

    char reference_filename[2048];
    if (!create_path_from_relative(&context, __FILE__, true, reference_filename, 2048,
                                   "/visuals/reference_weights.txt")) {
        ERR(c);
    }

    char result_buffer[2048];
    memset(&result_buffer[0], 0, 2048);
    bool are_equal;
    REQUIRE(flow_compare_file_contents(&context, filename, reference_filename, &result_buffer[0], 2048, &are_equal));
    ERR(c);
    CAPTURE(result_buffer);
    if (!are_equal) {
        char diff_command[4096];

        flow_snprintf(diff_command, 4096, "diff -w %s %s", filename, reference_filename);
        int ignore_result = system(diff_command); // just for the benefit of STDOUT
    }
    REQUIRE(are_equal);

    REQUIRE(flow_context_begin_terminate(&context) == true);
    flow_context_end_terminate(&context);
}
