#pragma once
#include "helpers.h"

#ifdef __cplusplus
extern "C" {
#endif

bool diff_image_pixels(flow_c * c, struct flow_bitmap_bgra * a, struct flow_bitmap_bgra * b, size_t * diff_count,
                       size_t * total_delta, size_t print_this_many_differences, size_t stop_counting_at);

bool load_image(flow_c * c, char * checksum, struct flow_bitmap_bgra ** ref, void * bitmap_owner,
                const char * storage_relative_to);

bool get_image_dimensions(flow_c * c, uint8_t * bytes, size_t bytes_count, int32_t * width, int32_t * height);

bool visual_compare(flow_c * c, struct flow_bitmap_bgra * bitmap, const char * name, bool store_checksums,
                    size_t off_by_one_byte_differences_permitted, const char * file_, const char * func_,
                    int line_number, const char * storage_relative_to);

bool visual_compare_two(flow_c * c, struct flow_bitmap_bgra * a, struct flow_bitmap_bgra * b,
                        const char * comparison_title, double * out_dssim, bool save_bitmaps, bool generate_visual_diff,
                        const char * file_, const char * func_, int line_number, const char * storage_relative_to);
int64_t flow_getline(char ** lineptr, size_t * n, FILE * stream);

#ifdef __cplusplus
}
#endif
