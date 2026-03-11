#![forbid(unsafe_code)]

//! Graph builder — constructs the internal graph from `imageflow-commands` types.
//!
//! Supports both sequential pipelines (steps execute in order) and explicit
//! DAGs. In sequential mode, each step's output feeds into the next step's input.

use std::collections::HashMap;

use imageflow_commands::{Pipeline, Step};

use crate::error::{GraphError, Result};
use crate::node::{EdgeKind, GraphNode, NodeIndex, NodeParams, StableNodeId};

/// Internal edge representation.
#[derive(Debug, Clone)]
pub(crate) struct GraphEdge {
    pub from: NodeIndex,
    pub to: NodeIndex,
    pub kind: EdgeKind,
}

/// Mutable graph under construction.
///
/// Used by the builder and expansion passes. Converted to a `CompiledGraph`
/// after all expansion is complete.
pub struct GraphBuilder {
    pub(crate) nodes: Vec<GraphNode>,
    pub(crate) edges: Vec<GraphEdge>,
    next_stable_id: u64,
}

impl GraphBuilder {
    /// Create an empty graph builder.
    pub fn new() -> Self {
        Self { nodes: Vec::new(), edges: Vec::new(), next_stable_id: 1 }
    }

    /// Add a node and return its index.
    pub fn add_node(
        &mut self,
        def: &'static dyn crate::node::NodeDef,
        params: NodeParams,
    ) -> NodeIndex {
        let ix = NodeIndex(self.nodes.len() as u32);
        let stable_id = StableNodeId(self.next_stable_id);
        self.next_stable_id += 1;
        self.nodes.push(GraphNode {
            def,
            params,
            stable_id,
            frame_estimate: crate::estimate::FrameEstimate::None,
            result: crate::node::NodeResult::None,
        });
        ix
    }

    /// Add an edge between two nodes.
    pub fn add_edge(&mut self, from: NodeIndex, to: NodeIndex, kind: EdgeKind) -> Result<()> {
        if from.0 as usize >= self.nodes.len() {
            return Err(GraphError::InvalidNodeIndex(from));
        }
        if to.0 as usize >= self.nodes.len() {
            return Err(GraphError::InvalidNodeIndex(to));
        }
        self.edges.push(GraphEdge { from, to, kind });
        Ok(())
    }

    /// Get a node by index.
    pub fn node(&self, ix: NodeIndex) -> Option<&GraphNode> {
        self.nodes.get(ix.0 as usize)
    }

    /// Get a mutable node by index.
    pub fn node_mut(&mut self, ix: NodeIndex) -> Option<&mut GraphNode> {
        self.nodes.get_mut(ix.0 as usize)
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Find parent nodes connected to `ix` with the given edge kind.
    pub fn parents_of_kind(&self, ix: NodeIndex, kind: EdgeKind) -> Vec<NodeIndex> {
        self.edges.iter().filter(|e| e.to == ix && e.kind == kind).map(|e| e.from).collect()
    }

    /// Find child nodes connected from `ix`.
    pub fn children_of(&self, ix: NodeIndex) -> Vec<NodeIndex> {
        self.edges.iter().filter(|e| e.from == ix).map(|e| e.to).collect()
    }

    /// Build from a sequential pipeline.
    ///
    /// Creates a linear chain: each step's output feeds into the next.
    /// Multi-input steps (DrawImage, Watermark) create additional decode
    /// nodes connected via Canvas edges.
    pub fn from_sequential(steps: &[Step]) -> Result<Self> {
        let mut builder = Self::new();

        // For sequential mode, we need node definitions.
        // The builder creates placeholder nodes — the engine resolves
        // actual NodeDef references during compilation.
        let mut prev: Option<NodeIndex> = None;

        for step in steps {
            let ix = builder.add_node(&PLACEHOLDER_DEF, NodeParams::Command(step.clone()));

            // Chain sequential edges
            if let Some(prev_ix) = prev {
                builder.add_edge(prev_ix, ix, EdgeKind::Input)?;
            }

            prev = Some(ix);
        }

        Ok(builder)
    }

    /// Build from a Pipeline (sequential or graph).
    pub fn from_pipeline(pipeline: &Pipeline) -> Result<Self> {
        match pipeline {
            Pipeline::Steps(steps) => Self::from_sequential(steps),
            Pipeline::Graph(graph) => Self::from_graph(graph),
        }
    }

    /// Build from an explicit graph definition.
    fn from_graph(graph: &imageflow_commands::Graph) -> Result<Self> {
        let mut builder = Self::new();

        // Map string node IDs to NodeIndex
        let mut id_map: HashMap<String, NodeIndex> = HashMap::new();

        for (name, gnode) in &graph.nodes {
            let ix = builder.add_node(&PLACEHOLDER_DEF, NodeParams::Command(gnode.step.clone()));
            id_map.insert(name.clone(), ix);
        }

        // Wire up inline `input` references
        for (name, gnode) in &graph.nodes {
            if let Some(input_id) = &gnode.input {
                let to_ix = id_map[name];
                // Find the node with matching NodeId
                let from_name =
                    id_map.iter().find(|(_, ix)| ix.0 == input_id.0).map(|(n, _)| n.clone());
                if let Some(from_name) = from_name {
                    let from_ix = id_map[&from_name];
                    builder.add_edge(from_ix, to_ix, EdgeKind::Input)?;
                }
            }
        }

        // Wire up explicit edges
        for edge in &graph.edges {
            let from_ix = NodeIndex(edge.from.0);
            let to_ix = NodeIndex(edge.to.0);
            let kind = match edge.kind {
                imageflow_commands::EdgeKind::Input => EdgeKind::Input,
                imageflow_commands::EdgeKind::Canvas => EdgeKind::Canvas,
            };
            builder.add_edge(from_ix, to_ix, kind)?;
        }

        Ok(builder)
    }

    /// Topological sort. Returns node indices in execution order.
    ///
    /// Returns `Err(CycleDetected)` if the graph contains cycles.
    pub fn topological_sort(&self) -> Result<Vec<NodeIndex>> {
        let n = self.nodes.len();
        let mut in_degree = vec![0u32; n];
        for edge in &self.edges {
            in_degree[edge.to.0 as usize] += 1;
        }

        let mut queue: Vec<NodeIndex> =
            (0..n as u32).filter(|&i| in_degree[i as usize] == 0).map(NodeIndex).collect();

        let mut order = Vec::with_capacity(n);

        while let Some(ix) = queue.pop() {
            order.push(ix);
            for edge in &self.edges {
                if edge.from == ix {
                    in_degree[edge.to.0 as usize] -= 1;
                    if in_degree[edge.to.0 as usize] == 0 {
                        queue.push(edge.to);
                    }
                }
            }
        }

        if order.len() != n {
            return Err(GraphError::CycleDetected);
        }

        Ok(order)
    }
}

impl Default for GraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Placeholder node definition used during graph construction.
///
/// The real `NodeDef` is resolved during compilation based on the
/// `Step` variant in the node's params.
#[derive(Debug)]
struct PlaceholderNodeDef;

impl crate::node::NodeDef for PlaceholderNodeDef {
    fn fqn(&self) -> &'static str {
        "imageflow.placeholder"
    }

    fn edges_required(&self) -> (crate::node::EdgesIn, crate::node::EdgesOut) {
        (crate::node::EdgesIn::OneOptionalInput, crate::node::EdgesOut::Any)
    }

    fn estimate(
        &self,
        input: Option<crate::estimate::FrameInfo>,
        _params: &NodeParams,
    ) -> crate::estimate::FrameEstimate {
        match input {
            Some(info) => crate::estimate::FrameEstimate::Some(info),
            None => crate::estimate::FrameEstimate::None,
        }
    }
}

static PLACEHOLDER_DEF: PlaceholderNodeDef = PlaceholderNodeDef;

#[cfg(test)]
mod tests {
    use super::*;
    use imageflow_commands::{
        ConstrainStep, ConstraintMode, DecodeStep, EncodeStep, OutputFormat, Step,
    };

    #[test]
    fn sequential_pipeline_builds() {
        let steps = vec![
            Step::Decode(DecodeStep { io_id: 0, color: None, hints: None, ultrahdr: None }),
            Step::Constrain(ConstrainStep {
                mode: ConstraintMode::Fit,
                w: Some(800),
                h: Some(600),
                gravity: None,
                background: None,
                hints: None,
            }),
            Step::Encode(EncodeStep {
                io_id: 1,
                format: Some(OutputFormat::Jpeg),
                quality: None,
                color: None,
                ultrahdr: None,
                prefer_lossless_jpeg: false,
                hints: None,
                matte: None,
            }),
        ];

        let builder = GraphBuilder::from_sequential(&steps).unwrap();
        assert_eq!(builder.node_count(), 3);

        let order = builder.topological_sort().unwrap();
        assert_eq!(order.len(), 3);
        // First node should be decode (no incoming edges)
        assert_eq!(order[0], NodeIndex(0));
    }

    #[test]
    fn topological_sort_detects_empty() {
        let builder = GraphBuilder::new();
        let order = builder.topological_sort().unwrap();
        assert!(order.is_empty());
    }
}
