#include <glenn/png/png.h>
#include "catch.hpp"
#include "unistd.h"
#include <sys/types.h>
#include <sys/stat.h>
#include <errno.h>
#include <stdio.h>

#include "imageflow.h"

#include "fastscaling_private.h"
#include "weighting_test_helpers.h"
#include "trim_whitespace.h"
#include "string.h"
#include "lcms2.h"
#include "png.h"
#include "curl/curl.h"
#include "curl/easy.h"
#include "helpers.h"

#define ERR(c) REQUIRE_FALSE(Context_print_and_exit_if_err(c))

TEST_CASE ("create tiny graph", "")
{
    Context * c = Context_create();
    flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    int32_t last;

    last = flow_node_create_canvas(c, &g, -1, Bgra32, 400, 300, 0xFFFFFFFF);
    last = flow_node_create_scale(c, &g, last, 300, 200);
    last = flow_node_create_resource_placeholder(c, &g, last, 0);

    ERR(c);

    REQUIRE(g->edges[0].from == 0);
    REQUIRE(g->edges[0].to == 1);
    REQUIRE(g->edge_count == 2);
    REQUIRE(g->node_count == 3);


    flow_graph_destroy(c, g);
    Context_destroy(c);
}

TEST_CASE ("delete a node from a graph", "")
{
    Context * c = Context_create();
    flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    int32_t last;


    last = flow_node_create_canvas(c, &g, -1, Bgra32, 400, 300, 0xFFFFFFFF);
    last = flow_node_create_scale(c, &g, last, 300, 200);
    last = flow_node_create_resource_placeholder(c, &g, last, 0);
    ERR(c);

    REQUIRE(g->edges[0].from == 0);
    REQUIRE(g->edges[0].to == 1);
    REQUIRE(g->edges[1].from == 1);
    REQUIRE(g->edges[1].to == 2);
    REQUIRE(g->edge_count == 2);
    REQUIRE(g->node_count == 3);

    flow_node_delete(c,g,last);
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



    flow_graph_destroy(c, g);
    Context_destroy(c);
}


TEST_CASE ("clone an edge", "")
{
    Context * c = Context_create();
    flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
    int32_t last;
    last = flow_node_create_canvas(c, &g, -1, Bgra32, 400, 300, 0xFFFFFFFF);
    last = flow_node_create_scale(c, &g, last, 300, 200);

    ERR(c);

    REQUIRE(g->edges[0].from == 0);
    REQUIRE(g->edges[0].to == 1);
    REQUIRE(g->edge_count == 1);
    REQUIRE(g->node_count == 2);

    flow_edge_duplicate(c,&g, 0);

    ERR(c);

    REQUIRE(g->edge_count == 2);
    REQUIRE(g->node_count == 2);
    REQUIRE(g->edges[1].from == 0);
    REQUIRE(g->edges[1].to == 1);


    flow_graph_destroy(c, g);
    Context_destroy(c);
}


//TODO test paths where adding nodes/edges exceeds the max size



TEST_CASE("execute tiny graph", "")
{


    Context * c = Context_create();
    flow_utils_ensure_directory_exists( "node_frames");
    struct flow_graph *g = nullptr;
    struct flow_job *job = nullptr;

    int32_t result_resource_id;
    BitmapBgra * result = nullptr;

    g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last;

    last = flow_node_create_canvas(c, &g, -1, Bgra32, 400, 300, 0xFFFFFFFF);
    last = flow_node_create_scale(c, &g, last, 300, 200);
    last = flow_node_create_resource_placeholder(c, &g, last, 0);

    job = flow_job_create(c);
    ERR(c);

    result_resource_id = flow_job_add_bitmap_bgra(c,job, FLOW_OUTPUT, /* graph placeholder index */ 0);


    if (!flow_job_insert_resources_into_graph(c, job, &g)){
        ERR(c);
    }
    REQUIRE(g->edges[2].from == 1);
    REQUIRE(g->edges[2].to == 3);


    if (!flow_job_execute(c, job, &g)){
        ERR(c);
    }

    REQUIRE(result_resource_id == 2048);
    result = flow_job_get_bitmap_bgra(c, job, result_resource_id);

    ERR(c);


    REQUIRE(result != NULL);
    REQUIRE(result->w == 300);


    BitmapBgra_destroy(c,result);
    flow_job_destroy(c,job);
    flow_graph_destroy(c, g);
    Context_destroy(c);
}



TEST_CASE("decode and scale png", "")
{

    Context * c = Context_create();
    struct flow_graph *g = nullptr;
    struct flow_job *job = nullptr;

    int32_t result_resource_id;
    BitmapBgra * result = nullptr;

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
    uint8_t image_bytes_literal[] = {0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82};


    int32_t input_resource_id = flow_job_add_buffer(c,job, FLOW_INPUT, input_placeholder, (void*) &image_bytes_literal[0], sizeof(image_bytes_literal), false);


    result_resource_id = flow_job_add_bitmap_bgra(c,job, FLOW_OUTPUT, output_placeholder);


    if (!flow_job_insert_resources_into_graph(c, job, &g)){
        ERR(c);
    }
    if (!flow_job_execute(c, job, &g)){
        ERR(c);
    }

    result = flow_job_get_bitmap_bgra(c, job, result_resource_id);

    ERR(c);


    REQUIRE(result != NULL);
    REQUIRE(result->w == 300);


    BitmapBgra_destroy(c,result);
    flow_job_destroy(c,job);
    flow_graph_destroy(c, g);
    Context_destroy(c);
}

TEST_CASE("decode, scale, and re-encode png", "")
{

    Context * c = Context_create();
    struct flow_graph *g = nullptr;
    struct flow_job *job = nullptr;

    int32_t result_resource_id;
    struct flow_job_resource_buffer * result = nullptr;

    g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);


    int32_t input_placeholder = 0;
    int32_t output_placeholder = 1;

    int32_t last;
    last = flow_node_create_resource_placeholder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 120, 120);
    last = flow_node_create_resource_placeholder(c, &g, last, output_placeholder);



    job = flow_job_create(c);
    ERR(c);
    flow_job_configure_recording(c, job, true, true, true, false, false);


    size_t bytes_count = 0;

    uint8_t * bytes = get_bytes_cached("http://z.zr.io/ri/8s.jpg?format=png&width=800", &bytes_count);

    int32_t input_resource_id = flow_job_add_buffer(c,job, FLOW_INPUT, input_placeholder, (void*) bytes, bytes_count, false);


    result_resource_id = flow_job_add_buffer(c,job, FLOW_OUTPUT, output_placeholder, NULL, 0, true);


    if (!flow_job_insert_resources_into_graph(c, job, &g)){
        ERR(c);
    }
    if (!flow_job_execute(c, job, &g)){
        ERR(c);
    }

    result = flow_job_get_buffer(c, job, result_resource_id);

    ERR(c);


    REQUIRE(result != NULL);

    FILE *fh = fopen("graph_scaled_png.png", "w");
    if ( fh != NULL ) {
        if (fwrite(result->buffer, result->buffer_size, 1,fh) != 1){
            REQUIRE(false);
        }
    }
    fclose(fh);


    flow_job_destroy(c,job);
    flow_graph_destroy(c, g);
    Context_destroy(c);
}

//Assumes placeholders 0 and 1 for input/output respectively
bool execute_graph_for_url(Context * c, const char * input_image_url, const char * output_image_path, struct flow_graph ** graph_ref){
    struct flow_job * job = flow_job_create(c);
    ERR(c);
    flow_job_configure_recording(c, job, true, true, true, false, false);

    int32_t input_placeholder = 0;
    int32_t output_placeholder = 1;


    size_t bytes_count = 0;
    uint8_t * bytes = get_bytes_cached(input_image_url, &bytes_count);

    int32_t input_resource_id = flow_job_add_buffer(c,job, FLOW_INPUT, input_placeholder, (void*) bytes, bytes_count, false);


    int32_t result_resource_id = flow_job_add_buffer(c,job, FLOW_OUTPUT, output_placeholder, NULL, 0, true);


    if (!flow_job_insert_resources_into_graph(c, job, graph_ref)){
        ERR(c);
    }
    if (!flow_job_execute(c, job, graph_ref)){
        ERR(c);
    }

    struct flow_job_resource_buffer * result = flow_job_get_buffer(c, job, result_resource_id);

    ERR(c);

    REQUIRE(result != NULL);

    FILE *fh = fopen(output_image_path, "w");
    if ( fh != NULL ) {
        if (fwrite(result->buffer, result->buffer_size, 1,fh) != 1){
            REQUIRE(false);
        }
    }
    fclose(fh);
    flow_job_destroy(c,job);
    return true;
}

TEST_CASE("scale and flip and crop png", "")
{
    Context * c = Context_create();
    struct flow_graph *g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;

    last = flow_node_create_resource_placeholder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 120, 120);
    last = flow_node_create_primitive_flip_vertical(c, &g, last);
    last = flow_node_create_primitive_crop(c, &g, last, 20, 10, 80, 40);
    last = flow_node_create_resource_placeholder(c, &g, last, output_placeholder);

    execute_graph_for_url(c, "http://z.zr.io/ri/8s.jpg?format=png&width=800", "graph_flipped_cropped_png.png", &g);

    flow_graph_destroy(c, g);
    Context_destroy(c);
}


TEST_CASE("scale copy rect", "")
{
    Context * c = Context_create();
    struct flow_graph *g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t last, input_placeholder = 0, output_placeholder = 1;

    last = flow_node_create_resource_placeholder(c, &g, -1, input_placeholder);
    last = flow_node_create_scale(c, &g, last, 200, 200);
    int32_t canvas = flow_node_create_canvas(c, &g, -1, Bgra32, 300,300, 0);
    last = flow_node_create_primitive_copy_rect_to_canvas(c, &g, last, 0,0,150,150, 50,50);
    flow_edge_create(c, &g,canvas, last, flow_edgetype_canvas );
    last = flow_node_create_resource_placeholder(c, &g, last, output_placeholder);

    execute_graph_for_url(c, "http://z.zr.io/ri/8s.jpg?format=png&width=800", "graph_scaled_blitted_png.png", &g);

    flow_graph_destroy(c, g);
    Context_destroy(c);
}

TEST_CASE("test frame clone", "")
{
    Context * c = Context_create();
    struct flow_graph *g = flow_graph_create(c, 10, 10, 200, 2.0);
    ERR(c);

    int32_t input_placeholder = 0, output_placeholder = 1;

    int32_t input = flow_node_create_resource_placeholder(c, &g, -1, input_placeholder);
    int32_t clone_a  = flow_node_create_clone(c, &g, input);
    int32_t clone_b  = flow_node_create_clone(c, &g, input);
    flow_node_create_primitive_flip_vertical(c,&g,clone_b); //mutate b, leave a alone

    flow_node_create_resource_placeholder(c, &g, clone_a, output_placeholder);

    execute_graph_for_url(c, "http://z.zr.io/ri/8s.jpg?format=png&width=400", "unflipped.png", &g);

    flow_graph_destroy(c, g);
    Context_destroy(c);
}
