# Imageflow Project Instructions

## SIMD & Dispatch Crates

`multiversion` is allowed in this project for autovectorization dispatch on scalar loops (e.g., `scaling.rs`). Prefer the defaults provided by `multiversed` for new code — use `multiversion` only where `multiversed` doesn't fit.

For explicit SIMD intrinsics, use `archmage` (already in use for `transpose.rs`).

## f32/f64 Clamping

**Do NOT replace `min(max(...))` patterns with `.clamp()` on floats.** `f32::clamp()` propagates NaN, while `min(max(...))` suppresses it. In image processing pipelines, NaN propagation turns a single bad pixel into a full-image corruption. The `min(max(...))` pattern is intentional NaN defense.

## Git Workflow

Always commit `cargo fmt` changes as a separate commit from code changes.

## Test Commands

All integration tests live in `imageflow_core/tests/integration/` as a single binary.

```bash
just test                    # run all tests with nextest
just test-filter NAME        # run tests matching NAME
just test-create-baselines   # run tests, create baselines for NEW tests only
just test-list               # list all test names
just test-build              # compile-check tests without running
```

**There is no replace/update command.** Checksums are append-only. Existing baselines
are never overwritten. Tests pass if the hash matches OR the output is within the
section's declared tolerance (via zensim pixel comparison). New baselines are only
created for tests that have no existing section in the checksums file.

Checksum files: `imageflow_core/tests/integration/visuals/*.checksums`
Reference images: `imageflow_core/tests/integration/visuals/images/`

## Zen Crate Delegation

Pipeline filters (blur, sharpen, color_filter, color_adjust, color_matrix) delegate to `zenfilters::srgb_filters`. Premultiply in `canvas_to_premul_f32` uses `garb::bytes::premultiply_alpha_f32`.

Compositing (DrawImage, Watermark) delegates to `zenresize::execute_layout_with_background` with `SliceBackground` for canvas-sized backgrounds. zenresize handles OffsetBackground wrapping and canvas fill internally.

## Premultiply Dedup Status

garb is the canonical SIMD premultiply. Cross-crate dedup is limited because:
- zenimage has BGRA layout + fused srgb↔linear↔premul (garb is RGBA-only, premul-only)
- zenjxl-decoder has generic channel count + 1e-10 threshold + jxl_simd framework
- zenresize has test-only utilities with intentionally different thresholds

Further consolidation requires adding BGRA support and fused operations to garb.

## Delayed TODOs

- **Licensing/caching module** (`imageflow_helpers/src/unused/`): ~2300 lines of draft licensing, caching, and polling code. Currently unreferenced (no `mod` declaration). Needs review, modernization, and wiring into the build when ready to complete.
