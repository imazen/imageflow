//! Dependency aggregator for the Imageflow crates.
//!
//! This crate (lib name `imageflow_api`) carries no code of its own; it exists so
//! that a single dependency pulls in `imageflow_types`, `imageflow_helpers`,
//! `imageflow_riapi` and `imageflow_core` together. There is nothing here to test —
//! it previously held an `it_works` that asserted `2 + 2 == 4`, which passed
//! whatever this crate did or did not export.
