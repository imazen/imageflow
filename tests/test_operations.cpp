#include <lib/trim_whitespace.h>
#include "catch.hpp"
#include "helpers_visual.h"

// Test with opaque and transparent images
// Test using random dots instead of rectangles to see if overlaps are correct.

TEST_CASE("Test detect_content", "")
{
    flow_c * c = flow_context_create();

    flow_bitmap_bgra * b = flow_bitmap_bgra_create(c, 100, 100, true, flow_bgra32);

    flow_bitmap_bgra_fill_rect(c, b, 0, 0, 100, 100, 0xFF000000);
    flow_bitmap_bgra_fill_rect(c, b, 2, 2, 70, 70, 0xFF0000FF);
    flow_context_print_and_exit_if_err(c);

    flow_rect r = detect_content(c, b, 1);
    flow_context_print_and_exit_if_err(c);

    REQUIRE(r.x1 == 2);
    REQUIRE(r.y1 == 0);
    REQUIRE(r.x2 == 71);
    REQUIRE(r.y2 == 71);

    flow_context_destroy(c);
}
