/*
 * Copyright (c) Imazen LLC.
 * No part of this project, including this file, may be copied, modified,
 * propagated, or distributed except as permitted in COPYRIGHT.txt.
 * Licensed under the Apache License, Version 2.0.
 */
#include "imageflow.h"

bool test_contrib_windows(flow_context* context, char* msg);

bool function_bounded(flow_context* context, flow_interpolation_details* details, char* msg, double input_start_value,
                      double stop_at_abs, double input_step, double result_low_threshold, double result_high_threshold,
                      const char* name);

bool function_bounded_bi(flow_context* context, flow_interpolation_details* details, char* msg,
                         double input_start_value, double stop_at_abs, double input_step, double result_low_threshold,
                         double result_high_threshold, const char* name);

bool test_details(flow_context* context, flow_interpolation_details* details, char* msg, double expected_first_crossing,
                  double expected_second_crossing, double expected_near0, double near0_threshold, double expected_end);

char* test_filter(flow_context* context, flow_interpolation_filter filter, char* msg, double expected_first_crossing,
                  double expected_second_crossing, double expected_near0, double near0_threshold, double expected_end);

bool test_weight_distrib(flow_context* context, char* msg);

flow_interpolation_details* sample_filter(flow_context* context, flow_interpolation_filter filter, double x_from,
                                          double x_to, double* buffer, int samples);
