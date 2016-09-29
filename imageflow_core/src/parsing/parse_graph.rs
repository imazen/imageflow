extern crate imageflow_serde as s;
use ffi::EdgeKind;
use std;
use std::collections::HashMap;

extern crate rustc_serialize;
use parsing::rustc_serialize::hex::FromHex;


pub struct GraphTranslator {
    ctx: *mut ::ffi::Context,
}

impl GraphTranslator {
    pub fn new(ctx: *mut ::ffi::Context) -> GraphTranslator {
        GraphTranslator { ctx: ctx }
    }

    fn color_to_i32(&self, c: s::Color) -> std::result::Result<u32, std::num::ParseIntError> {
        match c {
            s::Color::Srgb(srgb) => {
                match srgb {
                    s::ColorSrgb::Hex(hex_srgb) => u32::from_str_radix(hex_srgb.as_str(), 16),
                }
            }
        }
    }

    unsafe fn create_node(&self, g: &mut ::flow::graph::Graph, node: s::Node) -> i32 {
        match node {

            s::Node::Decode { io_id } => ::flow::graph::node_create_decoder(self.ctx, g, -1, io_id),
            s::Node::Encode { io_id, encoder_id, encoder, hints: _ } => {
                let encoder_id = encoder_id.unwrap_or(match encoder.unwrap_or(s::Encoder::Png) {
                    s::Encoder::Jpeg => 4,
                    s::Encoder::Png => 2,
                });
                let encoder_hints = ::ffi::EncoderHints {
                    jpeg_quality: 100,
                    disable_png_alpha: false,
                };



                ::flow::graph::node_create_encoder(self.ctx,
                                                g,
                                                -1,
                                                io_id,
                                                encoder_id,
                                                &encoder_hints as *const ::ffi::EncoderHints)
            }
            s::Node::Crop { x1, y1, x2, y2 } => {
                ::flow::graph::node_create_primitive_crop(self.ctx, g, -1, x1, y1, x2, y2)
            }
            s::Node::FlipV => ::flow::graph::node_create_primitive_flip_vertical(self.ctx, g, -1),
            s::Node::FlipH => ::flow::graph::node_create_primitive_flip_horizontal(self.ctx, g, -1),
            s::Node::Rotate90 => ::flow::graph::node_create_rotate_90(self.ctx, g, -1),
            s::Node::Rotate180 => ::flow::graph::node_create_rotate_180(self.ctx, g, -1),
            s::Node::Rotate270 => ::flow::graph::node_create_rotate_270(self.ctx, g, -1),
            s::Node::CreateCanvas { format, w, h, color } => {
                let ffi_format = match format {
                    s::PixelFormat::Bgr24 => ::flow::graph::PixelFormat::BGR24,
                    s::PixelFormat::Bgra32 => ::flow::graph::PixelFormat::BGRA32,
                    s::PixelFormat::Gray8 => ::flow::graph::PixelFormat::Gray8,
                };

                ::flow::graph::node_create_canvas(self.ctx,
                                               g,
                                               -1,
                                               ffi_format,
                                               w,
                                               h,
                                               self.color_to_i32(color).unwrap())
            }
            s::Node::CopyRectToCanvas { from_x, from_y, width, height, x, y } => {
                ::flow::graph::node_create_primitive_copy_rect_to_canvas(self.ctx,
                                                                      g,
                                                                      -1,
                                                                      from_x,
                                                                      from_y,
                                                                      width,
                                                                      height,
                                                                      x,
                                                                      y)
            }
            s::Node::Transpose => ::flow::graph::node_create_transpose(self.ctx, g, -1),
            s::Node::ExpandCanvas { left, top, right, bottom, color } => {
                ::flow::graph::node_create_expand_canvas(self.ctx,
                                                      g,
                                                      -1,
                                                      left,
                                                      top,
                                                      right,
                                                      bottom,
                                                      self.color_to_i32(color).unwrap())
            }
            s::Node::Scale{ w, h, down_filter, up_filter,
                sharpen_percent, flags} => {
                ::flow::graph::node_create_scale(self.ctx, g, -1, w, h, down_filter.unwrap_or(s::Filter::RobidouxSharp) as i32, up_filter.unwrap_or(s::Filter::Ginseng) as i32,  flags.unwrap_or(1), sharpen_percent.unwrap_or(0f32) )
            }
            s::Node::FillRect { x1, x2, y1, y2, color } => {
                ::flow::graph::node_create_fill_rect(self.ctx,
                                                  g,
                                                  -1,
                                                  x1,
                                                  y1,
                                                  x2,
                                                  y2,
                                                  self.color_to_i32(color).unwrap())
            },


            s::Node::FlowBitmapBgraPtr {ptr_to_flow_bitmap_bgra_ptr} => {
                let ptr_to_ptr = ptr_to_flow_bitmap_bgra_ptr as *mut *mut ::ffi::FlowBitmapBgra;
                ::ffi::flow_node_create_bitmap_bgra_reference(self.ctx, g, -1, ptr_to_ptr)
            }

        }
    }


    unsafe fn create_edge(&self,
                          g: &mut ::flow::graph::Graph,
                          from_node: i32,
                          to_node: i32,
                          edge_kind: ::ffi::EdgeKind)
                          -> i32 {
        ::flow::graph::edge_create(self.ctx, g, from_node, to_node, edge_kind)
    }

    pub unsafe fn translate_graph(&self, from: s::Graph) -> ::flow::graph::Graph {
        let mut g = ::flow::graph::create(self.ctx, 10, 10, 3000, 2.0f32);

        let mut node_id_map: HashMap<i32, i32> = HashMap::new();

        for (old_id, node) in from.nodes {
            let new_id = self.create_node(&mut g, node);
            if new_id < 0 {
                panic!("node creation failed");
            }
            node_id_map.insert(old_id.parse::<i32>().unwrap(), new_id);
        }

        for edge in from.edges {
            let from_id = node_id_map[&edge.from];
            let to_id = node_id_map[&edge.to];
            let new_edge_kind = match edge.kind {
                s::EdgeKind::Input => EdgeKind::Input,
                s::EdgeKind::Canvas => EdgeKind::Canvas,
            };

            let edge_id = self.create_edge(&mut g, from_id, to_id, new_edge_kind);
            if edge_id < 0 {
                panic!("edge creation failed");
            }
        }
        return g;
    }
}
