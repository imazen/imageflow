use flow::definitions::{Graph, Node, NodeParams, EdgeKind};
use flow::nodes;
use internal_prelude::works_everywhere::*;

pub struct GraphTranslator {
}

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

    pub fn translate_graph(&self, from: s::Graph) -> Result<Graph> {
        let mut g = Graph::with_capacity(10, 10); //Estimate better than this

        let mut node_id_map: HashMap<i32, NodeIndex<u32>> = HashMap::new();

        for (old_id, node) in from.nodes {
            let new_id = g.add_node(Node::from(node));

            node_id_map.insert(old_id.parse::<i32>().unwrap(), new_id);
        }

        for edge in from.edges {
            let from_id = node_id_map[&edge.from];
            let to_id = node_id_map[&edge.to];
            let new_edge_kind = match edge.kind {
                s::EdgeKind::Input => EdgeKind::Input,
                s::EdgeKind::Canvas => EdgeKind::Canvas,
            };

            match g.add_edge(from_id, to_id, new_edge_kind) {
                Err(daggy::WouldCycle(_)) => {
                    return Err(FlowError::GraphCyclic);
                }
                _ => {}
            }
        }
        Ok(g)
    }
}
