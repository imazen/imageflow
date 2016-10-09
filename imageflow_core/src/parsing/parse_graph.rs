extern crate imageflow_serde as s;
use ffi::EdgeKind;
use std;
use std::collections::HashMap;

extern crate rustc_serialize;
use parsing::rustc_serialize::hex::FromHex;
use daggy::{Dag,EdgeIndex,NodeIndex};
use flow::nodes as nodes;
use flow::definitions::{Node, NodeParams};

pub struct GraphTranslator {
    ctx: *mut ::ffi::Context,
}

impl GraphTranslator {
    pub fn new(ctx: *mut ::ffi::Context) -> GraphTranslator {
        GraphTranslator { ctx: ctx }
    }


    unsafe fn create_node(&self, g: &mut ::flow::graph::Graph, node: s::Node) -> NodeIndex<u32> {
        let new_node = match node {

//            s::Node::Decode { io_id } => ::flow::graph::node_create_decoder(self.ctx, g, -1, io_id),
//            s::Node::Encode { io_id, encoder_id, encoder, hints: _ } => {
//                let encoder_id = encoder_id.unwrap_or(match encoder.unwrap_or(s::Encoder::Png) {
//                    s::Encoder::Jpeg => 4,
//                    s::Encoder::Png => 2,
//                });
//                let encoder_hints = ::ffi::EncoderHints {
//                    jpeg_encode_quality: 100,
//                    disable_png_alpha: false,
//                };
//
//
//
//                ::flow::graph::node_create_encoder(self.ctx,
//                                                g,
//                                                -1,
//                                                io_id,
//                                                encoder_id,
//                                                &encoder_hints as *const ::ffi::EncoderHints)
//            }
//            s::Node::Crop { x1, y1, x2, y2 } => {
//                ::flow::graph::node_create_primitive_crop(self.ctx, g, -1, x1, y1, x2, y2)
//            }
            s::Node::FlowBitmapBgraPtr{..} => Node::new(&nodes::BITMAP_BGRA_POINTER, NodeParams::Json(node)),
            s::Node::FlipV => Node::new(&nodes::FLIP_V, NodeParams::Json(node)),
            s::Node::FlipH => Node::new(&nodes::FLIP_H, NodeParams::Json(node)),
            s::Node::Rotate90 => Node::new(&nodes::ROTATE_90, NodeParams::Json(node)),
            s::Node::Rotate180 => Node::new(&nodes::ROTATE_180, NodeParams::Json(node)),
            s::Node::Rotate270 => Node::new(&nodes::ROTATE_270, NodeParams::Json(node)),
            //s::Node::Transpose => Node::new(&nodes::TRANSPOSE, NodeParams::Json(node)),
            s::Node::CreateCanvas{..} => Node::new(&nodes::CREATE_CANVAS, NodeParams::Json(node)),
            s::Node::CopyRectToCanvas{..} => Node::new(&nodes::COPY_RECT, NodeParams::Json(node)),
            s::Node::FillRect{..} => Node::new(&nodes::FILL_RECT, NodeParams::Json(node)),
            s::Node::Scale{..} => Node::new(&nodes::SCALE, NodeParams::Json(node)),
//            s::Node::ExpandCanvas { left, top, right, bottom, color } => {
//                ::flow::graph::node_create_expand_canvas(self.ctx,
//                                                      g,
//                                                      -1,
//                                                      left,
//                                                      top,
//                                                      right,
//                                                      bottom,
//                                                      self.color_to_i32(color).unwrap())
//            }

//
            _ => Node::new(&nodes::NO_OP, NodeParams::Json(node)),
        };
        g.add_node(new_node)
    }



    pub unsafe fn translate_graph(&self, from: s::Graph) -> ::flow::graph::Graph {
        let mut g = ::flow::graph::create(self.ctx, 10, 10, 3000, 2.0f32);

        let mut node_id_map: HashMap<i32, NodeIndex<u32>> = HashMap::new();

        for (old_id, node) in from.nodes {
            let new_id = self.create_node(&mut g, node);

            node_id_map.insert(old_id.parse::<i32>().unwrap(), new_id);
        }

        for edge in from.edges {
            let from_id = node_id_map[&edge.from];
            let to_id = node_id_map[&edge.to];
            let new_edge_kind = match edge.kind {
                s::EdgeKind::Input => EdgeKind::Input,
                s::EdgeKind::Canvas => EdgeKind::Canvas,
            };

            g.add_edge(from_id, to_id, new_edge_kind).unwrap();
        }
        return g;
    }
}
