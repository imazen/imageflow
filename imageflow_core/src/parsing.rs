use flow::definitions::{Graph, Node, NodeParams, EdgeKind};
use flow::nodes;
use internal_prelude::works_everywhere::*;
use ::ffi;
use ::rustc_serialize::hex::FromHex;
use ::rustc_serialize::base64::FromBase64;
use ::{Context,Job,IoProxy};

#[derive(Default)]
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

        let mut node_id_map: HashMap<i32, NodeIndex> = HashMap::new();

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

            if let  Err(daggy::WouldCycle(_)) = g.add_edge(from_id, to_id, new_edge_kind) {
                return Err(nerror!(ErrorKind::GraphCyclic));
            }
        }
        Ok(g)
    }
}


pub struct IoTranslator<'a> {
    ctx: *mut ::ffi::ImageflowContext,
    c: &'a Context,
}
impl<'a> IoTranslator<'a> {
    pub fn new(c: &'a Context) -> IoTranslator<'a> {
        IoTranslator { ctx: c.flow_c(), c: c }
    }

    fn create_io_proxy_from_enum(&self,
                                 io_enum: s::IoEnum,
                                 dir: s::IoDirection)
                                 -> Result<RefMut<IoProxy>> {
        match io_enum {
            s::IoEnum::ByteArray(vec) => {
                let bytes = vec;
                self.c.create_io_from_copy_of_slice(&bytes)
            }
            s::IoEnum::Base64(b64_string) => {
                let bytes = b64_string.as_str().from_base64().unwrap();
                self.c.create_io_from_copy_of_slice(&bytes)
            }
            s::IoEnum::BytesHex(hex_string) => {
                let bytes = hex_string.as_str().from_hex().unwrap();
                self.c.create_io_from_copy_of_slice(&bytes)
            }
            s::IoEnum::Filename(path) => {
                self.c.create_io_from_filename(&path, dir)
            }
            s::IoEnum::Url(url) => {
                let bytes = ::imageflow_helpers::fetching::fetch_bytes(&url).unwrap();
                self.c.create_io_from_copy_of_slice(&bytes)
            }
            s::IoEnum::OutputBuffer |
            s::IoEnum::OutputBase64 => {
                self.c.create_io_output_buffer()
            },
            s::IoEnum::Placeholder => {
                panic!("Placeholder was never substituted!")
            }
        }
    }

    pub fn add_to_job(&self, job: &mut Job, io_vec: Vec<s::IoObject>) {
        let mut io_list = Vec::new();
        for io_obj in io_vec {
            //TODO: add format!("Failed to create IO for {:?}", &io_obj)
            let proxy = self.create_io_proxy_from_enum(io_obj.io, io_obj.direction).unwrap();

            io_list.push((proxy, io_obj.io_id, io_obj.direction));
        }

        for io_list in io_list.iter_mut() {
            job.add_io(&mut io_list.0, io_list.1, io_list.2).unwrap();
        }

    }
}
