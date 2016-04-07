#pragma once
#include "helpers.h"

#ifdef __cplusplus
extern "C" {
#endif

bool diff_image_pixels(flow_c * c, struct flow_bitmap_bgra * a, struct flow_bitmap_bgra * b, size_t * diff_count,
                       size_t limit_output_count);

bool load_image(flow_c * c, char * checksum, struct flow_bitmap_bgra ** ref, void * bitmap_owner);

bool get_image_dimensions(flow_c * c, uint8_t * bytes, size_t bytes_count, int32_t * width, int32_t * height);

bool visual_compare(flow_c * c, struct flow_bitmap_bgra * bitmap, const char * name, bool store_checksums,
                    const char * file_, const char * func_, int line_number);

bool visual_compare_two(flow_c * c, struct flow_bitmap_bgra * a, struct flow_bitmap_bgra * b,
                        const char * comparison_title, double * out_dssim, bool save_bitmaps, bool generate_visual_diff,
                        const char * file_, const char * func_, int line_number);
int64_t flow_getline(char ** lineptr, size_t * n, FILE * stream);

#ifdef __cplusplus
}
#endif
