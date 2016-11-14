#include "png.h"
#include "catch.hpp"
#include "helpers.h"
#include "rewrite_in_rust/rewrite_in_rust.h"

// Port Priority 0 - lowest priority to create rust equivalent. 1 - medium priority, 2 - likely useful

// Assumes placeholders 0 and 1 for input/output respectively
bool execute_graph_for_url(flow_c * c, const char * input_image_url, const char * output_image_path,
                           struct flow_graph ** graph_ref)
{
    size_t bytes_count = 0;
    uint8_t * bytes = get_bytes_cached(c, &bytes_count, input_image_url, __FILE__);

    struct flow_job * job = flow_job_create(c);
    ERR(c);
    flow_job_configure_recording(c, job, false, false, false, false, false);

    int32_t input_placeholder = 0;
    int32_t output_placeholder = 1;

    struct flow_io * input = flow_io_create_from_memory(c, flow_io_mode_read_seekable, bytes, bytes_count, job, NULL);
    struct flow_io * output = flow_io_create_for_output_buffer(c, job);

    flow_job_add_io(c, job, input, input_placeholder, FLOW_INPUT);
    flow_job_add_io(c, job, output, output_placeholder, FLOW_OUTPUT);

    if (!flow_job_set_default_encoder(c, job, output_placeholder, flow_codec_type_encode_png)) {
        ERR(c);
    }

    if (!flow_job_execute(c, job, graph_ref)) {
        ERR(c);
    }

    if (!flow_io_write_output_buffer_to_file(c, output, output_image_path)) {
        ERR(c);
    }
    flow_job_destroy(c, job);
    return true;
}

bool execute_graph_for_bitmap_bgra(flow_c * c, struct flow_graph ** graph_ref)
{
    struct flow_job * job = flow_job_create(c);
    ERR(c);
    flow_job_configure_recording(c, job, false, false, false, false, false);

    if (!flow_job_execute(c, job, graph_ref)) {
        ERR(c);
    }
    ERR(c);
    flow_job_destroy(c, job);
    return true;
}


// Port priority 2
TEST_CASE("Test reading from memory, writing to file (test flow_io through job)", "")
{
    flow_c * c = flow_context_create();
    struct flow_graph * g = nullptr;
    struct flow_job * job = nullptr;
    struct flow_bitmap_bgra * result = nullptr;

    g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t input_placeholder = 0;

    int32_t last;
    last = flow_node_create_decoder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 300, 200, (flow_interpolation_filter_Robidoux),
                                  (flow_interpolation_filter_Robidoux), 0, 0);
    last = flow_node_create_encoder(c, &g, last, 1, flow_codec_type_encode_png, NULL);

    job = flow_job_create(c);
    ERR(c);
    uint8_t image_bytes_literal[]
        = { 0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00,
            0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
            0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82 };

    struct flow_io * input_io = flow_io_create_from_memory(c, flow_io_mode_read_seekable, &image_bytes_literal[0],
                                                           sizeof(image_bytes_literal), job, NULL);

    if (!flow_job_add_io(c, job, input_io, input_placeholder, FLOW_INPUT)) {
        ERR(c);
    }
    struct flow_io * output_io = flow_io_create_for_file(c, flow_io_mode_write_seekable, "test_io.png", job);

    if (!flow_job_add_io(c, job, output_io, 1, FLOW_OUTPUT)) {
        ERR(c);
    }

    if (!flow_job_execute(c, job, &g)) {
        ERR(c);
    }

    ERR(c);

    flow_context_destroy(c);
    c = NULL;
    g = NULL;
    last = -1;
    job = NULL;
    input_io = NULL;

    c = flow_context_create();

    g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    last = flow_node_create_decoder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 300, 200, (flow_interpolation_filter_Robidoux),
                                  (flow_interpolation_filter_Robidoux), 0, 0);
    last = flow_node_create_bitmap_bgra_reference(c, &g, last, &result);

    job = flow_job_create(c);
    ERR(c);
    input_io = flow_io_create_for_file(c, flow_io_mode_read_seekable, "test_io.png", job);

    if (!flow_job_add_io(c, job, input_io, input_placeholder, FLOW_INPUT)) {
        ERR(c);
    }

    if (!flow_job_execute(c, job, &g)) {
        ERR(c);
    }

    REQUIRE(result != NULL);
    REQUIRE(result->w == 300);

    // unlink ("test_io.png");

    flow_context_destroy(c);
}


// Port priority 1

TEST_CASE("Decode GIF frame, scale, encode as PNG", "")
{
    flow_context * c = flow_context_create();
    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;

    last = flow_node_create_decoder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 120, 120, (flow_interpolation_filter_Robidoux),
                                  (flow_interpolation_filter_Robidoux), 0, 0);
    last = flow_node_create_encoder_placeholder(c, &g, last, output_placeholder);

    execute_graph_for_url(c, "http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/mountain_800.gif",
                          "gif_scaled.png", &g);

    flow_context_destroy(c);
}



// Port priority 1
TEST_CASE("export frames of animated gif", "")
{
    int32_t last, input_placeholder = 0;

    flow_context * c = flow_context_create();
    REQUIRE(c != NULL);
    // Get the input gif
    struct flow_io * input = get_io_for_cached_url(
        c, "http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/example-animated.gif", c,
        __FILE__); //"http://i.kinja-img.com/gawker-media/image/upload/s--dM0nT5E4--/mn3sov5id06ppjkfb1b2.gif", c);
    ERR(c);
    // Create the job and add the input
    struct flow_job * job = flow_job_create(c);
    ERR(c);
    flow_job_add_io(c, job, input, input_placeholder, FLOW_INPUT);
    // Now we can read metadata about the input
    struct flow_decoder_info info;
    if (!flow_job_get_decoder_info(c, job, input_placeholder, &info)) {
        ERR(c);
    }

    REQUIRE(info.frame_count == 68);
    // Loop through each frame, add a corresponding output file, and execute the operation
    for (int i = 0; i < (int64_t)info.frame_count; i++) {
        struct flow_io * output = flow_io_create_for_output_buffer(c, job);
        int32_t output_placeholder = input_placeholder + 1 + i;
        flow_job_add_io(c, job, output, output_placeholder, FLOW_OUTPUT);

        struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
        ERR(c);
        last = flow_node_create_decoder(c, &g, -1, input_placeholder);
        last = flow_node_create_encoder(c, &g, last, output_placeholder, flow_codec_type_encode_png, NULL);

        if (!flow_job_decoder_switch_frame(c, job, input_placeholder, i)) {
            ERR(c);
        }

        if (!flow_job_execute(c, job, &g)) {
            ERR(c);
        }
        char output_image_path[255];
        flow_snprintf(output_image_path, sizeof(output_image_path), "exported_gif_frame_%i.png", i);

        if (!flow_io_write_output_buffer_to_file(c, output, output_image_path)) {
            ERR(c);
        }
    }

    flow_job_destroy(c, job);
    flow_context_destroy(c);
}


// Port priority 1
TEST_CASE("Roundtrip flipping both horizontal and vertical", "")
{
    // Vertical and horizatal roundtrip should be separate tests
    // This didn't catch the bug where horizontal and vertical nodes both flipped vertically
    flow_c * c = flow_context_create();
    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;
    struct flow_bitmap_bgra * input;
    struct flow_bitmap_bgra * output;

    last = flow_node_create_bitmap_bgra_reference(c, &g, -1, &input);
    last = flow_node_create_clone(c, &g, last);
    last = flow_node_create_primitive_flip_vertical(c, &g, last);
    last = flow_node_create_primitive_flip_horizontal(c, &g, last);
    last = flow_node_create_primitive_flip_horizontal(c, &g, last);
    last = flow_node_create_primitive_flip_vertical(c, &g, last);
    last = flow_node_create_bitmap_bgra_reference(c, &g, last, &output);

    input = BitmapBgra_create_test_image(c);
    execute_graph_for_bitmap_bgra(c, &g);

    ERR(c);
    bool equal = false;
    if (!flow_bitmap_bgra_compare(c, input, output, &equal)) {
        ERR(c);
    }
    REQUIRE(equal);

    flow_context_destroy(c);
}

// Port priority 1
TEST_CASE("copy_rect_to_canvas (smoke test)", "")
{
    flow_c * c = flow_context_create();
    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;

    last = flow_node_create_decoder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 200, 200, (flow_interpolation_filter_Robidoux),
                                  (flow_interpolation_filter_Robidoux), 0, 0);
    int32_t canvas = flow_node_create_canvas(c, &g, -1, flow_bgra32, 300, 300, 0);
    last = flow_node_create_primitive_copy_rect_to_canvas(c, &g, last, 0, 0, 150, 150, 50, 50);
    flow_edge_create(c, &g, canvas, last, flow_edgetype_canvas);
    last = flow_node_create_expand_canvas(c, &g, last, 10, 20, 30, 40, 0xFF99FF99);
    last = flow_node_create_encoder_placeholder(c, &g, last, output_placeholder);

    execute_graph_for_url(c, "http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/mountain_800.png",
                          "graph_scaled_blitted_png.png", &g);

    flow_context_destroy(c);
}

// Port priority 2
TEST_CASE("Test graph with 3 nodes pulling from decoder (smoke test)", "")
{
    // This test case helped expose a flaw in graph creation, where we swapped max_edges and max_nodes and caused memory
    // overlap
    // It also showed how that post_optimize_flatten calls which create pre_optimize_flattenable nodes
    // Can cause execution to fail in fewer than 6 passes. We may want to re-evaluate our graph exeuction approach
    flow_c * c = flow_context_create();
    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t input_placeholder = 0, output_placeholder = 1;

    int32_t input = flow_node_create_decoder(c, &g, -1, input_placeholder);
    int32_t clone_a = flow_node_create_clone(c, &g, input);
    clone_a = flow_node_create_rotate_90(c, &g, clone_a);
    int32_t clone_b = flow_node_create_clone(c, &g, input);
    clone_b = flow_node_create_rotate_180(c, &g, clone_b);
    int32_t clone_c = flow_node_create_clone(c, &g, input);
    clone_c = flow_node_create_rotate_270(c, &g, clone_c);
    flow_node_create_encoder_placeholder(c, &g, clone_a, output_placeholder);

    execute_graph_for_url(c, "http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation.png",
                          "rotated.png", &g);

    flow_context_destroy(c);
}

// Port priority 2
TEST_CASE(
    "Verify origin nodes (like decoders) are prevented from having inputs; encoder nodes can't have more than 1 input",
    "")
{
    flow_c * c = flow_context_create();
    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    struct flow_bitmap_bgra * p;
    int32_t first = flow_node_create_bitmap_bgra_reference(c, &g, -1, &p);
    int32_t encoder = flow_node_create_encoder_placeholder(c, &g, first, 0);
    int32_t second = flow_node_create_clone(c, &g, encoder);
    REQUIRE_FALSE(flow_context_has_error(c));
    REQUIRE_FALSE(flow_graph_validate(c, g));
    REQUIRE(flow_context_error_reason(c) == flow_status_Graph_invalid);

    flow_context_clear_error(c);
    // Remove the invalid outbound node & edge
    flow_node_delete(c, g, second);

    // Canvas input not permitted to encoder
    int32_t canvas = flow_node_create_clone(c, &g, -1);
    flow_edge_create(c, &g, canvas, encoder, flow_edgetype_canvas);
    // We shouldn't have an error until we call validate
    REQUIRE_FALSE(flow_context_has_error(c));
    REQUIRE_FALSE(flow_graph_validate(c, g));
    REQUIRE(flow_context_error_reason(c) == flow_status_Invalid_inputs_to_node);

    flow_context_destroy(c);
}
