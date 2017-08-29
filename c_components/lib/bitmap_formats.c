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

#include "imageflow_private.h"
#include <string.h>

const int FLOW_MAX_BYTES_PP = 16;

// Ha, ha. The result of sx * sy * FLOW_MAX_BYTES_PP will overflow if the result is bigger than INT_MAX
// causing it to wrap around and be true. This is what the sx < INT_MAX / sy code does

static bool are_valid_bitmap_dimensions(int sx, int sy)
{
    return (sx > 0 && sy > 0 // positive dimensions
            && sx < INT_MAX / sy // no integer overflow
            && sx * FLOW_MAX_BYTES_PP < ((INT_MAX - FLOW_MAX_BYTES_PP) / sy)); // then we can safely check
}

typedef enum allocation_kind {
    allocation_kind_none,
    allocation_kind_bgra_header,
    allocation_kind_float_header,
    allocation_kind_bgra_pixbuf,
    allocation_kind_float_pixbuf,
} allocation_kind;

// Uncomment below for troubleshooting allocation/free order
// static bool header_destructor(flow_c * c, void * thing){
//    printf("Destroying %p\n", thing);
//    return true;
//}
// static bool pixbuf_destructor(flow_c * c, void * thing){
//    printf("Destroying %p\n", thing);
//
//    return true;
//}

static bool bitmap_allocation_hook(flow_c * context, void * ptr, size_t byte_count, allocation_kind kind)
{

    //    flow_destructor_function on_destroy = header_destructor;
    //    if (kind == allocation_kind_bgra_pixbuf || kind == allocation_kind_float_pixbuf){
    //        on_destroy = pixbuf_destructor;
    //        printf("Allocating %p-%p (pixel buffer) of size %zd\n", ptr, (void *)((size_t)ptr + byte_count),
    //        byte_count);
    //    }else{
    //        printf("Allocating %p-%p (bitmap header) of size %zd\n", ptr, (void*)((size_t)ptr + byte_count),
    //        byte_count);
    //    }
    //
    //    if (!flow_set_destructor(context,ptr, on_destroy)){
    //        FLOW_error_return(context);
    //    }
    return true;
}

uint32_t flow_pixel_format_bytes_per_pixel(flow_pixel_format format) {
    switch(format){
        case flow_bgr24: return 3;
        case flow_bgra32: return 4;
        case flow_bgr32: return 4;
        case flow_gray8: return 1;
    }
    fprintf( stderr, "Invalid flow_pixel_format %d", format);
    exit(70);
}
flow_pixel_format flow_effective_pixel_format(struct flow_bitmap_bgra * b) {
    return b->fmt;
}
uint32_t flow_pixel_format_channels(flow_pixel_format format) {
    switch(format){
        case flow_bgr24: return 3;
        case flow_bgra32: return 4;
        case flow_bgr32: return 3;
        case flow_gray8: return 1;
    }
    fprintf( stderr, "Invalid flow_pixel_format %d", format);
    exit(70);
}


FLOW_HINT_HOT FLOW_HINT_PURE

    struct flow_bitmap_bgra *
    flow_bitmap_bgra_create_header(flow_c * context, int sx, int sy)
{
    struct flow_bitmap_bgra * im;
    if (!are_valid_bitmap_dimensions(sx, sy)) {
        FLOW_error(context, flow_status_Invalid_dimensions);
        return NULL;
    }
    size_t byte_count = sizeof(struct flow_bitmap_bgra);
    im = (struct flow_bitmap_bgra *)FLOW_calloc(context, 1, byte_count);
    if (im == NULL) {
        FLOW_error(context, flow_status_Out_of_memory);
        return NULL;
    }
    if (!bitmap_allocation_hook(context, im, byte_count, allocation_kind_bgra_header)) {
        FLOW_destroy(context, im);
        FLOW_error_return_null(context);
    }
    im->w = sx;
    im->h = sy;
    im->pixels = NULL;
    return im;
}

struct flow_bitmap_bgra * flow_bitmap_bgra_create(flow_c * context, int sx, int sy, bool zeroed,
                                                  flow_pixel_format format)
{
    struct flow_bitmap_bgra * im = flow_bitmap_bgra_create_header(context, sx, sy);
    if (im == NULL) {
        FLOW_add_to_callstack(context);
        return NULL;
    }
    im->fmt = format;

    int unpadded_stride = im->w * flow_pixel_format_bytes_per_pixel(im->fmt);
    // Pad rows to 64 bytes (this does not guarantee memory alignment, just stride alignment)
    const int alignment = 64;
    int padding = unpadded_stride % alignment == 0 ? 0 : (alignment - unpadded_stride % alignment);

    im->stride = unpadded_stride + padding;

    size_t byte_count = im->h * im->stride;
    if (zeroed) {
        im->pixels = (unsigned char *)FLOW_calloc_owned(context, byte_count, sizeof(unsigned char), im);
    } else {
        im->pixels = (unsigned char *)FLOW_malloc_owned(context, byte_count, im);
    }
    if (im->pixels == NULL) {
        FLOW_destroy(context, im);
        FLOW_error(context, flow_status_Out_of_memory);
        return NULL;
    }
    if (!bitmap_allocation_hook(context, im->pixels, byte_count, allocation_kind_bgra_pixbuf)) {
        FLOW_destroy(context, im);
        FLOW_error_return_null(context);
    }
    return im;
}

void flow_bitmap_bgra_destroy(flow_c * context, struct flow_bitmap_bgra * im) { FLOW_destroy(context, im); }

struct flow_bitmap_float * flow_bitmap_float_create_header(flow_c * context, int sx, int sy, int channels)
{
    struct flow_bitmap_float * im;

    if (!are_valid_bitmap_dimensions(sx, sy)) {
        FLOW_error(context, flow_status_Invalid_dimensions);
    }
    size_t byte_count = sizeof(struct flow_bitmap_float);
    im = (struct flow_bitmap_float *)FLOW_calloc(context, 1, byte_count);
    if (im == NULL) {
        FLOW_error(context, flow_status_Out_of_memory);
        return NULL;
    }
    if (!bitmap_allocation_hook(context, im, byte_count, allocation_kind_float_header)) {
        FLOW_destroy(context, im);
        FLOW_error_return_null(context);
    }
    im->w = sx;
    im->h = sy;
    im->pixels = NULL;
    im->pixels_borrowed = true;
    im->channels = channels;

    int unpadded_stride = sx * channels;
    // Pad rows to 64 bytes (this does not guarantee memory alignment, just stride alignment)
    const int alignment = 16;
    int padding = unpadded_stride % alignment == 0 ? 0 : (alignment - (unpadded_stride % alignment));

    im->float_stride = unpadded_stride + padding;
    im->float_count = im->float_stride * sy;
    im->alpha_meaningful = channels == 4;
    im->alpha_premultiplied = true;
    return im;
}

struct flow_bitmap_float * flow_bitmap_float_create(flow_c * context, int sx, int sy, int channels, bool zeroed)
{
    struct flow_bitmap_float * im = flow_bitmap_float_create_header(context, sx, sy, channels);
    if (im == NULL) {
        FLOW_add_to_callstack(context);
        return NULL;
    }
    im->pixels_borrowed = false;

    size_t byte_count = im->float_count * sizeof(float);
    if (zeroed) {
        im->pixels = (float *)FLOW_calloc_owned(context, 1, byte_count, im);
    } else {
        im->pixels = (float *)FLOW_malloc_owned(context, byte_count, im);
    }
    if (im->pixels == NULL) {
        FLOW_destroy(context, im);
        FLOW_error(context, flow_status_Out_of_memory);
        return NULL;
    }
    if (!bitmap_allocation_hook(context, im->pixels, byte_count, allocation_kind_float_pixbuf)) {
        FLOW_destroy(context, im);
        FLOW_error_return_null(context);
    }

    return im;
}

void flow_bitmap_float_destroy(flow_c * context, struct flow_bitmap_float * im) { FLOW_destroy(context, im); }

bool flow_bitmap_bgra_compare(flow_c * c, struct flow_bitmap_bgra * a, struct flow_bitmap_bgra * b, bool * equal_out)
{
    if (a == NULL || b == NULL) {
        FLOW_error(c, flow_status_Null_argument);
        return false;
    }
    *equal_out = false;
    if (a->w != b->w || a->h != b->h || a->fmt != b->fmt) {
        return true;
    }
    // TODO: compare bgcolor and alpha_meaningful?
    // Dont' compare the full stride (padding), it could be windowed!
    uint32_t row_length = umin(b->stride, b->w * flow_pixel_format_bytes_per_pixel(b->fmt));
    for (uint32_t i = 0; i < b->h; i++) {
        if (memcmp(a->pixels + (i * a->stride), b->pixels + (i * b->stride), row_length) != 0) {
            *equal_out = false;
            return true;
        }
    }
    *equal_out = true;
    return true;
}

bool flow_bitmap_bgra_fill_rect(flow_c * c, struct flow_bitmap_bgra * b, uint32_t x1, uint32_t y1, uint32_t x2,
                                uint32_t y2, uint32_t color_srgb_argb)
{
    if (x1 >= x2 || y1 >= y2 || y2 > b->h || x2 > b->w) {
        FLOW_error(c, flow_status_Invalid_argument);
        // Either out of bounds or has a width or height of zero.
        return false;
    }

    uint8_t step = flow_pixel_format_bytes_per_pixel(b->fmt);

    uint8_t * topleft = b->pixels + (b->stride * y1) + step * x1;

    size_t rect_width_bytes = step * (x2 - x1);

    uint32_t color = color_srgb_argb;
    if (step == 1) {
        // TODO: use gamma-correct grayscale conversion
        FLOW_error(c, flow_status_Unsupported_pixel_format);
        return false;
    } else if (step == 3) {
        // we just ignore the alpha bits
    }
    for (uint32_t byte_offset = 0; byte_offset < rect_width_bytes; byte_offset += step) {
        memcpy(topleft + byte_offset, &color, step);
    }
    // Copy downwards
    for (uint32_t y = 1; y < (y2 - y1); y++) {
        memcpy(topleft + (b->stride * y), topleft, rect_width_bytes);
    }
    return true;
}
