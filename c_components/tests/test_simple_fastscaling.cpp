/*
 * Copyright (c) Imazen LLC.
 * No part of this project, including this file, may be copied, modified,
 * propagated, or distributed except as permitted in COPYRIGHT.txt.
 * Licensed under the GNU Affero General Public License, Version 3.0.
 * Commercial licenses available at http://imageresizing.net/
 */
#include "catch.hpp"
#include "imageflow_private.h"
#include <stdio.h>
#include <string.h>
#include "jpeglib.h"
#include "png.h"

extern "C" void keep7() {}

bool test(int sx, int sy, flow_pixel_format sbpp, int cx, int cy, flow_pixel_format cbpp, bool transpose, bool flipx,
          bool flipy, flow_interpolation_filter filter);

bool test(int sx, int sy, flow_pixel_format sbpp, int cx, int cy, flow_pixel_format cbpp, bool transpose, bool flipx,
          bool flipy, flow_interpolation_filter filter)
{
    flow_c * context = flow_context_create();
    if (context == NULL) {
        return false;
    }
    struct flow_bitmap_bgra * source = flow_bitmap_bgra_create(context, sx, sy, true, sbpp);
    if (source == NULL) {
        flow_context_destroy(context);
        return false;
    }
    struct flow_bitmap_bgra * canvas = flow_bitmap_bgra_create(context, cx, cy, true, cbpp);
    if (canvas == NULL) {
        flow_bitmap_bgra_destroy(context, source);
        flow_context_destroy(context);
        return false;
    }

    struct flow_RenderDetails * details = flow_RenderDetails_create_with(context, filter);
    if (details == NULL) {
        flow_bitmap_bgra_destroy(context, source);
        flow_bitmap_bgra_destroy(context, canvas);
        flow_context_destroy(context);
        return false;
    }
    details->sharpen_percent_goal = 50;
    details->post_flip_x = flipx;
    details->post_flip_y = flipy;
    details->post_transpose = transpose;

    float sepia[25]
        = { .769f, .686f, .534f, 0, 0, .189f, .168f, .131f, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0 };

    memcpy(&details->color_matrix_data, &sepia, sizeof sepia);

    details->apply_color_matrix = true;

    flow_RenderDetails_render(context, details, source, canvas);
    flow_RenderDetails_destroy(context, details);

    flow_bitmap_bgra_destroy(context, source);
    flow_bitmap_bgra_destroy(context, canvas);

    flow_context_destroy(context);

    return true;
}

TEST_CASE("Render - Running 3 x 20 operations", "[fastscaling]")
{
    for (int i = 0; i < 20; i++) {
        if (flow_interpolation_filter_exists((flow_interpolation_filter)i)) {
            test(1200, 100, flow_bgr24, 400, 223, flow_bgra32, true, true, false, (flow_interpolation_filter)i);
            test(44, 33, flow_bgr24, 800, 600, flow_bgra32, false, true, true, (flow_interpolation_filter)i);
            test(1200, 800, flow_bgra32, 200, 150, flow_bgra32, false, false, false, (flow_interpolation_filter)i);
        }
    }
}
