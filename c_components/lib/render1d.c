#include "imageflow_private.h"
#include <stdio.h>
#include <string.h>
#include "imageflow.h"

static bool ScaleAndRender1D(flow_c * context, struct flow_colorcontext_info * colorcontext,
                             struct flow_bitmap_bgra * pSrc, struct flow_bitmap_bgra * pDst,
                             const struct flow_interpolation_details * details, bool transpose)
{
    struct flow_interpolation_line_contributions * contrib = NULL;
    struct flow_bitmap_float * source_buf = NULL;
    struct flow_bitmap_float * dest_buf = NULL;

    uint32_t from_count = pSrc->w;
    uint32_t to_count = transpose ? pDst->h : pDst->w;

    bool success = true;

    if (details->window == 0) {
        FLOW_error(context, flow_status_Invalid_argument);
        return false;
    }

    // How many rows to buffer and process at a time.
    const uint32_t buffer_row_count = 4; // using buffer=5 seems about 6% better than most other non-zero values.

    // How many bytes per pixel are we scaling?
    flow_pixel_format scaling_format = flow_effective_pixel_format (pSrc);

    // TODO: measure; it might be faster to round 3->4 and ignore the data
    uint32_t float_channels = flow_pixel_format_channels(scaling_format);

    flow_prof_start(context, "contributions_calc", false);

    contrib = flow_interpolation_line_contributions_create(context, to_count, from_count, details);
    if (contrib == NULL) {
        FLOW_add_to_callstack(context);
        success = false;
        goto cleanup;
    }
    flow_prof_stop(context, "contributions_calc", true, false);

    flow_prof_start(context, "create_bitmap_float (buffers)", false);

    source_buf = flow_bitmap_float_create(context, from_count, buffer_row_count, float_channels, false);
    if (source_buf == NULL) {
        FLOW_add_to_callstack(context);
        success = false;
        goto cleanup;
    }
    dest_buf = flow_bitmap_float_create(context, to_count, buffer_row_count, float_channels, false);
    if (dest_buf == NULL) {
        FLOW_add_to_callstack(context);
        success = false;
        goto cleanup;
    }
    source_buf->alpha_meaningful = scaling_format == flow_bgra32;
    dest_buf->alpha_meaningful = source_buf->alpha_meaningful;

    source_buf->alpha_premultiplied = source_buf->channels == 4;
    dest_buf->alpha_premultiplied = source_buf->alpha_premultiplied;

    flow_prof_stop(context, "create_bitmap_float (buffers)", true, false);

    /* Scale each set of lines */
    for (uint32_t source_start_row = 0; source_start_row < pSrc->h; source_start_row += buffer_row_count) {
        const uint32_t row_count = umin(pSrc->h - source_start_row, buffer_row_count);

        flow_prof_start(context, "convert_srgb_to_linear", false);
        if (!flow_bitmap_float_convert_srgb_to_linear(context, colorcontext, pSrc, source_start_row, source_buf, 0,
                                                      row_count)) {
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

        //        if (!ApplyConvolutionsFloat1D(context, r, dest_buf, 0, row_count, contrib->percent_negative)) {
        //            FLOW_add_to_callstack (context);
        //            success=false;
        //            goto cleanup;
        //        }
        //        if (details->apply_color_matrix && call_number == 2) {
        //            if (!ApplyColorMatrix(context, r, dest_buf, row_count)) {
        //                FLOW_add_to_callstack (context);
        //                success=false;
        //                goto cleanup;
        //            }
        //        }

        flow_prof_start(context, "pivoting_composite_linear_over_srgb", false);
        if (!flow_bitmap_float_composite_linear_over_srgb(context, colorcontext, dest_buf, 0, pDst, source_start_row,
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

bool flow_node_execute_render_to_canvas_1d(flow_c * c, struct flow_bitmap_bgra * input,
                                           struct flow_bitmap_bgra * canvas,
                                           struct flow_nodeinfo_render_to_canvas_1d * info)
{

    //    if (info->canvas_x != 0 || info->canvas_y != 0
    //        || info->scale_to_width != (int32_t)(info->transpose_on_write ? canvas->h : canvas->w)) {
    //        FLOW_error(c, flow_status_Not_implemented); // Requires cropping the target canvas
    //        return false;
    //    }
    //    if (info->filter_list != NULL || info->sharpen_percent_goal != 0) {
    //        FLOW_error(c, flow_status_Not_implemented); // Requires cropping the target canvas
    //        return false;
    //    }
    struct flow_interpolation_details * d = flow_interpolation_details_create_from(c, info->interpolation_filter);
    if (d == NULL) {
        FLOW_error_return(c);
    }

    struct flow_colorcontext_info colorcontext;
    flow_colorcontext_init(c, &colorcontext, info->scale_in_colorspace, 0, 0, 0);

    if (!ScaleAndRender1D(c, &colorcontext, input, canvas, d, info->transpose_on_write)) {
        flow_interpolation_details_destroy(c, d);
        FLOW_error_return(c);
    }

    flow_interpolation_details_destroy(c, d);
    return true;
}
