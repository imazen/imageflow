extern crate imageflow_serde as s;
use std::collections::HashMap;
use std;

use ffi::EdgeKind;

pub struct GraphTranslator {
    ctx: *mut ::ffi::Context
}

impl GraphTranslator{
    pub fn new (ctx: *mut ::ffi::Context) -> GraphTranslator{
        GraphTranslator{ ctx: ctx}
    }

/*

typedef enum flow_codec_type {
    flow_codec_type_null = 0,
    flow_codec_type_decode_png = 1,
    flow_codec_type_encode_png = 2,
    flow_codec_type_decode_jpeg = 3,
    flow_codec_type_encode_jpeg = 4,
    flow_codec_type_decode_gif = 5
} flow_codec_type;
*/

    unsafe fn create_node(&self,  g: *mut *mut ::ffi::Graph, node: s::Node )-> i32{
        match node {
            s::Node::FlipV => ::ffi::flow_node_create_primitive_flip_vertical(self.ctx, g, -1),
            s::Node::Decode{io_id } => ::ffi::flow_node_create_decoder(self.ctx, g, -1, io_id ),
            s::Node::Encode{io_id, encoder_id: enc_id, encoder: _, hints: _} => ::ffi::flow_node_create_encoder(self.ctx, g, -1, io_id, enc_id.unwrap_or(2), std::ptr::null() as *const ::ffi::EncoderHints),
            _ => panic!("Node not implemented")
        }
    }

    unsafe fn create_edge(&self,  g: *mut *mut ::ffi::Graph, from_node: i32, to_node: i32, edge_kind: ::ffi::EdgeKind )-> i32{
        ::ffi::flow_edge_create(self.ctx, g, from_node,to_node,edge_kind)
    }


    pub unsafe fn translate_graph(&self, from: s::Graph) -> *mut ::ffi::Graph {
        let mut g = ::ffi::flow_graph_create(self.ctx, 10, 10, 3000, 2.0f32);

        let mut node_id_map: HashMap<i32,i32> = HashMap::new();

        for (old_id, node) in from.nodes {
            let new_id = self.create_node( &mut g, node);
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
                s::EdgeKind::Canvas => EdgeKind::Canvas
            };

            let edge_id = self.create_edge(&mut g, from_id,to_id , new_edge_kind);
            if edge_id < 0 {
                panic!("edge creation failed");
            }
        }
        return g
    }
}


