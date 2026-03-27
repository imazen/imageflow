//! Zen-crate streaming pipeline for imageflow.
//!
//! Translates v2 [`Node`](imageflow_types::Node) operations into
//! [`zennode::NodeInstance`] objects, resolves format/quality via `zencodecs`,
//! and executes through `zenpipe`'s streaming pipeline.
//!
//! # Entry points
//!
//! - [`zen_build`] — execute a `Build001` request (extracts IO, runs pipeline)
//! - [`zen_execute`] — execute a `Framewise` with pre-extracted IO bytes
//! - [`zen_get_image_info`] — probe without decoding
//! - [`execute_framewise`] — lower-level, takes `&HashMap<i32, Vec<u8>>`
//!
//! Gated behind the `zen-pipeline` feature.

mod captured;
mod color;
mod context_bridge;
mod converter;
mod execute;
mod nodes;
mod preset_map;
pub mod riapi;
mod translate;

pub use captured::CapturedBitmap;

// High-level API (v2 JSON request types in, JobResult out).
pub use context_bridge::{zen_build, zen_execute, zen_get_image_info, ZenBuildOutput};

// Lower-level API (raw bytes in, ZenEncodeResult out).
pub use execute::{execute_framewise, ZenEncodeResult, ZenError};

// RIAPI expansion.
pub use riapi::RiapiEngine;
