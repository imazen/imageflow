#![forbid(unsafe_code)]

use core::fmt;

use crate::NodeIndex;

/// Graph engine result type.
pub type Result<T> = core::result::Result<T, GraphError>;

/// Errors from graph construction and execution.
#[derive(Debug)]
pub enum GraphError {
    /// A node references a nonexistent node index.
    InvalidNodeIndex(NodeIndex),
    /// Edge requirements violated (wrong number of inputs/outputs).
    InvalidEdges { node: NodeIndex, message: String },
    /// Node parameters failed validation.
    InvalidParams { node: NodeIndex, message: String },
    /// Graph contains a cycle (not a DAG).
    CycleDetected,
    /// Estimation failed — can't determine output dimensions.
    EstimationFailed { node: NodeIndex, message: String },
    /// Expansion exceeded maximum iterations (likely infinite loop).
    ExpansionLimitExceeded { max_passes: u32 },
    /// Node doesn't support the requested execution mode.
    UnsupportedOperation { node: NodeIndex, operation: String },
    /// I/O binding not found for the given id.
    IoNotFound(i32),
    /// Resource limit exceeded.
    LimitExceeded(String),
    /// Pipeline execution timeout.
    Timeout,
    /// External error (from codec, resize, etc.).
    External(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphError::InvalidNodeIndex(ix) => write!(f, "invalid node index: {ix:?}"),
            GraphError::InvalidEdges { node, message } => {
                write!(f, "invalid edges at {node:?}: {message}")
            }
            GraphError::InvalidParams { node, message } => {
                write!(f, "invalid params at {node:?}: {message}")
            }
            GraphError::CycleDetected => write!(f, "cycle detected in graph"),
            GraphError::EstimationFailed { node, message } => {
                write!(f, "estimation failed at {node:?}: {message}")
            }
            GraphError::ExpansionLimitExceeded { max_passes } => {
                write!(f, "expansion exceeded {max_passes} passes")
            }
            GraphError::UnsupportedOperation { node, operation } => {
                write!(f, "unsupported operation at {node:?}: {operation}")
            }
            GraphError::IoNotFound(id) => write!(f, "I/O binding not found: {id}"),
            GraphError::LimitExceeded(msg) => write!(f, "resource limit exceeded: {msg}"),
            GraphError::Timeout => write!(f, "pipeline execution timed out"),
            GraphError::External(e) => write!(f, "external error: {e}"),
        }
    }
}

impl std::error::Error for GraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GraphError::External(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for GraphError {
    fn from(e: Box<dyn std::error::Error + Send + Sync>) -> Self {
        GraphError::External(e)
    }
}
