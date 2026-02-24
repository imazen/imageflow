# Imageflow Project Instructions

## SIMD & Dispatch Crates

`multiversion` is allowed in this project for autovectorization dispatch on scalar loops (e.g., `scaling.rs`). Prefer the defaults provided by `multiversed` for new code — use `multiversion` only where `multiversed` doesn't fit.

For explicit SIMD intrinsics, use `archmage` (already in use for `transpose.rs`).

## Git Workflow

Always commit `cargo fmt` changes as a separate commit from code changes.

## Delayed TODOs

- **Licensing/caching module** (`imageflow_helpers/src/unused/`): ~2300 lines of draft licensing, caching, and polling code. Currently unreferenced (no `mod` declaration). Needs review, modernization, and wiring into the build when ready to complete.
