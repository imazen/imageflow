
use daggy::{Dag, EdgeIndex, NodeIndex};
use ffi::*;
use libc::{self, int32_t, uint32_t};
use petgraph;
use std::ffi::CStr;


pub type Graph = Dag<::flow::definitions::Node, EdgeKind>;

pub fn create(context: *mut Context,
              max_edges: u32,
              max_nodes: u32)
              -> Graph {
    Graph::with_capacity(max_nodes as usize, max_edges as usize)
}


