//! Unified error type for imageflow_core.

use std::fmt;

#[derive(Debug)]
pub enum FlowError {
    /// Invalid JSON in request.
    InvalidJson(serde_json::Error),
    /// Invalid pipeline configuration.
    InvalidPipeline(String),
    /// I/O object not found.
    IoNotFound(i32),
    /// Codec error (decode/encode failure).
    Codec(String),
    /// Layout computation failed.
    Layout(String),
    /// Resize operation failed.
    Resize(String),
    /// Color management error.
    ColorManagement(String),
    /// Security limit exceeded.
    LimitExceeded(String),
    /// Operation cancelled.
    Cancelled,
    /// Generic internal error.
    Internal(String),
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlowError::InvalidJson(e) => write!(f, "invalid JSON: {e}"),
            FlowError::InvalidPipeline(msg) => write!(f, "invalid pipeline: {msg}"),
            FlowError::IoNotFound(id) => write!(f, "I/O object not found: {id}"),
            FlowError::Codec(msg) => write!(f, "codec error: {msg}"),
            FlowError::Layout(msg) => write!(f, "layout error: {msg}"),
            FlowError::Resize(msg) => write!(f, "resize error: {msg}"),
            FlowError::ColorManagement(msg) => write!(f, "color management error: {msg}"),
            FlowError::LimitExceeded(msg) => write!(f, "limit exceeded: {msg}"),
            FlowError::Cancelled => write!(f, "operation cancelled"),
            FlowError::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for FlowError {}

impl From<serde_json::Error> for FlowError {
    fn from(e: serde_json::Error) -> Self {
        FlowError::InvalidJson(e)
    }
}

/// HTTP status code for an error.
impl FlowError {
    pub fn http_status(&self) -> u32 {
        match self {
            FlowError::InvalidJson(_) | FlowError::InvalidPipeline(_) => 400,
            FlowError::IoNotFound(_) => 404,
            FlowError::LimitExceeded(_) => 413,
            FlowError::Cancelled => 503,
            _ => 500,
        }
    }
}
