#![forbid(unsafe_code)]
//! # imageflow_core v4
//!
//! Image processing pipeline built on the zen crate ecosystem.
//! This is orchestration glue — all real work is delegated to:
//!
//! - **zencodecs** — decode/encode all formats
//! - **zenlayout** — geometry computation, RIAPI, constraint modes
//! - **zenresize** — 31-filter SIMD resampler with streaming
//! - **zenpixels** — pixel types, ICC/CICP, format descriptors
//! - **zenpixels-convert** — format conversion, CMS, negotiation
//! - **zenjpeg** — lossless JPEG ops, quality probe, re-encode calibration
//! - **ultrahdr-core** — UltraHDR gain map encode/decode

pub mod context;
pub mod error;
pub mod io;
pub mod pipeline;

pub use context::Context;
pub use error::FlowError;
pub use imageflow_types as types;
pub use io::{IoDirection, IoProxy};

// Re-exports for convenience
pub use enough::{Stop, Unstoppable};
