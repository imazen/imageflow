/*
 * Copyright (c) Imazen LLC.
 * No part of this project, including this file, may be copied, modified,
 * propagated, or distributed except as permitted in COPYRIGHT.txt.
 * Licensed under the GNU Affero General Public License, Version 3.0.
 * Commercial licenses available at http://imageresizing.net/
 */
#ifdef _MSC_VER
#pragma unmanaged
#endif

#include "fastscaling_private.h"
#include <stdio.h>
#include <string.h>

bool test (int sx, int sy, BitmapPixelFormat sbpp, int cx, int cy, BitmapPixelFormat cbpp, bool transpose, bool flipx, bool flipy, InterpolationFilter filter);

bool test (int sx, int sy, BitmapPixelFormat sbpp, int cx, int cy, BitmapPixelFormat cbpp, bool transpose, bool flipx, bool flipy, InterpolationFilter filter)
{
    Context * context = Context_create();
    if (context == NULL){
        return false;
    }
    BitmapBgra * source = BitmapBgra_create(context, sx, sy, true, sbpp);
    if (source == NULL){
        Context_destroy(context);
        return false;
    }
    BitmapBgra * canvas = BitmapBgra_create(context, cx, cy, true, cbpp);
    if (canvas == NULL){
        BitmapBgra_destroy(context, source);
        Context_destroy(context);
        return false;
    }

    RenderDetails * details = RenderDetails_create_with(context, filter);
    if (details == NULL){
        BitmapBgra_destroy(context, source);
        BitmapBgra_destroy(context, canvas);
        Context_destroy(context);
        return false;
    }
    details->sharpen_percent_goal = 50;
    details->post_flip_x = flipx;
    details->post_flip_y = flipy;
    details->post_transpose = transpose;

    float sepia[25] = { .769f, .686f, .534f, 0, 0,
                        .189f, .168f, .131f, 0, 0,
                        0, 0, 0, 1, 0,
                        0, 0, 0, 0, 1,
                        0, 0, 0, 0, 0
                      };

    memcpy( &details->color_matrix_data, &sepia, sizeof sepia);

    details->apply_color_matrix = true;


    RenderDetails_render(context, details, source, canvas);
    RenderDetails_destroy(context, details);

    BitmapBgra_destroy(context, source);
    BitmapBgra_destroy(context, canvas);

    Context_destroy(context);
    Context_free_static_caches();

    return true;
}



int main(void)
{

    printf( "Running 3 x 20 operations\n" );
    for (int i =0; i < 20; i++) {
        if (InterpolationDetails_interpolation_filter_exists((InterpolationFilter)i)){
            test (1200, 100, Bgr24, 400, 223, Bgra32, true, true, false, (InterpolationFilter)i);
            test (44, 33, Bgr24, 800, 600, Bgra32, false, true, true, (InterpolationFilter)i);
            test (1200, 800, Bgra32, 200, 150, Bgra32, false, false, false, (InterpolationFilter)i);
        }
    }
    return 0;

}
