#![forbid(unsafe_code)]

//! Graph execution engine.
//!
//! Takes a `CompiledGraph` and executes it, producing results.
//!
//! ## Execution model
//!
//! The engine supports two execution strategies:
//!
//! ### Streaming (default)
//! Pull-based strip pipeline inspired by zenpipe. The encoder (sink) drives
//! execution by requesting strips. Each strip flows backward through the
//! graph until it reaches a source (decoder). Memory usage is
//! O(strip_height × width × graph_depth).
//!
//! ### Eager (fallback)
//! Full-frame materialization. Used when any node in the pipeline requires
//! full-frame access (histogram, whitespace detection, transpose).
//!
//! The engine automatically selects the right strategy based on the
//! `StreamingSupport` classification of nodes in the compiled graph.

use crate::compiler::CompiledGraph;
use crate::error::Result;
use crate::node::NodeIndex;

/// Execution engine for compiled graphs.
///
/// The engine is parameterized over external dependencies (codecs, I/O)
/// via the `ExecutionContext` trait. This keeps the graph engine independent
/// of any specific codec implementation.
pub struct GraphEngine {
    graph: CompiledGraph,
}

impl GraphEngine {
    /// Create an engine from a compiled graph.
    pub fn new(graph: CompiledGraph) -> Self {
        Self { graph }
    }

    /// Get the compiled graph.
    pub fn graph(&self) -> &CompiledGraph {
        &self.graph
    }

    /// Execute the graph.
    ///
    /// The `context` provides codec and I/O implementations. This design
    /// keeps the graph engine codec-agnostic — it works with zencodecs,
    /// imageflow's C codecs, or any other implementation.
    pub fn execute(&mut self, context: &mut dyn ExecutionContext) -> Result<ExecutionResult> {
        let mut results = Vec::new();

        for &ix in self.graph.execution_order() {
            let node = self.graph.node(ix).ok_or(crate::error::GraphError::InvalidNodeIndex(ix))?;

            // Dispatch based on node type
            match &node.params {
                crate::node::NodeParams::Command(step) => {
                    context.execute_step(ix, step)?;
                }
                crate::node::NodeParams::Internal(_) => {
                    context.execute_internal(ix)?;
                }
            }
        }

        // Collect encode results
        for &ix in self.graph.execution_order() {
            if let Some(node) = self.graph.node(ix) {
                if let crate::node::NodeResult::Encoded { io_id, ref format, w, h, bytes } =
                    node.result
                {
                    results.push(EncodeOutput { io_id, format: format.clone(), w, h, bytes });
                }
            }
        }

        Ok(ExecutionResult { outputs: results })
    }
}

/// Trait for providing codec and I/O implementations to the engine.
///
/// This is the extension point where concrete codec implementations
/// (zencodecs, imageflow C codecs, etc.) plug in.
pub trait ExecutionContext {
    /// Execute a pipeline step command.
    fn execute_step(&mut self, node: NodeIndex, step: &imageflow_commands::Step) -> Result<()>;

    /// Execute an engine-internal operation (from node expansion).
    fn execute_internal(&mut self, node: NodeIndex) -> Result<()>;
}

/// Result of graph execution.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub outputs: Vec<EncodeOutput>,
}

/// Result of a single encode operation.
#[derive(Debug, Clone)]
pub struct EncodeOutput {
    pub io_id: i32,
    pub format: String,
    pub w: u32,
    pub h: u32,
    pub bytes: u64,
}
