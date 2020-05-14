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
#include <emmintrin.h>


FLOW_HINT_HOT FLOW_HINT_UNSAFE_MATH_OPTIMIZATIONS
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
            const __m128 * __restrict source_buffer
                = (__m128 *)(from->pixels + ((from_row + row) * from->float_stride));
            __m128 * __restrict dest_buffer = (__m128 *)(to->pixels + ((to_row + row) * to->float_stride));

            for (ndx = 0; ndx < dest_buffer_count; ndx++) {
                __m128 sums = { 0.0f };
                const int left = weights[ndx].Left;
                const int right = weights[ndx].Right;

                const float * __restrict weightArray = weights[ndx].Weights;
                int i;

                /* Accumulate each channel */
                for (i = left; i <= right; i++) {
// TODO: Do a better job with this.
#ifdef __clang__
                    __m128 factor = _mm_set1_ps(weightArray[i - left]);
                    sums += factor * source_buffer[i];
#else
                    __m128 factor = _mm_set1_ps(weightArray[i - left]);
                    __m128 mid = _mm_mul_ps(factor, source_buffer[i]);
                    sums = _mm_add_ps(sums, mid);
#endif
                }

                dest_buffer[ndx] = sums;
            }
        }
    } else if (from_step == 3 && to_step == 3) {
        for (uint32_t row = 0; row < row_count; row++) {
            const float * __restrict source_buffer = from->pixels + ((from_row + row) * from->float_stride);
            float * __restrict dest_buffer = to->pixels + ((to_row + row) * to->float_stride);

            for (ndx = 0; ndx < dest_buffer_count; ndx++) {
                float bgr[3] = { 0.0f, 0.0f, 0.0f };
                const int left = weights[ndx].Left;
                const int right = weights[ndx].Right;

                const float * weightArray = weights[ndx].Weights;
                int i;

                /* Accumulate each channel */
                for (i = left; i <= right; i++) {
                    const float weight = weightArray[i - left];

                    bgr[0] += weight * source_buffer[i * from_step];
                    bgr[1] += weight * source_buffer[i * from_step + 1];
                    bgr[2] += weight * source_buffer[i * from_step + 2];
                }

                dest_buffer[ndx * to_step] = bgr[0];
                dest_buffer[ndx * to_step + 1] = bgr[1];
                dest_buffer[ndx * to_step + 2] = bgr[2];
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
