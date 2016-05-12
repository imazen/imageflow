#include <lib/trim_whitespace.h>
#include "catch.hpp"
#include "helpers_visual.h"

// Test with opaque and transparent images
// Test using random dots instead of rectangles to see if overlaps are correct.

flow_rect test_detect_content_for(uint32_t w, uint32_t h, uint32_t x1, uint32_t y1, uint32_t x2, uint32_t y2,
                                  uint32_t color_srgb_argb)
{
    flow_c * c = flow_context_create();

    flow_bitmap_bgra * b = flow_bitmap_bgra_create(c, w, h, true, flow_bgra32);

    flow_bitmap_bgra_fill_rect(c, b, 0, 0, w, h, 0xFF000000);
    flow_bitmap_bgra_fill_rect(c, b, x1, y1, x2, y2, color_srgb_argb);

    char path[256];
    flow_snprintf(&path[0], sizeof(path), "rect_%i_%i_%i_%i.png", x1, y1, x2, y2);
    write_frame_to_disk(c, &path[0], b);

    flow_context_print_and_exit_if_err(c);

    flow_rect r = detect_content(c, b, 1);
    flow_context_print_and_exit_if_err(c);

    flow_context_destroy(c);
    return r;
}
TEST_CASE("Test detect_content", "")
{
    flow_rect r;

    r = test_detect_content_for(10, 10, 1, 1, 9, 9, 0xFF0000FF);

    CAPTURE(r.x1);
    CAPTURE(r.y1);
    CAPTURE(r.x2);
    CAPTURE(r.y2);
    REQUIRE(r.x2 == 9);
    REQUIRE(r.y2 == 9);
    REQUIRE(r.x1 == 1);
    REQUIRE(r.y1 == 1);

    r = test_detect_content_for(100, 100, 2, 3, 70, 70, 0xFF0000FF);

    CAPTURE(r.x1);
    CAPTURE(r.y1);
    CAPTURE(r.x2);
    CAPTURE(r.y2);
    REQUIRE(r.x2 == 70);
    REQUIRE(r.y2 == 70);
    REQUIRE(r.x1 == 2);
    REQUIRE(r.y1 == 3);
}
