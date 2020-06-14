

//
//// Port priority 2
//TEST_CASE("Test graph with 3 nodes pulling from decoder (smoke test)", "")
//{
//// This test case helped expose a flaw in graph creation, where we swapped max_edges and max_nodes and caused memory
//// overlap
//// It also showed how that post_optimize_flatten calls which create pre_optimize_flattenable nodes
//// Can cause execution to fail in fewer than 6 passes. We may want to re-evaluate our graph execution approach
//flow_c * c = flow_context_create();
//struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
//ERR(c);
//
//i32 input_placeholder = 0, output_placeholder = 1;
//
//i32 input = flow_node_create_decoder(c, &g, -1, input_placeholder);
//i32 clone_a = flow_node_create_clone(c, &g, input);
//clone_a = flow_node_create_rotate_90(c, &g, clone_a);
//i32 clone_b = flow_node_create_clone(c, &g, input);
//clone_b = flow_node_create_rotate_180(c, &g, clone_b);
//i32 clone_c = flow_node_create_clone(c, &g, input);
//clone_c = flow_node_create_rotate_270(c, &g, clone_c);
//flow_node_create_encoder_placeholder(c, &g, clone_a, output_placeholder);
//
//execute_graph_for_url(c, "http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/orientation.png",
//"rotated.png", &g);
//
//flow_context_destroy(c);
//}
//
//// Port priority 2
//TEST_CASE(
//"Verify origin nodes (like decoders) are prevented from having inputs; encoder nodes can't have more than 1 input",
//"")
//{
//flow_c * c = flow_context_create();
//struct flow_graph * g = flow_graph_create(c, 10, 10, 200, 2.0);
//ERR(c);
//
//struct flow_bitmap_bgra * p;
//i32 first = flow_node_create_bitmap_bgra_reference(c, &g, -1, &p);
//i32 encoder = flow_node_create_encoder_placeholder(c, &g, first, 0);
//i32 second = flow_node_create_clone(c, &g, encoder);
//REQUIRE_FALSE(flow_context_has_error(c));
//REQUIRE_FALSE(flow_graph_validate(c, g));
//REQUIRE(flow_context_error_reason(c) == flow_status_Graph_invalid);
//
//flow_context_clear_error(c);
//// Remove the invalid outbound node & edge
//flow_node_delete(c, g, second);
//
//// Canvas input not permitted to encoder
//i32 canvas = flow_node_create_clone(c, &g, -1);
//flow_edge_create(c, &g, canvas, encoder, flow_edgetype_canvas);
//// We shouldn't have an error until we call validate
//REQUIRE_FALSE(flow_context_has_error(c));
//REQUIRE_FALSE(flow_graph_validate(c, g));
//REQUIRE(flow_context_error_reason(c) == flow_status_Invalid_inputs_to_node);
//
//flow_context_destroy(c);
//}
