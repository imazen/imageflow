use crate::flow::definitions::{EdgeKind, Graph, Node, NodeParams};
use crate::flow::nodes;
use crate::internal_prelude::works_everywhere::*;
use crate::{Context, IoProxy};
#[derive(Default)]
pub struct GraphTranslator {}

impl GraphTranslator {
    pub fn new() -> GraphTranslator {
        GraphTranslator {}
    }

    pub fn translate_framewise(&self, framewise: s::Framewise) -> Result<Graph> {
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
            edges.push(s::Edge { from: i as i32, to: i as i32 + 1, kind: s::EdgeKind::Input });
        }
        let _ = edges.pop();
        s::Graph { nodes, edges }
    }

    pub fn translate_graph(&self, from: s::Graph) -> Result<Graph> {
        const MAX_GRAPH_NODES: usize = 2048;
        if from.nodes.len() > MAX_GRAPH_NODES {
            return Err(nerror!(
                ErrorKind::GraphInvalid,
                "Graph has {} nodes, exceeding maximum of {}",
                from.nodes.len(),
                MAX_GRAPH_NODES
            ));
        }
        if from.edges.len() > MAX_GRAPH_NODES * 2 {
            return Err(nerror!(
                ErrorKind::GraphInvalid,
                "Graph has {} edges, exceeding maximum of {}",
                from.edges.len(),
                MAX_GRAPH_NODES * 2
            ));
        }

        let mut g = Graph::with_capacity(10, 10); //Estimate better than this

        let mut node_id_map: HashMap<i32, NodeIndex> = HashMap::new();

        for (old_id, node) in from.nodes {
            let new_id = g.add_node(Node::from(node));

            let parsed_id = old_id.parse::<i32>().map_err(|_| {
                nerror!(ErrorKind::GraphInvalid, "Node key '{}' is not a valid i32", old_id)
            })?;
            node_id_map.insert(parsed_id, new_id);
        }

        for edge in from.edges {
            let from_id = *node_id_map.get(&edge.from).ok_or_else(|| {
                nerror!(ErrorKind::GraphInvalid, "Edge references missing node {}", edge.from)
            })?;
            let to_id = *node_id_map.get(&edge.to).ok_or_else(|| {
                nerror!(ErrorKind::GraphInvalid, "Edge references missing node {}", edge.to)
            })?;
            let new_edge_kind = match edge.kind {
                s::EdgeKind::Input => EdgeKind::Input,
                s::EdgeKind::Canvas => EdgeKind::Canvas,
            };

            if let Err(daggy::WouldCycle(_)) = g.add_edge(from_id, to_id, new_edge_kind) {
                return Err(nerror!(ErrorKind::GraphCyclic));
            }
        }
        Ok(g)
    }
}

pub struct IoTranslator;
impl IoTranslator {
    pub fn add_all(&self, c: &mut Context, io_vec: Vec<s::IoObject>) -> Result<()> {
        for io_obj in io_vec {
            //TODO: add format!("Failed to create IO for {:?}", &io_obj)
            self.add(c, io_obj.io_id, io_obj.io, io_obj.direction)?;
        }
        Ok(())
    }
    fn add(
        &self,
        c: &mut Context,
        io_id: i32,
        io_enum: s::IoEnum,
        dir: s::IoDirection,
    ) -> Result<()> {
        match io_enum {
            s::IoEnum::ByteArray(vec) => {
                c.add_copied_input_buffer(io_id, &vec).map_err(|e| e.at(here!()))
            }
            s::IoEnum::Base64(b64_string) => {
                //TODO: test and disable slow methods
                use base64::Engine;
                let decoded_vec = base64::engine::general_purpose::STANDARD
                    .decode(&b64_string)
                    .map_err(|e| nerror!(ErrorKind::InvalidArgument, "base64: {}", e))?;
                c.add_copied_input_buffer(io_id, &decoded_vec).map_err(|e| e.at(here!()))
            }
            s::IoEnum::BytesHex(hex_string) => {
                let bytes = hex::decode(hex_string.as_str())
                    .map_err(|e| nerror!(ErrorKind::InvalidArgument, "hex: {}", e))?;
                c.add_copied_input_buffer(io_id, &bytes).map_err(|e| e.at(here!()))
            }
            s::IoEnum::Filename(path) => c.add_file(io_id, dir, &path),

            s::IoEnum::OutputBuffer | s::IoEnum::OutputBase64 => {
                c.add_output_buffer(io_id).map_err(|e| e.at(here!()))
            }
            s::IoEnum::Placeholder => Err(nerror!(
                ErrorKind::GraphInvalid,
                "Io Placeholder {} was never substituted",
                io_id
            )),
        }
    }
}
