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

#include <string.h>

bool BitmapBgra_convert_srgb_to_linear(Context* context, BitmapBgra* src, uint32_t from_row, BitmapFloat* dest,
                                       uint32_t dest_row, uint32_t row_count)
{
    if (src->w != dest->w || BitmapPixelFormat_bytes_per_pixel(src->fmt) < dest->channels) {
        CONTEXT_error(context, Invalid_internal_state);
        return false;
    }
    if (!(from_row + row_count <= src->h && dest_row + row_count <= dest->h)) {
        CONTEXT_error(context, Invalid_internal_state);
        return false;
    }

    const uint32_t w = src->w;
    const uint32_t units = w * BitmapPixelFormat_bytes_per_pixel(src->fmt);
    const uint32_t from_step = BitmapPixelFormat_bytes_per_pixel(src->fmt);
    const uint32_t to_step = dest->channels;
    const uint32_t copy_step = umin(from_step, to_step);

    for (uint32_t row = 0; row < row_count; row++) {
        uint8_t* src_start = src->pixels + (from_row + row) * src->stride;

        float* buf = dest->pixels + (dest->float_stride * (row + dest_row));
        if (copy_step == 3) {
            for (uint32_t to_x = 0, bix = 0; bix < units; to_x += to_step, bix += from_step) {
                buf[to_x] = Context_srgb_to_floatspace(context, src_start[bix]);
                buf[to_x + 1] = Context_srgb_to_floatspace(context, src_start[bix + 1]);
                buf[to_x + 2] = Context_srgb_to_floatspace(context, src_start[bix + 2]);
            }
            // We're only working on a portion... dest->alpha_premultiplied = false;
        } else if (copy_step == 4) {
            for (uint32_t to_x = 0, bix = 0; bix < units; to_x += to_step, bix += from_step) {
                {
                    const float alpha = ((float)src_start[bix + 3]) / 255.0f;
                    buf[to_x] = alpha * Context_srgb_to_floatspace(context, src_start[bix]);
                    buf[to_x + 1] = alpha * Context_srgb_to_floatspace(context, src_start[bix + 1]);
                    buf[to_x + 2] = alpha * Context_srgb_to_floatspace(context, src_start[bix + 2]);
                    buf[to_x + 3] = alpha;
                }
            }
            // We're only working on a portion... dest->alpha_premultiplied = true;
        } else {
            CONTEXT_error(context, Unsupported_pixel_format);
            return false;
        }
    }
    return true;
}

/*
static void unpack24bitRow(uint32_t width, unsigned char* sourceLine, unsigned char* destArray){
    for (uint32_t i = 0; i < width; i++){

        memcpy(destArray + i * 4, sourceLine + i * 3, 3);
        destArray[i * 4 + 3] = 255;
    }
}
*/

bool BitmapBgra_flip_vertical(Context* context, BitmapBgra* b)
{
    void* swap = CONTEXT_malloc(context, b->stride);
    if (swap == NULL) {
        CONTEXT_error(context, Out_of_memory);
        return false;
    }
    // Dont' copy the full stride (padding), it could be windowed!
    uint32_t row_length = umin(b->stride, b->w * BitmapPixelFormat_bytes_per_pixel(b->fmt));
    for (uint32_t i = 0; i < b->h / 2; i++) {
        void* top = b->pixels + (i * b->stride);
        void* bottom = b->pixels + ((b->h - 1 - i) * b->stride);
        memcpy(swap, top, row_length);
        memcpy(top, bottom, row_length);
        memcpy(bottom, swap, row_length);
    }
    CONTEXT_free(context, swap);
    return true;
}

bool BitmapBgra_flip_horizontal(Context* context, BitmapBgra* b)
{
    uint32_t swap[4];
    // Dont' copy the full stride (padding), it could be windowed!
    for (uint32_t y = 0; y < b->h; y++) {
        uint8_t* left = b->pixels + (y * b->stride);
        uint8_t* right = b->pixels + (y * b->stride) + (BitmapPixelFormat_bytes_per_pixel(b->fmt) * (b->w - 1));
        while (left < right) {
            memcpy(&swap, left, BitmapPixelFormat_bytes_per_pixel(b->fmt));
            memcpy(left, right, BitmapPixelFormat_bytes_per_pixel(b->fmt));
            memcpy(right, &swap, BitmapPixelFormat_bytes_per_pixel(b->fmt));
            left += BitmapPixelFormat_bytes_per_pixel(b->fmt);
            right -= BitmapPixelFormat_bytes_per_pixel(b->fmt);
        }
    }
    return true;
}

/*
static int  copy_bitmap_bgra(BitmapBgra * src, BitmapBgra * dst)
{
    // TODO: check sizes / overflows
    if (dst->w != src->w || dst->h != src->h) return -1;

    if (src->fmt == dst->fmt)
    {
        const uint32_t bytes_pp = BitmapPixelFormat_bytes_per_pixel (src->fmt);
        // recalculate line width as it can be different from the stride
        for (uint32_t y = 0; y < src->h; y++)
            memcpy(dst->pixels + y*dst->stride, src->pixels + y*src->stride, src->w*bytes_pp);
    }
    else if (src->fmt == Bgr24 && dst->fmt == Bgra32)
    {
        for (uint32_t y = 0; y < src->h; y++)
            unpack24bitRow(src->w, src->pixels + y*src->stride, dst->pixels + y*dst->stride);
    }
    else{
        return -2;
    }
    return 0;
}
*/

static bool BitmapFloat_blend_matte(Context* context, BitmapFloat* src, const uint32_t from_row,
                                    const uint32_t row_count, const uint8_t* const matte)
{
    // We assume that matte is BGRA, regardless.

    const float matte_a = ((float)matte[3]) / 255.0f;
    const float b = Context_srgb_to_floatspace(context, matte[0]);
    const float g = Context_srgb_to_floatspace(context, matte[1]);
    const float r = Context_srgb_to_floatspace(context, matte[2]);

    for (uint32_t row = from_row; row < from_row + row_count; row++) {
        uint32_t start_ix = row * src->float_stride;
        uint32_t end_ix = start_ix + src->w * src->channels;

        for (uint32_t ix = start_ix; ix < end_ix; ix += 4) {
            const float src_a = src->pixels[ix + 3];
            const float a = (1.0f - src_a) * matte_a;
            const float final_alpha = src_a + a;

            src->pixels[ix] = (src->pixels[ix] + b * a) / final_alpha;
            src->pixels[ix + 1] = (src->pixels[ix + 1] + g * a) / final_alpha;
            src->pixels[ix + 2] = (src->pixels[ix + 2] + r * a) / final_alpha;
            src->pixels[ix + 3] = final_alpha;
        }
    }

    // Ensure alpha is demultiplied
    return true;
}

bool BitmapFloat_demultiply_alpha(Context* context, BitmapFloat* src, const uint32_t from_row, const uint32_t row_count)
{
    for (uint32_t row = from_row; row < from_row + row_count; row++) {
        uint32_t start_ix = row * src->float_stride;
        uint32_t end_ix = start_ix + src->w * src->channels;

        for (uint32_t ix = start_ix; ix < end_ix; ix += 4) {
            const float alpha = src->pixels[ix + 3];
            if (alpha > 0) {
                src->pixels[ix] /= alpha;
                src->pixels[ix + 1] /= alpha;
                src->pixels[ix + 2] /= alpha;
            }
        }
    }
    return true;
}

bool BitmapFloat_copy_linear_over_srgb(Context* context, BitmapFloat* src, const uint32_t from_row, BitmapBgra* dest,
                                       const uint32_t dest_row, const uint32_t row_count, const uint32_t from_col,
                                       const uint32_t col_count, const bool transpose)
{

    const uint32_t dest_bytes_pp = BitmapPixelFormat_bytes_per_pixel(dest->fmt);

    const uint32_t dest_row_stride = transpose ? dest_bytes_pp : dest->stride;
    const uint32_t dest_pixel_stride = transpose ? dest->stride : dest_bytes_pp;
    const uint32_t srcitems = umin(from_col + col_count, src->w) * src->channels;
    const uint32_t ch = src->channels;
    const bool copy_alpha = dest->fmt == Bgra32 && src->channels == 4 && src->alpha_meaningful;
    const bool clean_alpha = !copy_alpha && dest->fmt == Bgra32;

    for (uint32_t row = 0; row < row_count; row++) {
        float* src_row = src->pixels + (row + from_row) * src->float_stride;

        uint8_t* dest_row_bytes = dest->pixels + (dest_row + row) * dest_row_stride + (from_col * dest_pixel_stride);

        for (uint32_t ix = from_col * ch; ix < srcitems; ix += ch) {
            dest_row_bytes[0] = Context_floatspace_to_srgb(context, src_row[ix]);
            dest_row_bytes[1] = Context_floatspace_to_srgb(context, src_row[ix + 1]);
            dest_row_bytes[2] = Context_floatspace_to_srgb(context, src_row[ix + 2]);
            if (copy_alpha) {
                dest_row_bytes[3] = uchar_clamp_ff(src_row[ix + 3] * 255.0f);
            }
            if (clean_alpha) {
                dest_row_bytes[3] = 0xff;
            }
            dest_row_bytes += dest_pixel_stride;
        }
    }
    return true;
}

static bool BitmapFloat_compose_linear_over_srgb(Context* context, BitmapFloat* src, const uint32_t from_row,
                                                 BitmapBgra* dest, const uint32_t dest_row, const uint32_t row_count,
                                                 const uint32_t from_col, const uint32_t col_count,
                                                 const bool transpose)
{

    const uint32_t dest_bytes_pp = BitmapPixelFormat_bytes_per_pixel(dest->fmt);
    const uint32_t dest_row_stride = transpose ? dest_bytes_pp : dest->stride;
    const uint32_t dest_pixel_stride = transpose ? dest->stride : dest_bytes_pp;
    const uint32_t srcitems = umin(from_col + col_count, src->w) * src->channels;
    const uint32_t ch = src->channels;

    const bool dest_alpha = dest->fmt == Bgra32 && dest->alpha_meaningful;

    const uint8_t dest_alpha_index = dest_alpha ? 3 : 0;
    const float dest_alpha_to_float_coeff = dest_alpha ? 1.0f / 255.0f : 0.0f;
    const float dest_alpha_to_float_offset = dest_alpha ? 0.0f : 1.0f;
    for (uint32_t row = 0; row < row_count; row++) {
        // const float * const __restrict src_row = src->pixels + (row + from_row) * src->float_stride;
        float* src_row = src->pixels + (row + from_row) * src->float_stride;

        uint8_t* dest_row_bytes = dest->pixels + (dest_row + row) * dest_row_stride + (from_col * dest_pixel_stride);

        for (uint32_t ix = from_col * ch; ix < srcitems; ix += ch) {

            const uint8_t dest_b = dest_row_bytes[0];
            const uint8_t dest_g = dest_row_bytes[1];
            const uint8_t dest_r = dest_row_bytes[2];
            const uint8_t dest_a = dest_row_bytes[dest_alpha_index];

            const float src_b = src_row[ix + 0];
            const float src_g = src_row[ix + 1];
            const float src_r = src_row[ix + 2];
            const float src_a = src_row[ix + 3];
            const float a = (1.0f - src_a) * (dest_alpha_to_float_coeff * dest_a + dest_alpha_to_float_offset);

            const float b = Context_srgb_to_floatspace(context, dest_b) * a + src_b;
            const float g = Context_srgb_to_floatspace(context, dest_g) * a + src_g;
            const float r = Context_srgb_to_floatspace(context, dest_r) * a + src_r;

            const float final_alpha = src_a + a;

            dest_row_bytes[0] = Context_floatspace_to_srgb(context, b / final_alpha);
            dest_row_bytes[1] = Context_floatspace_to_srgb(context, g / final_alpha);
            dest_row_bytes[2] = Context_floatspace_to_srgb(context, r / final_alpha);
            if (dest_alpha) {
                dest_row_bytes[3] = uchar_clamp_ff(final_alpha * 255);
            }
            dest_row_bytes += dest_pixel_stride;
        }
    }
    return true;
}

bool BitmapFloat_pivoting_composite_linear_over_srgb(Context* context, BitmapFloat* src, uint32_t from_row,
                                                     BitmapBgra* dest, uint32_t dest_row, uint32_t row_count,
                                                     bool transpose)
{
    if (transpose ? src->w != dest->h : src->w != dest->w) {
        // TODO: Add more bounds checks
        CONTEXT_error(context, Invalid_internal_state);
        return false;
    }

    if (src->alpha_meaningful && src->channels == 4 && dest->compositing_mode == Blend_with_matte) {
        if (!BitmapFloat_blend_matte(context, src, from_row, row_count, dest->matte_color)) {
            CONTEXT_add_to_callstack(context);
            return false;
        }
        src->alpha_premultiplied = false;
    }
    if (src->channels == 4 && src->alpha_premultiplied && dest->compositing_mode != Blend_with_self) {
        // Demultiply
        if (!BitmapFloat_demultiply_alpha(context, src, from_row, row_count)) {
            CONTEXT_add_to_callstack(context);
            return false;
        }
    }

    bool can_compose = dest->compositing_mode == Blend_with_self && src->alpha_meaningful && src->channels == 4;

    if (can_compose && !src->alpha_premultiplied) {
        // Something went wrong. We should always have alpha premultiplied.
        CONTEXT_error(context, Invalid_internal_state);
        return false;
    }

    // Tiling does not appear to show benefits when benchmarking - only breifly investigated
    bool tile_when_transposing = false;

    if (transpose && tile_when_transposing) {

        // Let's try to tile within 2kb, get some cache coherency
        const float dest_opt_rows = 2048.0f / (float)dest->stride;

        const int tile_width = max(4, (int)dest_opt_rows);
        const int tiles = src->w / tile_width;

        if (can_compose) {
            for (int i = 0; i < tiles; i++) {
                if (!BitmapFloat_compose_linear_over_srgb(context, src, from_row, dest, dest_row, row_count,
                                                          i * tile_width, tile_width, transpose)) {
                    CONTEXT_add_to_callstack(context);
                    return false;
                }
            }
        } else {
            for (int i = 0; i < tiles; i++) {
                if (!BitmapFloat_copy_linear_over_srgb(context, src, from_row, dest, dest_row, row_count,
                                                       i * tile_width, tile_width, transpose)) {
                    CONTEXT_add_to_callstack(context);
                    return false;
                }
            }
        }
    } else {
        if (can_compose) {
            if (!BitmapFloat_compose_linear_over_srgb(context, src, from_row, dest, dest_row, row_count, 0, src->w,
                                                      transpose)) {
                CONTEXT_add_to_callstack(context);
                return false;
            }
        } else {
            if (!BitmapFloat_copy_linear_over_srgb(context, src, from_row, dest, dest_row, row_count, 0, src->w,
                                                   transpose)) {
                CONTEXT_add_to_callstack(context);
                return false;
            }
        }
    }

    return true;
}
