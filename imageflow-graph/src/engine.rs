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
use crate::error::{GraphError, Result};
use crate::estimate::FrameInfo;
use crate::node::{EdgeKind, InternalParams, NodeIndex, NodeParams};

// ─── Frame Handle ──────────────────────────────────────────────────────

/// Opaque handle to a frame managed by the `ExecutionContext` implementation.
///
/// The engine routes these between nodes based on graph topology.
/// The actual pixel data lives in the context (e.g., BitmapsContainer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameHandle(pub u64);

// ─── Step Output ───────────────────────────────────────────────────────

/// Result of executing a single step.
#[derive(Debug, Clone)]
pub enum StepOutput {
    /// Step produced a frame (most operations).
    Frame { handle: FrameHandle, info: FrameInfo },
    /// Step encoded to an output (sink node, e.g., Encode).
    Encoded { io_id: i32, format: String, w: u32, h: u32, bytes: u64 },
    /// Step consumed its input without producing output.
    Consumed,
}

// ─── Execution Context Trait ───────────────────────────────────────────

/// Trait for providing codec and I/O implementations to the engine.
///
/// This is the extension point where concrete codec implementations
/// (zencodecs, imageflow C codecs, etc.) plug in. The engine handles
/// graph topology and frame routing; the context handles pixel operations.
///
/// ## Frame lifecycle
///
/// 1. Source nodes (Decode, CreateCanvas) return `StepOutput::Frame`
/// 2. The engine tracks the frame handle at each node
/// 3. Processing nodes receive parent frame handles via `inputs`/`canvas`
/// 4. Sink nodes (Encode) consume frames and return `StepOutput::Encoded`
///
/// Frame handles are opaque to the engine. The context implementation
/// manages actual pixel storage (e.g., BitmapsContainer, heap buffers).
pub trait ExecutionContext {
    /// Execute a pipeline step command.
    ///
    /// - `node`: index of the node being executed
    /// - `step`: the operation (Decode, Resize, Encode, etc.)
    /// - `inputs`: frame handles from Input-edge parents (usually 0 or 1)
    /// - `canvas`: frame handle from Canvas-edge parent (for compositing)
    fn execute_step(
        &mut self,
        node: NodeIndex,
        step: &imageflow_commands::Step,
        inputs: &[FrameHandle],
        canvas: Option<FrameHandle>,
    ) -> Result<StepOutput>;

    /// Execute an engine-internal operation (from node expansion).
    ///
    /// Internal ops are generated when high-level nodes (Constrain,
    /// CommandString) expand into primitives (Resize, Crop, Pad).
    fn execute_internal(
        &mut self,
        node: NodeIndex,
        params: &InternalParams,
        inputs: &[FrameHandle],
    ) -> Result<StepOutput>;
}

// ─── Engine ────────────────────────────────────────────────────────────

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
    ///
    /// The engine:
    /// 1. Iterates nodes in topological order
    /// 2. Resolves parent frame handles via edge connectivity
    /// 3. Delegates each step to the context
    /// 4. Tracks output frame handles for downstream nodes
    /// 5. Collects encode results into `ExecutionResult`
    pub fn execute(&mut self, context: &mut dyn ExecutionContext) -> Result<ExecutionResult> {
        let node_count = self.graph.node_count();
        let order = self.graph.execution_order().to_vec();

        // Track frame handles per node (parallel to node indices)
        let mut frames: Vec<Option<FrameHandle>> = vec![None; node_count];
        let mut results = Vec::new();

        for &ix in &order {
            let node = self.graph.node(ix).ok_or(GraphError::InvalidNodeIndex(ix))?;

            // Resolve parent frame handles from graph edges
            let input_parents = self.graph.builder.parents_of_kind(ix, EdgeKind::Input);
            let canvas_parents = self.graph.builder.parents_of_kind(ix, EdgeKind::Canvas);

            let inputs: Vec<FrameHandle> =
                input_parents.iter().filter_map(|&p| frames[p.0 as usize]).collect();

            let canvas = canvas_parents.first().and_then(|&p| frames[p.0 as usize]);

            // Dispatch to context
            let output = match &node.params {
                NodeParams::Command(step) => context.execute_step(ix, step, &inputs, canvas)?,
                NodeParams::Internal(params) => context.execute_internal(ix, params, &inputs)?,
            };

            // Record output
            match output {
                StepOutput::Frame { handle, .. } => {
                    frames[ix.0 as usize] = Some(handle);
                }
                StepOutput::Encoded { io_id, ref format, w, h, bytes } => {
                    results.push(EncodeOutput { io_id, format: format.clone(), w, h, bytes });
                }
                StepOutput::Consumed => {}
            }
        }

        Ok(ExecutionResult { outputs: results })
    }
}

// ─── Result Types ──────────────────────────────────────────────────────

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
