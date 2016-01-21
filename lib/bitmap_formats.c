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

const int MAX_BYTES_PP = 16;


// Ha, ha. The result of sx * sy * MAX_BYTES_PP will overflow if the result is bigger than INT_MAX
// causing it to wrap around and be true. This is what the sx < INT_MAX / sy code does

static bool are_valid_bitmap_dimensions(int sx, int sy)
{
    return (
               sx > 0 && sy > 0 // positive dimensions
               && sx < INT_MAX / sy // no integer overflow
               && sx * MAX_BYTES_PP < ((INT_MAX - MAX_BYTES_PP) / sy)); // then we can safely check
}


uint32_t BitmapPixelFormat_bytes_per_pixel (BitmapPixelFormat format)
{
    return (uint32_t)format;
}


BitmapBgra * BitmapBgra_create_header(Context * context, int sx, int sy)
{
    BitmapBgra * im;
    if (!are_valid_bitmap_dimensions(sx, sy)) {
        CONTEXT_error(context, Invalid_BitmapBgra_dimensions);
        return NULL;
    }
    im = (BitmapBgra *)CONTEXT_calloc(context, 1, sizeof(BitmapBgra));
    if (im == NULL) {
        CONTEXT_error(context, Out_of_memory);
        return NULL;
    }
    im->w = sx;
    im->h = sy;
    im->pixels = NULL;
    im->pixels_readonly = true;
    im->stride_readonly = true;
    im->borrowed_pixels = true;
    im->can_reuse_space = false;
    return im;
}


BitmapBgra * BitmapBgra_create(Context * context, int sx, int sy, bool zeroed, BitmapPixelFormat format)
{
    BitmapBgra * im = BitmapBgra_create_header(context, sx, sy);
    if (im == NULL) {
        CONTEXT_add_to_callstack (context);
        return NULL;
    }
    im->fmt = format;
    im->stride = im->w * BitmapPixelFormat_bytes_per_pixel(im->fmt);
    im->pixels_readonly = false;
    im->stride_readonly = false;
    im->borrowed_pixels = false;
    im->alpha_meaningful = im->fmt == Bgra32;
    if (zeroed) {
        im->pixels = (unsigned char *)CONTEXT_calloc(context, im->h * im->stride, sizeof(unsigned char));
    } else {
        im->pixels = (unsigned char *)CONTEXT_malloc(context, im->h * im->stride);
    }
    if (im->pixels == NULL) {
        CONTEXT_free(context, im);
        CONTEXT_error(context, Out_of_memory);
        return NULL;
    }
    return im;
}

void BitmapBgra_destroy(Context* context, BitmapBgra * im)
{
    if (im == NULL) return;
    if (!im->borrowed_pixels) {
        CONTEXT_free(context, im->pixels);
    }
    CONTEXT_free(context, im);
}


BitmapFloat * BitmapFloat_create_header(Context* context,int sx, int sy, int channels)
{
    BitmapFloat * im;

    if (!are_valid_bitmap_dimensions(sx, sy)) {
        CONTEXT_error(context, Invalid_BitmapFloat_dimensions);
    }

    im = (BitmapFloat *)CONTEXT_calloc(context,1,sizeof(BitmapFloat));
    if (im == NULL) {
        CONTEXT_error(context, Out_of_memory);
        return NULL;
    }
    im->w = sx;
    im->h = sy;
    im->pixels = NULL;
    im->pixels_borrowed = true;
    im->channels = channels;
    im->float_stride = sx * channels;
    im->float_count = im->float_stride * sy;
    im->alpha_meaningful = channels == 4;
    im->alpha_premultiplied = true;
    return im;
}


BitmapFloat * BitmapFloat_create(Context* context, int sx, int sy, int channels, bool zeroed)
{
    BitmapFloat * im = BitmapFloat_create_header(context, sx, sy, channels);
    if (im == NULL) {
        CONTEXT_add_to_callstack (context);
        return NULL;
    }
    im->pixels_borrowed = false;
    if (zeroed) {
        im->pixels = (float*)CONTEXT_calloc(context,im->float_count, sizeof(float));
    } else {
        im->pixels = (float *)CONTEXT_malloc(context,im->float_count * sizeof(float));
    }
    if (im->pixels == NULL) {
        CONTEXT_free(context, im);
        CONTEXT_error(context, Out_of_memory);
        return NULL;
    }
    return im;
}


void BitmapFloat_destroy(Context* context, BitmapFloat * im)
{
    if (im == NULL) return;

    if (!im->pixels_borrowed) {
        CONTEXT_free(context, im->pixels);
    }
    im->pixels = NULL;
    CONTEXT_free(context, im);
}

