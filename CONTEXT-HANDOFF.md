# Imageflow3 Context Handoff

## What This Is

The `imageflow3` branch rewires imageflow's execution engine to use the zen crate ecosystem (zenpipe, zencodecs, zennode) instead of the v2 C-based graph engine. The v2 JSON API wire format is preserved. The zen pipeline streams — 13MB peak for an 8K image vs 133MB with full-frame.

## Branch State

**Branch**: `imageflow3` (21 commits on `main`)
**Pushed**: yes, to `origin/imageflow3`
**Working tree**: clean
**Test results**: 58/192 integration tests pass with `zen-default` feature. 187/192 pass with v2 engine (1 pre-existing failure).

All zen crate changes are committed and pushed in their respective repos under `~/work/zen/`.

## How to Build and Test

```bash
# v2 engine (default, no zen):
cargo test -p imageflow_core --test integration

# Zen pipeline available alongside v2 (opt-in via v1/zen-build endpoint):
cargo check -p imageflow_core --features "zen-pipeline,c-codecs"

# Zen pipeline as default engine (replaces v2 for all execute_inner/build_inner):
cargo test -p imageflow_core --features "zen-default,c-codecs" --test integration

# Zen-only integration tests (5 tests, always pass):
cargo test -p imageflow_core --features "zen-pipeline,c-codecs" --test zen_pipeline_test

# Heaptrack:
cargo build --example zen_heaptrack -p imageflow_core --features zen-pipeline --no-default-features --release
heaptrack target/release/examples/zen_heaptrack /path/to/image.jpg
```

## File Map

### imageflow (this repo)

```
imageflow_core/src/zen/
├── mod.rs              (32 lines)  Module root, public exports
├── execute.rs          (644 lines) Orchestration: decode → pipeline → encode
├── translate.rs        (477 lines) v2 Node → zennode NodeInstance mapping
├── preset_map.rs       (332 lines) v2 EncoderPreset → zencodecs CodecIntent
├── context_bridge.rs   (164 lines) Build001/Execute001 → zen pipeline → JobResult
├── riapi.rs            (162 lines) Dual RIAPI: legacy (Ir4Expand) + zen-native
├── captured.rs         (24 lines)  CapturedBitmap struct (should be replaced)

imageflow_core/src/context.rs       Modified: zen_input_bytes, zen_captured_bitmaps,
                                    zen_execute_inner, zen-default routing
imageflow_core/src/codecs/mod.rs    Modified: write_output_bytes on CodecInstanceContainer
imageflow_core/src/json/endpoints/v1.rs  Modified: v1/zen-build endpoint
imageflow_core/src/lib.rs           Modified: #[cfg(feature = "zen-pipeline")] pub mod zen
imageflow_core/Cargo.toml           Modified: zen crate deps, zen-pipeline/zen-default features
imageflow_core/tests/zen_pipeline_test.rs     5 integration tests for zen path
imageflow_core/tests/integration/common/mod.rs  Modified: zen_captured_bitmaps fallback
imageflow_core/examples/zen_heaptrack.rs      Heaptrack profiling binary

IMAGEFLOW3-PLAN.md                  Design document (partially outdated — see memory)
```

### Zen crate changes (all committed, in ~/work/zen/)

```
zencodec/     0.1.5 published: DynEncoder: Send, consuming job(self), box_streaming_decoder
zencodecs/    StreamingEncoder + Send, JPEG/PNG streaming decode dispatch via job_static
zenpipe/      ordering.rs, DecoderSource lazy format, bridge param fixes (down_filter)
zenjpeg/      Cow::Owned streaming decode, owned config, job_static()
zenpng/       Cow::Owned streaming decode, owned config, job_static()
zengif/       zencodec 0.1.5 consuming job(self)
zenjxl/       zencodec 0.1.5 consuming job(self)
zenavif/      zencodec 0.1.5 consuming job(self)
zenwebp/      zencodec 0.1.5 consuming job(self)
zenbitmaps/   zencodec 0.1.5 consuming job(self)
zentiff/      zencodec 0.1.5 consuming job(self)
heic/         zencodec 0.1.5 consuming job(self)
```

## What Works

- **Streaming decode** for JPEG and PNG (Cow::Owned + job_static → 'static decoders)
- **Streaming pipeline** through zenpipe (strip-based, zero materialization between ops)
- **Streaming encode** via EncoderSink (DynEncoder: Send)
- **Geometry**: FlipH/V, Rotate90/180/270, Transpose, Crop, Constrain (all 9 modes), Region, ExpandCanvas, ApplyOrientation
- **Filters**: Grayscale, Sepia, Invert, Contrast, Brightness, Saturation (mapped to zenfilters)
- **Resize**: Via zenresize Constrain node with all filter types
- **Format selection**: zencodecs Auto/Format/legacy presets, quality profiles, DPR adjustment
- **RIAPI**: CommandString expansion via Ir4Expand, zen-native path via zennode registry
- **DAG execution**: Multi-input compositing, fan-out materialization via build_pipeline_dag
- **CreateCanvas**: Solid-color MaterializedSource
- **CaptureBitmapKey**: Dimension capture (pixel data capture partially working)
- **Heaptrack verified**: 13MB for 8K, 10.7MB for 1.4K, 24.6MB for 5K

## Test Failure Breakdown (130 failures with zen-default)

| Category | Count | Root Cause | Fix Location |
|----------|-------|------------|-------------|
| ICC visual | 23 | No ICC→sRGB transform in pipeline | zenpipe: insert IccTransformSource |
| Codec visual | 20 | Different rendering engine, checksum mismatches | New baselines after other fixes |
| Smoke | 18 | Mix: animated GIF, security limits, bitmap capture | Multiple zen crates |
| Canvas | 18 | FillRect, RoundCorners unsupported | zenlayout: add nodes |
| Encoder | 11 | CaptureBitmapKey needs bitmap data not just dims | Farbfeld encode at capture |
| Scaling | 9 | Resize rendering differences | Investigate: filter/colorspace diffs |
| PNG CMS | 9 | gAMA/cHRM/cICP handling | zenpipe: ICC transform node |
| Watermark | 7 | Composition not wired in translate.rs | Wire DAG composite path |
| Trim | 5 | CropWhitespace unsupported | Already in zenpipe, needs wiring |
| Orientation | 4 | Likely EXIF handling edge cases | Investigate |
| Color conv | 3 | Matte compositing differences | Investigate |
| IDCT | 2 | JPEG decode differences (zenjpeg vs mozjpeg) | Expected |

## Architectural Issues to Fix

### 1. execute.rs is a monolith (644 lines)

**Problem**: Three code paths for decode (streaming, full-frame, canvas), three for encode (EncoderSink, materialize+one-shot, canvas encode), three for capture (stream, materialize+capture, canvas+capture). Every new feature multiplies the paths.

**Fix**: Use `zenpipe::orchestrate::stream()` which already handles decode/encode separation via the bridge. execute.rs should be ~200 lines: probe → translate → hand to orchestrate → collect results.

### 2. Side-channel anti-pattern in TranslatedPipeline

**Problem**: `TranslatedPipeline` has fields for decode_io_id, encode_io_id, decoder_commands, create_canvas, preset — all extracted as side-channels during translation. execute.rs then handles each specially.

**Fix**: Translate everything to zennode instances. Decode/Encode-role nodes get separated by zenpipe's bridge naturally. CreateCanvas becomes a Source-role zennode node. The bridge already handles role-based separation.

### 3. CaptureBitmapKey should produce farbfeld output buffer

**Problem**: Currently stores raw pixels in CapturedBitmap with manual metadata. Tests that need pixel comparison (not just dimensions) don't work. Feature-gated test code (`#[cfg(feature = "zen-pipeline")]`) is fragile.

**Fix**: At capture point, materialize the pipeline, encode to farbfeld (lossless, exact pixels), store as output buffer with synthetic io_id. Test infra decodes farbfeld for dimensions and pixel comparison. No feature gates needed in test code.

### 4. zen_input_bytes stash is wrong

**Problem**: Input bytes are copied into a HashMap on Context when added via add_copied_input_buffer/add_input_vector. This doubles memory for every input.

**Fix**: Add `read_all_bytes()` to IoProxy — seek to 0, read_to_end, seek back. IoProxy implements Read + Seek already. No copy needed.

### 5. sRGB conversion doesn't handle ICC profiles

**Problem**: `ensure_srgb_rgba8()` converts the pixel format descriptor but doesn't do ICC→sRGB transforms. Images with embedded ICC profiles (Display P3, Adobe RGB, etc.) won't be correctly converted to sRGB.

**Fix**: Use zenpipe's `IccTransformSource` with moxcms. The transform should be inserted by the bridge or orchestration layer when the decode source has a non-sRGB ICC profile. This is a zenpipe bridge enhancement.

### 6. Missing zennode nodes for v2 operations

Operations that return `Unsupported` in translate.rs and need zennode nodes:

| v2 Node | Zen Crate | Node Needed |
|---------|-----------|-------------|
| FillRect | zenlayout | FillRect node (draw solid rect on existing image) |
| RoundImageCorners | zenlayout or new | RoundCorners node |
| DrawImageExact | zenpipe | Composite with resize (DAG) |
| Watermark | zenpipe | Composite with gravity/opacity (DAG) |
| CropWhitespace | zenpipe | Already has CropWhitespace NodeOp, needs bridge |
| WhiteBalanceHistogramAreaThresholdSrgb | zenfilters | Analysis + correction node |
| ColorMatrixSrgb | zenfilters | 5×5 matrix transform node |
| CreateCanvas | zennode | Source-role node for solid color |

## Design Principles (Established This Session)

1. **Scene-referred by default, sRGB opt-in.** Pipeline preserves source color space. sRGB conversion is explicit.
2. **Fix gaps in zen crates, not in imageflow shim.** Every test failure should map to a zen crate improvement.
3. **Keep zennode in the path.** V2→zennode translation exercises v-next code.
4. **Dual paths for A/B comparison.** Legacy RIAPI (imageflow_riapi) + zen-native (zennode registry).
5. **Streaming by default.** Full-frame materialization only when unavoidable (analysis ops, fan-out).
6. **No unsafe, no hacks.** The consuming `job(self)` and `Cow::Owned` solutions were clean. The `Send` issue was solved by questioning whether `Send` was needed at all (it wasn't for the encode loop).

## Key Decisions Made

- **zencodec 0.1.5**: `DecoderConfig::job(self)` consumes config (no GAT lifetime). Published.
- **zencodec 0.1.4**: `DynEncoder: Send` supertrait. Published.
- **Streaming decode**: JPEG/PNG via Cow::Owned + job_static. Config cloned into job.
- **Streaming encode**: Direct push_rows loop (no Send needed). EncoderSink also works now.
- **Format discovery**: DecoderSource eagerly decodes first batch to discover pixel format.
- **v2 compat**: `zen-default` feature routes execute_inner/build_inner through zen pipeline.
- **Ordering**: zenpipe's `ordering.rs` has canonical_sort + optimize_node_order. Imageflow tells it which strategy.
- **v3 wire format**: Separate types (NodeV3, Build003) — v2 types frozen. Design documented but not implemented yet.

## What I'd Do Differently

### Don't build execute.rs incrementally

I built execute.rs by starting with the simplest case (linear steps, JPEG in, JPEG out) and bolting on features one at a time: CommandString expansion, CreateCanvas, CaptureBitmapKey, DAG mode, sRGB conversion. Each addition created a new code path instead of generalizing the existing one. The result is 644 lines of spaghetti with 9 branches (3 decode × 3 encode).

**Next time**: Start from zenpipe's `orchestrate::stream()` API which already handles the probe→decode→pipeline→encode flow. Write the v2→zennode translation, hand the nodes to orchestrate, collect results. Add features by adding zennode nodes, not by adding code paths in execute.rs.

### Don't stash bytes on Context

I added `zen_input_bytes: HashMap<i32, Vec<u8>>` to Context and hooked every `add_input_*` method to copy bytes into it. This doubles memory for every input and is fragile (easy to miss a code path — `add_input_buffer` was missed initially).

**Next time**: Add `IoProxy::read_to_vec(&mut self) -> Vec<u8>` (seek to 0, read all, seek back). Read bytes on demand from the existing IoProxy when the zen pipeline needs them. Zero extra copies.

### Don't invent CapturedBitmap

I created `CapturedBitmap { width, height, pixels, format }` — a poor man's pixel buffer. Then I added `zen_captured_bitmaps` to Context and `#[cfg(feature)]` branches in test code.

**Next time**: Encode to farbfeld (or BMP — both are lossless, trivial format) at the capture point. Store as an output buffer with a synthetic io_id (e.g., `io_id = -capture_id - 1`). The test infrastructure reads the buffer and decodes it. No new types, no feature gates in tests, no raw pixel management.

### Don't handle CreateCanvas as a special source in execute.rs

I added a `create_canvas` field to `TranslatedPipeline` and a separate 50-line code path in execute.rs for canvas sources. This duplicates the encode/capture logic.

**Next time**: Make CreateCanvas a zennode node with `NodeRole::Decode` (source role). The zenpipe bridge separates Decode-role nodes and uses them to build sources. The orchestration layer sees "decode config says solid color" and creates a filled MaterializedSource. No special case in execute.rs.

### Don't put sRGB conversion in execute.rs

I added `ensure_srgb_rgba8()` as an ad-hoc wrapper around the decode source. It converts the pixel format descriptor but doesn't do ICC transforms. Real images with Display P3 or Adobe RGB ICC profiles need moxcms, not a format swap.

**Next time**: ICC→sRGB should be a pipeline option, not a post-decode hack. zenpipe has `IccTransformSource`. The bridge should insert it when the user requests sRGB output (v2 compat default). This is a zenpipe bridge enhancement — add a `color_management: Option<CmsPolicy>` to `ProcessConfig`.

### Don't fix zen crate API issues with workarounds in imageflow

When the JPEG streaming decoder needed `'static` but produced `'a`, I first tried transmute, then Box::leak, then job_static(). Each was a workaround for a zencodec trait limitation. The right fix was changing the trait: `job(self)` consuming. That took longer to land but was permanent.

**Next time**: When a zen crate API doesn't fit, fix the API. Don't accumulate workarounds in the consumer. The consumer (imageflow) should be thin. Every workaround is technical debt that v-next inherits.

### Don't expand CommandString in execute.rs

I added `expand_command_strings()` in execute.rs — 60 lines that probe the source, find the decode io_id (from both Node::Decode and CommandString::decode), call Ir4Expand, inject Decode nodes, and splice expanded steps into the node list. This is graph construction logic that doesn't belong in the execution module.

**Next time**: Handle CommandString in translate.rs (or a separate expansion module). The expansion is a pre-processing step that converts `[CommandString]` → `[Decode, Constrain, Encode, ...]`. It should happen before translation, not interleaved with it. The expanded steps then flow through the normal translate → bridge → execute path.

### Do build integration tests from day one

I wrote the execute.rs pipeline first, then the heaptrack profiling binary, then the endpoint wiring, and only added integration tests late. By then, architectural issues (DuplicateIoId, missing decode info, CaptureBitmapKey) surfaced as test failures that required ad-hoc fixes.

**Next time**: Write one integration test that exercises the full JSON API path (Build001 → zen pipeline → output buffer → verify) before building execute.rs. Let the test drive the API design. The test tells you immediately when a design choice doesn't fit the real usage pattern.

## What to Read First

1. `IMAGEFLOW3-PLAN.md` — overall design (v2/v3 versioning, ordering, file structure)
2. `imageflow_core/src/zen/execute.rs` — current orchestration (needs refactor)
3. `imageflow_core/src/zen/translate.rs` — v2 Node → zennode mapping
4. `/home/lilith/work/zen/zenpipe/src/bridge/mod.rs` — zenpipe's bridge API
5. `/home/lilith/work/zen/zenpipe/src/orchestrate.rs` — zenpipe's high-level API
6. `/home/lilith/work/zen/zenpipe/ORDERING-DESIGN.md` — ordering model
7. Memory: `~/.claude/projects/-home-lilith-work-imageflow/memory/project_imageflow3_bridge.md`
