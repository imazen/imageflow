
use daggy::{Dag, EdgeIndex, NodeIndex};
use ffi::*;
use libc::{self, int32_t, uint32_t};
use petgraph;
use std::ffi::CStr;


pub type Graph = Dag<::flow::definitions::Node, EdgeKind>;

pub fn print_to_stdout(c: *mut Context, g: &Graph) -> bool {
    true
}

pub fn create(context: *mut Context,
              max_edges: u32,
              max_nodes: u32,
              max_info_bytes: u32,
              growth_factor: f32)
              -> Graph {
    Graph::with_capacity(max_nodes as usize, max_edges as usize)
}

pub fn node_create_decoder(c: *mut Context,
                           g: &mut Graph,
                           prev_node: i32,
                           placeholder_id: i32)
                           -> i32 {
    // int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_decoder);
    // if (id < 0) {
    // FLOW_add_to_callstack(c);
    // return id;
    // }
    //
    // struct flow_nodeinfo_codec * info = (struct flow_nodeinfo_codec *)flow_node_get_info_pointer(*g, id);
    // info->placeholder_id = placeholder_id;
    // info->codec = NULL;
    // info->downscale_hints.downscale_if_wider_than = -1;
    // info->downscale_hints.or_if_taller_than = -1;
    // info->downscale_hints.downscaled_min_height = -1;
    // info->downscale_hints.downscaled_min_width = -1;
    // info->downscale_hints.gamma_correct_for_srgb_during_spatial_luma_scaling = false;
    // info->downscale_hints.scale_luma_spatially = false;
    // info->encoder_hints.jpeg_encode_quality = 0;
    // info->encoder_hints.disable_png_alpha = false;
    // return id;
    //
    0
}

pub fn node_create_encoder(c: *mut Context,
                           g: &mut Graph,
                           prev_node: i32,
                           placeholder_id: i32,
                           desired_encoder_id: i64,
                           hints: *const EncoderHints)
                           -> i32 {
    // int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_encoder);
    // if (id < 0) {
    // FLOW_add_to_callstack(c);
    // return id;
    // }
    //
    // struct flow_nodeinfo_codec * info = (struct flow_nodeinfo_codec *)flow_node_get_info_pointer(*g, id);
    // info->placeholder_id = placeholder_id;
    // info->codec = NULL;
    // info->desired_encoder_id = desired_encoder_id;
    //
    // info->encoder_hints.jpeg_encode_quality = 90;
    // info->encoder_hints.disable_png_alpha = false;
    // if (hints != NULL) {
    // memcpy(&info->encoder_hints, hints, sizeof(struct flow_encoder_hints));
    // }
    // return id;
    //
    0
}
