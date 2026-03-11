#![forbid(unsafe_code)]

//! Graph compiler — transforms a mutable `GraphBuilder` into a compiled,
//! executable `CompiledGraph`.
//!
//! ## Compilation phases
//!
//! 1. **Validate** — Check edge counts match node requirements
//! 2. **Estimate** — Propagate dimensions (topological order)
//! 3. **Expand** — Decompose high-level nodes (Constrain → Resize + Crop)
//! 4. **Re-estimate** — Update dimensions after expansion
//! 5. **Optimize** — Fuse operations, detect fast paths
//! 6. **Freeze** — Produce immutable `CompiledGraph`
//!
//! Inspired by:
//! - zenimage's `DagCompiler` (estimate → expand → build → stream)
//! - imageflow v3's multi-pass execution engine
//! - zenpipe's operation fusion (`TransformSource`)

use crate::builder::GraphBuilder;
use crate::error::{GraphError, Result};
use crate::node::{NodeIndex, StreamingSupport};

/// Maximum expansion passes before giving up.
const MAX_EXPANSION_PASSES: u32 = 10;

/// A compiled, immutable graph ready for execution.
///
/// All high-level nodes have been expanded, dimensions are known,
/// and the execution order is fixed.
pub struct CompiledGraph {
    /// The finalized graph.
    pub(crate) builder: GraphBuilder,
    /// Topological execution order.
    pub(crate) execution_order: Vec<NodeIndex>,
    /// Detected fast paths.
    pub(crate) fast_paths: Vec<FastPath>,
}

/// Fast path optimizations detected during compilation.
#[derive(Debug, Clone)]
pub enum FastPath {
    /// JPEG → orient → JPEG: use lossless DCT transform.
    LosslessJpeg { decode_node: NodeIndex, encode_node: NodeIndex },
    /// No pixel operations between decode and encode: passthrough.
    Passthrough { decode_node: NodeIndex, encode_node: NodeIndex },
}

impl CompiledGraph {
    /// Compile a graph builder into an executable graph.
    pub fn compile(mut builder: GraphBuilder) -> Result<Self> {
        // Phase 1: Topological sort (validates DAG property)
        let order = builder.topological_sort()?;

        // Phase 2: Estimate dimensions
        Self::estimate_pass(&mut builder, &order)?;

        // Phase 3: Expand high-level nodes
        let mut passes = 0;
        loop {
            let expanded = Self::expansion_pass(&mut builder)?;
            if !expanded {
                break;
            }
            passes += 1;
            if passes >= MAX_EXPANSION_PASSES {
                return Err(GraphError::ExpansionLimitExceeded {
                    max_passes: MAX_EXPANSION_PASSES,
                });
            }
        }

        // Phase 4: Re-sort and re-estimate after expansion
        let execution_order = builder.topological_sort()?;
        Self::estimate_pass(&mut builder, &execution_order)?;

        // Phase 5: Detect fast paths
        let fast_paths = Self::detect_fast_paths(&builder, &execution_order);

        Ok(CompiledGraph { builder, execution_order, fast_paths })
    }

    /// Propagate dimension estimates through the graph.
    fn estimate_pass(builder: &mut GraphBuilder, order: &[NodeIndex]) -> Result<()> {
        for &ix in order {
            let input_info = {
                let parents = builder.parents_of_kind(ix, crate::node::EdgeKind::Input);
                parents
                    .first()
                    .and_then(|&p| builder.node(p).and_then(|n| n.frame_estimate.as_info()))
            };

            let node = builder.node(ix).ok_or(GraphError::InvalidNodeIndex(ix))?;
            let estimate = node.def.estimate(input_info, &node.params);

            let node = builder.node_mut(ix).ok_or(GraphError::InvalidNodeIndex(ix))?;
            node.frame_estimate = estimate;
        }
        Ok(())
    }

    /// Run one expansion pass. Returns true if any nodes were expanded.
    fn expansion_pass(builder: &mut GraphBuilder) -> Result<bool> {
        // For now, expansion is a placeholder — actual expansion logic
        // (Constrain → Resize + Crop, etc.) will be implemented when
        // concrete NodeDef implementations are added.
        let _expandable: Vec<_> = (0..builder.node_count() as u32)
            .map(NodeIndex)
            .filter(|&ix| builder.node(ix).is_some_and(|n| n.def.can_expand()))
            .collect();

        // No expansion implemented yet
        Ok(false)
    }

    /// Detect fast paths in the compiled graph.
    fn detect_fast_paths(builder: &GraphBuilder, order: &[NodeIndex]) -> Vec<FastPath> {
        let mut fast_paths = Vec::new();

        // Look for decode → [orient-only] → encode patterns
        for window in order.windows(2) {
            let (a, b) = (window[0], window[1]);
            let a_node = match builder.node(a) {
                Some(n) => n,
                None => continue,
            };
            let b_node = match builder.node(b) {
                Some(n) => n,
                None => continue,
            };

            // Check for decode → encode (passthrough)
            let is_decode = matches!(
                a_node.params,
                crate::node::NodeParams::Command(imageflow_commands::Step::Decode(_))
            );
            let is_encode = matches!(
                b_node.params,
                crate::node::NodeParams::Command(imageflow_commands::Step::Encode(_))
            );

            if is_decode && is_encode {
                fast_paths.push(FastPath::Passthrough { decode_node: a, encode_node: b });
            }
        }

        fast_paths
    }

    /// Get the execution order.
    pub fn execution_order(&self) -> &[NodeIndex] {
        &self.execution_order
    }

    /// Get detected fast paths.
    pub fn fast_paths(&self) -> &[FastPath] {
        &self.fast_paths
    }

    /// Get a node by index.
    pub fn node(&self, ix: NodeIndex) -> Option<&crate::node::GraphNode> {
        self.builder.node(ix)
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize {
        self.builder.node_count()
    }

    /// Classify nodes by streaming support.
    pub fn streaming_classification(&self) -> Vec<(NodeIndex, StreamingSupport)> {
        self.execution_order
            .iter()
            .map(|&ix| {
                let support = self
                    .builder
                    .node(ix)
                    .map(|n| n.def.streaming_support())
                    .unwrap_or(StreamingSupport::Eager);
                (ix, support)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use imageflow_commands::{DecodeStep, EncodeStep, OutputFormat, Step};

    #[test]
    fn compile_simple_pipeline() {
        let steps = vec![
            Step::Decode(DecodeStep { io_id: 0, color: None, hints: None, ultrahdr: None }),
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
        let compiled = CompiledGraph::compile(builder).unwrap();

        assert_eq!(compiled.node_count(), 2);
        assert_eq!(compiled.execution_order().len(), 2);
        // Should detect passthrough fast path
        assert_eq!(compiled.fast_paths().len(), 1);
        assert!(matches!(compiled.fast_paths()[0], FastPath::Passthrough { .. }));
    }
}
