//! C ABI for imageflow — stable FFI surface for language bindings.
//!
//! All functions are thread-safe. A single context may be accessed from
//! multiple threads (internal locks serialize access).
//!
//! # ABI Version 4.0
//!
//! Breaking change from v3: internals fully rewritten to use zen crate ecosystem.
//! The C function signatures and JSON wire format are preserved for compatibility.

#![forbid(unsafe_code)]

include!("abi_version.rs");

use imageflow_core::Context;
use std::sync::Arc;

/// Opaque context handle passed across FFI boundary.
///
/// Wraps `imageflow_core::Context` in an `Arc` for shared ownership.
pub struct ImageflowContext {
    inner: Arc<Context>,
}

/// JSON response handle — owns the response bytes.
pub struct ImageflowJsonResponse {
    status_code: i32,
    json: Vec<u8>,
}

impl ImageflowContext {
    fn new() -> Self {
        Self {
            inner: Arc::new(Context::new()),
        }
    }
}

// ─── Public API (safe Rust, intended for Rust callers) ─────────────────

impl ImageflowContext {
    /// Create a new context.
    pub fn create() -> Box<Self> {
        Box::new(Self::new())
    }

    /// Add an input buffer.
    pub fn add_input_buffer(&self, io_id: i32, data: &[u8]) -> Result<(), String> {
        self.inner
            .add_input_buffer(io_id, data)
            .map_err(|e| e.to_string())
    }

    /// Add an output buffer slot.
    pub fn add_output_buffer(&self, io_id: i32) -> Result<(), String> {
        self.inner
            .add_output_buffer(io_id)
            .map_err(|e| e.to_string())
    }

    /// Get the output buffer for a given io_id.
    pub fn get_output_buffer(&self, io_id: i32) -> Result<Vec<u8>, String> {
        self.inner
            .get_output_buffer(io_id)
            .map_err(|e| e.to_string())
    }

    /// Send a JSON message and get a JSON response.
    pub fn send_json(&self, method: &str, json: &[u8]) -> ImageflowJsonResponse {
        let resp = self.inner.send_json(method, json);
        ImageflowJsonResponse {
            status_code: resp.status_code,
            json: resp.response_json,
        }
    }

    /// ABI version compatibility check.
    pub fn abi_compatible(major: u32, minor: u32) -> bool {
        major == IMAGEFLOW_ABI_VER_MAJOR && minor <= IMAGEFLOW_ABI_VER_MINOR
    }
}

// Note: The actual C FFI functions (extern "C") will be added when
// the safe Rust API is stable. For now, language bindings can use
// ImageflowContext directly from Rust.
//
// The C ABI will preserve these function signatures:
//   imageflow_context_create() -> *mut ImageflowContext
//   imageflow_context_destroy(*mut ImageflowContext)
//   imageflow_context_send_json(*mut ImageflowContext, method, json) -> *mut JsonResponse
//   imageflow_context_add_input_buffer(*mut ImageflowContext, io_id, *const u8, len)
//   imageflow_context_add_output_buffer(*mut ImageflowContext, io_id)
//   imageflow_context_get_output_buffer_by_id(*mut ImageflowContext, io_id, *mut *const u8, *mut usize)
//   imageflow_json_response_read(*mut JsonResponse, *mut i32, *mut *const u8, *mut usize)
//   imageflow_json_response_destroy(*mut JsonResponse)
//   imageflow_abi_compatible(major, minor) -> bool
//   imageflow_abi_version_major() -> u32
//   imageflow_abi_version_minor() -> u32
