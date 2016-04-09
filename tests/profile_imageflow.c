#include "helpers_visual.h"

#define ERR2(c)                                                                                                        \
    if (has_err(c, __FILE__, __LINE__, __func__)) {                                                                    \
        return 42;                                                                                                     \
    }

int main(void)
{

    flow_c * c = flow_context_create();
    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR2(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;

    last = flow_node_create_decoder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 1600, 800, (flow_interpolation_filter_Robidoux),
                                  (flow_interpolation_filter_Robidoux));

    struct flow_bitmap_bgra * b;
    last = flow_node_create_bitmap_bgra_reference(c, &g, last, &b);
    // last = flow_node_create_encoder(c, &g, last, output_placeholder, flow_codec_type_encode_png);

    size_t bytes_count = 0;
    uint8_t * bytes
        = get_bytes_cached(c, &bytes_count, "http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/u6.jpg");

    struct flow_job * job = flow_job_create(c);
    ERR2(c);
    // flow_job_configure_recording(c, job, true, true, true, true, true);

    struct flow_io * input = flow_io_create_from_memory(c, flow_io_mode_read_seekable, bytes, bytes_count, job, NULL);
    struct flow_io * output = flow_io_create_for_output_buffer(c, job);

    flow_job_add_io(c, job, input, input_placeholder, FLOW_INPUT);
    flow_job_add_io(c, job, output, output_placeholder, FLOW_OUTPUT);

    int original_width, original_height;
    if (!get_image_dimensions(c, bytes, bytes_count, &original_width, &original_height))
        ERR2(c);
    //    long new_w = (original_width * 6 + 8 - 1L) / 8L;
    //    long new_h = (original_height * 6 + 8 - 1L) / 8L;
    if (!flow_job_decoder_set_downscale_hints_by_placeholder_id(c, job, input_placeholder, original_width * 2,
                                                                original_height * 2, original_width * 2,
                                                                original_height * 2 - 1, false, false)) {
        ERR2(c);
    }

    if (!flow_job_set_default_encoder(c, job, output_placeholder, flow_codec_type_encode_png)) {
        ERR2(c);
    }

    if (!flow_job_execute(c, job, &g)) {
        ERR2(c);
    }

    if (!flow_io_write_output_buffer_to_file(c, output, "u6_1600.png")) {
        ERR2(c);
    }
    flow_graph_print_to_dot(c, g, stdout, "");
    flow_job_destroy(c, job);

    flow_context_destroy(c);
    // code
    return 0; // Zero indicates success, while any
    // Non-Zero value indicates a failure/error
}
