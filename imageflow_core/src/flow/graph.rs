use ffi::*;
use libc::{self, int32_t,uint32_t};
use std::ffi::CStr;
use petgraph;
use daggy::{Dag,EdgeIndex,NodeIndex};


pub type Graph = Dag<::flow::definitions::Node,EdgeKind>;

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

pub fn edge_create(c: *mut Context,
                   g: &mut Graph,
                   from: i32,
                   to: i32,
                   kind: EdgeKind) -> i32 {
  //edges are nodeindex, not just u32
  //FIXME: error management. We should return something else than i32
  // we should also get index as U32 instead of i32
  g.add_edge(NodeIndex::new(from as usize), NodeIndex::new(to as usize), kind).unwrap_or(EdgeIndex::new(0usize)).index() as i32
}

pub fn node_create_decoder(c: *mut Context,
                                g: &mut Graph,
                                prev_node: i32,
                                placeholder_id: i32) -> i32 {
    /*
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_decoder);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }

    struct flow_nodeinfo_codec * info = (struct flow_nodeinfo_codec *)flow_node_get_info_pointer(*g, id);
    info->placeholder_id = placeholder_id;
    info->codec = NULL;
    info->downscale_hints.downscale_if_wider_than = -1;
    info->downscale_hints.or_if_taller_than = -1;
    info->downscale_hints.downscaled_min_height = -1;
    info->downscale_hints.downscaled_min_width = -1;
    info->downscale_hints.gamma_correct_for_srgb_during_spatial_luma_scaling = false;
    info->downscale_hints.scale_luma_spatially = false;
    info->encoder_hints.jpeg_encode_quality = 0;
    info->encoder_hints.disable_png_alpha = false;
    return id;
*/
    0
}

pub fn node_create_canvas(c: *mut Context,
                                   g: &mut Graph,
                                   prev_node: i32,
                                   format: PixelFormat,
                                   width: usize,
                                   height: usize,
                                   bgcolor: u32)
                                   -> i32 {
    0
}

pub fn node_create_scale(c: *mut Context,
                                  g: &mut Graph,
                                  prev_node: i32,
                                  width: usize,
                                  height: usize,
                                  downscale_filter: i32,
                                  upscale_filter: i32,
                                  flags: usize,
                                  sharpen: f32)
                                  -> i32 {
    0
}

pub fn node_create_expand_canvas(c: *mut Context,
                                          g: &mut Graph,
                                          prev_node: i32,
                                          left: u32,
                                          top: u32,
                                          right: u32,
                                          bottom: u32,
                                          canvas_color_srgb: u32)
                                          -> i32 {
    0
}

pub fn node_create_fill_rect(c: *mut Context,
                                      g: &mut Graph,
                                      prev_node: i32,
                                      x1: u32,
                                      y1: u32,
                                      x2: u32,
                                      y2: u32,
                                      color_srgb: u32)
                                      -> i32 { 0 }

pub fn node_create_bitmap_bgra_reference(c: *mut Context,
                                              g: &mut Graph,
                                              prev_node: i32, reference: *mut *mut FlowBitmapBgra) -> i32 { 0 }
pub fn node_create_rotate_90(c: *mut Context, g: &mut Graph, prev_node: i32) -> i32 { 0 }
pub fn node_create_rotate_180(c: *mut Context, g: &mut Graph, prev_node: i32) -> i32 { 0 }
pub fn node_create_rotate_270(c: *mut Context, g: &mut Graph, prev_node: i32) -> i32 { 0 }

pub fn node_create_transpose(c: *mut Context, g: &mut Graph, prev_node: i32) -> i32 { 0 }

pub fn node_create_primitive_copy_rect_to_canvas(c: *mut Context,
                                                          g: &mut Graph,
                                                          prev_node: i32,
                                                          from_x: u32,
                                                          from_y: u32,
                                                          width: u32,
                                                          height: u32,
                                                          x: u32,
                                                          y: u32)
                                                          -> i32 { 0 }

pub fn node_create_encoder(c: *mut Context,
                                    g: &mut Graph,
                                    prev_node: i32,
                                    placeholder_id: i32,
                                    desired_encoder_id: i64,
                                    hints: *const EncoderHints)
                                    -> i32 {
/*
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_encoder);
    if (id < 0) {
        FLOW_add_to_callstack(c);
        return id;
    }

    struct flow_nodeinfo_codec * info = (struct flow_nodeinfo_codec *)flow_node_get_info_pointer(*g, id);
    info->placeholder_id = placeholder_id;
    info->codec = NULL;
    info->desired_encoder_id = desired_encoder_id;

    info->encoder_hints.jpeg_encode_quality = 90;
    info->encoder_hints.disable_png_alpha = false;
    if (hints != NULL) {
        memcpy(&info->encoder_hints, hints, sizeof(struct flow_encoder_hints));
    }
    return id;
*/
    0
}

pub fn node_create_primitive_flip_vertical(c: *mut Context,
                                                    g: &mut Graph,
                                                    prev_node: i32)
                                                    -> i32 { 0 }

pub fn node_create_primitive_flip_horizontal(c: *mut Context,
                                                      g: &mut Graph,
                                                      prev_node: i32)
                                                      -> i32 { 0 }

pub fn node_create_primitive_crop(c: *mut Context,
                                           g: &mut Graph,
                                           prev_node: i32,
                                           x1: u32,
                                           y1: u32,
                                           x2: u32,
                                           y2: u32)
                                           -> i32 { 0 }

extern "C" {
/*
    fn flow_graph_print_to_stdout(c: *mut Context, g: *const Graph) -> bool;

    fn flow_graph_create(context: *mut Context,
                             max_edges: u32,
                             max_nodes: u32,
                             max_info_bytes: u32,
                             growth_factor: f32)
                             -> *mut Graph;


    fn flow_edge_create(c: *mut Context,
                            g: *mut *mut Graph,
                            from: i32,
                            to: i32,
                            kind: EdgeKind)
                            -> i32;
    fn flow_node_create_decoder(c: *mut Context,
                                    g: *mut *mut Graph,
                                    prev_node: i32,
                                    placeholder_id: i32)
                                    -> i32;
    fn flow_node_create_canvas(c: *mut Context,
                                   g: *mut *mut Graph,
                                   prev_node: i32,
                                   format: PixelFormat,
                                   width: usize,
                                   height: usize,
                                   bgcolor: u32)
                                   -> i32;

    fn flow_node_create_scale(c: *mut Context,
                                  g: *mut *mut Graph,
                                  prev_node: i32,
                                  width: usize,
                                  height: usize,
                                  downscale_filter: i32,
                                  upscale_filter: i32,
                                  flags: usize,
                                  sharpen: f32)
                                  -> i32;

    fn flow_node_create_expand_canvas(c: *mut Context,
                                          g: *mut *mut Graph,
                                          prev_node: i32,
                                          left: u32,
                                          top: u32,
                                          right: u32,
                                          bottom: u32,
                                          canvas_color_srgb: u32)
                                          -> i32;

    fn flow_node_create_fill_rect(c: *mut Context,
                                      g: *mut *mut Graph,
                                      prev_node: i32,
                                      x1: u32,
                                      y1: u32,
                                      x2: u32,
                                      y2: u32,
                                      color_srgb: u32)
                                      -> i32;

pub fn flow_node_create_bitmap_bgra_reference(c: *mut Context,
                                              g: *mut *mut Graph,
                                              prev_node: i32, reference: *mut *mut FlowBitmapBgra) -> i32 { 0 }
    fn flow_node_create_rotate_90(c: *mut Context, g: *mut *mut Graph, prev_node: i32) -> i32;
    fn flow_node_create_rotate_180(c: *mut Context, g: *mut *mut Graph, prev_node: i32) -> i32;
    fn flow_node_create_rotate_270(c: *mut Context, g: *mut *mut Graph, prev_node: i32) -> i32;

    fn flow_node_create_transpose(c: *mut Context, g: *mut *mut Graph, prev_node: i32) -> i32;

    fn flow_node_create_primitive_copy_rect_to_canvas(c: *mut Context,
                                                          g: *mut *mut Graph,
                                                          prev_node: i32,
                                                          from_x: u32,
                                                          from_y: u32,
                                                          width: u32,
                                                          height: u32,
                                                          x: u32,
                                                          y: u32)
                                                          -> i32;

    fn flow_node_create_encoder(c: *mut Context,
                                    g: *mut *mut Graph,
                                    prev_node: i32,
                                    placeholder_id: i32,
                                    desired_encoder_id: i64,
                                    hints: *const EncoderHints)
                                    -> i32;

    fn flow_node_create_primitive_flip_vertical(c: *mut Context,
                                                    g: *mut *mut Graph,
                                                    prev_node: i32)
                                                    -> i32;

    fn flow_node_create_primitive_flip_horizontal(c: *mut Context,
                                                      g: *mut *mut Graph,
                                                      prev_node: i32)
                                                      -> i32;

    fn flow_node_create_primitive_crop(c: *mut Context,
                                           g: *mut *mut Graph,
                                           prev_node: i32,
                                           x1: u32,
                                           y1: u32,
                                           x2: u32,
                                           y2: u32)
                                           -> i32;
*/
}

