//Test writing to physical file


//
//// Port priority 2
//TEST_CASE("Test reading from memory, writing to file (test flow_io through job)", "")
//{
//flow_c * c = flow_context_create();
//struct flow_graph * g = nullptr;
//struct flow_job * job = nullptr;
//struct flow_bitmap_bgra * result = nullptr;
//
//g = flow_graph_create(c, 10, 10, 200, 2.0);
//ERR(c);
//
//i32 input_placeholder = 0;
//
//i32 last;
//last = flow_node_create_decoder(c, &g, -1, input_placeholder);
//last = flow_node_create_scale(c, &g, last, 300, 200, (flow_interpolation_filter_Robidoux),
//(flow_interpolation_filter_Robidoux), 0, 0);
//last = flow_node_create_encoder(c, &g, last, 1, flow_codec_type_encode_png, NULL);
//
//job = flow_job_create(c);
//ERR(c);
//uint8_t image_bytes_literal[]
//= { 0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00,
//0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00,
//0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
//0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82 };
//
//struct flow_io * input_io = flow_io_create_from_memory(c, flow_io_mode_read_seekable, &image_bytes_literal[0],
//sizeof(image_bytes_literal), job, NULL);
//
//if (!flow_job_add_io(c, job, input_io, input_placeholder, FLOW_INPUT)) {
//ERR(c);
//}
//struct flow_io * output_io = flow_io_create_for_file(c, flow_io_mode_write_seekable, "test_io.png", job);
//
//if (!flow_job_add_io(c, job, output_io, 1, FLOW_OUTPUT)) {
//ERR(c);
//}
//
//if (!flow_job_execute(c, job, &g)) {
//ERR(c);
//}
//
//ERR(c);
//
//flow_context_destroy(c);
//c = NULL;
//g = NULL;
//last = -1;
//job = NULL;
//input_io = NULL;
//
//c = flow_context_create();
//
//g = flow_graph_create(c, 10, 10, 200, 2.0);
//ERR(c);
//
//last = flow_node_create_decoder(c, &g, -1, input_placeholder);
//last = flow_node_create_scale(c, &g, last, 300, 200, (flow_interpolation_filter_Robidoux),
//(flow_interpolation_filter_Robidoux), 0, 0);
//last = flow_node_create_bitmap_bgra_reference(c, &g, last, &result);
//
//job = flow_job_create(c);
//ERR(c);
//input_io = flow_io_create_for_file(c, flow_io_mode_read_seekable, "test_io.png", job);
//
//if (!flow_job_add_io(c, job, input_io, input_placeholder, FLOW_INPUT)) {
//ERR(c);
//}
//
//if (!flow_job_execute(c, job, &g)) {
//ERR(c);
//}
//
//REQUIRE(result != NULL);
//REQUIRE(result->w == 300);
//
//// unlink ("test_io.png");
//
//flow_context_destroy(c);
//}

//TODO: decode gif frame
//export gif frames to disk (even if offset deltas)
