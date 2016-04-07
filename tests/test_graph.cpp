#include "png.h"
#include "catch.hpp"
#include "helpers.h"

// Assumes placeholders 0 and 1 for input/output respectively
bool execute_graph_for_url(flow_c * c, const char * input_image_url, const char * output_image_path,
                           struct flow_graph ** graph_ref)
{
    size_t bytes_count = 0;
    uint8_t * bytes = get_bytes_cached(c, &bytes_count, input_image_url);

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

TEST_CASE("create tiny graph", "")
{
    flow_c * c = flow_context_create();
    flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    int32_t last;

    last = flow_node_create_canvas(c, &g, -1, flow_bgra32, 400, 300, 0xFFFFFFFF);
    last = flow_node_create_scale(c, &g, last, 300, 200, (flow_interpolation_filter_Robidoux),
                                  (flow_interpolation_filter_Robidoux));
    last = flow_node_create_encoder_placeholder(c, &g, last, 0);

    ERR(c);

    REQUIRE(g->edges[0].from == 0);
    REQUIRE(g->edges[0].to == 1);
    REQUIRE(g->edge_count == 2);
    REQUIRE(g->node_count == 3);

    flow_context_destroy(c);
}

TEST_CASE("delete a node from a graph", "")
{
    flow_c * c = flow_context_create();
    flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    int32_t last;

    last = flow_node_create_canvas(c, &g, -1, flow_bgra32, 400, 300, 0xFFFFFFFF);
    last = flow_node_create_scale(c, &g, last, 300, 200, (flow_interpolation_filter_Robidoux),
                                  (flow_interpolation_filter_Robidoux));
    last = flow_node_create_encoder_placeholder(c, &g, last, 0);
    ERR(c);

    REQUIRE(g->edges[0].from == 0);
    REQUIRE(g->edges[0].to == 1);
    REQUIRE(g->edges[1].from == 1);
    REQUIRE(g->edges[1].to == 2);
    REQUIRE(g->edge_count == 2);
    REQUIRE(g->node_count == 3);

    flow_node_delete(c, g, last);
    ERR(c);

    REQUIRE(g->edge_count == 1);
    REQUIRE(g->node_count == 2);
    REQUIRE(g->nodes[last].type == flow_ntype_Null);
    REQUIRE(g->nodes[last].info_byte_index == -1);
    REQUIRE(g->nodes[last].info_bytes == 0);
    REQUIRE(g->edges[1].type == flow_edgetype_null);
    REQUIRE(g->edges[1].info_bytes == 0);
    REQUIRE(g->edges[1].info_byte_index == -1);
    REQUIRE(g->edges[1].from == -1);
    REQUIRE(g->edges[1].to == -1);

    flow_context_destroy(c);
}

TEST_CASE("clone an edge", "")
{
    flow_c * c = flow_context_create();
    flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    int32_t last;
    last = flow_node_create_canvas(c, &g, -1, flow_bgra32, 400, 300, 0xFFFFFFFF);
    last = flow_node_create_scale(c, &g, last, 300, 200, (flow_interpolation_filter_Robidoux),
                                  (flow_interpolation_filter_Robidoux));

    ERR(c);

    REQUIRE(g->edges[0].from == 0);
    REQUIRE(g->edges[0].to == 1);
    REQUIRE(g->edge_count == 1);
    REQUIRE(g->node_count == 2);

    flow_edge_duplicate(c, &g, 0);

    ERR(c);

    REQUIRE(g->edge_count == 2);
    REQUIRE(g->node_count == 2);
    REQUIRE(g->edges[1].from == 0);
    REQUIRE(g->edges[1].to == 1);

    flow_context_destroy(c);
}

// TODO test paths where adding nodes/edges exceeds the max size

TEST_CASE("execute tiny graph", "")
{

    flow_c * c = flow_context_create();
    flow_utils_ensure_directory_exists("node_frames");
    struct flow_graph * g = nullptr;
    struct flow_job * job = nullptr;

    struct flow_bitmap_bgra * result = nullptr;

    g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last;

    last = flow_node_create_canvas(c, &g, -1, flow_bgra32, 400, 300, 0xFFFFFFFF);
    //    last = flow_node_create_fill_rect()
    last = flow_node_create_scale(c, &g, last, 300, 200, (flow_interpolation_filter_Robidoux),
                                  (flow_interpolation_filter_Robidoux));
    last = flow_node_create_bitmap_bgra_reference(c, &g, last, &result);

    job = flow_job_create(c);
    ERR(c);
    REQUIRE(g->edges[1].from == 1);
    REQUIRE(g->edges[1].to == 2);

    if (!flow_job_execute(c, job, &g)) {
        ERR(c);
    }
    ERR(c);

    REQUIRE(result != NULL);
    REQUIRE(result->w == 300);

    flow_context_destroy(c);
}

TEST_CASE("decode and scale png", "")
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
                                  (flow_interpolation_filter_Robidoux));
    last = flow_node_create_bitmap_bgra_reference(c, &g, last, &result);

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

    if (!flow_job_execute(c, job, &g)) {
        ERR(c);
    }

    ERR(c);

    REQUIRE(result != NULL);
    REQUIRE(result->w == 300);

    flow_context_destroy(c);
}

uint8_t image_bytes_literal[]
    = { 0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00,
        0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
        0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82 };

bool create_operation_graph(flow_c * c, struct flow_graph ** graph_ref, int32_t input_placeholder,
                            int32_t output_placeholder, struct flow_decoder_info * info)
{

    REQUIRE(info->frame0_post_decode_format == flow_bgra32);
    REQUIRE(info->frame0_width == 1);
    REQUIRE(info->frame0_height == 1);
    REQUIRE(strcmp(info->preferred_extension, "png") == 0);
    REQUIRE(strcmp(info->preferred_mime_type, "image/png") == 0);
    REQUIRE(info->codec_id == flow_codec_type_decode_png);

    // We'll create a simple operation graph that scales the image up 200%
    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    if (g == NULL) {
        FLOW_add_to_callstack(c);
        return false;
    }
    int32_t last = flow_node_create_decoder(c, &g, -1, input_placeholder);
    // Double the original width/height
    last = flow_node_create_scale(c, &g, last, info->frame0_width * 2, info->frame0_height * 2,
                                  (flow_interpolation_filter_Robidoux), (flow_interpolation_filter_Robidoux));
    // Keep the original format if png or jpeg
    size_t encoder_id = info->codec_id == flow_codec_type_decode_jpeg ? flow_codec_type_encode_jpeg
                                                                      : flow_codec_type_encode_png;
    last = flow_node_create_encoder(c, &g, last, output_placeholder, encoder_id);

    if (flow_context_has_error(c)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    *graph_ref = g;
    return true;
}

bool scale_image_inner(flow_c * c, flow_io * input, flow_io * output)
{
    // We associate codecs and nodes using integer IDs that you select
    int32_t input_placeholder = 42;
    int32_t output_placeholder = 0xbad1dea;

    // We create a job to create I/O resources and attach them to our abstract graph above
    struct flow_job * job = flow_job_create(c);
    if (job == NULL) {
        FLOW_add_to_callstack(c);
        return false;
    }
    // Uncomment to make an animation. Requires sudo apt-get install libav-tools graphviz gifsicle
    // flow_job_configure_recording(c, job, true, true, true, true, true);

    // Add I/O to the job. First bytes are read here
    if (!flow_job_add_io(c, job, input, input_placeholder, FLOW_INPUT)
        || !flow_job_add_io(c, job, output, output_placeholder, FLOW_OUTPUT)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    // Let's read information about the input file
    struct flow_decoder_info info;
    if (!flow_job_get_decoder_info(c, job, input_placeholder, &info)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    // And give it to the operation graph designer
    struct flow_graph * g;
    if (!create_operation_graph(c, &g, input_placeholder, output_placeholder, &info)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    // Execute the graph we created
    if (!flow_job_execute(c, job, &g)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    return true;
}

bool scale_image_to_disk()
{
    // flow_context provides error tracking and memory management
    flow_c * c = flow_context_create();
    if (c == NULL) {
        return false;
    }
    // We're going to use an embedded image, but you could get bytes from anywhere
    struct flow_io * input = flow_io_create_from_memory(c, flow_io_mode_read_seekable, &image_bytes_literal[0],
                                                        sizeof(image_bytes_literal), c, NULL);
    // Output to an in-memory expanding buffer. This could be a stream or file instead.
    struct flow_io * output = flow_io_create_for_output_buffer(c, c);

    // Using an inner function makes it easier to deal with errors
    if (input == NULL || output == NULL || !scale_image_inner(c, input, output)) {
        FLOW_add_to_callstack(c);
        flow_context_print_error_to(c, stderr); // prints the callstack, too
        flow_context_destroy(c);
        return false;
    }
    // Write the output to file. We could use flow_io_get_output_buffer to get the bytes directly if we wanted them
    if (!flow_io_write_output_buffer_to_file(c, output, "graph_scaled_png.png")) {

        FLOW_add_to_callstack(c);
        flow_context_print_error_to(c, stderr);
        flow_context_destroy(c);
        return false;
    }
    // This will destroy the input/output objects, but if there are underlying streams that need to be
    // closed, you would do that here after flow_context_destroy
    flow_context_destroy(c);
    return true;
}

TEST_CASE("decode, scale, and re-encode png", "") { REQUIRE(scale_image_to_disk()); }

TEST_CASE("scale and flip and crop png", "")
{
    flow_c * c = flow_context_create();
    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;

    last = flow_node_create_decoder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 120, 120, (flow_interpolation_filter_Robidoux),
                                  (flow_interpolation_filter_Robidoux));
    last = flow_node_create_primitive_flip_vertical(c, &g, last);
    last = flow_node_create_primitive_crop(c, &g, last, 20, 10, 80, 40);
    last = flow_node_create_encoder_placeholder(c, &g, last, output_placeholder);

    execute_graph_for_url(c, "http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/mountain_800.png", "graph_flipped_cropped_png.png", &g);

    flow_context_destroy(c);
}

TEST_CASE("scale gif", "")
{
    flow_context * c = flow_context_create();
    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;

    last = flow_node_create_decoder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 120, 120, (flow_interpolation_filter_Robidoux),
                                  (flow_interpolation_filter_Robidoux));
    last = flow_node_create_encoder_placeholder(c, &g, last, output_placeholder);

    execute_graph_for_url(c, "http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/mountain_800.gif", "gif_scaled.png", &g);

    flow_context_destroy(c);
}

TEST_CASE("read gif overlapped", "")
{
    flow_context * c = flow_context_create();
    REQUIRE(c != NULL);
    // Get the input gif
    struct flow_io * input = get_io_for_cached_url(
        c, "http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/example-animated.gif",
        c); //"http://i.kinja-img.com/gawker-media/image/upload/s--dM0nT5E4--/mn3sov5id06ppjkfb1b2.gif", c);
    ERR(c);
    struct flow_io * input2 = get_io_for_cached_url(
        c, "http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/example-animated.gif",
        c); //"http://i.kinja-img.com/gawker-media/image/upload/s--dM0nT5E4--/mn3sov5id06ppjkfb1b2.gif", c);
    ERR(c);
    // Create the job and add the input
    struct flow_job * job = flow_job_create(c);
    ERR(c);
    flow_job_add_io(c, job, input, 0, FLOW_INPUT);
    flow_job_add_io(c, job, input2, 1, FLOW_INPUT);
    // Now we can read metadata about the input
    struct flow_decoder_info info;
    if (!flow_job_get_decoder_info(c, job, 0, &info)) {
        ERR(c);
    }
    if (!flow_job_get_decoder_info(c, job, 1, &info)) {
        ERR(c);
    }
    flow_context_destroy(c);
}

TEST_CASE("export frames of animated gif", "")
{
    int32_t last, input_placeholder = 0;

    flow_context * c = flow_context_create();
    REQUIRE(c != NULL);
    // Get the input gif
    struct flow_io * input = get_io_for_cached_url(
        c, "http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/example-animated.gif",
        c); //"http://i.kinja-img.com/gawker-media/image/upload/s--dM0nT5E4--/mn3sov5id06ppjkfb1b2.gif", c);
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
        last = flow_node_create_encoder(c, &g, last, output_placeholder, flow_codec_type_encode_png);

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

TEST_CASE("scale and flip and crop jpg", "")
{
    flow_c * c = flow_context_create();
    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;

    last = flow_node_create_decoder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 120, 120, (flow_interpolation_filter_Robidoux),
                                  (flow_interpolation_filter_Robidoux));
    last = flow_node_create_primitive_flip_vertical(c, &g, last);
    last = flow_node_create_primitive_crop(c, &g, last, 20, 10, 80, 40);

    last = flow_node_create_encoder(c, &g, last, output_placeholder, flow_codec_type_encode_jpeg);

    execute_graph_for_url(c, "http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/mountain_800.jpg", "graph_flipped_cropped_from_jpeg.jpg", &g);

    flow_context_destroy(c);
}

TEST_CASE("benchmark scaling large progressive jpg", "")
{
    flow_c * c = flow_context_create();
    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;

    last = flow_node_create_decoder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 800, 800, (flow_interpolation_filter_Robidoux),
                                  (flow_interpolation_filter_Robidoux));
    last = flow_node_create_encoder(c, &g, last, output_placeholder, flow_codec_type_encode_jpeg);

    execute_graph_for_url(c, "http://s3.amazonaws.com/resizer-dynamic-downloads/imageflow_test_suite/4kx4k.jpg",
                          "graph_large_jpeg.jpg", &g);

    flow_context_destroy(c);
}

TEST_CASE("benchmark scaling large jpg", "")
{
    flow_c * c = flow_context_create();
    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;

    last = flow_node_create_decoder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 800, 800, (flow_interpolation_filter_Robidoux),
                                  (flow_interpolation_filter_Robidoux));
    last = flow_node_create_encoder(c, &g, last, output_placeholder, flow_codec_type_encode_jpeg);

    execute_graph_for_url(c,
                          "http://s3.amazonaws.com/resizer-dynamic-downloads/imageflow_test_suite/4kx4k_baseline.jpg",
                          "graph_large_jpeg.jpg", &g);

    flow_context_destroy(c);
}

TEST_CASE("Roundtrip flipping", "")
{
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

TEST_CASE("scale copy rect", "")
{
    flow_c * c = flow_context_create();
    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;

    last = flow_node_create_decoder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 200, 200, (flow_interpolation_filter_Robidoux),
                                  (flow_interpolation_filter_Robidoux));
    int32_t canvas = flow_node_create_canvas(c, &g, -1, flow_bgra32, 300, 300, 0);
    last = flow_node_create_primitive_copy_rect_to_canvas(c, &g, last, 0, 0, 150, 150, 50, 50);
    flow_edge_create(c, &g, canvas, last, flow_edgetype_canvas);
    last = flow_node_create_expand_canvas(c, &g, last, 10, 20, 30, 40, 0xFF99FF99);
    last = flow_node_create_encoder_placeholder(c, &g, last, output_placeholder);

    execute_graph_for_url(c, "http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/mountain_800.png", "graph_scaled_blitted_png.png", &g);

    flow_context_destroy(c);
}

TEST_CASE("test frame clone", "")
{
    flow_c * c = flow_context_create();
    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t input_placeholder = 0, output_placeholder = 1;

    int32_t input = flow_node_create_decoder(c, &g, -1, input_placeholder);
    int32_t clone_a = flow_node_create_clone(c, &g, input);
    int32_t clone_b = flow_node_create_clone(c, &g, input);
    int32_t last = flow_node_create_primitive_flip_vertical(c, &g, clone_b); // mutate b, leave a alone
    flow_node_create_transpose(c, &g, last);

    flow_node_create_encoder_placeholder(c, &g, clone_a, output_placeholder);

    execute_graph_for_url(c, "http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/mountain_400.png", "unflipped.png", &g);

    flow_context_destroy(c);
}

TEST_CASE("test rotation", "")
{
    flow_c * c = flow_context_create();
    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t input_placeholder = 0, output_placeholder = 1;

    int32_t input = flow_node_create_decoder(c, &g, -1, input_placeholder);
    int32_t a = flow_node_create_rotate_90(c, &g, input);
    int32_t b = flow_node_create_rotate_180(c, &g, input);
    int32_t c_n = flow_node_create_rotate_270(c, &g, input);
    flow_node_create_encoder_placeholder(c, &g, a, output_placeholder);

    execute_graph_for_url(c, "http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation.png", "rotated.png", &g);

    flow_context_destroy(c);
}

TEST_CASE("test memory corruption", "")
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

    execute_graph_for_url(c, "http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation.png", "rotated.png", &g);

    flow_context_destroy(c);
}

TEST_CASE("check for cycles", "")
{
    // This test case helped expose a flaw in graph creation, where we swapped max_edges and max_nodes and caused memory
    // overlap
    // It also showed how that post_optimize_flatten calls which create pre_optimize_flattenable nodes
    // Can cause execution to fail in fewer than 6 passes. We may want to re-evaluate our graph exeuction approach
    flow_c * c = flow_context_create();
    struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t first = flow_node_create_clone(c, &g, -1);
    int32_t second = flow_node_create_clone(c, &g, first);
    int32_t third = flow_node_create_clone(c, &g, second);
    flow_edge_create(c, &g, third, first, flow_edgetype_input); // make a cycle

    REQUIRE_FALSE(flow_graph_validate(c, g));
    REQUIRE(flow_context_error_reason(c) == flow_status_Graph_is_cyclic);

    flow_context_clear_error(c);

    int32_t fourth = flow_node_create_clone(c, &g, third);

    REQUIRE_FALSE(flow_graph_validate(c, g));
    REQUIRE(flow_context_error_reason(c) == flow_status_Graph_is_cyclic);

    flow_context_destroy(c);
}
TEST_CASE("test outbound edge validation", "")
{
    // This test case helped expose a flaw in graph creation, where we swapped max_edges and max_nodes and caused memory
    // overlap
    // It also showed how that post_optimize_flatten calls which create pre_optimize_flattenable nodes
    // Can cause execution to fail in fewer than 6 passes. We may want to re-evaluate our graph exeuction approach
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
