#include "catch.hpp"
#include "helpers.h"
#include "test_weighting_helpers.h"

TEST_CASE("Test contrib windows", "[fastscaling]")
{

    char msg[256];

    flow_context context;
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

    flow_context context;
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
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Cubic, msg, 1, 2, 1, 0.08, 2)
          == nullptr);

    // BSpline is a smoothing filter, always positive
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_CubicBSpline, msg, 0, 0, 1.75,
                      0.08, 2) == nullptr);

    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Mitchell, msg, 1.0f, 1.75f, 1,
                      0.08, 1.75) == nullptr);

    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Robidoux, msg, 1, 1.65, 1, 0.08,
                      1.75) == nullptr);
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_RobidouxSharp, msg, 1, 1.8, 1,
                      0.08, 1.8) == nullptr);

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
