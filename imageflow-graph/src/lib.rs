#![forbid(unsafe_code)]
//! # imageflow-graph
//!
//! Graph-based image processing engine for the imageflow/zen ecosystem.
//!
//! This crate provides the execution engine that takes `imageflow-commands`
//! pipeline definitions and executes them. It supports both sequential
//! pipelines and DAG-based graphs.
//!
//! ## Architecture
//!
//! The engine follows a multi-phase execution model inspired by zenimage:
//!
//! 1. **Build** — Parse commands into an internal graph representation
//! 2. **Validate** — Check edge counts, parameter validity
//! 3. **Estimate** — Propagate frame dimensions through the graph
//! 4. **Expand** — Decompose high-level nodes (Constrain → Resize + Crop)
//! 5. **Optimize** — Fuse compatible operations, detect fast paths
//! 6. **Execute** — Stream pixels through the compiled pipeline
//!
//! ## Design principles
//!
//! - **Pull-based streaming**: Encoders pull strips from upstream sources.
//!   Memory is O(strip_height × width × depth), not O(width × height).
//! - **Operation fusion**: Adjacent per-pixel operations are fused into
//!   a single pass (from zenpipe's `TransformSource` pattern).
//! - **Capabilities-based dispatch**: Nodes declare streaming support,
//!   edge requirements, and expansion capability — the engine adapts.
//! - **Fast paths**: Lossless JPEG transforms, decode-time downscaling,
//!   passthrough when no pixel operations are needed.

mod builder;
pub mod codec_selector;
mod compiler;
mod engine;
mod error;
mod estimate;
pub mod key_router;
mod node;
pub mod quality;
pub mod srcset;

pub use builder::GraphBuilder;
pub use compiler::CompiledGraph;
pub use engine::{
    EncodeOutput, ExecutionContext, ExecutionResult, FrameHandle, GraphEngine, StepOutput,
};
pub use error::{GraphError, Result};
pub use estimate::{FrameEstimate, FrameInfo};
pub use node::{
    EdgeKind, GraphNode, InternalParams, NodeDef, NodeIndex, NodeParams, NodeResult, StableNodeId,
    StreamingSupport,
};
