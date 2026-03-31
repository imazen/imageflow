//! Zen-crate streaming pipeline for imageflow.
//!
//! This module re-exports the imageflow v2 compatibility layer from
//! `zenpipe::imageflow_compat`. The actual implementation lives upstream
//! in the zenpipe crate.
//!
//! # Entry points
//!
//! - [`zen_build`] — execute a `Build001` request
//! - [`zen_execute`] — execute a `Framewise` with pre-extracted IO bytes
//! - [`zen_get_image_info`] — probe without decoding
//!
//! Gated behind the `zen-pipeline` feature.

// The v2 compatibility layer lives in zenpipe::imageflow_compat.
// context_bridge stays here because it depends on imageflow_core::errors.
mod context_bridge;

// Re-export the public API.
pub use context_bridge::{zen_build, zen_execute, zen_get_image_info, ZenBuildOutput};
pub use zenpipe::imageflow_compat::captured::CapturedBitmap;
pub use zenpipe::imageflow_compat::execute::{execute_framewise, ZenEncodeResult, ZenError};
pub use zenpipe::imageflow_compat::riapi::RiapiEngine;
