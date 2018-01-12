#include <assert.h>
#include "imageflow_private.h"

#define ERR2(c)                                                                                                        \
    if (has_err(c, __FILE__, __LINE__, __func__)) {                                                                    \
        return 42;                                                                                                     \
    }

static int64_t transpose(int w, int h, flow_pixel_format fmt, int runs)
{

    flow_c * c = flow_context_create();

    struct flow_bitmap_bgra * a = flow_bitmap_bgra_create(c, w, h, true, fmt);
    flow_bitmap_bgra_fill_rect(c, a, 0, 0, a->w, a->h, 0xFF0000FF);

    struct flow_bitmap_bgra * b = flow_bitmap_bgra_create(c, h, w, true, fmt);
    int64_t start = flow_get_high_precision_ticks();

    for (int i = 0; i < runs; i++) {
        if (!flow_bitmap_bgra_transpose(c, a, b))
            exit(77);
    }
    int64_t end = flow_get_high_precision_ticks();

    flow_bitmap_bgra_destroy(c, a);
    flow_bitmap_bgra_destroy(c, b);

    return end - start;
}

void profile_main(void)
{
    int fmt = 4;
    for (int w = 1; w < 5000; w += 631)
        for (int h = 1; h < 5000; h += 631) {
            int runs = 10;
            int ticks = transpose(w, h, (flow_pixel_format)fmt, runs);
            double ms = ticks / runs * 1000.0 / (float)flow_get_profiler_ticks_per_second();
            fprintf(stdout, "Transposing %dx%d to %dx%d (fmt %d) took %.05fms\n", w, h, h, w, fmt, ms);
        }
}
