extern crate imageflow_serde as s;
use ffi::EdgeKind;
use std;
use std::collections::HashMap;

extern crate rustc_serialize;
use daggy::{Dag, EdgeIndex, NodeIndex};
use flow::definitions::{Node, NodeParams};
use flow::nodes;
use parsing::rustc_serialize::hex::FromHex;

pub struct GraphTranslator {
}

impl GraphTranslator {
    pub fn new() -> GraphTranslator {
        GraphTranslator { }
    }


    pub fn translate_framewise(&self, framewise: s::Framewise) -> ::flow::graph::Graph {
        let graph = match framewise {
            s::Framewise::Graph(g) => g,
            s::Framewise::Steps(s) => self.steps_to_graph(s),
        };
        self.translate_graph(graph)
    }


    fn steps_to_graph(&self, steps: Vec<s::Node>) -> s::Graph {
        let mut nodes = HashMap::new();
        let mut edges = vec![];
        for (i, item) in steps.into_iter().enumerate() {
            nodes.insert(i.to_string(), item);
            edges.push(s::Edge {
                from: i as i32,
                to: i as i32 + 1,
                kind: s::EdgeKind::Input,
            });
        }
        let _ = edges.pop();
        s::Graph {
            nodes: nodes,
            edges: edges,
        }
    }



    fn create_node(&self, g: &mut ::flow::graph::Graph, node: s::Node) -> NodeIndex<u32> {
        let new_node = match node {
            s::Node::Crop { .. } => Node::new(&nodes::CROP, NodeParams::Json(node)),
            s::Node::Decode { .. } => Node::new(&nodes::DECODER, NodeParams::Json(node)),
            s::Node::FlowBitmapBgraPtr { .. } =>
                Node::new(&nodes::BITMAP_BGRA_POINTER, NodeParams::Json(node)),
            s::Node::FlipV => Node::new(&nodes::FLIP_V, NodeParams::Json(node)),
            s::Node::FlipH => Node::new(&nodes::FLIP_H, NodeParams::Json(node)),
            s::Node::Rotate90 => Node::new(&nodes::ROTATE_90, NodeParams::Json(node)),
            s::Node::Rotate180 => Node::new(&nodes::ROTATE_180, NodeParams::Json(node)),
            s::Node::Rotate270 => Node::new(&nodes::ROTATE_270, NodeParams::Json(node)),
            s::Node::ApplyOrientation { .. } => Node::new(&nodes::APPLY_ORIENTATION, NodeParams::Json(node)),
            s::Node::Transpose => Node::new(&nodes::TRANSPOSE, NodeParams::Json(node)),
            s::Node::Resample1D{ ..} => Node::new(&nodes::SCALE_1D, NodeParams::Json(node)),
            s::Node::Encode { .. }=> Node::new(&nodes::ENCODE, NodeParams::Json(node)),
            s::Node::CreateCanvas { .. } =>
                Node::new(&nodes::CREATE_CANVAS, NodeParams::Json(node)),
            s::Node::CopyRectToCanvas { .. } =>
                Node::new(&nodes::COPY_RECT, NodeParams::Json(node)),
            s::Node::FillRect { .. } => Node::new(&nodes::FILL_RECT, NodeParams::Json(node)),
            s::Node::Resample2D { .. } => Node::new(&nodes::SCALE, NodeParams::Json(node)),
            s::Node::ExpandCanvas { .. } =>
                Node::new(&nodes::EXPAND_CANVAS, NodeParams::Json(node))

        };
        g.add_node(new_node)
    }



    pub fn translate_graph(&self, from: s::Graph) -> ::flow::graph::Graph {
        let mut g = ::flow::graph::create(10, 10);

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
        g
    }
}
