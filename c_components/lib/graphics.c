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

#ifdef _MSC_VER
#define BESSEL_01 _j1
#else
#ifdef __GLIBC__
#define BESSEL_01 __builtin_j1
#else
#define BESSEL_01 j1
#endif
#endif

#include "imageflow_private.h"

#include <stdlib.h>
#include <emmintrin.h>
#include <string.h>
#include <immintrin.h>

#ifndef _MSC_VER
#include <alloca.h>
#else
#pragma unmanaged
#ifndef alloca
#include <malloc.h>
#define alloca _alloca
#endif
#endif

#ifdef _MSC_VER
#define likely(x) (x)
#define unlikely(x) (x)
#else
#define likely(x) (__builtin_expect(!!(x), 1))
#define unlikely(x) (__builtin_expect(!!(x), 0))
#endif

static void derive_cubic_coefficients(double B, double C, struct flow_interpolation_details * out)
{
    double bx2 = B + B;
    out->p1 = 1.0 - (1.0 / 3.0) * B;
    out->p2 = -3.0 + bx2 + C;
    out->p3 = 2.0 - 1.5 * B - C;
    out->q1 = (4.0 / 3.0) * B + 4.0 * C;
    out->q2 = -8.0 * C - bx2;
    out->q3 = B + 5.0 * C;
    out->q4 = (-1.0 / 6.0) * B - C;
}

static double filter_flex_cubic(const struct flow_interpolation_details * d, double x)
{
    const double t = (double)fabs(x) / d->blur;

    if (t < 1.0) {
        return (d->p1 + t * (t * (d->p2 + t * d->p3)));
    }
    if (t < 2.0) {
        return (d->q1 + t * (d->q2 + t * (d->q3 + t * d->q4)));
    }
    return (0.0);
}
static double filter_bicubic_fast(const struct flow_interpolation_details * d, double t)
{
    double abs_t = (double)fabs(t) / d->blur;
    double abs_t_sq = abs_t * abs_t;
    if (abs_t < 1)
        return 1 - 2 * abs_t_sq + abs_t_sq * abs_t;
    if (abs_t < 2)
        return (4 - 8 * abs_t + 5 * abs_t_sq - abs_t_sq * abs_t);
    return 0;
}

static double filter_sinc(const struct flow_interpolation_details * d, double t)
{
    const double abs_t = (double)fabs(t) / d->blur;
    if (abs_t == 0) {
        return 1; // Avoid division by zero
    }
    if (abs_t > d->window) {
        return 0;
    }
    const double a = abs_t * IR_PI;
    return sin(a) / a;
}

static double filter_box(const struct flow_interpolation_details * d, double t)
{

    const double x = t / d->blur;
    return (x >= -1 * d->window && x < d->window) ? 1 : 0;
}

static double filter_triangle(const struct flow_interpolation_details * d, double t)
{
    const double x = (double)fabs(t) / d->blur;
    if (x < 1.0)
        return (1.0 - x);
    return (0.0);
}

static double filter_sinc_windowed(const struct flow_interpolation_details * d, double t)
{
    const double x = t / d->blur;
    const double abs_t = (double)fabs(x);
    if (abs_t == 0) {
        return 1; // Avoid division by zero
    }
    if (abs_t > d->window) {
        return 0;
    }
    return d->window * sin(IR_PI * x / d->window) * sin(x * IR_PI) / (IR_PI * IR_PI * x * x);
}

static double filter_jinc(const struct flow_interpolation_details * d, double t)
{
    const double x = fabs(t) / d->blur;
    if (x == 0.0)
        return (0.5 * IR_PI);
    return (BESSEL_01(IR_PI * x) / x);
    ////x crossing #1 1.2196698912665045
}

/*

static inline double window_jinc (double x) {
    double x_a = x * 1.2196698912665045;
    if (x == 0.0)
        return 1;
    return (BesselOrderOne (IR_PI*x_a) / (x_a * IR_PI * 0.5));
    ////x crossing #1 1.2196698912665045
}

static double filter_window_jinc (const struct flow_interpolation_details * d, double t) {
    return window_jinc (t / (d->blur * d->window));
}
*/

static double filter_ginseng(const struct flow_interpolation_details * d, double t)
{
    // Sinc windowed by jinc
    const double abs_t = (double)fabs(t) / d->blur;
    const double t_pi = abs_t * IR_PI;

    if (abs_t == 0) {
        return 1; // Avoid division by zero
    }
    if (abs_t > 3) {
        return 0;
    }
    const double jinc_input = 1.2196698912665045 * t_pi / d->window;
    const double jinc_output = BESSEL_01(jinc_input) / (jinc_input * 0.5);

    return jinc_output * sin(t_pi) / (t_pi);
}

#define TONY 0.00001

double flow_interpolation_details_percent_negative_weight(const struct flow_interpolation_details * details)
{
    const int samples = 50;
    double step = details->window / (double)samples;

    double last_height = details->filter(details, -step);
    double positive_area = 0;
    double negative_area = 0;
    for (int i = 0; i <= samples + 2; i++) {
        const double height = details->filter(details, i * step);
        const double area = (height + last_height) / 2.0 * step;
        last_height = height;
        if (area > 0)
            positive_area += area;
        else
            negative_area -= area;
    }
    return negative_area / positive_area;
}

struct flow_interpolation_details * flow_interpolation_details_create(flow_c * context)
{
    struct flow_interpolation_details * d = FLOW_calloc_array(context, 1, struct flow_interpolation_details);
    if (d == NULL) {
        FLOW_error(context, flow_status_Out_of_memory);
        return NULL;
    }
    d->blur = 1;
    d->window = 2;
    d->p1 = d->q1 = 0;
    d->p2 = d->q2 = d->p3 = d->q3 = d->q4 = 1;
    d->sharpen_percent_goal = 0;
    return d;
}

struct flow_interpolation_details * flow_interpolation_details_create_bicubic_custom(flow_c * context, double window,
                                                                                     double blur, double B, double C)
{
    struct flow_interpolation_details * d = flow_interpolation_details_create(context);
    if (d != NULL) {
        d->blur = blur;
        derive_cubic_coefficients(B, C, d);
        d->filter = filter_flex_cubic;
        d->window = window;
    } else {
        FLOW_add_to_callstack(context);
    }
    return d;
}
struct flow_interpolation_details * flow_interpolation_details_create_custom(flow_c * context, double window,
                                                                             double blur,
                                                                             flow_detailed_interpolation_method filter)
{
    struct flow_interpolation_details * d = flow_interpolation_details_create(context);
    if (d != NULL) {
        d->blur = blur;
        d->filter = filter;
        d->window = window;
    } else {
        FLOW_add_to_callstack(context);
    }
    return d;
}

void flow_interpolation_details_destroy(flow_c * context, struct flow_interpolation_details * details)
{
    FLOW_free(context, details);
}

static struct flow_interpolation_details *
InterpolationDetails_create_from_internal(flow_c * context, flow_interpolation_filter filter, bool checkExistenceOnly)
{
    bool ex = checkExistenceOnly;
    struct flow_interpolation_details * truePtr = (struct flow_interpolation_details *)-1;
    switch (filter) {
        case flow_interpolation_filter_Linear:
        case flow_interpolation_filter_Triangle:
            return ex ? truePtr : flow_interpolation_details_create_custom(context, 1, 1, filter_triangle);

        case flow_interpolation_filter_RawLanczos2:
            return ex ? truePtr : flow_interpolation_details_create_custom(context, 2, 1, filter_sinc);
        case flow_interpolation_filter_RawLanczos3:
            return ex ? truePtr : flow_interpolation_details_create_custom(context, 3, 1, filter_sinc);
        case flow_interpolation_filter_RawLanczos2Sharp:
            return ex ? truePtr : flow_interpolation_details_create_custom(context, 2, 0.9549963639785485, filter_sinc);
        case flow_interpolation_filter_RawLanczos3Sharp:
            return ex ? truePtr : flow_interpolation_details_create_custom(context, 3, 0.9812505644269356, filter_sinc);

        // Hermite and BSpline no negative weights
        case flow_interpolation_filter_CubicBSpline:
            return ex ? truePtr : flow_interpolation_details_create_bicubic_custom(context, 2, 1, 1, 0);

        case flow_interpolation_filter_Lanczos2:
            return ex ? truePtr : flow_interpolation_details_create_custom(context, 2, 1, filter_sinc_windowed);
        case flow_interpolation_filter_Lanczos:
            return ex ? truePtr : flow_interpolation_details_create_custom(context, 3, 1, filter_sinc_windowed);
        case flow_interpolation_filter_Lanczos2Sharp:
            return ex ? truePtr
                      : flow_interpolation_details_create_custom(context, 2, 0.9549963639785485, filter_sinc_windowed);
        case flow_interpolation_filter_LanczosSharp:
            return ex ? truePtr
                      : flow_interpolation_details_create_custom(context, 3, 0.9812505644269356, filter_sinc_windowed);

        case flow_interpolation_filter_CubicFast:
            return ex ? truePtr : flow_interpolation_details_create_custom(context, 2, 1, filter_bicubic_fast);
        case flow_interpolation_filter_Cubic:
            return ex ? truePtr : flow_interpolation_details_create_bicubic_custom(context, 2, 1, 0, 1);
        case flow_interpolation_filter_CubicSharp:
            return ex ? truePtr
                      : flow_interpolation_details_create_bicubic_custom(context, 2, 0.9549963639785485, 0, 1);
        case flow_interpolation_filter_CatmullRom:
            return ex ? truePtr : flow_interpolation_details_create_bicubic_custom(context, 2, 1, 0, 0.5);
        case flow_interpolation_filter_CatmullRomFast:
            return ex ? truePtr : flow_interpolation_details_create_bicubic_custom(context, 1, 1, 0, 0.5);
        case flow_interpolation_filter_CatmullRomFastSharp:
            return ex ? truePtr : flow_interpolation_details_create_bicubic_custom(context, 1, 13.0 / 16.0, 0, 0.5);
        case flow_interpolation_filter_Mitchell:
            return ex ? truePtr : flow_interpolation_details_create_bicubic_custom(context, 2, 1, 1.0 / 3.0, 1.0 / 3.0);
        case flow_interpolation_filter_MitchellFast:
            return ex ? truePtr : flow_interpolation_details_create_bicubic_custom(context, 1, 1, 1.0 / 3.0, 1.0 / 3.0);

        case flow_interpolation_filter_NCubic:
            return ex ? truePtr : flow_interpolation_details_create_bicubic_custom(
                                      context, 2.5, 1. / 1.1685777620836932, 0.37821575509399867, 0.31089212245300067);
        case flow_interpolation_filter_NCubicSharp:
            return ex ? truePtr : flow_interpolation_details_create_bicubic_custom(
                                      context, 2.5, 1. / 1.105822933719019, 0.2620145123990142, 0.3689927438004929);
        case flow_interpolation_filter_Robidoux:
            return ex ? truePtr : flow_interpolation_details_create_bicubic_custom(context, 2, 1, 0.37821575509399867,
                                                                                   0.31089212245300067);
        case flow_interpolation_filter_Fastest:
            return ex ? truePtr : flow_interpolation_details_create_bicubic_custom(
                                      context, 0.74, 0.74, 0.37821575509399867, 0.31089212245300067);

        case flow_interpolation_filter_RobidouxFast:
            return ex ? truePtr : flow_interpolation_details_create_bicubic_custom(
                                      context, 1.05, 1, 0.37821575509399867, 0.31089212245300067);
        case flow_interpolation_filter_RobidouxSharp:
            return ex ? truePtr : flow_interpolation_details_create_bicubic_custom(context, 2, 1, 0.2620145123990142,
                                                                                   0.3689927438004929);
        case flow_interpolation_filter_Hermite:
            return ex ? truePtr : flow_interpolation_details_create_bicubic_custom(context, 1, 1, 0, 0);
        case flow_interpolation_filter_Box:
            return ex ? truePtr : flow_interpolation_details_create_custom(context, 0.5, 1, filter_box);

        case flow_interpolation_filter_Ginseng:
            return ex ? truePtr : flow_interpolation_details_create_custom(context, 3, 1, filter_ginseng);

        case flow_interpolation_filter_GinsengSharp:
            return ex ? truePtr
                      : flow_interpolation_details_create_custom(context, 3, 0.9812505644269356, filter_ginseng);

        case flow_interpolation_filter_Jinc:
            return ex ? truePtr : flow_interpolation_details_create_custom(context, 6, 1.0, filter_jinc);
    }
    if (!checkExistenceOnly) {
        FLOW_error_msg(context, flow_status_Invalid_argument, "Invalid interpolation filter %d", (int)filter);
    }
    return NULL;
}

struct flow_interpolation_details * flow_interpolation_details_create_from(flow_c * context,
                                                                           flow_interpolation_filter filter)
{
    return InterpolationDetails_create_from_internal(context, filter, false);
}

bool flow_interpolation_filter_exists(flow_interpolation_filter filter)
{
    return (InterpolationDetails_create_from_internal(NULL, filter, true) != NULL);
}

static struct flow_interpolation_line_contributions *
LineContributions_alloc(flow_c * context, const uint32_t line_length, const uint32_t windows_size)
{
    struct flow_interpolation_line_contributions * res = (struct flow_interpolation_line_contributions *)FLOW_malloc(
        context, sizeof(struct flow_interpolation_line_contributions));
    if (res == NULL) {
        FLOW_error(context, flow_status_Out_of_memory);
        return NULL;
    }
    res->WindowSize = windows_size;
    res->LineLength = line_length;
    res->ContribRow = (struct flow_interpolation_pixel_contributions *)FLOW_malloc(
        context, line_length * sizeof(struct flow_interpolation_pixel_contributions));
    if (!res->ContribRow) {
        FLOW_free(context, res);
        FLOW_error(context, flow_status_Out_of_memory);
        return NULL;
    }

    float * allWeights = FLOW_calloc_array(context, windows_size * line_length, float);
    if (!allWeights) {
        FLOW_free(context, res->ContribRow);
        FLOW_free(context, res);
        FLOW_error(context, flow_status_Out_of_memory);
        return NULL;
    }

    for (uint32_t i = 0; i < line_length; i++)
        res->ContribRow[i].Weights = allWeights + (i * windows_size);

    return res;
}

void flow_interpolation_line_contributions_destroy(flow_c * context, struct flow_interpolation_line_contributions * p)
{

    if (p != NULL) {
        if (p->ContribRow != NULL) {
            FLOW_free(context, p->ContribRow[0].Weights);
        }
        FLOW_free(context, p->ContribRow);
    }
    FLOW_free(context, p);
}

struct flow_interpolation_line_contributions *
flow_interpolation_line_contributions_create(flow_c * context, const uint32_t output_line_size,
                                             const uint32_t input_line_size,
                                             const struct flow_interpolation_details * details)
{
    const double sharpen_ratio = flow_interpolation_details_percent_negative_weight(details);
    const double desired_sharpen_ratio = fmin(0.999999999f, fmax(sharpen_ratio, details->sharpen_percent_goal / 100.0));

    const double scale_factor = (double)output_line_size / (double)input_line_size;
    const double downscale_factor = fmin(1.0, scale_factor);
    const double half_source_window = (details->window + 0.5) / downscale_factor;

    const uint32_t allocated_window_size = (int)ceil(2 * (half_source_window - TONY)) + 1;
    uint32_t u, ix;
    struct flow_interpolation_line_contributions * res
        = LineContributions_alloc(context, output_line_size, allocated_window_size);
    if (res == NULL) {
        FLOW_add_to_callstack(context);
        return NULL;
    }
    double negative_area = 0;
    double positive_area = 0;

    for (u = 0; u < output_line_size; u++) {
        const double center_src_pixel = ((double)u + 0.5) / scale_factor - 0.5;

        const int left_edge = (int)floor(center_src_pixel) - ((allocated_window_size - 1) / 2);
        const int right_edge = left_edge + allocated_window_size - 1;

        const uint32_t left_src_pixel = (uint32_t)int_max(0, left_edge);
        const uint32_t right_src_pixel = (uint32_t)int_min(right_edge, (int)input_line_size - 1);

        // Net weight
        double total_weight = 0.0;
        // Sum of negative and positive weights
        double total_negative_weight = 0.0;
        double total_positive_weight = 0.0;

        const uint32_t source_pixel_count = right_src_pixel - left_src_pixel + 1;

        if (source_pixel_count > allocated_window_size) {
            flow_interpolation_line_contributions_destroy(context, res);
            FLOW_error(context, flow_status_Invalid_internal_state);
            return NULL;
        }

        res->ContribRow[u].Left = left_src_pixel;
        res->ContribRow[u].Right = right_src_pixel;

        float * weights = res->ContribRow[u].Weights;

        for (ix = left_src_pixel; ix <= right_src_pixel; ix++) {
            int tx = ix - left_src_pixel;
            double add = (*details->filter)(details, downscale_factor *((double)ix - center_src_pixel));
            if (fabs(add) <= 0.00000002) {
                add = 0.0;
                // Weights below a certain threshold make consistent x-plat
                // integration test results impossible. pos/neg zero, etc.
                // They should be rounded down to zero at the threshold at which results are consistent.
            }
            weights[tx] = (float)add;
            total_weight += add;
            total_negative_weight += fmin(0, add);
            total_positive_weight += fmax(0, add);
        }

        float neg_factor, pos_factor;
        neg_factor = pos_factor = (float)(1.0f / total_weight);

        //printf("cur= %f cur+= %f cur-= %f desired_sharpen_ratio=%f sharpen_ratio-=%f\n", total_weight, total_positive_weight, total_negative_weight, desired_sharpen_ratio, sharpen_ratio);


        if (total_weight <= 0.0f || desired_sharpen_ratio > sharpen_ratio) {
            if (total_negative_weight < 0.0f){
                if (desired_sharpen_ratio < 1.0f){
                    double target_positive_weight = 1.0f / (1.0f - desired_sharpen_ratio);
                    double target_negative_weight = desired_sharpen_ratio * -target_positive_weight;


                    pos_factor = (float)(target_positive_weight / total_positive_weight);
                    neg_factor = (float)(target_negative_weight / total_negative_weight);

                    if (total_negative_weight == 0) neg_factor = 1.0f;

                    //printf("target=%f target-=%f, pos_factor=%f neg_factor=%f\n", total_positive_weight - target_negative_weight,  target_negative_weight, pos_factor, neg_factor);
                }
            } else if (total_weight == 0){
                // In this situation we have a problem to report
            }
        }
        //printf("\n");

        for (ix = 0; ix < source_pixel_count; ix++) {
            if (weights[ix] < 0) {
                weights[ix] *= neg_factor;
                negative_area -= weights[ix];
            } else {
                weights[ix] *= pos_factor;
                positive_area += weights[ix];
            }
        }

        // Shrink to improve perf & result consistency
        int32_t iix;
        // Shrink region from the right
        for (iix = source_pixel_count - 1; iix >= 0; iix--) {
            if (weights[iix] != 0)
                break;
            res->ContribRow[u].Right--;
        }
        // Shrink region from the left
        for (iix = 0; iix < (int32_t)source_pixel_count; iix++) {
            if (weights[0] != 0)
                break;
            res->ContribRow[u].Weights++;
            weights++;
            res->ContribRow[u].Left++;
        }
    }
    res->percent_negative = negative_area / positive_area;
    return res;
}


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


static void multiply_row(float * row, const size_t length, const float coefficient)
{
    for (size_t i = 0; i < length; i++) {
        row[i] *= coefficient;
    }
}
FLOW_HINT_HOT FLOW_HINT_UNSAFE_MATH_OPTIMIZATIONS static void add_row(float * mutate_row, float * input_row,
                                                                      const size_t length)
{
    for (size_t i = 0; i < length; i++) {
        mutate_row[i] += input_row[i];
    }
}


static struct flow_bitmap_bgra * crop(flow_c * c, struct flow_bitmap_bgra * b, uint32_t x, uint32_t y, uint32_t w, uint32_t h){
    if (h + y > b->h || w + x > b->w) {
        FLOW_error(c, flow_status_Invalid_argument);
        return NULL;
    }

    struct flow_bitmap_bgra * cropped_canvas = flow_bitmap_bgra_create_header(c, w, h);

    uint32_t bpp = flow_pixel_format_bytes_per_pixel(b->fmt);
    if (cropped_canvas == NULL) {
        FLOW_error_return_null(c);
    }
    cropped_canvas->fmt = b->fmt;
    memcpy(&cropped_canvas->matte_color[0],&b->matte_color[0], sizeof(uint8_t[4]));
    cropped_canvas->compositing_mode = b->compositing_mode;


    cropped_canvas->pixels = b->pixels +  y * b->stride + x * bpp;
    cropped_canvas->stride = b->stride;
    return cropped_canvas;
}

FLOW_HINT_HOT FLOW_HINT_UNSAFE_MATH_OPTIMIZATIONS

bool
flow_node_execute_scale2d_render1d(flow_c * c, struct flow_bitmap_bgra * input, struct flow_bitmap_bgra * uncropped_canvas,
                                   struct flow_nodeinfo_scale2d_render_to_canvas1d * info)
{
    if (info->h + info->y > uncropped_canvas->h|| info->w + info->x > uncropped_canvas->w) {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }

    struct flow_bitmap_bgra * cropped_canvas = (info->x == 0 && info->y == 0 && info->w == uncropped_canvas->w && info->h == uncropped_canvas->h) ? uncropped_canvas : crop(c, uncropped_canvas, info->x, info-> y, info->w, info->h);
    if (cropped_canvas == NULL) {
        FLOW_error_return(c);
    }

    flow_pixel_format input_fmt = flow_effective_pixel_format(input);
    flow_pixel_format canvas_fmt = flow_effective_pixel_format(cropped_canvas);

    if (input_fmt != flow_bgra32 && input_fmt != flow_bgr32) {
        FLOW_error(c, flow_status_Not_implemented);
        return false;
    }
    if (canvas_fmt != flow_bgra32 && canvas_fmt != flow_bgr32) {
        FLOW_error(c, flow_status_Not_implemented);
        return false;
    }

    struct flow_colorcontext_info colorcontext;
    flow_colorcontext_init(c, &colorcontext, info->scale_in_colorspace, 0, 0, 0);

    // Use details as a parent structure to ensure everything gets freed
    struct flow_interpolation_details * details = flow_interpolation_details_create_from(c, info->interpolation_filter);
    if (details == NULL) {
        FLOW_error_return(c);
    }
    details->sharpen_percent_goal = info->sharpen_percent_goal;

    struct flow_interpolation_line_contributions * contrib_v = NULL;
    struct flow_interpolation_line_contributions * contrib_h = NULL;

    flow_prof_start(c, "contributions_calc", false);

    contrib_v = flow_interpolation_line_contributions_create(c, info->h, input->h, details);
    if (contrib_v == NULL || !flow_set_owner(c, contrib_v, details)) {
        FLOW_destroy(c, details);
        FLOW_error_return(c);
    }
    contrib_h = flow_interpolation_line_contributions_create(c, info->w, input->w, details);
    if (contrib_h == NULL || !flow_set_owner(c, contrib_h, details)) {
        FLOW_destroy(c, details);
        FLOW_error_return(c);
    }
    flow_prof_stop(c, "contributions_calc", true, false);

    flow_prof_start(c, "create_bitmap_float (buffers)", false);

    struct flow_bitmap_float * source_buf = flow_bitmap_float_create_header(c, input->w, 1, 4);
    if (source_buf == NULL || !flow_set_owner(c, source_buf, details)) {
        FLOW_destroy(c, details);
        FLOW_error_return(c);
    }
    struct flow_bitmap_float * dest_buf = flow_bitmap_float_create(c, info->w, 1, 4, true);
    if (dest_buf == NULL || !flow_set_owner(c, dest_buf, details)) {
        FLOW_destroy(c, details);
        FLOW_error_return(c);
    }
    source_buf->alpha_meaningful = input_fmt == flow_bgra32;
    dest_buf->alpha_meaningful = source_buf->alpha_meaningful;

    source_buf->alpha_premultiplied = source_buf->channels == 4;
    dest_buf->alpha_premultiplied = source_buf->alpha_premultiplied;

    flow_prof_stop(c, "create_bitmap_float (buffers)", true, false);

    // Determine how many rows we need to buffer
    int32_t max_input_rows = 0;
    for (uint32_t i = 0; i < contrib_v->LineLength; i++) {
        int inputs = contrib_v->ContribRow[i].Right - contrib_v->ContribRow[i].Left + 1;
        if (inputs > max_input_rows)
            max_input_rows = inputs;
    }

    // Allocate space
    size_t row_floats = 4 * input->w;
    float * buf = (float *)FLOW_malloc_owned(c, sizeof(float) * row_floats * (max_input_rows + 1), details);
    float ** rows = (float **)FLOW_malloc_owned(c, sizeof(float *) * max_input_rows, details);
    float * row_coefficients = (float *)FLOW_malloc_owned(c, sizeof(float) * max_input_rows, details);
    int32_t * row_indexes = (int32_t *)FLOW_malloc_owned(c, sizeof(int32_t) * max_input_rows, details);
    if (buf == NULL || rows == NULL || row_coefficients == NULL || row_indexes == NULL) {
        FLOW_destroy(c, details);
        FLOW_error_return(c);
    }
    float * output_address = &buf[row_floats * max_input_rows];
    for (int i = 0; i < max_input_rows; i++) {
        rows[i] = &buf[4 * input->w * i];
        row_coefficients[i] = 1;
        row_indexes[i] = -1;
    }

    for (uint32_t out_row = 0; out_row < cropped_canvas->h; out_row++) {
        struct flow_interpolation_pixel_contributions contrib = contrib_v->ContribRow[out_row];
        // Clear output row
        memset(output_address, 0, sizeof(float) * row_floats);

        for (int input_row = contrib.Left; input_row <= contrib.Right; input_row++) {
            // Try to find row in buffer if already loaded
            bool loaded = false;
            int active_buf_ix = -1;
            for (int buf_row = 0; buf_row < max_input_rows; buf_row++) {
                if (row_indexes[buf_row] == input_row) {
                    active_buf_ix = buf_row;
                    loaded = true;
                    break;
                }
            }
            // Not loaded?
            if (!loaded) {
                for (int buf_row = 0; buf_row < max_input_rows; buf_row++) {
                    if (row_indexes[buf_row] < contrib.Left) {
                        active_buf_ix = buf_row;
                        loaded = false;
                        break;
                    }
                }
            }
            if (active_buf_ix < 0) {
                FLOW_destroy(c, details);
                FLOW_error(c, flow_status_Invalid_internal_state); // Buffer too small!
                return false;
            }
            if (!loaded) {
                // Load row
                source_buf->pixels = rows[active_buf_ix];

                flow_prof_start(c, "convert_srgb_to_linear", false);
                if (!flow_bitmap_float_convert_srgb_to_linear(c, &colorcontext, input, input_row, source_buf, 0, 1)) {
                    FLOW_destroy(c, details);
                    FLOW_error_return(c);
                }
                flow_prof_stop(c, "convert_srgb_to_linear", true, false);

                row_coefficients[active_buf_ix] = 1;
                row_indexes[active_buf_ix] = input_row;
                loaded = true;
            }
            float weight = contrib.Weights[input_row - contrib.Left];
            if (fabs(weight) > 0.00000002) {
                // Apply coefficient, update tracking
                float delta_coefficient = weight / row_coefficients[active_buf_ix];
                multiply_row(rows[active_buf_ix], row_floats, delta_coefficient);
                row_coefficients[active_buf_ix] = weight;

                // Add row
                add_row(output_address, rows[active_buf_ix], row_floats);
            }
        }

        // The container now points to the row which has been vertically scaled
        source_buf->pixels = output_address;

        // Now scale horizontally!
        flow_prof_start(c, "ScaleBgraFloatRows", false);
        if (!flow_bitmap_float_scale_rows(c, source_buf, 0, dest_buf, 0, 1, contrib_h->ContribRow)) {
            FLOW_destroy(c, details);
            FLOW_error_return(c);
        }
        flow_prof_stop(c, "ScaleBgraFloatRows", true, false);

        if (!flow_bitmap_float_composite_linear_over_srgb(c, &colorcontext, dest_buf, 0, cropped_canvas, out_row, 1, false)) {
            FLOW_destroy(c, details);
            FLOW_error_return(c);
        }
    }
    FLOW_destroy(c, cropped_canvas == uncropped_canvas ? NULL : cropped_canvas);
    FLOW_destroy(c, details);
    return true;
}
struct flow_convolution_kernel * flow_convolution_kernel_create(flow_c * context, uint32_t radius)
{
    struct flow_convolution_kernel * k = FLOW_calloc_array(context, 1, struct flow_convolution_kernel);
    // For the actual array;
    float * a = FLOW_calloc_array(context, radius * 2 + 1, float);
    // we assume a maximum of 4 channels are going to need buffering during convolution
    float * buf = (float *)FLOW_malloc(context, (radius + 2) * 4 * sizeof(float));

    if (k == NULL || a == NULL || buf == NULL) {
        FLOW_free(context, k);
        FLOW_free(context, a);
        FLOW_free(context, buf);
        FLOW_error(context, flow_status_Out_of_memory);
        return NULL;
    }
    k->kernel = a;
    k->width = radius * 2 + 1;
    k->buffer = buf;
    k->radius = radius;
    return k;
}
void flow_convolution_kernel_destroy(flow_c * context, struct flow_convolution_kernel * kernel)
{
    if (kernel != NULL) {
        FLOW_free(context, kernel->kernel);
        FLOW_free(context, kernel->buffer);
        kernel->kernel = NULL;
        kernel->buffer = NULL;
    }
    FLOW_free(context, kernel);
}

struct flow_convolution_kernel * flow_convolution_kernel_create_gaussian(flow_c * context, double stdDev,
                                                                         uint32_t radius)
{
    struct flow_convolution_kernel * k = flow_convolution_kernel_create(context, radius);
    if (k != NULL) {
        for (uint32_t i = 0; i < k->width; i++) {

            k->kernel[i] = (float)ir_gaussian(abs((int)radius - (int)i), stdDev);
        }
    }
    return k;
}

double flow_convolution_kernel_sum(struct flow_convolution_kernel * kernel)
{
    double sum = 0;
    for (uint32_t i = 0; i < kernel->width; i++) {
        sum += kernel->kernel[i];
    }
    return sum;
}

void flow_convolution_kernel_normalize(struct flow_convolution_kernel * kernel, float desiredSum)
{
    double sum = flow_convolution_kernel_sum(kernel);
    if (sum == 0)
        return; // nothing to do here, zeroes are as normalized as you can get ;)
    float factor = (float)(desiredSum / sum);
    for (uint32_t i = 0; i < kernel->width; i++) {
        kernel->kernel[i] *= factor;
    }
}
struct flow_convolution_kernel * flow_convolution_kernel_create_gaussian_normalized(flow_c * context, double stdDev,
                                                                                    uint32_t radius)
{
    struct flow_convolution_kernel * kernel = flow_convolution_kernel_create_gaussian(context, stdDev, radius);
    if (kernel != NULL) {
        flow_convolution_kernel_normalize(kernel, 1);
    }
    return kernel;
}

struct flow_convolution_kernel * flow_convolution_kernel_create_gaussian_sharpen(flow_c * context, double stdDev,
                                                                                 uint32_t radius)
{
    struct flow_convolution_kernel * kernel = flow_convolution_kernel_create_gaussian(context, stdDev, radius);
    if (kernel != NULL) {
        double sum = flow_convolution_kernel_sum(kernel);
        for (uint32_t i = 0; i < kernel->width; i++) {
            if (i == radius) {
                kernel->kernel[i] = (float)(2 * sum - kernel->kernel[i]);
            } else {
                kernel->kernel[i] *= -1;
            }
        }
        flow_convolution_kernel_normalize(kernel, 1);
    }
    return kernel;
}

bool flow_bitmap_float_convolve_rows(flow_c * context, struct flow_bitmap_float * buf,
                                     struct flow_convolution_kernel * kernel, uint32_t convolve_channels,
                                     uint32_t from_row, int row_count)
{

    const uint32_t radius = kernel->radius;
    const float threshold_min = kernel->threshold_min_change;
    const float threshold_max = kernel->threshold_max_change;

    // Do nothing unless the image is at least half as wide as the kernel.
    if (buf->w < radius + 1)
        return true;

    const uint32_t buffer_count = radius + 1;
    const uint32_t w = buf->w;
    const int32_t int_w = (int32_t)buf->w;
    const uint32_t step = buf->channels;

    const uint32_t until_row = row_count < 0 ? buf->h : from_row + (unsigned)row_count;

    const uint32_t ch_used = convolve_channels;

    float * __restrict buffer = kernel->buffer;
    float * __restrict avg = &kernel->buffer[buffer_count * ch_used];

    const float * __restrict kern = kernel->kernel;

    const int wrap_mode = 0;

    for (uint32_t row = from_row; row < until_row; row++) {

        float * __restrict source_buffer = &buf->pixels[row * buf->float_stride];
        int circular_idx = 0;

        for (uint32_t ndx = 0; ndx < w + buffer_count; ndx++) {
            // Flush old value
            if (ndx >= buffer_count) {
                memcpy(&source_buffer[(ndx - buffer_count) * step], &buffer[circular_idx * ch_used],
                       ch_used * sizeof(float));
            }
            // Calculate and enqueue new value
            if (ndx < w) {
                const int left = ndx - radius;
                const int right = ndx + radius;
                int i;

                memset(avg, 0, sizeof(float) * ch_used);

                if (left < 0 || right >= (int32_t)w) {
                    if (wrap_mode == 0) {
                        // Only sample what's present, and fix the average later.
                        float total_weight = 0;
                        /* Accumulate each channel */
                        for (i = left; i <= right; i++) {
                            if (i > 0 && i < int_w) {
                                const float weight = kern[i - left];
                                total_weight += weight;
                                for (uint32_t j = 0; j < ch_used; j++)
                                    avg[j] += weight * source_buffer[i * step + j];
                            }
                        }
                        for (uint32_t j = 0; j < ch_used; j++)
                            avg[j] = avg[j] / total_weight;
                    } else if (wrap_mode == 1) {
                        // Extend last pixel to be used for all missing inputs
                        /* Accumulate each channel */
                        for (i = left; i <= right; i++) {
                            const float weight = kern[i - left];
                            const uint32_t ix = EVIL_CLAMP(i, 0, int_w - 1);
                            for (uint32_t j = 0; j < ch_used; j++)
                                avg[j] += weight * source_buffer[ix * step + j];
                        }
                    }
                } else {
                    /* Accumulate each channel */
                    for (i = left; i <= right; i++) {
                        const float weight = kern[i - left];
                        for (uint32_t j = 0; j < ch_used; j++)
                            avg[j] += weight * source_buffer[i * step + j];
                    }
                }

                // Enqueue difference
                memcpy(&buffer[circular_idx * ch_used], avg, ch_used * sizeof(float));

                if (threshold_min > 0 || threshold_max > 0) {
                    float change = 0;
                    for (uint32_t j = 0; j < ch_used; j++)
                        change += (float)fabs(source_buffer[ndx * step + j] - avg[j]);

                    if (change < threshold_min || change > threshold_max) {
                        memcpy(&buffer[circular_idx * ch_used], &source_buffer[ndx * step], ch_used * sizeof(float));
                    }
                }
            }
            circular_idx = (circular_idx + 1) % buffer_count;
        }
    }
    return true;
}

static bool BitmapFloat_boxblur_rows(flow_c * context, struct flow_bitmap_float * image, uint32_t radius,
                                     uint32_t passes, const uint32_t convolve_channels, float * work_buffer,
                                     uint32_t from_row, int row_count)
{
    const uint32_t buffer_count = radius + 1;
    const uint32_t w = image->w;
    const uint32_t step = image->channels;
    const uint32_t until_row = row_count < 0 ? image->h : from_row + (unsigned)row_count;
    const uint32_t ch_used = image->channels;
    float * __restrict buffer = work_buffer;
    const uint32_t std_count = radius * 2 + 1;
    const float std_factor = 1.0f / (float)(std_count);
    for (uint32_t row = from_row; row < until_row; row++) {
        float * __restrict source_buffer = &image->pixels[row * image->float_stride];
        for (uint32_t pass_index = 0; pass_index < passes; pass_index++) {
            int circular_idx = 0;
            float sum[4] = { 0, 0, 0, 0 };
            uint32_t count = 0;
            for (uint32_t ndx = 0; ndx < radius; ndx++) {
                for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                    sum[ch] += source_buffer[ndx * step + ch];
                }
                count++;
            }
            for (uint32_t ndx = 0; ndx < w + buffer_count; ndx++) { // Pixels
                if (ndx >= buffer_count) { // same as ndx > radius
                    // Remove trailing item from average
                    for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                        sum[ch] -= source_buffer[(ndx - radius - 1) * step + ch];
                    }
                    count--;
                    // Flush old value
                    memcpy(&source_buffer[(ndx - buffer_count) * step], &buffer[circular_idx * ch_used],
                           ch_used * sizeof(float));
                }
                // Calculate and enqueue new value
                if (ndx < w) {
                    if (ndx < w - radius) {
                        for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                            sum[ch] += source_buffer[(ndx + radius) * step + ch];
                        }
                        count++;
                    }
                    // Enqueue averaged value
                    if (count != std_count) {
                        for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                            buffer[circular_idx * ch_used + ch] = sum[ch] / (float)count; // Recompute factor
                        }
                    } else {
                        for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                            buffer[circular_idx * ch_used + ch] = sum[ch] * std_factor;
                        }
                    }
                }
                circular_idx = (circular_idx + 1) % buffer_count;
            }
        }
    }
    return true;
}
static bool BitmapFloat_boxblur_misaligned_rows(flow_c * context, struct flow_bitmap_float * image, uint32_t radius,
                                                int align, const uint32_t convolve_channels, float * work_buffer,
                                                uint32_t from_row, int row_count)
{
    if (align != 1 && align != -1) {
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false;
    }
    const uint32_t buffer_count = radius + 2;
    const uint32_t w = image->w;
    const uint32_t step = image->channels;
    const uint32_t until_row = row_count < 0 ? image->h : from_row + (unsigned)row_count;
    const uint32_t ch_used = image->channels;
    float * __restrict buffer = work_buffer;
    const uint32_t write_offset = align == -1 ? 0 : 1;
    for (uint32_t row = from_row; row < until_row; row++) {
        float * __restrict source_buffer = &image->pixels[row * image->float_stride];
        int circular_idx = 0;
        float sum[4] = { 0, 0, 0, 0 };
        float count = 0;
        for (uint32_t ndx = 0; ndx < radius; ndx++) {
            float factor = (ndx == radius - 1) ? 0.5f : 1;
            for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                sum[ch] += source_buffer[ndx * step + ch] * factor;
            }
            count += factor;
        }
        for (uint32_t ndx = 0; ndx < w + buffer_count - write_offset; ndx++) { // Pixels
            // Calculate new value
            if (ndx < w) {
                if (ndx < w - radius) {
                    for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                        sum[ch] += source_buffer[(ndx + radius) * step + ch] * 0.5f;
                    }
                    count += 0.5f;
                }
                if (ndx < w - radius + 1) {
                    for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                        sum[ch] += source_buffer[(ndx - 1 + radius) * step + ch] * 0.5f;
                    }
                    count += 0.5f;
                }
                // Remove trailing items from average
                if (ndx >= radius) {
                    for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                        sum[ch] -= source_buffer[(ndx - radius) * step + ch] * 0.5f;
                    }
                    count -= 0.5f;
                }
                if (ndx >= radius + 1) {
                    for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                        sum[ch] -= source_buffer[(ndx - 1 - radius) * step + ch] * 0.5f;
                    }
                    count -= 0.5f;
                }
            }
            // Flush old value
            if (ndx >= buffer_count - write_offset) {
                memcpy(&source_buffer[(ndx + write_offset - buffer_count) * step], &buffer[circular_idx * ch_used],
                       ch_used * sizeof(float));
            }
            // enqueue new value
            if (ndx < w) {
                for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                    buffer[circular_idx * ch_used + ch] = sum[ch] / (float)count;
                }
            }
            circular_idx = (circular_idx + 1) % buffer_count;
        }
    }
    return true;
}

uint32_t flow_bitmap_float_approx_gaussian_calculate_d(float sigma, uint32_t bitmap_width)
{
    uint32_t d = (int)floorf(1.8799712059732503768118239636082839397552400554574537f * sigma + 0.5f);
    d = umin(d, (bitmap_width - 1) / 2); // Never exceed half the size of the buffer.
    return d;
}

uint32_t flow_bitmap_float_approx_gaussian_buffer_element_count_required(float sigma, uint32_t bitmap_width)
{
    return flow_bitmap_float_approx_gaussian_calculate_d(sigma, bitmap_width) * 2 + 12; // * sizeof(float);
}
bool flow_bitmap_float_approx_gaussian_blur_rows(flow_c * context, struct flow_bitmap_float * image, float sigma,
                                                 float * buffer, size_t buffer_element_count, uint32_t from_row,
                                                 int row_count)
{
    // Ensure sigma is large enough for approximation to be accurate.
    if (sigma < 2) {
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false;
    }

    // Ensure the buffer is large enough
    if (flow_bitmap_float_approx_gaussian_buffer_element_count_required(sigma, image->w) > buffer_element_count) {
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false;
    }

    // http://www.w3.org/TR/SVG11/filters.html#feGaussianBlur
    // For larger values of 's' (s >= 2.0), an approximation can be used :
    // Three successive box - blurs build a piece - wise quadratic convolution kernel, which approximates the Gaussian
    // kernel to within roughly 3 % .
    uint32_t d = flow_bitmap_float_approx_gaussian_calculate_d(sigma, image->w);
    //... if d is odd, use three box - blurs of size 'd', centered on the output pixel.
    if (d % 2 > 0) {
        if (!BitmapFloat_boxblur_rows(context, image, d / 2, 3, image->channels, buffer, from_row, row_count)) {
            FLOW_error_return(context);
        }
    } else {
        // ... if d is even, two box - blurs of size 'd'
        // (the first one centered on the pixel boundary between the output pixel and the one to the left,
        //  the second one centered on the pixel boundary between the output pixel and the one to the right)
        // and one box blur of size 'd+1' centered on the output pixel.
        if (!BitmapFloat_boxblur_misaligned_rows(context, image, d / 2, -1, image->channels, buffer, from_row,
                                                 row_count)) {
            FLOW_error_return(context);
        }
        if (!BitmapFloat_boxblur_misaligned_rows(context, image, d / 2, 1, image->channels, buffer, from_row,
                                                 row_count)) {
            FLOW_error_return(context);
        }
        if (!BitmapFloat_boxblur_rows(context, image, d / 2 + 1, 1, image->channels, buffer, from_row, row_count)) {
            FLOW_error_return(context);
        }
    }
    return true;
}


FLOW_HINT_HOT FLOW_HINT_UNSAFE_MATH_OPTIMIZATIONS static inline void transpose4x4_SSE(float * A, float * B,
                                                                                      const int lda, const int ldb)
{
    __m128 row1 = _mm_loadu_ps(&A[0 * lda]);
    __m128 row2 = _mm_loadu_ps(&A[1 * lda]);
    __m128 row3 = _mm_loadu_ps(&A[2 * lda]);
    __m128 row4 = _mm_loadu_ps(&A[3 * lda]);
    _MM_TRANSPOSE4_PS(row1, row2, row3, row4);
    _mm_storeu_ps(&B[0 * ldb], row1);
    _mm_storeu_ps(&B[1 * ldb], row2);
    _mm_storeu_ps(&B[2 * ldb], row3);
    _mm_storeu_ps(&B[3 * ldb], row4);
}

FLOW_HINT_HOT
static inline void transpose_block_SSE4x4(float * A, float * B, const int n, const int m, const int lda, const int ldb,
                                          const int block_size)
{
    //#pragma omp parallel for collapse(2)
    for (int i = 0; i < n; i += block_size) {
        for (int j = 0; j < m; j += block_size) {
            int max_i2 = i + block_size < n ? i + block_size : n;
            int max_j2 = j + block_size < m ? j + block_size : m;
            for (int i2 = i; i2 < max_i2; i2 += 4) {
                for (int j2 = j; j2 < max_j2; j2 += 4) {
                    transpose4x4_SSE(&A[i2 * lda + j2], &B[j2 * ldb + i2], lda, ldb);
                }
            }
        }
    }
}

FLOW_HINT_HOT FLOW_HINT_UNSAFE_MATH_OPTIMIZATIONS

bool
flow_bitmap_bgra_transpose(flow_c * c, struct flow_bitmap_bgra * from, struct flow_bitmap_bgra * to)
{
    if (from->w != to->h || from->h != to->w || from->fmt != to->fmt) {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }

    if (from->fmt != flow_bgra32 && from->fmt != flow_bgr32) {
        if (!flow_bitmap_bgra_transpose_slow(c, from, to)) {
            FLOW_add_to_callstack(c);
            return false;
        }
        return true;
    }

    // We require 8 when we only need 4 - in case we ever want to enable avx (like if we make it faster)
    const int min_block_size = 8;

    // Strides must be multiple of required alignments
    if (from->stride % min_block_size != 0 || to->stride % min_block_size != 0) {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }
    // 256 (1024x1024 bytes) at 18.18ms, 128 at 18.6ms,  64 at 20.4ms, 16 at 25.71ms
    int block_size = 128;

    int cropped_h = from->h - from->h % min_block_size;
    int cropped_w = from->w - from->w % min_block_size;

    transpose_block_SSE4x4((float *)from->pixels, (float *)to->pixels, cropped_h, cropped_w, from->stride / 4,
                           to->stride / 4, block_size);

    // Copy missing bits
    for (uint32_t x = cropped_h; x < to->w; x++) {
        for (uint32_t y = 0; y < to->h; y++) {
            *((uint32_t *)&to->pixels[x * 4 + y * to->stride]) = *((uint32_t *)&from->pixels[x * from->stride + y * 4]);
        }
    }

    for (uint32_t x = 0; x < (uint32_t)cropped_h; x++) {
        for (uint32_t y = cropped_w; y < to->h; y++) {
            *((uint32_t *)&to->pixels[x * 4 + y * to->stride]) = *((uint32_t *)&from->pixels[x * from->stride + y * 4]);
        }
    }

    return true;
}

bool flow_bitmap_bgra_transpose_slow(flow_c * c, struct flow_bitmap_bgra * from, struct flow_bitmap_bgra * to)
{
    if (from->w != to->h || from->h != to->w || from->fmt != to->fmt) {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }

    if (from->fmt == flow_bgra32 || from->fmt == flow_bgr32) {
        for (uint32_t x = 0; x < to->w; x++) {
            for (uint32_t y = 0; y < to->h; y++) {
                *((uint32_t *)&to->pixels[x * 4 + y * to->stride])
                    = *((uint32_t *)&from->pixels[x * from->stride + y * 4]);
            }
        }
        return true;
    } else if (from->fmt == flow_bgr24) {
        int from_stride = from->stride;
        int to_stride = to->stride;
        for (uint32_t x = 0, x_stride = 0, x_3 = 0; x < to->w; x++, x_stride += from_stride, x_3 += 3) {
            for (uint32_t y = 0, y_stride = 0, y_3 = 0; y < to->h; y++, y_stride += to_stride, y_3 += 3) {

                to->pixels[x_3 + y_stride] = from->pixels[x_stride + y_3];
                to->pixels[x_3 + y_stride + 1] = from->pixels[x_stride + y_3 + 1];
                to->pixels[x_3 + y_stride + 2] = from->pixels[x_stride + y_3 + 2];
            }
        }
        return true;
    } else {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }
}


FLOW_HINT_HOT
bool flow_bitmap_float_convert_srgb_to_linear(flow_c * context, struct flow_colorcontext_info * colorcontext,
                                              struct flow_bitmap_bgra * src, uint32_t from_row,
                                              struct flow_bitmap_float * dest, uint32_t dest_row, uint32_t row_count)
{
    if
        unlikely(src->w != dest->w)
    {
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false;
    }
    if
        unlikely(!(from_row + row_count <= src->h && dest_row + row_count <= dest->h))
    {
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false;
    }

    const uint32_t w = src->w;

    const uint32_t units = w * flow_pixel_format_bytes_per_pixel(src->fmt);
    const uint32_t from_step = flow_pixel_format_bytes_per_pixel(src->fmt);
    const uint32_t from_copy = flow_pixel_format_channels(flow_effective_pixel_format(src));

    const uint32_t to_step = dest->channels;
    const uint32_t copy_step = umin(from_copy, to_step);

    if
        unlikely(copy_step != 3 && copy_step != 4)
    {
        FLOW_error_msg(context, flow_status_Unsupported_pixel_format, "copy_step=%d", copy_step);
        return false;
    }
    if
        unlikely(copy_step == 4 && from_step != 4 && to_step != 4)
    {
        FLOW_error_msg(context, flow_status_Unsupported_pixel_format, "copy_step=%d, from_step=%d, to_step=%d",
                       copy_step, from_step, to_step);
        return false;
    }
    if
        likely(copy_step == 4)
    {
        for (uint32_t row = 0; row < row_count; row++) {
            uint8_t * src_start = src->pixels + (from_row + row) * src->stride;
            float * buf = dest->pixels + (dest->float_stride * (row + dest_row));
            for (uint32_t to_x = 0, bix = 0; bix < units; to_x += 4, bix += 4) {
                {
                    const float alpha = ((float)src_start[bix + 3]) / 255.0f;
                    buf[to_x] = alpha * flow_colorcontext_srgb_to_floatspace(colorcontext, src_start[bix]);
                    buf[to_x + 1] = alpha * flow_colorcontext_srgb_to_floatspace(colorcontext, src_start[bix + 1]);
                    buf[to_x + 2] = alpha * flow_colorcontext_srgb_to_floatspace(colorcontext, src_start[bix + 2]);
                    buf[to_x + 3] = alpha;
                }
            }
        }
    }
    else {

#define CONVERT_LINEAR(from_step, to_step)                                                                             \
    for (uint32_t row = 0; row < row_count; row++) {                                                                   \
        uint8_t * src_start = src->pixels + (from_row + row) * src->stride;                                            \
        float * buf = dest->pixels + (dest->float_stride * (row + dest_row));                                          \
        for (uint32_t to_x = 0, bix = 0; bix < units; to_x += to_step, bix += from_step) {                             \
            buf[to_x] = flow_colorcontext_srgb_to_floatspace(colorcontext, src_start[bix]);                            \
            buf[to_x + 1] = flow_colorcontext_srgb_to_floatspace(colorcontext, src_start[bix + 1]);                    \
            buf[to_x + 2] = flow_colorcontext_srgb_to_floatspace(colorcontext, src_start[bix + 2]);                    \
        }                                                                                                              \
    }

        if (from_step == 3 && to_step == 3) {
            CONVERT_LINEAR(3, 3)
        } else if (from_step == 4 && to_step == 3) {
            CONVERT_LINEAR(4, 3)
        } else if (from_step == 3 && to_step == 4) {
            CONVERT_LINEAR(3, 4)
        } else if (from_step == 4 && to_step == 4) {
            CONVERT_LINEAR(4, 4)
        } else {
            FLOW_error_msg(context, flow_status_Unsupported_pixel_format, "copy_step=%d, from_step=%d, to_step=%d",
                           copy_step, from_step, to_step);
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
FLOW_HINT_HOT
bool flow_bitmap_bgra_flip_vertical(flow_c * context, struct flow_bitmap_bgra * b)
{
    void * swap = FLOW_malloc(context, b->stride);
    if (swap == NULL) {
        FLOW_error(context, flow_status_Out_of_memory);
        return false;
    }
    // Dont' copy the full stride (padding), it could be windowed!
    // Todo: try multiple swap rows? 5ms isn't bad, but could be better
    uint32_t row_length = umin(b->stride, b->w * flow_pixel_format_bytes_per_pixel(b->fmt));
    for (uint32_t i = 0; i < b->h / 2; i++) {
        void * top = b->pixels + (i * b->stride);
        void * bottom = b->pixels + ((b->h - 1 - i) * b->stride);
        memcpy(swap, top, row_length);
        memcpy(top, bottom, row_length);
        memcpy(bottom, swap, row_length);
    }
    FLOW_free(context, swap);
    return true;
}
FLOW_HINT_HOT
bool flow_bitmap_bgra_flip_horizontal(flow_c * context, struct flow_bitmap_bgra * b)
{
    if (b->fmt == flow_bgra32 || b->fmt == flow_bgr32) {
        // 12ms simple
        for (uint32_t y = 0; y < b->h; y++) {
            uint32_t * left = (uint32_t *)(b->pixels + (y * b->stride));
            uint32_t * right = (uint32_t *)(b->pixels + (y * b->stride) + 4 * (b->w - 1));
            while (left < right) {
                uint32_t swap = *left;
                *left = *right;
                *right = swap;
                left++;
                right--;
            }
        }

    } else if (b->fmt == flow_bgr24) {
        uint32_t swap[4];
        // Dont' copy the full stride (padding), it could be windowed!
        for (uint32_t y = 0; y < b->h; y++) {
            uint8_t * left = b->pixels + (y * b->stride);
            uint8_t * right = b->pixels + (y * b->stride) + (3 * (b->w - 1));
            while (left < right) {
                memcpy(&swap, left, 3);
                memcpy(left, right, 3);
                memcpy(right, &swap, 3);
                left += 3;
                right -= 3;
            }
        }
    } else {
        uint32_t swap[4];
        // Dont' copy the full stride (padding), it could be windowed!
        for (uint32_t y = 0; y < b->h; y++) {
            uint8_t * left = b->pixels + (y * b->stride);
            uint8_t * right = b->pixels + (y * b->stride) + (flow_pixel_format_bytes_per_pixel(b->fmt) * (b->w - 1));
            while (left < right) {
                memcpy(&swap, left, flow_pixel_format_bytes_per_pixel(b->fmt));
                memcpy(left, right, flow_pixel_format_bytes_per_pixel(b->fmt));
                memcpy(right, &swap, flow_pixel_format_bytes_per_pixel(b->fmt));
                left += flow_pixel_format_bytes_per_pixel(b->fmt);
                right -= flow_pixel_format_bytes_per_pixel(b->fmt);
            }
        }
    }
    return true;
}

static bool flow_bitmap_float_blend_matte(flow_c * context, struct flow_colorcontext_info * colorcontext,
                                          struct flow_bitmap_float * src, const uint32_t from_row,
                                          const uint32_t row_count, const uint8_t * const matte)
{
    // We assume that matte is BGRA, regardless.

    const float matte_a = ((float)matte[3]) / 255.0f;
    const float b = flow_colorcontext_srgb_to_floatspace(colorcontext, matte[0]);
    const float g = flow_colorcontext_srgb_to_floatspace(colorcontext, matte[1]);
    const float r = flow_colorcontext_srgb_to_floatspace(colorcontext, matte[2]);

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

bool flow_bitmap_float_demultiply_alpha(flow_c * context, struct flow_bitmap_float * src, const uint32_t from_row,
                                        const uint32_t row_count)
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

bool flow_bitmap_float_copy_linear_over_srgb(flow_c * context, struct flow_colorcontext_info * colorcontext,
                                             struct flow_bitmap_float * src, const uint32_t from_row,
                                             struct flow_bitmap_bgra * dest, const uint32_t dest_row,
                                             const uint32_t row_count, const uint32_t from_col,
                                             const uint32_t col_count, const bool transpose)
{

    const uint32_t dest_bytes_pp = flow_pixel_format_bytes_per_pixel(dest->fmt);

    const uint32_t srcitems = umin(from_col + col_count, src->w) * src->channels;

    const flow_pixel_format dest_fmt = flow_effective_pixel_format(dest);

    const uint32_t ch = src->channels;
    const bool copy_alpha = dest_fmt == flow_bgra32 && ch == 4 && src->alpha_meaningful;
    const bool clean_alpha = !copy_alpha && dest_fmt == flow_bgra32;
    const uint32_t dest_row_stride = transpose ? dest_bytes_pp : dest->stride;
    const uint32_t dest_pixel_stride = transpose ? dest->stride : dest_bytes_pp;

#define FLOAT_COPY_LINEAR(ch, dest_pixel_stride, copy_alpha, clean_alpha)                                              \
    for (uint32_t row = 0; row < row_count; row++) {                                                                   \
        float * src_row = src->pixels + (row + from_row) * src->float_stride;                                          \
        uint8_t * dest_row_bytes = dest->pixels + (dest_row + row) * dest_row_stride + (from_col * dest_pixel_stride); \
        for (uint32_t ix = from_col * ch; ix < srcitems; ix += ch) {                                                   \
            dest_row_bytes[0] = flow_colorcontext_floatspace_to_srgb(colorcontext, src_row[ix]);                       \
            dest_row_bytes[1] = flow_colorcontext_floatspace_to_srgb(colorcontext, src_row[ix + 1]);                   \
            dest_row_bytes[2] = flow_colorcontext_floatspace_to_srgb(colorcontext, src_row[ix + 2]);                   \
            if (copy_alpha) {                                                                                          \
                dest_row_bytes[3] = uchar_clamp_ff(src_row[ix + 3] * 255.0f);                                          \
            }                                                                                                          \
            if (clean_alpha) {                                                                                         \
                dest_row_bytes[3] = 0xff;                                                                              \
            }                                                                                                          \
            dest_row_bytes += dest_pixel_stride;                                                                       \
        }                                                                                                              \
    }
    if (dest_pixel_stride == 4) {
        if (ch == 3) {
            if (copy_alpha == true && clean_alpha == false) {
                FLOAT_COPY_LINEAR(3, 4, true, false)
            }
            if (copy_alpha == false && clean_alpha == false) {
                FLOAT_COPY_LINEAR(3, 4, false, false)
            }
            if (copy_alpha == false && clean_alpha == true) {
                FLOAT_COPY_LINEAR(3, 4, false, true)
            }
        }
        if (ch == 4) {
            if (copy_alpha == true && clean_alpha == false) {
                FLOAT_COPY_LINEAR(4, 4, true, false)
            }
            if (copy_alpha == false && clean_alpha == false) {
                FLOAT_COPY_LINEAR(4, 4, false, false)
            }
            if (copy_alpha == false && clean_alpha == true) {
                FLOAT_COPY_LINEAR(4, 4, false, true)
            }
        }
    } else {
        if (ch == 3) {
            if (copy_alpha == true && clean_alpha == false) {
                FLOAT_COPY_LINEAR(3, dest_pixel_stride, true, false)
            }
            if (copy_alpha == false && clean_alpha == false) {
                FLOAT_COPY_LINEAR(3, dest_pixel_stride, false, false)
            }
            if (copy_alpha == false && clean_alpha == true) {
                FLOAT_COPY_LINEAR(3, dest_pixel_stride, false, true)
            }
        }
        if (ch == 4) {
            if (copy_alpha == true && clean_alpha == false) {
                FLOAT_COPY_LINEAR(4, dest_pixel_stride, true, false)
            }
            if (copy_alpha == false && clean_alpha == false) {
                FLOAT_COPY_LINEAR(4, dest_pixel_stride, false, false)
            }
            if (copy_alpha == false && clean_alpha == true) {
                FLOAT_COPY_LINEAR(4, dest_pixel_stride, false, true)
            }
        }
    }
    return true;
}
FLOW_HINT_HOT

static bool BitmapFloat_compose_linear_over_srgb(flow_c * context, struct flow_colorcontext_info * colorcontext,
                                                 struct flow_bitmap_float * src, const uint32_t from_row,
                                                 struct flow_bitmap_bgra * dest, const uint32_t dest_row,
                                                 const uint32_t row_count, const uint32_t from_col,
                                                 const uint32_t col_count, const bool transpose)
{

    const uint32_t dest_bytes_pp = flow_pixel_format_bytes_per_pixel(dest->fmt);
    const uint32_t dest_row_stride = transpose ? dest_bytes_pp : dest->stride;
    const uint32_t dest_pixel_stride = transpose ? dest->stride : dest_bytes_pp;
    const uint32_t srcitems = umin(from_col + col_count, src->w) * src->channels;
    const uint32_t ch = src->channels;

    const flow_pixel_format dest_effective_format = flow_effective_pixel_format(dest);

    const bool dest_alpha = dest_effective_format == flow_bgra32;

    const uint8_t dest_alpha_index = dest_alpha ? 3 : 0;
    const float dest_alpha_to_float_coeff = dest_alpha ? 1.0f / 255.0f : 0.0f;
    const float dest_alpha_to_float_offset = dest_alpha ? 0.0f : 1.0f;
    for (uint32_t row = 0; row < row_count; row++) {
        // const float * const __restrict src_row = src->pixels + (row + from_row) * src->float_stride;
        float * src_row = src->pixels + (row + from_row) * src->float_stride;

        uint8_t * dest_row_bytes = dest->pixels + (dest_row + row) * dest_row_stride + (from_col * dest_pixel_stride);

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

            const float b = flow_colorcontext_srgb_to_floatspace(colorcontext, dest_b) * a + src_b;
            const float g = flow_colorcontext_srgb_to_floatspace(colorcontext, dest_g) * a + src_g;
            const float r = flow_colorcontext_srgb_to_floatspace(colorcontext, dest_r) * a + src_r;

            const float final_alpha = src_a + a;

            dest_row_bytes[0] = flow_colorcontext_floatspace_to_srgb(colorcontext, b / final_alpha);
            dest_row_bytes[1] = flow_colorcontext_floatspace_to_srgb(colorcontext, g / final_alpha);
            dest_row_bytes[2] = flow_colorcontext_floatspace_to_srgb(colorcontext, r / final_alpha);
            if (dest_alpha) {
                dest_row_bytes[3] = uchar_clamp_ff(final_alpha * 255);
            }
            // TODO: split out 4 and 3 so compiler can vectorize maybe?
            dest_row_bytes += dest_pixel_stride;
        }
    }
    return true;
}

bool flow_bitmap_float_composite_linear_over_srgb(flow_c * context, struct flow_colorcontext_info * colorcontext,
                                                  struct flow_bitmap_float * src_mut, uint32_t from_row,
                                                  struct flow_bitmap_bgra * dest, uint32_t dest_row, uint32_t row_count,
                                                  bool transpose)
{
    if (transpose ? src_mut->w != dest->h : src_mut->w != dest->w) {
        // TODO: Add more bounds checks
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false;
    }
    if (dest->compositing_mode == flow_bitmap_compositing_blend_with_self && src_mut->alpha_meaningful
        && src_mut->channels == 4) {
        if (!src_mut->alpha_premultiplied) {
            // Something went wrong. We should always have alpha premultiplied.
            FLOW_error(context, flow_status_Invalid_internal_state);
            return false;
        }
        // Compose
        if (!BitmapFloat_compose_linear_over_srgb(context, colorcontext, src_mut, from_row, dest, dest_row, row_count,
                                                  0, src_mut->w, transpose)) {
            FLOW_add_to_callstack(context);
            return false;
        }
    } else {
        if (src_mut->channels == 4 && src_mut->alpha_meaningful) {
            bool demultiply = src_mut->alpha_premultiplied;

            if (dest->compositing_mode == flow_bitmap_compositing_blend_with_matte) {
                if (!flow_bitmap_float_blend_matte(context, colorcontext, src_mut, from_row, row_count,
                                                   dest->matte_color)) {
                    FLOW_add_to_callstack(context);
                    return false;
                }
                demultiply = false;
            }
            if (demultiply) {
                // Demultiply before copy
                if (!flow_bitmap_float_demultiply_alpha(context, src_mut, from_row, row_count)) {
                    FLOW_add_to_callstack(context);
                    return false;
                }
            }
        }
        // Copy/overwrite
        if (!flow_bitmap_float_copy_linear_over_srgb(context, colorcontext, src_mut, from_row, dest, dest_row,
                                                     row_count, 0, src_mut->w, transpose)) {
            FLOW_add_to_callstack(context);
            return false;
        }
    }

    return true;
}


bool flow_bitmap_float_linear_to_luv_rows(flow_c * context, struct flow_bitmap_float * bit, const uint32_t start_row,
                                          const uint32_t row_count)
{
    if (!(start_row + row_count <= bit->h)) {
        FLOW_error(context, flow_status_Invalid_internal_state); // Don't access rows past the end of the bitmap
        return false;
    }
    if ((bit->w * bit->channels) != bit->float_stride) {
        FLOW_error(context, flow_status_Invalid_internal_state); // This algorithm can't handle padding, if present
        return false;
    }
    float * start_at = bit->float_stride * start_row + bit->pixels;

    const float * end_at = bit->float_stride * (start_row + row_count) + bit->pixels;

    for (float * pix = start_at; pix < end_at; pix++) {
        linear_to_luv(pix);
    }
    return true;
}

bool flow_bitmap_float_luv_to_linear_rows(flow_c * context, struct flow_bitmap_float * bit, const uint32_t start_row,
                                          const uint32_t row_count)
{
    if (!(start_row + row_count <= bit->h)) {
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false;
    }
    if ((bit->w * bit->channels) != bit->float_stride) {
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false;
    }
    float * start_at = bit->float_stride * start_row + bit->pixels;

    const float * end_at = bit->float_stride * (start_row + row_count) + bit->pixels;

    for (float * pix = start_at; pix < end_at; pix++) {
        luv_to_linear(pix);
    }
    return true;
}

bool flow_bitmap_bgra_apply_color_matrix(flow_c * context, struct flow_bitmap_bgra * bmp, const uint32_t row,
                                         const uint32_t count, float * const __restrict m[5])
{
    const uint32_t stride = bmp->stride;
    const uint32_t ch = flow_pixel_format_bytes_per_pixel(bmp->fmt);
    const uint32_t w = bmp->w;
    const uint32_t h = umin(row + count, bmp->h);
    const float m40 = m[4][0] * 255.0f;
    const float m41 = m[4][1] * 255.0f;
    const float m42 = m[4][2] * 255.0f;
    const float m43 = m[4][3] * 255.0f;

    if (ch == 4) {

        for (uint32_t y = row; y < h; y++)
            for (uint32_t x = 0; x < w; x++) {
                uint8_t * const __restrict data = bmp->pixels + stride * y + x * ch;

                const uint8_t r = uchar_clamp_ff(m[0][0] * data[2] + m[1][0] * data[1] + m[2][0] * data[0]
                                                 + m[3][0] * data[3] + m40);
                const uint8_t g = uchar_clamp_ff(m[0][1] * data[2] + m[1][1] * data[1] + m[2][1] * data[0]
                                                 + m[3][1] * data[3] + m41);
                const uint8_t b = uchar_clamp_ff(m[0][2] * data[2] + m[1][2] * data[1] + m[2][2] * data[0]
                                                 + m[3][2] * data[3] + m42);
                const uint8_t a = uchar_clamp_ff(m[0][3] * data[2] + m[1][3] * data[1] + m[2][3] * data[0]
                                                 + m[3][3] * data[3] + m43);

                uint8_t * newdata = bmp->pixels + stride * y + x * ch;
                newdata[0] = b;
                newdata[1] = g;
                newdata[2] = r;
                newdata[3] = a;
            }
    } else if (ch == 3) {

        for (uint32_t y = row; y < h; y++)
            for (uint32_t x = 0; x < w; x++) {
                unsigned char * const __restrict data = bmp->pixels + stride * y + x * ch;

                const uint8_t r = uchar_clamp_ff(m[0][0] * data[2] + m[1][0] * data[1] + m[2][0] * data[0] + m40);
                const uint8_t g = uchar_clamp_ff(m[0][1] * data[2] + m[1][1] * data[1] + m[2][1] * data[0] + m41);
                const uint8_t b = uchar_clamp_ff(m[0][2] * data[2] + m[1][2] * data[1] + m[2][2] * data[0] + m42);

                uint8_t * newdata = bmp->pixels + stride * y + x * ch;
                newdata[0] = b;
                newdata[1] = g;
                newdata[2] = r;
            }
    } else {
        FLOW_error(context, flow_status_Unsupported_pixel_format);
        return false;
    }
    return true;
}

bool flow_bitmap_float_apply_color_matrix(flow_c * context, struct flow_bitmap_float * bmp, const uint32_t row,
                                          const uint32_t count, float ** m)
{
    const uint32_t stride = bmp->float_stride;
    const uint32_t ch = bmp->channels;
    const uint32_t w = bmp->w;
    const uint32_t h = umin(row + count, bmp->h);
    switch (ch) {
        case 4: {
            for (uint32_t y = row; y < h; y++)
                for (uint32_t x = 0; x < w; x++) {
                    float * const __restrict data = bmp->pixels + stride * y + x * ch;

                    const float r
                        = (m[0][0] * data[2] + m[1][0] * data[1] + m[2][0] * data[0] + m[3][0] * data[3] + m[4][0]);
                    const float g
                        = (m[0][1] * data[2] + m[1][1] * data[1] + m[2][1] * data[0] + m[3][1] * data[3] + m[4][1]);
                    const float b
                        = (m[0][2] * data[2] + m[1][2] * data[1] + m[2][2] * data[0] + m[3][2] * data[3] + m[4][2]);
                    const float a
                        = (m[0][3] * data[2] + m[1][3] * data[1] + m[2][3] * data[0] + m[3][3] * data[3] + m[4][3]);

                    float * newdata = bmp->pixels + stride * y + x * ch;
                    newdata[0] = b;
                    newdata[1] = g;
                    newdata[2] = r;
                    newdata[3] = a;
                }
            return true;
        }
        case 3: {

            for (uint32_t y = row; y < h; y++)
                for (uint32_t x = 0; x < w; x++) {

                    float * const __restrict data = bmp->pixels + stride * y + x * ch;

                    const float r = (m[0][0] * data[2] + m[1][0] * data[1] + m[2][0] * data[0] + m[4][0]);
                    const float g = (m[0][1] * data[2] + m[1][1] * data[1] + m[2][1] * data[0] + m[4][1]);
                    const float b = (m[0][2] * data[2] + m[1][2] * data[1] + m[2][2] * data[0] + m[4][2]);

                    float * newdata = bmp->pixels + stride * y + x * ch;
                    newdata[0] = b;
                    newdata[1] = g;
                    newdata[2] = r;
                }
            return true;
        }
        default: {
            FLOW_error(context, flow_status_Unsupported_pixel_format);
            return false;
        }
    }
}

bool flow_bitmap_bgra_populate_histogram(flow_c * context, struct flow_bitmap_bgra * bmp, uint64_t * histograms,
                                         uint32_t histogram_size_per_channel, uint32_t histogram_count,
                                         uint64_t * pixels_sampled)
{
    const uint32_t row = 0;
    const uint32_t count = bmp->h;
    const uint32_t stride = bmp->stride;
    const uint32_t ch = flow_pixel_format_bytes_per_pixel(bmp->fmt);
    const uint32_t w = bmp->w;
    const uint32_t h = umin(row + count, bmp->h);

    if (histogram_size_per_channel != 256) {
        // We're restricting it to this for speed
        FLOW_error(context, flow_status_Invalid_argument);
        return false;
    }

    const int shift = 0; // 8 - intlog2(histogram_size_per_channel);

    if (ch == 4 || ch == 3) {

        if (histogram_count == 1) {

            for (uint32_t y = row; y < h; y++) {
                for (uint32_t x = 0; x < w; x++) {
                    uint8_t * const __restrict data = bmp->pixels + stride * y + x * ch;

                    histograms[(306 * data[2] + 601 * data[1] + 117 * data[0]) >> shift]++;
                }
            }
        } else if (histogram_count == 3) {
            for (uint32_t y = row; y < h; y++) {
                for (uint32_t x = 0; x < w; x++) {
                    uint8_t * const __restrict data = bmp->pixels + stride * y + x * ch;
                    histograms[data[2] >> shift]++;
                    histograms[(data[1] >> shift) + histogram_size_per_channel]++;
                    histograms[(data[0] >> shift) + 2 * histogram_size_per_channel]++;
                }
            }
        } else if (histogram_count == 2) {
            for (uint32_t y = row; y < h; y++) {
                for (uint32_t x = 0; x < w; x++) {
                    uint8_t * const __restrict data = bmp->pixels + stride * y + x * ch;
                    // Calculate luminosity and saturation
                    histograms[(306 * data[2] + 601 * data[1] + 117 * data[0]) >> shift]++;
                    histograms[histogram_size_per_channel
                               + (int_max(255, int_max(abs((int)data[2] - (int)data[1]),
                                                       abs((int)data[1] - (int)data[0]))) >> shift)]++;
                }
            }
        } else {
            FLOW_error(context, flow_status_Invalid_internal_state);
            return false;
        }

        *(pixels_sampled) = (h - row) * w;
    } else {
        FLOW_error(context, flow_status_Unsupported_pixel_format);
        return false;
    }
    return true;
}

// Gamma correction  http://www.4p8.com/eric.brasseur/gamma.html#formulas

#ifdef EXPOSE_SIGMOID

flow_static void colorcontext_sigmoid_internal(flow_colorcontext_info * c, float x_coefficient, float x_offset,
                                               float constant)
{
    c->sigmoid.constant = constant; // 1
    c->sigmoid.x_coeff = x_coefficient; // 2
    c->sigmoid.x_offset = x_offset; //-1
    c->sigmoid.y_offset = 0;
    c->sigmoid.y_coeff = 1;

    c->sigmoid.y_coeff = 1 / (sigmoid(&c->sigmoid, 1.0) - sigmoid(&c->sigmoid, 0));
    c->sigmoid.y_offset = -1 * sigmoid(&c->sigmoid, 0);
}

static float derive_constant(float x, float slope, float sign)
{
    return (float)((-2.0f * slope * fabs(x) + sign * sqrtf((float)(1.0f - 4.0f * slope * fabs(x))) + 1.0f) / 2.0f
                   * slope);
}

#endif

void flow_colorcontext_init(flow_c * context, struct flow_colorcontext_info * colorcontext,
                            flow_working_floatspace space, float a, float b, float c)
{
    colorcontext->floatspace = space;

    colorcontext->apply_srgb = (space & flow_working_floatspace_linear) > 0;
    colorcontext->apply_gamma = (space & flow_working_floatspace_gamma) > 0;

#ifdef EXPOSE_SIGMOID
    colorcontext->apply_sigmoid = (space & Floatspace_sigmoid) > 0;
    if ((space & Floatspace_sigmoid_3) > 0) {
        flow_colorcontext_sigmoid_internal(colorcontext, -2, a, derive_constant(a + b * -2, c, 1));
    } else if ((space & Floatspace_sigmoid_2) > 0) {
        flow_colorcontext_sigmoid_internal(colorcontext, -b, (1 + c) * b, -1 * (b + a));
    } else if ((space & Floatspace_sigmoid) > 0) {
        flow_colorcontext_sigmoid_internal(colorcontext, a, b, c);
    }
#endif
    if (colorcontext->apply_gamma) {
        colorcontext->gamma = a;
        colorcontext->gamma_inverse = (float)(1.0 / ((double)a));
    }

    for (uint32_t n = 0; n < 256; n++) {
        colorcontext->byte_to_float[n] = flow_colorcontext_srgb_to_floatspace_uncached(colorcontext, n);
    }
}
