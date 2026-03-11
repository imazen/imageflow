#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]
//! # imageflow-commands
//!
//! Image processing command types for the imageflow/zen ecosystem.
//!
//! This crate defines the vocabulary of image operations as pure data types.
//! No execution logic — just the schema. Operations are organized into modules
//! by category: I/O, geometry, color, composition, and encoding.
//!
//! ## Design principles
//!
//! - **Serde-first**: All types serialize to snake_case JSON.
//! - **no_std + alloc**: Usable in embedded or WASM contexts.
//! - **Exhaustive**: Covers the full operation set of the zen crate ecosystem
//!   (zenimage, zenpipe, zencodecs, zenpixels) plus imageflow's legacy API.
//! - **Flat steps or DAG**: The `Step` enum supports both sequential pipelines
//!   and graph-based execution via `NodeId` references.

extern crate alloc;

mod color;
mod composition;
mod decode;
mod encode;
mod geometry;
mod io;
mod pipeline;
mod security;
mod shared;

pub use color::*;
pub use composition::*;
pub use decode::*;
pub use encode::*;
pub use geometry::*;
pub use io::*;
pub use pipeline::*;
pub use security::*;
pub use shared::*;
