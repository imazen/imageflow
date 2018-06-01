// Copyright (c) Imazen LLC.
// No part of this project, including this file, may be copied, modified,
// propagated, or distributed except as permitted in COPYRIGHT.txt.
// Licensed under the Apache License, Version 2.0.

#include <stdio.h>

#include <string.h>

#include "imageflow_private.h"

extern "C" void keep10() {}

bool test_contrib_windows(flow_c * context, char * msg)
{
    int bad = -1;
    struct flow_interpolation_line_contributions * lct = 0;

    // assumes included edge cases

    struct flow_interpolation_details * triangle = flow_interpolation_details_create_from(
        context, flow_interpolation_filter::flow_interpolation_filter_Triangle);

    unsigned int from_w = 6;
    unsigned int to_w = 3;
    unsigned int corr36[3][2] = { { 0, 2 }, { 1, 4 }, { 3, 5 } };
    lct = flow_interpolation_line_contributions_create(context, to_w, from_w, triangle);

    for (uint32_t i = 0; i < lct->LineLength; i++)
        if (lct->ContribRow[i].Left != (int)corr36[i][0]) {
            bad = i;
            break;
        } else if (lct->ContribRow[i].Right != (int)corr36[i][1]) {
            bad = i;
            break;
        }

    if (bad != -1) {
        flow_snprintf(msg, 255, "at 6->3 invalid value (%d; %d) at %d, expected (%d; %d)", lct->ContribRow[bad].Left,
                      lct->ContribRow[bad].Right, bad, corr36[bad][0], corr36[bad][1]);
        flow_interpolation_line_contributions_destroy(context, lct);
        return false;
    }
    flow_interpolation_line_contributions_destroy(context, lct);

    from_w = 6;
    to_w = 4;
    unsigned int corr46[4][2] = { { 0, 1 }, { 1, 3 }, { 2, 4 }, { 4, 5 } };
    lct = flow_interpolation_line_contributions_create(context, to_w, from_w, triangle);
    flow_interpolation_details_destroy(context, triangle);

    for (uint32_t i = 0; i < lct->LineLength; i++)
        if (lct->ContribRow[i].Left != (int)corr46[i][0]) {
            bad = i;
            break;
        } else if (lct->ContribRow[i].Right != (int)corr46[i][1]) {
            bad = i;
            break;
        }

    if (bad != -1) {
        flow_snprintf(msg, 255, "at 6->4 invalid value (%d; %d) at %d, expected (%d; %d)", lct->ContribRow[bad].Left,
                      lct->ContribRow[bad].Right, bad, corr46[bad][0], corr46[bad][1]);
        flow_interpolation_line_contributions_destroy(context, lct);
        return false;
    }
    flow_interpolation_line_contributions_destroy(context, lct);
    return true;
}

bool function_bounded(flow_c * context, struct flow_interpolation_details * details, char * msg,
                      double input_start_value, double stop_at_abs, double input_step, double result_low_threshold,
                      double result_high_threshold, const char * name)
{
    double input_value = input_start_value;

    if (fabs(input_value) >= fabs(stop_at_abs))
        return true;

    double result_value = (*details->filter)(details, input_value);

    if (result_value < result_low_threshold) {
        flow_snprintf(msg + strlen(msg), 255 - strlen(msg), "value %.4f is below %.4f at x=%.4f (%s)", result_value,
                      result_low_threshold, input_value, name);
        return false;
    } else if (result_value > result_high_threshold) {
        flow_snprintf(msg + strlen(msg), 255 - strlen(msg), "value %.4f exceeds %.4f at x=%.4f (%s)", result_value,
                      result_high_threshold, input_value, name);
        return false;
    }

    return function_bounded(context, details, msg, input_value + input_step, stop_at_abs, input_step,
                            result_low_threshold, result_high_threshold, name);
}

bool function_bounded_bi(flow_c * context, struct flow_interpolation_details * details, char * msg,
                         double input_start_value, double stop_at_abs, double input_step, double result_low_threshold,
                         double result_high_threshold, const char * name)
{
    return function_bounded(context, details, msg, input_start_value, stop_at_abs, input_step, result_low_threshold,
                            result_high_threshold, name)
           && function_bounded(context, details, msg, input_start_value * -1.0f, stop_at_abs, input_step * -1.0f,
                               result_low_threshold, result_high_threshold, name);
}

bool test_details(flow_c * context, struct flow_interpolation_details * details, char * msg,
                  double expected_first_crossing, double expected_second_crossing, double expected_near0,
                  double near0_threshold, double expected_end)
{
    double top = (*details->filter)(details, 0);

    // Verify peak is at x = 0
    if (!function_bounded_bi(context, details, msg, 0, expected_end, 0.05, -500, top, "should peak at x=0"))
        return false;

    // Verify we drop below a certain threshold between expected_near0 and expected_second_crossing or expected_end
    if (!function_bounded_bi(context, details, msg, expected_near0,
                             expected_second_crossing > 0 ? expected_second_crossing : expected_end, 0.05, -500,
                             near0_threshold, "should near 0"))
        return false;

    char sub_msg[1024];
    flow_snprintf(sub_msg, 1024, "should end at expected_end (%f)", expected_end);

    // Ensure ended at expected_end
    if (!function_bounded_bi(context, details, msg, expected_end, expected_end + 1, 0.05, -0.0001f, 0.0001f, sub_msg))
        return false;

    if (expected_first_crossing != 0 && expected_second_crossing != 0) {
        // Ensure everything between the two crossings is negative
        if (!function_bounded_bi(context, details, msg, expected_first_crossing + 0.05, expected_second_crossing - 0.05,
                                 0.05, -500, -0.0001f, "should be negative between crossing 1 and 2"))
            return false;

        // Ensure everything between second crossing and end is positive - if significant
        if (expected_end > expected_second_crossing + 0.1) {

            flow_snprintf(sub_msg, 1024, "should be positive between crossing 2 (%f) and end (%f)",
                          expected_second_crossing, expected_end);
            if (!function_bounded_bi(context, details, msg, expected_second_crossing + 0.05, expected_end - 0.02, 0.02,
                                     0, 500, sub_msg))
                return false;
        }
    } else {
        // Ensure everything is non-negative
        if (!function_bounded_bi(context, details, msg, expected_near0, expected_end, 0.05, -0.0001, 500,
                                 "this function should only produce positive weights"))
            return false;
    }

    return true;
}

char * test_filter(flow_c * context, flow_interpolation_filter filter, char * msg, double expected_first_crossing,
                   double expected_second_crossing, double expected_near0, double near0_threshold, double expected_end)
{
    struct flow_interpolation_details * details = flow_interpolation_details_create_from(context, filter);
    flow_snprintf(msg, 255, "Filter=(%d) ", filter);
    bool result = test_details(context, details, msg, expected_first_crossing, expected_second_crossing, expected_near0,
                               near0_threshold, expected_end);
    flow_interpolation_details_destroy(context, details);
    if (!result)
        return msg;
    else
        return nullptr;
}

struct flow_interpolation_details * sample_filter(flow_c * context, flow_interpolation_filter filter, double x_from,
                                                  double x_to, double * buffer, int samples)
{
    struct flow_interpolation_details * details = flow_interpolation_details_create_from(context, filter);
    if (details == NULL)
        return NULL;
    for (int i = 0; i < samples; i++) {
        double x = (x_to - x_from) * ((double)i / (double)samples) + x_from;
        buffer[i] = details->filter(details, x);
    }
    return details;
}
