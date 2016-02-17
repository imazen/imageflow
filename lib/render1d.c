#include "fastscaling_private.h"
#include <stdio.h>
#include <string.h>
#include "../imageflow.h"



static bool ScaleAndRender1D(Context * context,
                             BitmapBgra * pSrc,
                             BitmapBgra * pDst,
                             const InterpolationDetails * details,
                             bool transpose)
{
    LineContributions * contrib = NULL;
    BitmapFloat * source_buf = NULL;
    BitmapFloat * dest_buf = NULL;

    uint32_t from_count = pSrc->w;
    uint32_t to_count = transpose ? pDst->h : pDst->w;

    bool success = true;

    if (details->window == 0) {
        CONTEXT_error(context, Invalid_argument);
        return false;
    }


    //How many rows to buffer and process at a time.
    const uint32_t buffer_row_count = 4; //using buffer=5 seems about 6% better than most other non-zero values.

    //How many bytes per pixel are we scaling?
    BitmapPixelFormat scaling_format = (pSrc->fmt == Bgra32 && !pSrc->alpha_meaningful) ? Bgr24 : pSrc->fmt;

    prof_start(context,"contributions_calc", false);

    contrib = LineContributions_create(context, to_count, from_count, details);
    if (contrib == NULL) {
        CONTEXT_add_to_callstack (context);
        success = false;
        goto cleanup;
    }
    prof_stop(context,"contributions_calc", true, false);


    prof_start(context,"create_bitmap_float (buffers)", false);

    source_buf = BitmapFloat_create(context, from_count, buffer_row_count, scaling_format, false);
    if (source_buf == NULL) {
        CONTEXT_add_to_callstack (context);
        success = false;
        goto cleanup;
    }
    dest_buf = BitmapFloat_create(context, to_count, buffer_row_count, scaling_format, false);
    if (dest_buf == NULL) {
        CONTEXT_add_to_callstack (context);
        success = false;
        goto cleanup;
    }
    source_buf->alpha_meaningful = pSrc->alpha_meaningful;
    dest_buf->alpha_meaningful = source_buf->alpha_meaningful;

    source_buf->alpha_premultiplied = source_buf->channels == 4;
    dest_buf->alpha_premultiplied = source_buf->alpha_premultiplied;

    prof_stop(context,"create_bitmap_float (buffers)", true, false);


    /* Scale each set of lines */
    for (uint32_t source_start_row = 0; source_start_row < pSrc->h; source_start_row += buffer_row_count) {
        const uint32_t row_count = umin(pSrc->h - source_start_row, buffer_row_count);

        prof_start(context,"convert_srgb_to_linear", false);
        if (!BitmapBgra_convert_srgb_to_linear(context,pSrc, source_start_row, source_buf, 0, row_count)) {
            CONTEXT_add_to_callstack (context);
            success=false;
            goto cleanup;
        }
        prof_stop(context,"convert_srgb_to_linear", true, false);

        prof_start(context,"ScaleBgraFloatRows", false);
        if (!BitmapFloat_scale_rows(context, source_buf, 0, dest_buf, 0, row_count, contrib->ContribRow)) {
            CONTEXT_add_to_callstack (context);
            success=false;
            goto cleanup;
        }
        prof_stop(context,"ScaleBgraFloatRows", true, false);


//        if (!ApplyConvolutionsFloat1D(context, r, dest_buf, 0, row_count, contrib->percent_negative)) {
//            CONTEXT_add_to_callstack (context);
//            success=false;
//            goto cleanup;
//        }
//        if (details->apply_color_matrix && call_number == 2) {
//            if (!ApplyColorMatrix(context, r, dest_buf, row_count)) {
//                CONTEXT_add_to_callstack (context);
//                success=false;
//                goto cleanup;
//            }
//        }

        prof_start(context,"pivoting_composite_linear_over_srgb", false);
        if (!BitmapFloat_pivoting_composite_linear_over_srgb(context, dest_buf, 0, pDst, source_start_row, row_count, transpose)) {
            CONTEXT_add_to_callstack (context);
            success=false;
            goto cleanup;
        }
        prof_stop(context,"pivoting_composite_linear_over_srgb", true, false);

    }
    //sRGB sharpening
    //Color matrix


    cleanup:
    //p->Start("Free Contributions,FloatBuffers", false);

    if (contrib != NULL) LineContributions_destroy(context, contrib);

    if (source_buf != NULL) BitmapFloat_destroy(context, source_buf);
    if (dest_buf != NULL) BitmapFloat_destroy(context, dest_buf);
    ///p->Stop("Free Contributions,FloatBuffers", true, false);

    return success;
}



bool flow_node_execute_render_to_canvas_1d(Context *c, struct flow_job * job, BitmapBgra * input, BitmapBgra * canvas, struct flow_nodeinfo_render_to_canvas_1d * info){

    if (info->canvas_x != 0 || info->canvas_y != 0 || info->scale_to_width != (int32_t)(info->transpose_on_write ? canvas->h : canvas->w)){
        CONTEXT_error(c, Not_implemented); //Requires cropping the target canvas
        return false;
    }
    if (info->filter_list != NULL || info->sharpen_percent_goal != 0){
        CONTEXT_error(c, Not_implemented); //Requires cropping the target canvas
        return false;
    }
    InterpolationDetails * d= InterpolationDetails_create_from(c, info->interpolation_filter);
    if (d == NULL){
        CONTEXT_error_return(c);
    }

    if (!ScaleAndRender1D(c,input, canvas,d, info->transpose_on_write)){
        InterpolationDetails_destroy(c,d);
        CONTEXT_error_return(c);
    }

    InterpolationDetails_destroy(c,d);
    return true;

}

