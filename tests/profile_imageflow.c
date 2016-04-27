#include "helpers_visual.h"

#define ERR2(c)                                                                                                        \
    if (has_err(c, __FILE__, __LINE__, __func__)) {                                                                    \
        return 42;                                                                                                     \
    }

int main(void)
{

    for (int flags = 0; flags < 4; flags++) {
        int64_t start = flow_get_high_precision_ticks();
        for (int i = 2; i < 8; i++) {
            flow_c * c = flow_context_create();

            if ((flags & 2) == 0) {
                flow_context_set_floatspace(c, flow_working_floatspace_as_is, 0, 0, 0);
            }

            size_t bytes_count = 0;
            uint8_t * bytes = get_bytes_cached(
                c, &bytes_count, "http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/u6.jpg");

            struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
            ERR2(c);

            int32_t last, input_placeholder = 0;

            struct flow_job * job = flow_job_create(c);
            ERR2(c);
            // flow_job_configure_recording(c, job, true, true, true, true, true);

            struct flow_io * input
                = flow_io_create_from_memory(c, flow_io_mode_read_seekable, bytes, bytes_count, job, NULL);
            flow_job_add_io(c, job, input, input_placeholder, FLOW_INPUT);

            int original_width, original_height;
            if (!get_image_dimensions(c, bytes, bytes_count, &original_width, &original_height))
                ERR2(c);

            int min_factor = 3;

            int32_t target_w = original_width / i;
            int32_t target_h = (int32_t)ceil((float)target_w / (float)original_width * (float)original_height);

            if (!flow_job_decoder_set_downscale_hints_by_placeholder_id(
                    c, job, input_placeholder, (target_w - 1) * min_factor, (target_h - 1) * min_factor,
                    (target_w - 1) * min_factor, (target_h - 1) * min_factor, (flags & 2) > 0, (flags & 2) > 0)) {
                ERR2(c);
            }

            last = flow_node_create_decoder(c, &g, -1, input_placeholder);
            last = flow_node_create_scale(c, &g, last, target_w, target_h, (flow_interpolation_filter_Robidoux),
                                          (flow_interpolation_filter_Robidoux), flags);

            struct flow_bitmap_bgra * b;
            last = flow_node_create_bitmap_bgra_reference(c, &g, last, &b);

            if (!flow_job_execute(c, job, &g)) {
                ERR2(c);
            }

            // flow_graph_print_to_dot(c, g, stdout, "");
            flow_job_destroy(c, job);

            flow_context_destroy(c);
        }
        int64_t end = flow_get_high_precision_ticks();
        int64_t ms = (end - start) * 1000 / flow_get_profiler_ticks_per_second();
        fprintf(stdout, "With flags=%d, took %dms\n", flags, (int32_t)ms);
        fflush(stdout);
        // code
    }
    return 0; // Zero indicates success, while any
    // Non-Zero value indicates a failure/error

    /*  Before changing idct_fast
     *  With flags=0, took 839ms: srgb, standard
        With flags=1, took 743ms: srgb, reducev
        With flags=2, took 3778ms: linear, standard
        With flags=3, took 1703ms: srgb, reducev


        Using fastpow in linear_to_srgb

        With flags=0, took 819ms
        With flags=1, took 738ms
        With flags=2, took 1588ms
        With flags=3, took 1062ms



     */
}
