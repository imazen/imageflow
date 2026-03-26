//! Zen-crate pipeline bridge for imageflow.
//!
//! Translates imageflow v2 [`Node`](imageflow_types::Node) operations into
//! [`zennode::NodeInstance`] objects, resolves format/quality via `zencodecs`,
//! and executes through `zenpipe`'s streaming pipeline.
//!
//! This module is gated behind the `zen-pipeline` feature.

mod translate;
mod preset_map;
mod execute;
mod context_bridge;

pub use execute::{execute_framewise, zen_get_image_info, ZenError, ZenEncodeResult};
pub use translate::TranslateError;
pub use context_bridge::{zen_build, zen_get_image_info as zen_probe, ZenBuildOutput};
