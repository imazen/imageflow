#include "imageflow_private.h"
#include "catch.hpp"

extern "C" void keep6() {}

// TODO: Test with opaque and transparent images
// TODO: Test using random dots instead of rectangles to see if overlaps are correct.



// TODO: Compare to a reference scaling

typedef void (*blockscale_fn)(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

blockscale_fn blockscale_funclist[]
    = { flow_scale_spatial_srgb_7x7, flow_scale_spatial_srgb_6x6, flow_scale_spatial_srgb_5x5,
        flow_scale_spatial_srgb_4x4, flow_scale_spatial_srgb_3x3, flow_scale_spatial_srgb_2x2,
        flow_scale_spatial_srgb_1x1, flow_scale_spatial_7x7,      flow_scale_spatial_6x6,
        flow_scale_spatial_5x5,      flow_scale_spatial_4x4,      flow_scale_spatial_3x3,
        flow_scale_spatial_2x2,      flow_scale_spatial_1x1 };

TEST_CASE("Test block downscaling", "")
{

    uint8_t input[64];
    memset(&input[0], 0, 64);
    uint8_t output[64];
    uint8_t * rows[8] = { &output[0],     &output[8],     &output[8 * 2], &output[8 * 3],
                          &output[8 * 4], &output[8 * 5], &output[8 * 6], &output[8 * 7] };

    for (size_t i = 0; i < sizeof(blockscale_funclist) / sizeof(blockscale_fn); i++) {
        blockscale_funclist[i](input, rows, 0);
    }
}


int64_t transpose(int w, int h, flow_pixel_format fmt, int runs)
{

    flow_c * c = flow_context_create();

    flow_bitmap_bgra * a = flow_bitmap_bgra_create(c, w, h, true, fmt);
    flow_bitmap_bgra_fill_rect(c, a, 0, 0, a->w, a->h, 0xFF0000FF);

    flow_bitmap_bgra * b = flow_bitmap_bgra_create(c, h, w, true, fmt);
    int64_t start = flow_get_high_precision_ticks();
    bool result;
    for (int i = 0; i < runs; i++) {
        result = flow_bitmap_bgra_transpose(c, a, b);
    }
    int64_t end = flow_get_high_precision_ticks();

    REQUIRE(result == true);

    // Comparison only makes sense for fast-path
    if (fmt == flow_bgra32) {
        flow_bitmap_bgra * reference = flow_bitmap_bgra_create(c, h, w, true, fmt);
        REQUIRE(flow_bitmap_bgra_transpose_slow(c, a, reference));

        REQUIRE(flow_bitmap_bgra_compare(c, b, reference, &result));

        REQUIRE(result == true);

        flow_bitmap_bgra_destroy(c, reference);
    }

    flow_bitmap_bgra_destroy(c, a);
    flow_bitmap_bgra_destroy(c, b);
    flow_context_destroy(c);
    return end - start;
}

int64_t scale2d(int w, int h, int to_w, int to_h, flow_pixel_format fmt, flow_working_floatspace floatspace, int runs)
{

    flow_c * c = flow_context_create();

    flow_bitmap_bgra * a = flow_bitmap_bgra_create(c, w, h, true, fmt);
    flow_bitmap_bgra_fill_rect(c, a, 0, 0, a->w, a->h, 0xFF0000FF);

    flow_bitmap_bgra * b = flow_bitmap_bgra_create(c, to_w, to_h, true, fmt);
    b->compositing_mode = flow_bitmap_compositing_replace_self;

    int64_t start = flow_get_high_precision_ticks();
    bool result;
    for (int i = 0; i < runs; i++) {

        struct flow_nodeinfo_scale2d_render_to_canvas1d info;
        info.interpolation_filter = flow_interpolation_filter_RobidouxFast;
        info.h = to_h;
        info.w = to_w;
        info.x = 0;
        info.y = 0;
        info.scale_in_colorspace = floatspace;
        info.sharpen_percent_goal = 0.0f;

        result = flow_node_execute_scale2d_render1d(c, a, b, &info);
    }
    int64_t end = flow_get_high_precision_ticks();

    flow_context_print_and_exit_if_err(c);
    REQUIRE(result == true);

    flow_bitmap_bgra_destroy(c, a);
    flow_bitmap_bgra_destroy(c, b);
    flow_context_destroy(c);
    return end - start;
}
int64_t flip_h(int w, int h, flow_pixel_format fmt, int runs)
{
    flow_c * c = flow_context_create();
    flow_bitmap_bgra * a = flow_bitmap_bgra_create(c, w, h, true, fmt);
    int64_t start = flow_get_high_precision_ticks();
    bool result;
    for (int i = 0; i < runs; i++) {
        result = flow_bitmap_bgra_flip_horizontal(c, a);
    }
    int64_t end = flow_get_high_precision_ticks();

    REQUIRE(result == true);
    flow_context_destroy(c);
    return end - start;
}

int64_t flip_v(int w, int h, flow_pixel_format fmt, int runs)
{
    flow_c * c = flow_context_create();
    flow_bitmap_bgra * a = flow_bitmap_bgra_create(c, w, h, true, fmt);
    int64_t start = flow_get_high_precision_ticks();
    bool result;
    for (int i = 0; i < runs; i++) {
        result = flow_bitmap_bgra_flip_vertical(c, a);
    }
    int64_t end = flow_get_high_precision_ticks();

    REQUIRE(result == true);
    flow_context_destroy(c);
    return end - start;
}

int64_t fill_rect(int w, int h, flow_pixel_format fmt, int runs)
{
    flow_c * c = flow_context_create();
    flow_bitmap_bgra * a = flow_bitmap_bgra_create(c, w, h, true, fmt);
    int64_t start = flow_get_high_precision_ticks();
    bool result;
    for (int i = 0; i < runs; i++) {
        result = flow_bitmap_bgra_fill_rect(c, a, 0, 0, a->w, a->h, 0xff00ffff);
    }
    int64_t end = flow_get_high_precision_ticks();

    REQUIRE(result == true);
    flow_context_destroy(c);
    return end - start;
}


// with optimizations
// Downscaling 2000x2000 (fmt 4) to 800x600 in space 0 took 28.27700ms
// Downscaling 2000x3373 (fmt 4) to 800x600 in space 0 took 39.97800ms
// Downscaling 3373x2000 (fmt 4) to 800x600 in space 0 took 55.82300ms
// Downscaling 3373x3373 (fmt 4) to 800x600 in space 0 took 86.47900ms
// Downscaling 2000x2000 (fmt 4) to 800x600 in space 1 took 35.90900ms
// Downscaling 2000x3373 (fmt 4) to 800x600 in space 1 took 41.41400ms
// Downscaling 3373x2000 (fmt 4) to 800x600 in space 1 took 46.17600ms
// Downscaling 3373x3373 (fmt 4) to 800x600 in space 1 took 64.85400ms
// Downscaling 2000x2000 (fmt 70) to 800x600 in space 0 took 23.66700ms
// Downscaling 2000x3373 (fmt 70) to 800x600 in space 0 took 33.61500ms
// Downscaling 3373x2000 (fmt 70) to 800x600 in space 0 took 38.35300ms
// Downscaling 3373x3373 (fmt 70) to 800x600 in space 0 took 57.24900ms
// Downscaling 2000x2000 (fmt 70) to 800x600 in space 1 took 27.36900ms
// Downscaling 2000x3373 (fmt 70) to 800x600 in space 1 took 37.64700ms
// Downscaling 3373x2000 (fmt 70) to 800x600 in space 1 took 42.05700ms
// Downscaling 3373x3373 (fmt 70) to 800x600 in space 1 took 58.35600ms

// Downscaling 2000x2000 (fmt 4) to 800x600 in space 0 took 25.35200ms
// Downscaling 2000x3373 (fmt 4) to 800x600 in space 0 took 40.26200ms
// Downscaling 3373x2000 (fmt 4) to 800x600 in space 0 took 49.52200ms
// Downscaling 3373x3373 (fmt 4) to 800x600 in space 0 took 89.35600ms
// Downscaling 2000x2000 (fmt 4) to 800x600 in space 1 took 41.83400ms
// Downscaling 2000x3373 (fmt 4) to 800x600 in space 1 took 48.20500ms
// Downscaling 3373x2000 (fmt 4) to 800x600 in space 1 took 45.61100ms
// Downscaling 3373x3373 (fmt 4) to 800x600 in space 1 took 68.49200ms
// Downscaling 2000x2000 (fmt 70) to 800x600 in space 0 took 21.65100ms
// Downscaling 2000x3373 (fmt 70) to 800x600 in space 0 took 33.15200ms
// Downscaling 3373x2000 (fmt 70) to 800x600 in space 0 took 36.12300ms
// Downscaling 3373x3373 (fmt 70) to 800x600 in space 0 took 56.46800ms
// Downscaling 2000x2000 (fmt 70) to 800x600 in space 1 took 26.31800ms
// Downscaling 2000x3373 (fmt 70) to 800x600 in space 1 took 38.19800ms
// Downscaling 3373x2000 (fmt 70) to 800x600 in space 1 took 40.43600ms
// Downscaling 3373x3373 (fmt 70) to 800x600 in space 1 took 61.35000ms
//
//


// TEST_CASE("Benchmark fill rect", "")
//{
//    for (int fmt = 4; fmt >= 3; fmt--)
//        for (int w = 1; w < 3000; w += 1373)
//            for (int h = 1; h < 3000; h += 1373) {
//                int runs = 50;
//
//                int ticks = fill_rect(w, h, (flow_pixel_format)fmt,runs);
//                double ms = ticks / runs * 1000.0 / (float)flow_get_profiler_ticks_per_second();
//                fprintf(stdout, "Fill rect %dx%d (fmt %d) took %.05fms\n", w, h, fmt, ms);
//            }
//}


#ifndef NDEBUG
const int MAX_RUNS=1;
#else
const int MAX_RUNS=1000;
#endif


TEST_CASE("Benchmark block downscaling", "")
{

    uint8_t input[64];
    memset(&input[0], 0, 64);
    uint8_t output[64];
    uint8_t * rows[8] = { &output[0],     &output[8],     &output[8 * 2], &output[8 * 3],
        &output[8 * 4], &output[8 * 5], &output[8 * 6], &output[8 * 7] };

    for (size_t i = 0; i < sizeof(blockscale_funclist) / sizeof(blockscale_fn); i++) {
        int reps = int_min(MAX_RUNS, 900);;
        int64_t start = flow_get_high_precision_ticks();
        for (int j = 0; j < reps; j++) {
            blockscale_funclist[i](input, rows, 0);
        }
        double ms = (flow_get_high_precision_ticks() - start) * 1000.0 / (float)flow_get_profiler_ticks_per_second();
        fprintf(stdout, "Block downscaling fn %d took %.05fms for %d reps (%0.2f megapixels)\n", (int)i, ms, reps,
                (float)(reps * 64) / 1000000.0f);
    }
}

TEST_CASE("Benchmark scale2d", "")
{
    flow_pixel_format formats[3] = { flow_bgra32, flow_bgr32 }; //, flow_bgr24 };
    flow_working_floatspace spaces[2] = { flow_working_floatspace_srgb, flow_working_floatspace_linear };
    for (int format_ix = 0; format_ix < 2; format_ix++)
        for (int space_ix = 0; space_ix < 2; space_ix++)
            for (int w = 2000; w < 4000; w += 1373)
                for (int h = 2000; h < 4000; h += 1373) {
                    int runs = int_min(MAX_RUNS, 5);

                    int ticks = scale2d(w, h, 800, 600, formats[format_ix], spaces[space_ix], runs);
                    double ms = ticks / runs * 1000.0 / (float)flow_get_profiler_ticks_per_second();
                    fprintf(stdout, "Downscaling %dx%d (fmt %d) to 800x600 in space %d took %.05fms\n", w, h,
                            formats[format_ix], spaces[space_ix], ms);
                }
}

TEST_CASE("Benchmark horizontal flip", "")
{

    for (int fmt = 4; fmt >= 3; fmt--)
        for (int w = 1; w < 3000; w += 1373)
            for (int h = 1; h < 3000; h += 1373) {
                int runs = int_min(MAX_RUNS, 50);

                int ticks = flip_h(w, h, (flow_pixel_format)fmt, runs);
                double ms = ticks / runs * 1000.0 / (float)flow_get_profiler_ticks_per_second();
                fprintf(stdout, "Horizontal flipping %dx%d (fmt %d) took %.05fms\n", w, h, fmt, ms);
            }
}

TEST_CASE("Benchmark vertical flip", "")
{
    for (int fmt = 4; fmt >= 3; fmt--)
        for (int w = 1; w < 3000; w += 1373)
            for (int h = 1; h < 3000; h += 1373) {
                int runs = int_min(MAX_RUNS, 50);

                int ticks = flip_v(w, h, (flow_pixel_format)fmt, runs);
                double ms = ticks / runs * 1000.0 / (float)flow_get_profiler_ticks_per_second();
                fprintf(stdout, "Vertical flipping %dx%d (fmt %d) took %.05fms\n", w, h, fmt, ms);
            }
}
TEST_CASE("Benchmark transpose", "")
{
    for (int fmt = 4; fmt >= 4; fmt--)
        for (int w = 1; w < 3000; w += 1373)
            for (int h = 1; h < 3000; h += 1373) {
                int runs = int_min(MAX_RUNS, 50);
                int ticks = transpose(w, h, (flow_pixel_format)fmt, runs);
                double ms = ticks / runs * 1000.0 / (float)flow_get_profiler_ticks_per_second();
                fprintf(stdout, "Transposing %dx%d to %dx%d (fmt %d) took %.05fms\n", w, h, h, w, fmt, ms);
            }
}
