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

bool flow_bitmap_float_scale_rows(flow_c * context, struct flow_bitmap_float * from, uint32_t from_row,
                                  struct flow_bitmap_float * to, uint32_t to_row, uint32_t row_count,
                                  struct flow_interpolation_pixel_contributions * weights)
{

    const uint32_t from_step = from->channels;
    const uint32_t to_step = to->channels;
    const uint32_t dest_buffer_count = to->w;
    const uint32_t min_channels = umin(from_step, to_step);
    uint32_t ndx;
    if (min_channels > 4) {
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false;
    }
    float avg[4];

    // if both have alpha, process it
    if (from_step == 4 && to_step == 4) {
        for (uint32_t row = 0; row < row_count; row++) {
            const float * __restrict source_buffer = from->pixels + ((from_row + row) * from->float_stride);
            float * __restrict dest_buffer = to->pixels + ((to_row + row) * to->float_stride);

            for (ndx = 0; ndx < dest_buffer_count; ndx++) {
                float r = 0, g = 0, b = 0, a = 0;
                const int left = weights[ndx].Left;
                const int right = weights[ndx].Right;

                const float * __restrict weightArray = weights[ndx].Weights;
                int i;

                /* Accumulate each channel */
                for (i = left; i <= right; i++) {
                    const float weight = weightArray[i - left];

                    b += weight * source_buffer[i * from_step];
                    g += weight * source_buffer[i * from_step + 1];
                    r += weight * source_buffer[i * from_step + 2];
                    a += weight * source_buffer[i * from_step + 3];
                }

                dest_buffer[ndx * to_step] = b;
                dest_buffer[ndx * to_step + 1] = g;
                dest_buffer[ndx * to_step + 2] = r;
                dest_buffer[ndx * to_step + 3] = a;
            }
        }
    } else if (from_step == 3 && to_step == 3) {
        for (uint32_t row = 0; row < row_count; row++) {
            const float * __restrict source_buffer = from->pixels + ((from_row + row) * from->float_stride);
            float * __restrict dest_buffer = to->pixels + ((to_row + row) * to->float_stride);

            for (ndx = 0; ndx < dest_buffer_count; ndx++) {
                float r = 0, g = 0, b = 0;
                const int left = weights[ndx].Left;
                const int right = weights[ndx].Right;

                const float * weightArray = weights[ndx].Weights;
                int i;

                /* Accumulate each channel */
                for (i = left; i <= right; i++) {
                    const float weight = weightArray[i - left];

                    b += weight * source_buffer[i * from_step];
                    g += weight * source_buffer[i * from_step + 1];
                    r += weight * source_buffer[i * from_step + 2];
                }

                dest_buffer[ndx * to_step] = b;
                dest_buffer[ndx * to_step + 1] = g;
                dest_buffer[ndx * to_step + 2] = r;
            }
        }
    } else {
        for (uint32_t row = 0; row < row_count; row++) {
            const float * __restrict source_buffer = from->pixels + ((from_row + row) * from->float_stride);
            float * __restrict dest_buffer = to->pixels + ((to_row + row) * to->float_stride);


            for (ndx = 0; ndx < dest_buffer_count; ndx++) {
                avg[0] = 0;
                avg[1] = 0;
                avg[2] = 0;
                avg[3] = 0;
                const int left = weights[ndx].Left;
                const int right = weights[ndx].Right;

                const float * __restrict weightArray = weights[ndx].Weights;

                /* Accumulate each channel */
                for (int i = left; i <= right; i++) {
                    const float weight = weightArray[i - left];

                    for (uint32_t j = 0; j < min_channels; j++)
                        avg[j] += weight * source_buffer[i * from_step + j];
                }

                for (uint32_t j = 0; j < min_channels; j++)
                    dest_buffer[ndx * to_step + j] = avg[j];
            }
        }
    }
    return true;
}
/*
This halves in sRGB space instead of linear. Not significantly faster on modern hardware, it appears?
#define  HALVING_TYPE unsigned short
#define TO_HALVING_TYPE(x) x
#define FROM_HALVING_TYPE(x) x
#define ALLOW_SHIFTING_HALVING_TYPE
*/

#define HALVING_TYPE float
#define TO_HALVING_TYPE(x) Context_srgb_to_floatspace(context, x)
#define FROM_HALVING_TYPE(x) Context_floatspace_to_srgb(context, x)
#define HALVE_ROW_NAME HalveRowByDivisorColorSpaceAware
#define HALVE_INTERNAL_NAME HalveInternalColorSpaceAware

static inline void HALVE_ROW_NAME(flow_c * context, const unsigned char * from, HALVING_TYPE * to,
                                  const unsigned int to_count, const int divisor, const int step)
{
    int to_b, from_b;
    const int to_bytes = to_count * step;
    const int divisor_stride = step * divisor;
    if (divisor > 4) {
        if (step == 3) {
            for (to_b = 0, from_b = 0; to_b < to_bytes; to_b += 3, from_b += divisor_stride) {
                for (int f = 0; f < divisor_stride; f += 3) {
                    to[to_b + 0] += TO_HALVING_TYPE(from[from_b + f + 0]);
                    to[to_b + 1] += TO_HALVING_TYPE(from[from_b + f + 1]);
                    to[to_b + 2] += TO_HALVING_TYPE(from[from_b + f + 2]);
                }
            }
        } else if (step == 4) {
            for (to_b = 0, from_b = 0; to_b < to_bytes; to_b += 4, from_b += divisor_stride) {
                for (int f = 0; f < divisor_stride; f += 4) {
                    to[to_b + 0] += TO_HALVING_TYPE(from[from_b + f + 0]);
                    to[to_b + 1] += TO_HALVING_TYPE(from[from_b + f + 1]);
                    to[to_b + 2] += TO_HALVING_TYPE(from[from_b + f + 2]);
                    to[to_b + 3] += TO_HALVING_TYPE(from[from_b + f + 3]);
                }
            }
        }
        return;
    }

    if (divisor == 2) {
        if (to_count % 2 == 0) {
            for (to_b = 0, from_b = 0; to_b < to_bytes; to_b += 2 * step, from_b += 4 * step) {
                for (int i = 0; i < 2 * step; i++) {
                    to[to_b + i] += TO_HALVING_TYPE(from[from_b + i]) + TO_HALVING_TYPE(from[from_b + i + step]);
                }
            }
        } else {
            for (to_b = 0, from_b = 0; to_b < to_bytes; to_b += step, from_b += 2 * step) {
                for (int i = 0; i < step; i++) {
                    to[to_b + i] += TO_HALVING_TYPE(from[from_b + i]) + TO_HALVING_TYPE(from[from_b + i + step]);
                }
            }
        }
        return;
    }
    if (divisor == 3) {
        for (to_b = 0, from_b = 0; to_b < to_bytes; to_b += step, from_b += 3 * step) {
            for (int i = 0; i < step; i++) {
                to[to_b + i] += TO_HALVING_TYPE(from[from_b + i]) + TO_HALVING_TYPE(from[from_b + i + step])
                                + TO_HALVING_TYPE(from[from_b + i + 2 * step]);
            }
        }
        return;
    }
    if (divisor == 4) {
        for (to_b = 0, from_b = 0; to_b < to_bytes; to_b += step, from_b += 4 * step) {
            for (int i = 0; i < step; i++) {
                to[to_b + i] += TO_HALVING_TYPE(from[from_b + i]) + TO_HALVING_TYPE(from[from_b + i + step])
                                + TO_HALVING_TYPE(from[from_b + i + 2 * step])
                                + TO_HALVING_TYPE(from[from_b + i + 3 * step]);
            }
        }
        return;
    }
}

static bool HALVE_INTERNAL_NAME(flow_c * context, const struct flow_bitmap_bgra * from, struct flow_bitmap_bgra * to,
                                const int to_w, const int to_h, const int to_stride, const int divisor)
{

    const int to_w_bytes = to_w * flow_pixel_format_bytes_per_pixel(to->fmt);
    HALVING_TYPE * buffer = (HALVING_TYPE *)FLOW_calloc(context, to_w_bytes, sizeof(HALVING_TYPE));
    if (buffer == NULL) {
        FLOW_error(context, flow_status_Out_of_memory);
        return false;
    }
    // Force the from and to formate to be the same
    if (from->fmt != to->fmt
        || (flow_pixel_format_bytes_per_pixel(from->fmt) != 3 && flow_pixel_format_bytes_per_pixel(from->fmt) != 4)) {
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false;
    }

    int y, b, d;
    const unsigned short divisorSqr = divisor * divisor;
#ifdef ALLOW_SHIFTING_HALVING_TYPE
    const unsigned int shift = isPowerOfTwo(divisorSqr) ? intlog2(divisorSqr) : 0;
#endif

    const uint32_t bytes_pp = flow_pixel_format_bytes_per_pixel(from->fmt);

    // TODO: Ensure that from is equal or greater than divisorx to_w and t_h
    // Ensure that shift > 0 && divisorSqr > 0 && divisor > 0
    for (y = 0; y < to_h; y++) {
        memset(buffer, 0, sizeof(HALVING_TYPE) * to_w_bytes);
        for (d = 0; d < divisor; d++) {
            HALVE_ROW_NAME(context, from->pixels + (y * divisor + d) * from->stride, buffer, to_w, divisor, bytes_pp);
        }
        unsigned char * dest_line = to->pixels + y * to_stride;
#ifdef ALLOW_SHIFTING_HALVING_TYPE
        if (shift == 2) {
            for (b = 0; b < to_w_bytes; b++) {
                dest_line[b] = FROM_HALVING_TYPE(buffer[b] >> 2);
            }
        } else if (shift == 3) {
            for (b = 0; b < to_w_bytes; b++) {
                dest_line[b] = FROM_HALVING_TYPE(buffer[b] >> 3);
            }
        } else if (shift > 0) {
            for (b = 0; b < to_w_bytes; b++) {
                dest_line[b] = FROM_HALVING_TYPE(buffer[b] >> shift);
            }
        }
        if (shift == 0) {
#endif

            for (b = 0; b < to_w_bytes; b++) {
                dest_line[b] = FROM_HALVING_TYPE(buffer[b] / divisorSqr);
            }
#ifdef ALLOW_SHIFTING_HALVING_TYPE
        }
#endif
    }

    FLOW_free(context, buffer);

    return true;
}

#undef HALVE_ROW_NAME
#undef HALVE_INTERNAL_NAME
#undef HALVING_TYPE
#undef TO_HALVING_TYPE
#undef FROM_HALVING_TYPE

#define HALVE_ROW_NAME HalveRowByDivisor
#define HALVE_INTERNAL_NAME HalveInternal

#define HALVING_TYPE unsigned short
#define TO_HALVING_TYPE(x) x
#define FROM_HALVING_TYPE(x) x
#define ALLOW_SHIFTING_HALVING_TYPE

//** Do not edit the following two functions; they are copy/pasted from above. **//

static inline void HALVE_ROW_NAME(flow_c * context, const unsigned char * from, HALVING_TYPE * to,
                                  const unsigned int to_count, const int divisor, const int step)
{
    int to_b, from_b;
    const int to_bytes = to_count * step;
    const int divisor_stride = step * divisor;
    if (divisor > 4) {
        if (step == 3) {
            for (to_b = 0, from_b = 0; to_b < to_bytes; to_b += 3, from_b += divisor_stride) {
                for (int f = 0; f < divisor_stride; f += 3) {
                    to[to_b + 0] += TO_HALVING_TYPE(from[from_b + f + 0]);
                    to[to_b + 1] += TO_HALVING_TYPE(from[from_b + f + 1]);
                    to[to_b + 2] += TO_HALVING_TYPE(from[from_b + f + 2]);
                }
            }
        } else if (step == 4) {
            for (to_b = 0, from_b = 0; to_b < to_bytes; to_b += 4, from_b += divisor_stride) {
                for (int f = 0; f < divisor_stride; f += 4) {
                    to[to_b + 0] += TO_HALVING_TYPE(from[from_b + f + 0]);
                    to[to_b + 1] += TO_HALVING_TYPE(from[from_b + f + 1]);
                    to[to_b + 2] += TO_HALVING_TYPE(from[from_b + f + 2]);
                    to[to_b + 3] += TO_HALVING_TYPE(from[from_b + f + 3]);
                }
            }
        }
        return;
    }

    if (divisor == 2) {
        if (to_count % 2 == 0) {
            for (to_b = 0, from_b = 0; to_b < to_bytes; to_b += 2 * step, from_b += 4 * step) {
                for (int i = 0; i < 2 * step; i++) {
                    to[to_b + i] += TO_HALVING_TYPE(from[from_b + i]) + TO_HALVING_TYPE(from[from_b + i + step]);
                }
            }
        } else {
            for (to_b = 0, from_b = 0; to_b < to_bytes; to_b += step, from_b += 2 * step) {
                for (int i = 0; i < step; i++) {
                    to[to_b + i] += TO_HALVING_TYPE(from[from_b + i]) + TO_HALVING_TYPE(from[from_b + i + step]);
                }
            }
        }
        return;
    }
    if (divisor == 3) {
        for (to_b = 0, from_b = 0; to_b < to_bytes; to_b += step, from_b += 3 * step) {
            for (int i = 0; i < step; i++) {
                to[to_b + i] += TO_HALVING_TYPE(from[from_b + i]) + TO_HALVING_TYPE(from[from_b + i + step])
                                + TO_HALVING_TYPE(from[from_b + i + 2 * step]);
            }
        }
        return;
    }
    if (divisor == 4) {
        for (to_b = 0, from_b = 0; to_b < to_bytes; to_b += step, from_b += 4 * step) {
            for (int i = 0; i < step; i++) {
                to[to_b + i] += TO_HALVING_TYPE(from[from_b + i]) + TO_HALVING_TYPE(from[from_b + i + step])
                                + TO_HALVING_TYPE(from[from_b + i + 2 * step])
                                + TO_HALVING_TYPE(from[from_b + i + 3 * step]);
            }
        }
        return;
    }
}

static bool HALVE_INTERNAL_NAME(flow_c * context, const struct flow_bitmap_bgra * from, struct flow_bitmap_bgra * to,
                                const int to_w, const int to_h, const int to_stride, const int divisor)
{

    const int to_w_bytes = to_w * flow_pixel_format_bytes_per_pixel(to->fmt);
    HALVING_TYPE * buffer = (HALVING_TYPE *)FLOW_calloc(context, to_w_bytes, sizeof(HALVING_TYPE));
    if (buffer == NULL) {
        FLOW_error(context, flow_status_Out_of_memory);
        return false;
    }
    // Force the from and to formate to be the same
    if (from->fmt != to->fmt
        || (flow_pixel_format_bytes_per_pixel(from->fmt) != 3 && flow_pixel_format_bytes_per_pixel(from->fmt) != 4)) {
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false;
    }

    int y, b, d;
    const unsigned short divisorSqr = divisor * divisor;
#ifdef ALLOW_SHIFTING_HALVING_TYPE
    const unsigned int shift = isPowerOfTwo(divisorSqr) ? intlog2(divisorSqr) : 0;
#endif

    const uint32_t bytes_pp = flow_pixel_format_bytes_per_pixel(from->fmt);

    // TODO: Ensure that from is equal or greater than divisorx to_w and t_h
    // Ensure that shift > 0 && divisorSqr > 0 && divisor > 0
    for (y = 0; y < to_h; y++) {
        memset(buffer, 0, sizeof(HALVING_TYPE) * to_w_bytes);
        for (d = 0; d < divisor; d++) {
            HALVE_ROW_NAME(context, from->pixels + (y * divisor + d) * from->stride, buffer, to_w, divisor, bytes_pp);
        }
        unsigned char * dest_line = to->pixels + y * to_stride;
#ifdef ALLOW_SHIFTING_HALVING_TYPE
        if (shift == 2) {
            for (b = 0; b < to_w_bytes; b++) {
                dest_line[b] = FROM_HALVING_TYPE(buffer[b] >> 2);
            }
        } else if (shift == 3) {
            for (b = 0; b < to_w_bytes; b++) {
                dest_line[b] = FROM_HALVING_TYPE(buffer[b] >> 3);
            }
        } else if (shift > 0) {
            for (b = 0; b < to_w_bytes; b++) {
                dest_line[b] = FROM_HALVING_TYPE(buffer[b] >> shift);
            }
        }
        if (shift == 0) {
#endif

            for (b = 0; b < to_w_bytes; b++) {
                dest_line[b] = FROM_HALVING_TYPE(buffer[b] / divisorSqr);
            }
#ifdef ALLOW_SHIFTING_HALVING_TYPE
        }
#endif
    }

    FLOW_free(context, buffer);

    return true;
}

//** Do not edit the above two functions; they are copy/pasted. **//

bool flow_halve(flow_c * context, const struct flow_bitmap_bgra * from, struct flow_bitmap_bgra * to, int divisor)
{

    bool r = false;
    if (context->colorspace.floatspace == flow_working_floatspace_as_is) {
        r = HalveInternal(context, from, to, to->w, to->h, to->stride, divisor);
    } else {
        r = HalveInternalColorSpaceAware(context, from, to, to->w, to->h, to->stride, divisor);
    }
    if (!r) {
        FLOW_add_to_callstack(context);
    }
    return r;
}

bool flow_halve_in_place(flow_c * context, struct flow_bitmap_bgra * from, int divisor)
{
    int to_w = from->w / divisor;
    int to_h = from->h / divisor;
    int to_stride = to_w * flow_pixel_format_bytes_per_pixel(from->fmt);
    bool r = false;
    if (context->colorspace.floatspace == flow_working_floatspace_as_is) {
        r = HalveInternal(context, from, from, to_w, to_h, to_stride, divisor);
    } else {
        r = HalveInternalColorSpaceAware(context, from, from, to_w, to_h, to_stride, divisor);
    }
    if (!r) {
        FLOW_add_to_callstack(context);
    }
    from->w = to_w;
    from->h = to_h;
    from->stride = to_stride;
    return r;
}
