/*
* Copyright (c) Imazen LLC.
* No part of this project, including this file, may be copied, modified,
* propagated, or distributed except as permitted in COPYRIGHT.txt.
* Licensed under the GNU Affero General Public License, Version 3.0.
* Commercial licenses available at http://imageresizing.net/
*/

#ifdef _MSC_VER
#pragma warning(disable : 4996)
#endif

#include "imageflow_private.h"
#include <stdio.h>
#include <string.h>

typedef struct flow_RendererStruct {
    flow_RenderDetails* details;
    bool destroy_details;
    flow_bitmap_bgra* source;
    bool destroy_source;
    flow_bitmap_bgra* canvas;
    flow_bitmap_bgra* transposed;
} flow_Renderer;

flow_Renderer* Renderer_create(flow_context* context, flow_bitmap_bgra* source, flow_bitmap_bgra* canvas,
                               flow_RenderDetails* details);
flow_Renderer* Renderer_create_in_place(flow_context* context, flow_bitmap_bgra* editInPlace,
                                        flow_RenderDetails* details);
bool Renderer_perform_render(flow_context* context, flow_Renderer* r);
void Renderer_destroy(flow_context* context, flow_Renderer* r);

flow_RenderDetails* flow_RenderDetails_create(flow_context* context)
{
    flow_RenderDetails* d = FLOW_calloc_array(context, 1, flow_RenderDetails);
    if (d == NULL) {
        FLOW_error(context, flow_status_Out_of_memory);
        return NULL;
    }
    for (int i = 0; i < 5; i++) {
        d->color_matrix[i] = &(d->color_matrix_data[i * 5]);
    }
    d->enable_profiling = false;
    d->halving_divisor = 0;
    d->interpolate_last_percent = 3;
    d->havling_acceptable_pixel_loss = 0;
    d->minimum_sample_window_to_interposharpen = 1.5;
    d->apply_color_matrix = false;
    return d;
}

flow_RenderDetails* flow_RenderDetails_create_with(flow_context* context, flow_interpolation_filter filter)
{
    flow_interpolation_details* id = flow_interpolation_details_create_from(context, filter);
    if (id == NULL) {
        FLOW_add_to_callstack(context);
        return NULL;
    }
    flow_RenderDetails* d = flow_RenderDetails_create(context);
    if (d == NULL) {
        FLOW_add_to_callstack(context);
        flow_interpolation_details_destroy(context, id);
    } else {
        d->interpolation = id;
    }
    return d;
}

void flow_RenderDetails_destroy(flow_context* context, flow_RenderDetails* d)
{
    if (d != NULL) {
        flow_interpolation_details_destroy(context, d->interpolation);
        flow_convolution_kernel_destroy(context, d->kernel_a);
        flow_convolution_kernel_destroy(context, d->kernel_b);
    }
    FLOW_free(context, d);
}

bool flow_RenderDetails_render(flow_context* context, flow_RenderDetails* details, flow_bitmap_bgra* source,
                               flow_bitmap_bgra* canvas)
{

    bool destroy_source = false;

    flow_Renderer* r = Renderer_create(context, source, canvas, details);
    if (r == NULL) {
        FLOW_add_to_callstack(context);
        if (destroy_source) {
            flow_bitmap_bgra_destroy(context, source);
        }
        return false;
    }
    r->destroy_details = false;
    r->destroy_source = destroy_source;
    bool result = Renderer_perform_render(context, r);
    if (!result) {
        FLOW_add_to_callstack(context);
    }
    Renderer_destroy(context, r);
    return result;
}

bool flow_RenderDetails_render_in_place(flow_context* context, flow_RenderDetails* details,
                                        flow_bitmap_bgra* edit_in_place)
{

    flow_Renderer* r = Renderer_create_in_place(context, edit_in_place, details);
    if (r == NULL) {
        FLOW_add_to_callstack(context);
        return false;
    }
    r->destroy_details = false;
    r->destroy_source = false;
    bool result = Renderer_perform_render(context, r);
    if (!result) {
        FLOW_add_to_callstack(context);
    }
    Renderer_destroy(context, r);
    return result;
}

static float Renderer_percent_loss(int from_width, int to_width, int from_height, int to_height, int divisor)
{
    int lost_columns = from_width % divisor;
    int lost_rows = from_height % divisor;
    float scale_factor_x = (float)to_width / (float)from_width;
    float scale_factor_y = (float)to_width / (float)from_width;
    return (float)fmax(lost_rows * scale_factor_y, lost_columns * scale_factor_x);
}

static int Renderer_determine_divisor(flow_Renderer* r)
{
    if (r->canvas == NULL)
        return 0;

    int width = r->details->post_transpose ? r->canvas->h : r->canvas->w;
    int height = r->details->post_transpose ? r->canvas->w : r->canvas->h;

    double divisor_max = fmin((double)r->source->w / (double)width, (double)r->source->h / (double)height);

    divisor_max = divisor_max / r->details->interpolate_last_percent;

    int divisor = (int)floor(divisor_max);
    while (divisor > 0
           && Renderer_percent_loss(r->source->w, width, r->source->h, height, divisor)
              > r->details->havling_acceptable_pixel_loss) {
        divisor--;
    }
    return max(1, divisor);
}

void Renderer_destroy(flow_context* context, flow_Renderer* r)
{
    if (r == NULL)
        return;
    if (r->destroy_source) {
        flow_bitmap_bgra_destroy(context, r->source);
    }
    r->source = NULL;
    flow_bitmap_bgra_destroy(context, r->transposed);
    r->transposed = NULL;
    r->canvas = NULL;
    if (r->destroy_details) {
        flow_RenderDetails_destroy(context, r->details);
        r->details = NULL;
    }

    FLOW_free(context, r);
}

flow_Renderer* Renderer_create_in_place(flow_context* context, flow_bitmap_bgra* editInPlace,
                                        flow_RenderDetails* details)
{
    if (details->post_transpose) {
        FLOW_error(context, flow_status_Invalid_argument);
        return NULL;
    }
    flow_Renderer* r = FLOW_calloc_array(context, 1, flow_Renderer);
    if (r == NULL) {
        FLOW_error(context, flow_status_Out_of_memory);
        return NULL;
    }
    if (details->enable_profiling) {
        uint32_t default_capacity = (r->source->h + r->source->w) * 20 + 5;
        if (!flow_context_enable_profiling(context, default_capacity)) {
            FLOW_add_to_callstack(context);
            FLOW_free(context, r);
            return NULL;
        }
    }
    r->source = editInPlace;
    r->destroy_source = false;
    r->details = details;
    return r;
}

flow_Renderer* Renderer_create(flow_context* context, flow_bitmap_bgra* source, flow_bitmap_bgra* canvas,
                               flow_RenderDetails* details)
{
    flow_Renderer* r = FLOW_calloc_array(context, 1, flow_Renderer);
    if (r == NULL) {
        FLOW_error(context, flow_status_Out_of_memory);
        return NULL;
    }
    r->source = source;
    r->canvas = canvas;
    r->destroy_source = false;
    r->details = details;
    if (details->enable_profiling) {
        uint32_t default_capacity = (r->source->w + r->source->h + r->canvas->w + r->canvas->h) * 20 + 50;
        if (!flow_context_enable_profiling(context, default_capacity)) {
            FLOW_add_to_callstack(context);
            FLOW_free(context, r);
            return NULL;
        }
    }
    if (r->details->halving_divisor == 0) {
        r->details->halving_divisor = Renderer_determine_divisor(r);
    }
    return r;
}

/*
static void SimpleRenderInPlace(void)
{
    //against source:

    //fliph
    //flipv

    //color matrix (srgb)

}
*/

// TODO: find better name
static bool HalveInTempImage(flow_context* context, flow_Renderer* r, int divisor)
{
    bool result = true;
    flow_prof_start(context, "create temp image for halving", false);
    int halved_width = (int)(r->source->w / divisor);
    int halved_height = (int)(r->source->h / divisor);
    flow_bitmap_bgra* tmp_im = flow_bitmap_bgra_create(context, halved_width, halved_height, true, r->source->fmt);
    if (tmp_im == NULL) {
        FLOW_add_to_callstack(context);
        return false;
    }
    // from here we have a temp image
    flow_prof_stop(context, "create temp image for halving", true, false);

    if (!flow_halve(context, r->source, tmp_im, divisor)) {
        // we cannot return here, or tmp_im will leak
        FLOW_add_to_callstack(context);
        result = false;
    }
    tmp_im->alpha_meaningful = r->source->alpha_meaningful;

    if (r->destroy_source) {
        flow_bitmap_bgra_destroy(context, r->source);
    }
    r->source = tmp_im;
    r->destroy_source = true; // Cleanup tmp_im
    return result;
}

static bool Renderer_complete_halving(flow_context* context, flow_Renderer* r)
{
    int divisor = r->details->halving_divisor;
    if (divisor <= 1) {
        return true;
    }
    bool result = true;
    flow_prof_start(context, "CompleteHalving", false);
    r->details->halving_divisor = 0; // Don't halve twice

    result = r->source->can_reuse_space ? flow_halve_in_place(context, r->source, divisor)
                                        : HalveInTempImage(context, r, divisor);
    if (!result) {
        FLOW_add_to_callstack(context);
    }

    flow_prof_stop(context, "CompleteHalving", true, false);
    return result;
}

static bool ApplyConvolutionsFloat1D(flow_context* context, const flow_Renderer* r, flow_bitmap_float* img,
                                     const uint32_t from_row, const uint32_t row_count, double sharpening_applied)
{
    if (r->details->kernel_a != NULL) {
        flow_prof_start(context, "convolve kernel a", false);
        if (!flow_bitmap_float_convolve_rows(context, img, r->details->kernel_a, img->channels, from_row, row_count)) {
            FLOW_add_to_callstack(context);
            return false;
        }
        flow_prof_stop(context, "convolve kernel a", true, false);
    }
    if (r->details->kernel_b != NULL) {
        flow_prof_start(context, "convolve kernel b", false);
        if (!flow_bitmap_float_convolve_rows(context, img, r->details->kernel_b, img->channels, from_row, row_count)) {
            FLOW_add_to_callstack(context);
            return false;
        }
        flow_prof_stop(context, "convolve kernel b", true, false);
    }
    if (r->details->sharpen_percent_goal > sharpening_applied + 0.01) {
        flow_prof_start(context, "SharpenBgraFloatRowsInPlace", false);
        if (!flow_bitmap_float_sharpen_rows(context, img, from_row, row_count,
                                            r->details->sharpen_percent_goal - sharpening_applied)) {
            FLOW_add_to_callstack(context);
            return false;
        }
        flow_prof_stop(context, "SharpenBgraFloatRowsInPlace", true, false);
    }
    return true;
}

static bool ApplyColorMatrix(flow_context* context, const flow_Renderer* r, flow_bitmap_float* img,
                             const uint32_t row_count)
{
    flow_prof_start(context, "apply_color_matrix_float", false);
    bool b = flow_bitmap_float_apply_color_matrix(context, img, 0, row_count, r->details->color_matrix);
    flow_prof_stop(context, "apply_color_matrix_float", true, false);
    return b;
}

static bool ScaleAndRender1D(flow_context* context, const flow_Renderer* r, flow_bitmap_bgra* pSrc,
                             flow_bitmap_bgra* pDst, const flow_RenderDetails* details, bool transpose, int call_number)
{
    flow_interpolation_line_contributions* contrib = NULL;
    flow_bitmap_float* source_buf = NULL;
    flow_bitmap_float* dest_buf = NULL;

    uint32_t from_count = pSrc->w;
    uint32_t to_count = transpose ? pDst->h : pDst->w;

    bool success = true;

    if (details->interpolation->window == 0) {
        FLOW_error(context, flow_status_Invalid_argument);
        return false;
    }

    // How many rows to buffer and process at a time.
    const uint32_t buffer_row_count = 4; // using buffer=5 seems about 6% better than most other non-zero values.

    // How many bytes per pixel are we scaling?
    flow_pixel_format scaling_format = (pSrc->fmt == flow_bgra32 && !pSrc->alpha_meaningful) ? flow_bgr24 : pSrc->fmt;

    flow_prof_start(context, "contributions_calc", false);

    contrib = flow_interpolation_line_contributions_create(context, to_count, from_count, details->interpolation);
    if (contrib == NULL) {
        FLOW_add_to_callstack(context);
        success = false;
        goto cleanup;
    }
    flow_prof_stop(context, "contributions_calc", true, false);

    flow_prof_start(context, "create_bitmap_float (buffers)", false);

    source_buf = flow_bitmap_float_create(context, from_count, buffer_row_count, scaling_format, false);
    if (source_buf == NULL) {
        FLOW_add_to_callstack(context);
        success = false;
        goto cleanup;
    }
    dest_buf = flow_bitmap_float_create(context, to_count, buffer_row_count, scaling_format, false);
    if (dest_buf == NULL) {
        FLOW_add_to_callstack(context);
        success = false;
        goto cleanup;
    }
    source_buf->alpha_meaningful = pSrc->alpha_meaningful;
    dest_buf->alpha_meaningful = source_buf->alpha_meaningful;

    source_buf->alpha_premultiplied = source_buf->channels == 4;
    dest_buf->alpha_premultiplied = source_buf->alpha_premultiplied;

    flow_prof_stop(context, "create_bitmap_float (buffers)", true, false);

    /* Scale each set of lines */
    for (uint32_t source_start_row = 0; source_start_row < pSrc->h; source_start_row += buffer_row_count) {
        const uint32_t row_count = umin(pSrc->h - source_start_row, buffer_row_count);

        flow_prof_start(context, "convert_srgb_to_linear", false);
        if (!flow_bitmap_float_convert_srgb_to_linear(context, pSrc, source_start_row, source_buf, 0, row_count)) {
            FLOW_add_to_callstack(context);
            success = false;
            goto cleanup;
        }
        flow_prof_stop(context, "convert_srgb_to_linear", true, false);

        flow_prof_start(context, "ScaleBgraFloatRows", false);
        if (!flow_bitmap_float_scale_rows(context, source_buf, 0, dest_buf, 0, row_count, contrib->ContribRow)) {
            FLOW_add_to_callstack(context);
            success = false;
            goto cleanup;
        }
        flow_prof_stop(context, "ScaleBgraFloatRows", true, false);

        if (!ApplyConvolutionsFloat1D(context, r, dest_buf, 0, row_count, contrib->percent_negative)) {
            FLOW_add_to_callstack(context);
            success = false;
            goto cleanup;
        }
        if (details->apply_color_matrix && call_number == 2) {
            if (!ApplyColorMatrix(context, r, dest_buf, row_count)) {
                FLOW_add_to_callstack(context);
                success = false;
                goto cleanup;
            }
        }

        flow_prof_start(context, "pivoting_composite_linear_over_srgb", false);
        if (!flow_bitmap_float_pivoting_composite_linear_over_srgb(context, dest_buf, 0, pDst, source_start_row,
                                                                   row_count, transpose)) {
            FLOW_add_to_callstack(context);
            success = false;
            goto cleanup;
        }
        flow_prof_stop(context, "pivoting_composite_linear_over_srgb", true, false);
    }
// sRGB sharpening
// Color matrix

cleanup:
    // p->Start("Free Contributions,FloatBuffers", false);

    if (contrib != NULL)
        flow_interpolation_line_contributions_destroy(context, contrib);

    if (source_buf != NULL)
        flow_bitmap_float_destroy(context, source_buf);
    if (dest_buf != NULL)
        flow_bitmap_float_destroy(context, dest_buf);
    /// p->Stop("Free Contributions,FloatBuffers", true, false);

    return success;
}

static bool Render1D(flow_context* context, const flow_Renderer* r, flow_bitmap_bgra* pSrc, flow_bitmap_bgra* pDst,
                     const flow_RenderDetails* details, bool transpose, int call_number)
{

    bool success = true;
    // How many rows to buffer and process at a time.
    uint32_t buffer_row_count = 4; // using buffer=5 seems about 6% better than most other non-zero values.

    // How many bytes per pixel are we scaling?
    flow_pixel_format scaling_format = (pSrc->fmt == flow_bgra32 && !pSrc->alpha_meaningful) ? flow_bgr24 : pSrc->fmt;

    flow_bitmap_float* buf = flow_bitmap_float_create(context, pSrc->w, buffer_row_count, scaling_format, false);
    if (buf == NULL) {
        return false;
    }
    buf->alpha_meaningful = pSrc->alpha_meaningful;
    buf->alpha_premultiplied = buf->channels == 4;

    /* Scale each set of lines */
    for (uint32_t source_start_row = 0; source_start_row < pSrc->h; source_start_row += buffer_row_count) {
        const uint32_t row_count = umin(pSrc->h - source_start_row, buffer_row_count);

        if (!flow_bitmap_float_convert_srgb_to_linear(context, pSrc, source_start_row, buf, 0, row_count)) {
            FLOW_add_to_callstack(context);
            success = false;
            goto cleanup;
        }
        if (!ApplyConvolutionsFloat1D(context, r, buf, 0, row_count, 0)) {
            FLOW_add_to_callstack(context);
            success = false;
            goto cleanup;
        }
        if (details->apply_color_matrix && call_number == 2) {
            if (!ApplyColorMatrix(context, r, buf, row_count)) {
                FLOW_add_to_callstack(context);
                success = false;
                goto cleanup;
            }
        }

        if (!flow_bitmap_float_pivoting_composite_linear_over_srgb(context, buf, 0, pDst, source_start_row, row_count,
                                                                   transpose)) {
            FLOW_add_to_callstack(context);
            success = false;
            goto cleanup;
        }
    }
// sRGB sharpening
// Color matrix

cleanup:
    flow_bitmap_float_destroy(context, buf);
    return success;
}

static bool RenderWrapper1D(flow_context* context, const flow_Renderer* r, flow_bitmap_bgra* pSrc,
                            flow_bitmap_bgra* pDst, const flow_RenderDetails* details, bool transpose, int call_number)
{
    bool perfect_size = transpose ? (pSrc->h == pDst->w && pDst->h == pSrc->w)
                                  : (pSrc->w == pDst->w && pSrc->h == pDst->h);
    // String^ name = String::Format("{0}Render1D (call {1})", perfect_size ? "" : "ScaleAnd", call_number);

    // try{
    // p->Start(name, false);
    if (perfect_size) {
        return Render1D(context, r, pSrc, pDst, details, transpose, call_number);
    } else {
        return ScaleAndRender1D(context, r, pSrc, pDst, details, transpose, call_number);
    }
    // }
    // finally{
    // p->Stop(name, true, true);
    //}
}
bool Renderer_perform_render(flow_context* context, flow_Renderer* r)
{
    flow_prof_start(context, "perform_render", false);
    if (!Renderer_complete_halving(context, r)) {
        FLOW_add_to_callstack(context);
        return false;
    }
    bool skip_last_transpose = r->details->post_transpose;

    // We can optimize certain code paths - later, if needed

    bool scaling_required
        = (r->canvas != NULL)
          && (r->details->post_transpose ? (r->canvas->w != r->source->h || r->canvas->h != r->source->w)
                                         : (r->canvas->h != r->source->h || r->canvas->w != r->source->w));

    if (scaling_required && r->details->interpolation == NULL) {
        FLOW_error(context, flow_status_Invalid_argument);
        return false;
    }

    /*
    bool someTranspositionRequired = r->details->sharpen_percent_goal > 0 ||
        skip_last_transpose ||
        r->details->kernel_a->radius > 0 ||
        r->details->kernel_b->radius > 0 ||
        scaling_required;

    if (!someTranspositionRequired && r->canvas == NULL){
        SimpleRenderInPlace();
        flow_prof_stop (context, "perform_render", true, false);
        return; //Nothing left to do here.
    }
    */

    bool vflip_source = (r->details->post_flip_y && !skip_last_transpose)
                        || (skip_last_transpose && r->details->post_flip_x);
    bool vflip_transposed
        = ((r->details->post_flip_x && !skip_last_transpose) || (skip_last_transpose && r->details->post_flip_y));

    // vertical flip before transposition is the same as a horizontal flip afterwards. Dealing with more pixels, though.
    if (vflip_source && !flow_bitmap_float_flip_vertical(context, r->source)) {
        FLOW_add_to_callstack(context);
        return false;
    }

    // Create transposition byffer
    // p->Start("allocate temp image(sy x dx)", false);

    /* Scale horizontally  */
    r->transposed = flow_bitmap_bgra_create(
        context, r->source->h, r->canvas == NULL ? r->source->w : (skip_last_transpose ? r->canvas->h : r->canvas->w),
        false, r->source->fmt);

    if (r->transposed == NULL) {
        FLOW_add_to_callstack(context);
        return false;
    }
    r->transposed->compositing_mode = flow_bitmap_compositing_replace_self;
    // p->Stop("allocate temp image(sy x dx)", true, false);

    // Don't composite if we're working in-place
    if (r->canvas == NULL) {
        r->source->compositing_mode = flow_bitmap_compositing_replace_self;
    }
    // Unsharpen when interpolating if we can
    if (r->details->interpolation != NULL && r->details->sharpen_percent_goal > 0
        && r->details->minimum_sample_window_to_interposharpen <= r->details->interpolation->window) {

        r->details->interpolation->sharpen_percent_goal = r->details->sharpen_percent_goal;
    }

    // Apply kernels, scale, and transpose
    if (!RenderWrapper1D(context, r, r->source, r->transposed, r->details, true, 1)) {
        FLOW_add_to_callstack(context);
        return false;
    }

    // Apply flip to transposed
    if (vflip_transposed && !flow_bitmap_float_flip_vertical(context, r->transposed)) {
        FLOW_add_to_callstack(context);
        return false;
    }
    // Restore the source bitmap if we flipped it in place incorrectly
    if (vflip_source && r->source->pixels_readonly && !flow_bitmap_float_flip_vertical(context, r->source)) {
        FLOW_add_to_callstack(context);
        return false;
    }

    flow_bitmap_bgra* finalDest = r->canvas == NULL ? r->source : r->canvas;

    // Apply kernels, color matrix, scale,  (transpose?) and (compose?)

    if (!RenderWrapper1D(context, r, r->transposed, finalDest, r->details, !skip_last_transpose, 2)) {
        FLOW_add_to_callstack(context);
        return false;
    }

    flow_prof_stop(context, "perform_render", true, false);
    // p->Stop("Render", true, false);
    // GC::KeepAlive(wbSource);
    // GC::KeepAlive(wbCanvas);
    return true; // is this correct?
}
