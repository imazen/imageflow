#include <png.h>
#include "catch.hpp"
#include "unistd.h"
#include <sys/types.h>
#include <sys/stat.h>
#include <errno.h>
#include <stdio.h>

#include "imageflow.h"

#include "imageflow_private.h"
#include "weighting_test_helpers.h"
#include "trim_whitespace.h"
#include "string.h"
#include "lcms2.h"
#include "png.h"
#include "curl/curl.h"
#include "curl/easy.h"
#include "helpers.h"

#define ERR(c) REQUIRE_FALSE(flow_context_print_and_exit_if_err(c))

// Assumes placeholders 0 and 1 for input/output respectively
bool execute_graph_for_url(flow_context* c, const char* input_image_url, const char* output_image_path,
                           struct flow_graph** graph_ref)
{
    struct flow_job* job = flow_job_create(c);
    ERR(c);
    flow_job_configure_recording(c, job, true, true, true, false, false);

    int32_t input_placeholder = 0;
    int32_t output_placeholder = 1;

    size_t bytes_count = 0;
    uint8_t* bytes = get_bytes_cached(c, &bytes_count, input_image_url);

    int32_t input_resource_id
        = flow_job_add_buffer(c, job, FLOW_INPUT, input_placeholder, (void*)bytes, bytes_count, false);

    int32_t result_resource_id = flow_job_add_buffer(c, job, FLOW_OUTPUT, output_placeholder, NULL, 0, true);

    if (!flow_job_insert_resources_into_graph(c, job, graph_ref)) {
        ERR(c);
    }
    if (!flow_job_execute(c, job, graph_ref)) {
        ERR(c);
    }

    struct flow_job_resource_buffer* result = flow_job_get_buffer(c, job, result_resource_id);

    ERR(c);

    REQUIRE(result != NULL);

    FILE* fh = fopen(output_image_path, "w");
    if (fh != NULL) {
        if (fwrite(result->buffer, result->buffer_size, 1, fh) != 1) {
            REQUIRE(false);
        }
    }
    fclose(fh);
    flow_job_destroy(c, job);
    return true;
}

bool execute_graph_for_bitmap_bgra(flow_context* c, flow_bitmap_bgra* input, flow_bitmap_bgra** out,
                                   struct flow_graph** graph_ref)
{
    struct flow_job* job = flow_job_create(c);
    ERR(c);
    flow_job_configure_recording(c, job, true, true, true, false, false);

    int32_t input_placeholder = 0;
    int32_t output_placeholder = 1;

    int32_t input_resource_id = flow_job_add_bitmap_bgra(c, job, FLOW_INPUT, input_placeholder, input);
    int32_t result_resource_id = flow_job_add_bitmap_bgra(c, job, FLOW_OUTPUT, output_placeholder, NULL);

    if (!flow_job_insert_resources_into_graph(c, job, graph_ref)) {
        ERR(c);
    }
    if (!flow_job_execute(c, job, graph_ref)) {
        ERR(c);
    }

    *out = flow_job_get_bitmap_bgra(c, job, result_resource_id);
    ERR(c);
    flow_job_destroy(c, job);
    return true;
}

TEST_CASE("create tiny graph", "")
{
    flow_context* c = flow_context_create();
    flow_graph* g = flow_graph_create(c, 10, 10, 200, 2.0);
    int32_t last;

    last = flow_node_create_canvas(c, &g, -1, flow_bgra32, 400, 300, 0xFFFFFFFF);
    last = flow_node_create_scale(c, &g, last, 300, 200);
    last = flow_node_create_resource_placeholder(c, &g, last, 0);

    ERR(c);

    REQUIRE(g->edges[0].from == 0);
    REQUIRE(g->edges[0].to == 1);
    REQUIRE(g->edge_count == 2);
    REQUIRE(g->node_count == 3);

    flow_context_destroy(c);
}

TEST_CASE("delete a node from a graph", "")
{
    flow_context* c = flow_context_create();
    flow_graph* g = flow_graph_create(c, 10, 10, 200, 2.0);
    int32_t last;

    last = flow_node_create_canvas(c, &g, -1, flow_bgra32, 400, 300, 0xFFFFFFFF);
    last = flow_node_create_scale(c, &g, last, 300, 200);
    last = flow_node_create_resource_placeholder(c, &g, last, 0);
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
    flow_context* c = flow_context_create();
    flow_graph* g = flow_graph_create(c, 10, 10, 200, 2.0);
    int32_t last;
    last = flow_node_create_canvas(c, &g, -1, flow_bgra32, 400, 300, 0xFFFFFFFF);
    last = flow_node_create_scale(c, &g, last, 300, 200);

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

    flow_context* c = flow_context_create();
    flow_utils_ensure_directory_exists("node_frames");
    struct flow_graph* g = nullptr;
    struct flow_job* job = nullptr;

    int32_t result_resource_id;
    flow_bitmap_bgra* result = nullptr;

    g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last;

    last = flow_node_create_canvas(c, &g, -1, flow_bgra32, 400, 300, 0xFFFFFFFF);
    //    last = flow_node_create_fill_rect()
    last = flow_node_create_scale(c, &g, last, 300, 200);
    last = flow_node_create_resource_placeholder(c, &g, last, 0);

    job = flow_job_create(c);
    ERR(c);

    result_resource_id = flow_job_add_bitmap_bgra(c, job, FLOW_OUTPUT, /* graph placeholder index */ 0, NULL);

    if (!flow_job_insert_resources_into_graph(c, job, &g)) {
        ERR(c);
    }
    REQUIRE(g->edges[2].from == 1);
    REQUIRE(g->edges[2].to == 3);

    if (!flow_job_execute(c, job, &g)) {
        ERR(c);
    }

    REQUIRE(result_resource_id == 2048);
    result = flow_job_get_bitmap_bgra(c, job, result_resource_id);

    ERR(c);

    REQUIRE(result != NULL);
    REQUIRE(result->w == 300);

    flow_context_destroy(c);
}

TEST_CASE("decode and scale png", "")
{

    flow_context* c = flow_context_create();
    struct flow_graph* g = nullptr;
    struct flow_job* job = nullptr;

    int32_t result_resource_id;
    flow_bitmap_bgra* result = nullptr;

    g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t input_placeholder = 0;
    int32_t output_placeholder = 1;

    int32_t last;
    last = flow_node_create_resource_placeholder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 300, 200);
    last = flow_node_create_resource_placeholder(c, &g, last, output_placeholder);

    job = flow_job_create(c);
    ERR(c);
    uint8_t image_bytes_literal[]
        = { 0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00,
            0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
            0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82 };

    int32_t input_resource_id = flow_job_add_buffer(c, job, FLOW_INPUT, input_placeholder,
                                                    (void*)&image_bytes_literal[0], sizeof(image_bytes_literal), false);

    result_resource_id = flow_job_add_bitmap_bgra(c, job, FLOW_OUTPUT, output_placeholder, NULL);

    if (!flow_job_insert_resources_into_graph(c, job, &g)) {
        ERR(c);
    }
    if (!flow_job_execute(c, job, &g)) {
        ERR(c);
    }

    result = flow_job_get_bitmap_bgra(c, job, result_resource_id);

    ERR(c);

    REQUIRE(result != NULL);
    REQUIRE(result->w == 300);

    flow_context_destroy(c);
}

bool scale_image_to_disk_inner(flow_context* c)
{
    // We'll create a simple graph
    struct flow_graph* g = flow_graph_create(c, 10, 10, 200, 2.0);
    if (g == NULL) {
        return false;
    }
    // We associate placeholders and resources with simple integers
    int32_t input_placeholder = 0;
    int32_t output_placeholder = 1;

    int32_t last;
    last = flow_node_create_resource_placeholder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 120, 120);
    last = flow_node_create_resource_placeholder(c, &g, last, output_placeholder);

    // We create a job to create I/O resources and attach them to our abstract graph above
    struct flow_job* job = flow_job_create(c);

    // We've done 4 mallocs, make sure we didn't run out of memory
    if (flow_context_has_error(c)) {
        return false;
    }

    // Uncomment to generate an animated graph of the process. Requires sudo apt-get install libav-tools graphviz
    // gifsicle
    // flow_job_configure_recording(c, job, true, true, true, true, true);

    // We're going to use an embedded image, but you could get bytes from anywhere
    uint8_t image_bytes_literal[]
        = { 0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00,
            0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
            0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82 };

    int32_t input_resource_id = flow_job_add_buffer(c, job, FLOW_INPUT, input_placeholder,
                                                    (void*)&image_bytes_literal[0], sizeof(image_bytes_literal), false);
    // Let's ask for an output buffer as a result
    int32_t result_resource_id = flow_job_add_buffer(c, job, FLOW_OUTPUT, output_placeholder, NULL, 0, true);

    // Insert resources into the graph
    if (!flow_job_insert_resources_into_graph(c, job, &g)) {
        return false;
    }
    struct flow_job_input_resource_info info;
    if (!flow_job_get_input_resource_info_by_placeholder_id(c, job, input_placeholder, &info)){
        return false;
    }
    REQUIRE(info.frame0_post_decode_format == flow_bgra32);
    REQUIRE(info.frame0_width == 1);
    REQUIRE(info.frame0_height == 1);
    REQUIRE(strcmp(info.preferred_extension, "png") == 0);
    REQUIRE(strcmp(info.preferred_mime_type, "image/png") == 0);
    REQUIRE(info.codec_type == flow_job_codec_type_decode_png);


    // Execute the graph
    if (!flow_job_execute(c, job, &g)) {
        return false;
    }
    // Access the output buffer
    struct flow_job_resource_buffer* result = flow_job_get_buffer(c, job, result_resource_id);
    // Now let's write it to disk
    FILE* fh = fopen("graph_scaled_png.png", "w");
    if (flow_context_has_error(c) || fh == NULL || fwrite(result->buffer, result->buffer_size, 1, fh) != 1) {
        if (fh != NULL)
            fclose(fh);
        return false;
    }
    fclose(fh);
    return true;
}

bool scale_image_to_disk()
{
    // The flow_context provides error tracking, profling, heap tracking.
    flow_context* c = flow_context_create();
    if (c == NULL) {
        return false;
    }
    if (!scale_image_to_disk_inner(c)) {
        flow_context_print_error_to(c, stderr);
        flow_context_destroy(c);
        return false;
    }

    flow_context_destroy(c);
    return true;
}

TEST_CASE("decode, scale, and re-encode png", "") { REQUIRE(scale_image_to_disk()); }

TEST_CASE("scale and flip and crop png", "")
{
    flow_context* c = flow_context_create();
    struct flow_graph* g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;

    last = flow_node_create_resource_placeholder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 120, 120);
    last = flow_node_create_primitive_flip_vertical(c, &g, last);
    last = flow_node_create_primitive_crop(c, &g, last, 20, 10, 80, 40);
    last = flow_node_create_resource_placeholder(c, &g, last, output_placeholder);

    execute_graph_for_url(c, "http://z.zr.io/ri/8s.jpg?format=png&width=800", "graph_flipped_cropped_png.png", &g);

    flow_context_destroy(c);
}

TEST_CASE("scale and flip and crop jpg", "")
{
    flow_context* c = flow_context_create();
    struct flow_graph* g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;

    last = flow_node_create_resource_placeholder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 120, 120);
    last = flow_node_create_primitive_flip_vertical(c, &g, last);
    last = flow_node_create_primitive_crop(c, &g, last, 20, 10, 80, 40);

    last = flow_node_create_encoder_placeholder(c, &g, last, output_placeholder, flow_job_codec_type_encode_jpeg);

    execute_graph_for_url(c, "http://z.zr.io/ri/8s.jpg?width=800", "graph_flipped_cropped_from_jpeg.jpg", &g);

    flow_context_destroy(c);
}

TEST_CASE("benchmark scaling large progressive jpg", "")
{
    flow_context* c = flow_context_create();
    struct flow_graph* g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;

    last = flow_node_create_resource_placeholder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 800, 800);
    last = flow_node_create_encoder_placeholder(c, &g, last, output_placeholder, flow_job_codec_type_encode_jpeg);

    execute_graph_for_url(c, "https://s3.amazonaws.com/resizer-dynamic-downloads/imageflow_test_suite/4kx4k.jpg",
                          "graph_large_jpeg.jpg", &g);

    flow_context_destroy(c);
}


TEST_CASE("benchmark scaling large jpg", "")
{
    flow_context* c = flow_context_create();
    struct flow_graph* g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;

    last = flow_node_create_resource_placeholder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 800, 800);
    last = flow_node_create_encoder_placeholder(c, &g, last, output_placeholder, flow_job_codec_type_encode_jpeg);

    execute_graph_for_url(c, "https://s3.amazonaws.com/resizer-dynamic-downloads/imageflow_test_suite/4kx4k_baseline.jpg",
                          "graph_large_jpeg.jpg", &g);

    flow_context_destroy(c);
}

TEST_CASE("Roundtrip flipping", "")
{
    flow_context* c = flow_context_create();
    struct flow_graph* g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;

    last = flow_node_create_resource_placeholder(c, &g, -1, input_placeholder);
    last = flow_node_create_clone(c, &g, last);
    last = flow_node_create_primitive_flip_vertical(c, &g, last);
    last = flow_node_create_primitive_flip_horizontal(c, &g, last);
    last = flow_node_create_primitive_flip_horizontal(c, &g, last);
    last = flow_node_create_primitive_flip_vertical(c, &g, last);
    last = flow_node_create_resource_placeholder(c, &g, last, output_placeholder);

    flow_bitmap_bgra* gradient = BitmapBgra_create_test_image(c);
    flow_bitmap_bgra* result;
    execute_graph_for_bitmap_bgra(c, gradient, &result, &g);

    ERR(c);
    bool equal = false;
    if (!flow_bitmap_bgra_compare(c, gradient, result, &equal)) {
        ERR(c);
    }
    REQUIRE(equal);

    flow_context_destroy(c);
}

TEST_CASE("scale copy rect", "")
{
    flow_context* c = flow_context_create();
    struct flow_graph* g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;

    last = flow_node_create_resource_placeholder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 200, 200);
    int32_t canvas = flow_node_create_canvas(c, &g, -1, flow_bgra32, 300, 300, 0);
    last = flow_node_create_primitive_copy_rect_to_canvas(c, &g, last, 0, 0, 150, 150, 50, 50);
    flow_edge_create(c, &g, canvas, last, flow_edgetype_canvas);
    last = flow_node_create_resource_placeholder(c, &g, last, output_placeholder);

    execute_graph_for_url(c, "http://z.zr.io/ri/8s.jpg?format=png&width=800", "graph_scaled_blitted_png.png", &g);

    flow_context_destroy(c);
}

TEST_CASE("test frame clone", "")
{
    flow_context* c = flow_context_create();
    struct flow_graph* g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t input_placeholder = 0, output_placeholder = 1;

    int32_t input = flow_node_create_resource_placeholder(c, &g, -1, input_placeholder);
    int32_t clone_a = flow_node_create_clone(c, &g, input);
    int32_t clone_b = flow_node_create_clone(c, &g, input);
    int32_t last = flow_node_create_primitive_flip_vertical(c, &g, clone_b); // mutate b, leave a alone
    flow_node_create_transpose(c, &g, last);

    flow_node_create_resource_placeholder(c, &g, clone_a, output_placeholder);

    execute_graph_for_url(c, "http://z.zr.io/ri/8s.jpg?format=png&width=400", "unflipped.png", &g);

    flow_context_destroy(c);
}

TEST_CASE("test rotation", "")
{
    flow_context* c = flow_context_create();
    struct flow_graph* g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t input_placeholder = 0, output_placeholder = 1;

    int32_t input = flow_node_create_resource_placeholder(c, &g, -1, input_placeholder);
    int32_t a = flow_node_create_rotate_90(c, &g, input);
    int32_t b = flow_node_create_rotate_180(c, &g, input);
    int32_t c_n = flow_node_create_rotate_270(c, &g, input);
    flow_node_create_resource_placeholder(c, &g, a, output_placeholder);

    execute_graph_for_url(c, "http://z.zr.io/ri/Oriented.jpg?format=png", "rotated.png", &g);

    flow_context_destroy(c);
}

TEST_CASE("test memory corruption", "")
{
    // This test case helped expose a flaw in graph creation, where we swapped max_edges and max_nodes and caused memory
    // overlap
    // It also showed how that post_optimize_flatten calls which create pre_optimize_flattenable nodes
    // Can cause execution to fail in fewer than 6 passes. We may want to re-evaluate our graph exeuction approach
    flow_context* c = flow_context_create();
    struct flow_graph* g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t input_placeholder = 0, output_placeholder = 1;

    int32_t input = flow_node_create_resource_placeholder(c, &g, -1, input_placeholder);
    int32_t clone_a = flow_node_create_clone(c, &g, input);
    clone_a = flow_node_create_rotate_90(c, &g, clone_a);
    int32_t clone_b = flow_node_create_clone(c, &g, input);
    clone_b = flow_node_create_rotate_180(c, &g, clone_b);
    int32_t clone_c = flow_node_create_clone(c, &g, input);
    clone_c = flow_node_create_rotate_270(c, &g, clone_c);
    flow_node_create_resource_placeholder(c, &g, clone_a, output_placeholder);

    execute_graph_for_url(c, "http://z.zr.io/ri/Oriented.jpg?format=png", "rotated.png", &g);

    flow_context_destroy(c);
}

TEST_CASE("check for cycles", "")
{
    // This test case helped expose a flaw in graph creation, where we swapped max_edges and max_nodes and caused memory
    // overlap
    // It also showed how that post_optimize_flatten calls which create pre_optimize_flattenable nodes
    // Can cause execution to fail in fewer than 6 passes. We may want to re-evaluate our graph exeuction approach
    flow_context* c = flow_context_create();
    struct flow_graph* g = flow_graph_create(c, 10, 10, 200, 2.0);
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

