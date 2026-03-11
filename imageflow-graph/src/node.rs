#![forbid(unsafe_code)]

use std::fmt;

use crate::error::Result;
use crate::estimate::{FrameEstimate, FrameInfo};

/// Index into the graph's node storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeIndex(pub(crate) u32);

impl fmt::Display for NodeIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "n{}", self.0)
    }
}

/// Stable identifier that survives graph mutations.
///
/// When nodes are expanded or replaced, child nodes get new indices,
/// but stable IDs persist for debugging, tracing, and visualization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StableNodeId(pub u64);

impl fmt::Display for StableNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// Edge type in the processing graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeKind {
    /// Primary input (most operations).
    #[default]
    Input,
    /// Canvas / background input (compositing operations).
    Canvas,
}

/// How a node participates in streaming execution.
///
/// This classification drives the DAG compiler's materialization decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingSupport {
    /// Can process strips on demand (row by row). Most operations.
    /// Memory: O(strip_height × width).
    Streaming,
    /// Requires full input frame before producing output.
    /// Forces materialization of upstream. Examples: histogram analysis,
    /// whitespace detection, certain blur implementations.
    Eager,
    /// Not directly executable — expands into other nodes first.
    /// Examples: Constrain → Resize + Crop, ApplyOrientation → Flip/Transpose.
    Passthrough,
}

/// Input edge requirements for a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgesIn {
    /// No input (source node: Decode, CreateCanvas).
    NoInput,
    /// Exactly one input (most operations).
    OneInput,
    /// Zero or one input (optional input).
    OneOptionalInput,
    /// One input + one canvas (compositing: DrawImage, Watermark, CopyRect).
    OneInputOneCanvas,
    /// Arbitrary edge counts.
    Arbitrary {
        /// Minimum Input edges.
        min_inputs: u32,
        /// Minimum Canvas edges.
        min_canvases: u32,
    },
}

/// Output edge requirements for a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgesOut {
    /// No output (sink node: Encode).
    None,
    /// Any number of outputs (fan-out supported).
    Any,
}

/// Core trait for graph node definitions.
///
/// Node definitions are typically static singletons (`&'static dyn NodeDef`).
/// They define the behavior contract for each operation type.
///
/// ## Lifecycle
///
/// 1. **Validation**: `edges_required()` + `validate_params()` — structure check
/// 2. **Estimation**: `estimate()` — propagate dimensions through the graph
/// 3. **Expansion**: `expand()` — decompose high-level nodes to primitives
/// 4. **Execution**: `execute()` — produce output pixels
///
/// Inspired by zenimage's `GraphNodeDef` with adaptations for
/// imageflow's multi-pass execution model.
pub trait NodeDef: fmt::Debug + Send + Sync {
    /// Fully qualified name (e.g., `"imageflow.resize"`, `"imageflow.constrain"`).
    fn fqn(&self) -> &'static str;

    /// Short name derived from FQN.
    fn name(&self) -> &'static str {
        self.fqn().rsplit('.').next().unwrap_or(self.fqn())
    }

    /// Required edge configuration.
    fn edges_required(&self) -> (EdgesIn, EdgesOut);

    /// Validate parameters. Called during validation pass.
    fn validate(&self, _params: &NodeParams) -> Result<()> {
        Ok(())
    }

    /// Estimate output frame info from input info.
    ///
    /// Called during estimation pass. Return `FrameEstimate::None` if inputs
    /// aren't ready, `FrameEstimate::Impossible` if decoder info is needed.
    fn estimate(&self, input: Option<FrameInfo>, params: &NodeParams) -> FrameEstimate;

    /// Can this node be expanded into sub-nodes?
    ///
    /// Expansion nodes (Constrain, ApplyOrientation, CommandString) decompose
    /// into primitives during the expansion pass.
    fn can_expand(&self) -> bool {
        false
    }

    /// Streaming support classification.
    fn streaming_support(&self) -> StreamingSupport {
        StreamingSupport::Streaming
    }
}

/// Parameters attached to a graph node.
///
/// Each node carries its operation parameters as an `imageflow_commands::Step`.
/// The `Internal` variant holds engine-generated parameters for expanded nodes.
#[derive(Debug, Clone)]
pub enum NodeParams {
    /// Parameters from a pipeline step command.
    Command(imageflow_commands::Step),
    /// Engine-internal parameters (from node expansion).
    Internal(InternalParams),
}

/// Engine-internal node parameters created during expansion.
#[derive(Debug, Clone)]
pub enum InternalParams {
    /// No-op passthrough.
    Noop,
    /// Resize with resolved dimensions.
    Resize { w: u32, h: u32, filter: Option<imageflow_commands::Filter> },
    /// Crop with resolved coordinates.
    Crop { x1: u32, y1: u32, x2: u32, y2: u32 },
    /// Pad with resolved offsets.
    Pad { left: u32, top: u32, right: u32, bottom: u32, color: [u8; 4] },
}

/// A node in the processing graph.
///
/// Holds the node definition (behavior), parameters (configuration),
/// current estimation state, and execution result.
pub struct GraphNode {
    /// The node definition (behavior).
    pub def: &'static dyn NodeDef,
    /// Node parameters.
    pub params: NodeParams,
    /// Stable identity for debugging.
    pub stable_id: StableNodeId,
    /// Current frame estimate.
    pub frame_estimate: FrameEstimate,
    /// Execution result.
    pub result: NodeResult,
}

impl fmt::Debug for GraphNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GraphNode")
            .field("def", &self.def.fqn())
            .field("stable_id", &self.stable_id)
            .field("estimate", &self.frame_estimate)
            .field("result", &self.result)
            .finish()
    }
}

/// Result of node execution.
#[derive(Debug, Clone, Default)]
pub enum NodeResult {
    /// Not yet executed.
    #[default]
    None,
    /// Result consumed by downstream node.
    Consumed,
    /// Frame data available (for materialized/eager nodes).
    Frame(FrameInfo),
    /// Encoded output (final result at a sink node).
    Encoded { io_id: i32, format: String, w: u32, h: u32, bytes: u64 },
}

impl NodeResult {
    pub fn is_executed(&self) -> bool {
        !matches!(self, NodeResult::None)
    }
}
