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


#include "fastscaling_private.h"

#include <stdlib.h>

static void derive_cubic_coefficients(double B, double C, InterpolationDetails * out)
{
    double bx2 = B + B;
    out->p1 = 1.0 - (1.0 / 3.0)*B;
    out->p2 = -3.0 + bx2 + C;
    out->p3 = 2.0 - 1.5*B - C;
    out->q1 = (4.0 / 3.0)*B + 4.0*C;
    out->q2 = -8.0*C - bx2;
    out->q3 = B + 5.0*C;
    out->q4 = (-1.0 / 6.0)*B - C;
}


static double filter_flex_cubic(const InterpolationDetails * d, double x)
{
    const double t = (double)fabs(x) / d->blur;

    if (t < 1.0) {
        return (d->p1 + t * (t* (d->p2 + t*d->p3)));
    }
    if (t < 2.0) {
        return(d->q1 + t*(d->q2 + t* (d->q3 + t*d->q4)));
    }
    return(0.0);
}
static double filter_bicubic_fast(const InterpolationDetails * d, double t)
{
    double abs_t = (double)fabs(t) / d->blur;
    double abs_t_sq = abs_t * abs_t;
    if (abs_t<1) return 1 - 2 * abs_t_sq + abs_t_sq*abs_t;
    if (abs_t<2) return  (4 - 8 * abs_t + 5 * abs_t_sq - abs_t_sq*abs_t);
    return 0;
}


static double filter_sinc(const InterpolationDetails * d, double t)
{
    const double abs_t = (double)fabs(t) / d->blur;
    if (abs_t == 0) {
        return 1;    //Avoid division by zero
    }
    if (abs_t > d->window) {
        return 0;
    }
    const double a = abs_t * IR_PI;
    return sin(a) / a;
}

static double filter_box(const InterpolationDetails * d, double t)
{

    const double x = t / d->blur;
    return (x >= -1 * d->window && x < d->window) ? 1 : 0;
}

static double filter_triangle(const InterpolationDetails * d, double t)
{
    const double x = (double)fabs(t) / d->blur;
    if (x < 1.0)
        return(1.0 - x);
    return(0.0);
}


static double filter_sinc_windowed(const InterpolationDetails * d, double t)
{
    const double x = t / d->blur;
    const double abs_t = (double)fabs(x);
    if (abs_t == 0) {
        return 1;    //Avoid division by zero
    }
    if (abs_t > d->window) {
        return 0;
    }
    return d->window * sin(IR_PI * x / d->window) * sin(x * IR_PI) / (IR_PI * IR_PI * x * x);
}




static double filter_jinc (const InterpolationDetails * d, double t) {
    const double x = fabs (t) / d->blur;
    if (x == 0.0)
        return(0.5*IR_PI);
    return(BESSEL_01 (IR_PI*x) / x);
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

static double filter_window_jinc (const InterpolationDetails * d, double t) {
    return window_jinc (t / (d->blur * d->window));
}
*/

static double filter_ginseng (const InterpolationDetails * d, double t)
{
    //Sinc windowed by jinc
    const double abs_t = (double)fabs (t) / d->blur;
    const double t_pi = abs_t * IR_PI;

    if (abs_t == 0) {
        return 1;    //Avoid division by zero
    }
    if (abs_t > 3) {
        return 0;
    }
    const double jinc_input = 1.2196698912665045 * t_pi / d->window;
    const double jinc_output = BESSEL_01 (jinc_input) / (jinc_input * 0.5);

    return jinc_output * sin (t_pi) / (t_pi);

}


#define TONY 0.00001

double InterpolationDetails_percent_negative_weight(const InterpolationDetails* details)
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
        if (area > 0) positive_area += area;
        else negative_area -= area;
    }
    return negative_area / positive_area;
}

InterpolationDetails* InterpolationDetails_create(Context * context)
{
    InterpolationDetails* d = CONTEXT_calloc_array(context, 1, InterpolationDetails);
    if (d == NULL) {
        CONTEXT_error(context, Out_of_memory);
        return NULL;
    }
    d->blur = 1;
    d->window = 2;
    d->p1 = d->q1 = 0;
    d->p2 = d->q2 = d->p3 = d->q3 = d->q4 = 1;
    d->sharpen_percent_goal = 0;
    return d;
}


InterpolationDetails * InterpolationDetails_create_bicubic_custom(Context * context, double window, double blur, double B, double C)
{
    InterpolationDetails * d = InterpolationDetails_create(context);
    if (d != NULL) {
        d->blur = blur;
        derive_cubic_coefficients(B, C, d);
        d->filter = filter_flex_cubic;
        d->window = window;
    }
    else{
        CONTEXT_add_to_callstack (context);
    }
    return d;
}
InterpolationDetails * InterpolationDetails_create_custom(Context * context, double window, double blur, detailed_interpolation_method filter)
{
    InterpolationDetails * d = InterpolationDetails_create(context);
    if (d != NULL) {
        d->blur = blur;
        d->filter = filter;
        d->window = window;
    }
    else{
        CONTEXT_add_to_callstack (context);
    }
    return d;
}

void InterpolationDetails_destroy(Context * context, InterpolationDetails * details)
{
    CONTEXT_free(context, details);
}

static InterpolationDetails * InterpolationDetails_create_from_internal(Context * context, InterpolationFilter filter, bool checkExistenceOnly)
{
    bool ex = checkExistenceOnly;
    InterpolationDetails * truePtr = (InterpolationDetails *)-1;
    switch (filter) {
    case Filter_Linear:
    case Filter_Triangle:
        return ex ? truePtr : InterpolationDetails_create_custom(context, 1, 1, filter_triangle);

    case Filter_RawLanczos2:
        return ex ? truePtr : InterpolationDetails_create_custom(context, 2, 1, filter_sinc);
    case Filter_RawLanczos3:
        return ex ? truePtr : InterpolationDetails_create_custom(context, 3, 1, filter_sinc);
    case Filter_RawLanczos2Sharp:
        return ex ? truePtr : InterpolationDetails_create_custom(context, 2, 0.9549963639785485, filter_sinc);
    case Filter_RawLanczos3Sharp:
        return ex ? truePtr : InterpolationDetails_create_custom(context, 3, 0.9812505644269356, filter_sinc);

    //Hermite and BSpline no negative weights
    case Filter_CubicBSpline:
        return ex ? truePtr : InterpolationDetails_create_bicubic_custom(context, 2, 1, 1, 0);

    case Filter_Lanczos2:
        return ex ? truePtr : InterpolationDetails_create_custom(context, 2, 1, filter_sinc_windowed);
    case Filter_Lanczos:
        return ex ? truePtr : InterpolationDetails_create_custom(context, 3, 1, filter_sinc_windowed);
    case Filter_Lanczos2Sharp:
        return ex ? truePtr :  InterpolationDetails_create_custom(context, 2, 0.9549963639785485, filter_sinc_windowed);
    case Filter_LanczosSharp:
        return ex ? truePtr : InterpolationDetails_create_custom(context, 3, 0.9812505644269356, filter_sinc_windowed);


    case Filter_CubicFast:
        return ex ? truePtr : InterpolationDetails_create_custom(context, 1, 1, filter_bicubic_fast);
    case Filter_Cubic:
        return ex ? truePtr :  InterpolationDetails_create_bicubic_custom(context, 2, 1, 0,1);
    case Filter_CubicSharp:
        return ex ? truePtr :  InterpolationDetails_create_bicubic_custom (context, 2, 0.9549963639785485, 0, 1);
    case Filter_CatmullRom:
        return ex ? truePtr : InterpolationDetails_create_bicubic_custom(context, 2, 1, 0, 0.5);
    case Filter_CatmullRomFast:
        return ex ? truePtr :  InterpolationDetails_create_bicubic_custom(context, 1, 1, 0, 0.5);
    case Filter_CatmullRomFastSharp:
        return ex ? truePtr :  InterpolationDetails_create_bicubic_custom(context, 1, 13.0 / 16.0, 0, 0.5);
    case Filter_Mitchell:
        return ex ? truePtr :  InterpolationDetails_create_bicubic_custom(context, 2, 7.0 / 8.0, 1.0 / 3.0, 1.0 / 3.0);
    case Filter_MitchellFast:
        return ex ? truePtr :  InterpolationDetails_create_bicubic_custom (context, 1, 7.0 / 8.0, 1.0 / 3.0, 1.0 / 3.0);


    case Filter_Robidoux:
        return ex ? truePtr :  InterpolationDetails_create_bicubic_custom(context, 2, 1. / 1.1685777620836932,
                0.37821575509399867, 0.31089212245300067);
    case Filter_Fastest:
        return ex ? truePtr : InterpolationDetails_create_bicubic_custom (context,0.74, 0.74,
            0.37821575509399867, 0.31089212245300067);


    case Filter_RobidouxFast:
        return ex ? truePtr :  InterpolationDetails_create_bicubic_custom (context, 1.05, 1. / 1.1685777620836932,
            0.37821575509399867, 0.31089212245300067);
    case Filter_RobidouxSharp:
        return ex ? truePtr :  InterpolationDetails_create_bicubic_custom(context, 2, 1. / 1.105822933719019,
                0.2620145123990142, 0.3689927438004929);
    case Filter_Hermite:
        return ex ? truePtr :  InterpolationDetails_create_bicubic_custom(context, 1, 1, 0, 0);
    case Filter_Box:
        return ex ? truePtr :  InterpolationDetails_create_custom(context, 0.5, 1, filter_box);

    case Filter_Ginseng:
        return ex ? truePtr :  InterpolationDetails_create_custom (context, 3, 1, filter_ginseng);

    case Filter_GinsengSharp:
        return ex ? truePtr : InterpolationDetails_create_custom (context, 3, 0.9812505644269356, filter_ginseng);



    case Filter_Jinc:
        return ex ? truePtr :  InterpolationDetails_create_custom (context, 3, 1.0 / 1.2196698912665045, filter_jinc);

    }
    if (!checkExistenceOnly){
        CONTEXT_error(context, Invalid_interpolation_filter);
    }
    return NULL;
}

InterpolationDetails * InterpolationDetails_create_from(Context * context, InterpolationFilter filter)
{
    return InterpolationDetails_create_from_internal(context, filter, false);
}

bool InterpolationDetails_interpolation_filter_exists(InterpolationFilter filter){
    return (InterpolationDetails_create_from_internal(NULL, filter, true) != NULL);
}

static LineContributions * LineContributions_alloc(Context * context, const uint32_t line_length, const uint32_t windows_size)
{
    LineContributions *res = (LineContributions *)CONTEXT_malloc(context, sizeof(LineContributions));
    if (res == NULL) {
        CONTEXT_error(context, Out_of_memory);
        return NULL;
    }
    res->WindowSize = windows_size;
    res->LineLength = line_length;
    res->ContribRow = (PixelContributions *)CONTEXT_malloc(context, line_length * sizeof(PixelContributions));
    if (!res->ContribRow) {
        CONTEXT_free(context, res);
        CONTEXT_error(context, Out_of_memory);
        return NULL;
    }

    float *allWeights = CONTEXT_calloc_array(context, windows_size * line_length, float);
    if (!allWeights) {
        CONTEXT_free(context, res->ContribRow);
        CONTEXT_free(context, res);
        CONTEXT_error(context, Out_of_memory);
        return NULL;
    }

    for (uint32_t i = 0; i < line_length; i++)
        res->ContribRow[i].Weights = allWeights + (i * windows_size);

    return res;
}

void LineContributions_destroy(Context * context, LineContributions * p)
{

    if (p != NULL) {
        if (p->ContribRow != NULL) {
            CONTEXT_free(context, p->ContribRow[0].Weights);
        }
        CONTEXT_free(context, p->ContribRow);

    }
    CONTEXT_free(context, p);
}


LineContributions *LineContributions_create(Context * context,  const uint32_t output_line_size, const uint32_t input_line_size,  const InterpolationDetails* details)
{
    const double sharpen_ratio =  InterpolationDetails_percent_negative_weight(details);
    const double desired_sharpen_ratio = details->sharpen_percent_goal / 100.0;
    const double extra_negative_weight = sharpen_ratio > 0 && desired_sharpen_ratio > 0 ?
                                         (desired_sharpen_ratio + sharpen_ratio) / sharpen_ratio :
                                         0;


    const double scale_factor = (double)output_line_size / (double)input_line_size;
    const double downscale_factor = fmin(1.0, scale_factor);
    const double half_source_window = details->window * 0.5 / downscale_factor;

    const uint32_t allocated_window_size = (int)ceil(2 * (half_source_window - TONY)) + 1;
    uint32_t u, ix;
    LineContributions *res = LineContributions_alloc(context, output_line_size, allocated_window_size);
    if (res == NULL){
        CONTEXT_add_to_callstack (context);
        return NULL;
    }
    double negative_area = 0;
    double positive_area = 0;

    for (u = 0; u < output_line_size; u++) {
        const double center_src_pixel = ((double)u + 0.5) / scale_factor - 0.5;

        const int left_edge = (int)ceil(center_src_pixel - half_source_window - 0.5 + TONY);
        const int right_edge = (int)floor(center_src_pixel + half_source_window + 0.5 - TONY);

        const uint32_t left_src_pixel = (uint32_t)max(0, left_edge);
        const uint32_t right_src_pixel = (uint32_t)min(right_edge, (int)input_line_size - 1);

        double total_weight = 0.0;

        const uint32_t source_pixel_count = right_src_pixel - left_src_pixel + 1;

        if (source_pixel_count > allocated_window_size) {
            LineContributions_destroy(context, res);
            CONTEXT_error(context, Invalid_internal_state);
            return NULL;
        }

        res->ContribRow[u].Left = left_src_pixel;
        res->ContribRow[u].Right = right_src_pixel;


        float *weights = res->ContribRow[u].Weights;

        //commented: additional weight for edges (doesn't seem to be too effective)
        //for (ix = left_edge; ix <= right_edge; ix++) {
        for (ix = left_src_pixel; ix <= right_src_pixel; ix++) {
            int tx = ix - left_src_pixel;
            //int tx = min(max(ix, left_src_pixel), right_src_pixel) - left_src_pixel;
            double add = (*details->filter)(details, downscale_factor * ((double)ix - center_src_pixel));
            if (add < 0 && extra_negative_weight != 0) {
                add *= extra_negative_weight;
            }
            weights[tx] = (float)add;
            total_weight += add;
        }

        if (total_weight <= TONY) {
            LineContributions_destroy(context, res);
            CONTEXT_error (context, Invalid_internal_state);
            return NULL;
        }


        const float total_factor = (float)(1.0f / total_weight);
        for (ix = 0; ix < source_pixel_count; ix++) {
            weights[ix] *= total_factor;
            if (weights[ix] < 0) {
                negative_area -= weights[ix];
            } else {
                positive_area += weights[ix];
            }
        }
    }
    res->percent_negative = negative_area / positive_area;
    return res;
}
