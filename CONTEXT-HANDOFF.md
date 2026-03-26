# Imageflow3 Context Handoff

## Current State (2026-03-26, session 2 end)

**Branch**: `imageflow3`, **168/192 passing** (87.5%) with v2 baselines.
Started at 58/192. Fixed +110 tests.

## Build

```bash
cargo test -p imageflow_core --features "zen-default,c-codecs" --test integration
# With baseline auto-accept:
UPDATE_CHECKSUMS=1 cargo test -p imageflow_core --features "zen-default,c-codecs" --test integration
```

**IMPORTANT**: zenpipe must be on `main` branch. The user has a `feat/serde` branch with breaking WIP — `cd ~/work/zen/zenpipe && git checkout main` before building.

## Workspace patches

`Cargo.toml` patches `zenpixels` and `zenpixels-convert` to local paths (`../zen/zenpixels/`). This affects BOTH v2 and zen paths — the zenpixels color correctness fix changes v2 output too. S3 baselines were generated pre-patch.

## Remaining 20 Failures

### Root cause: RIAPI identity Resample2D (affects ~10 tests)

`Ir4Expand` generates `Resample2D(WxH)` with `resample_when: SizeDiffersOrSharpeningRequested` even for `format=png` with no resize. The zen pipeline:
1. Translates this to a `NodeOp::Resize`
2. The graph compiles it through `ensure_format(RGBA8_SRGB)` + `ResizeSource`
3. Even identity resize (same dims) runs the resampling filter, changing pixel values

**The fix**: Skip `NodeOp::Resize` compilation when input dims == output dims AND no sharpening. My attempt at this in `graph.rs` caused regressions because skipping also skipped `ensure_format`. The fix needs to preserve format conversion but skip the actual resize.

Affected: transparent_png_to_png, transparent_png_to_png_rounded_corners, transparent_webp_to_webp, webp_to_webp_quality, webp_lossless/lossy_alpha_decode_and_scale, trim_whitespace, round_corners_command_string, webp_lossy_noalpha

### Other failures

| Test | Cause |
|------|-------|
| pngquant_command (2) | RIAPI `png.quality` keys not mapping to Pngquant preset — produces Lodepng |
| jpeg_crop | Off-by-4 from different JPEG IDCT |
| transparent_png_to_jpeg | Matte compositing on transparent PNG → JPEG |
| icc_display_p3_resize_filter | P3→sRGB gamut mapping changed by zenpixels fix |
| icc_p3_crop_and_resize | Same |
| jpeg_rotation_cropped | Uniform +29 brightness from decode |
| problematic_png_lossy | Different PNG quantization |
| corrupt_jpeg | Zen decoder more tolerant |
| png_cicp_bt709 | CICP transform not applied |
| branching_crop_whitespace | DAG mode CropWhitespace missing edge |

## Pipeline Tracing System (in progress)

Started in `~/work/zen/zenpipe/src/trace.rs` and `src/sources/tracing.rs`:
- `TraceConfig`: metadata-only or with PNG16 pixel dump
- `TracingSource`: identity passthrough that records format/dims at each node
- `PipelineTrace`: `to_text()` tabular output + `to_svg()` flow diagram
- `compile_traced()` on PipelineGraph wraps every node
- Gated behind `std` feature

**What's missing**:
- Wire through `build_pipeline()` and imageflow's `execute.rs`
- Per-node config/parameter capture in trace entries
- Pixel dump to actual PNG16 (currently writes raw bytes)
- SVG timeline/animation showing graph mutations
- `ensure_format()` tracing (implicit conversions not yet visible)
- Integration tests

## Zen Crate Changes (all on local `main` branches)

```
zenpipe/     trace.rs, sources/tracing.rs, graph compile_traced, ExpandCanvas/FillRect NodeOp,
             geometry fusion fixes, mixed coalesce fix, identity resize (reverted)
zenfilters/  node_to_filter() bridge
zengif/      ensure GIF trailer 0x3B
zencodec/    (reverted SourceColor fields — needs proper publish)
zensim/      score=100 for identical images, ignore RGB at alpha=0
```

## Key Mistakes This Session

1. Questioned the test system instead of debugging the actual code
2. Made random fixes without tracing — caused regressions from identity resize skip
3. Added `RemoveAlpha` for `Transparent` background_color — stripped alpha from all PNGs
4. Didn't build tracing system FIRST — would have caught all issues immediately
5. Repeatedly had to switch zenpipe from feat/serde back to main

## What to Do Next

1. **Complete pipeline tracing** — wire through build_pipeline, add ensure_format visibility
2. **Use tracing to fix identity resize** — the trace will show exactly where format changes
3. **Fix remaining 20** with full visibility into every conversion
4. **Move shims to zen crates**: gAMA/cHRM parsing → zenpng, linear matte → zenblend, animation → zenpipe
