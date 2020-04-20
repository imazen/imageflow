#include "imageflow_private.h"

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
