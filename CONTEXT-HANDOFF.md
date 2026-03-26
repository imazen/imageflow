# Imageflow3 Context Handoff

## Current State (2026-03-26, session 3 end)

**Branch**: `imageflow3`, **168/192 passing** (87.5%) with v2 baselines.

## Full Pipeline Tracing System — COMPLETE

Built a 4-layer tracing system in zenpipe (11 commits) + zennode (1 commit).
Wired into imageflow via `ZENPIPE_TRACE` env var.

### Usage

```bash
# Print pipeline trace to stderr
ZENPIPE_TRACE=1 just test-filter transparent_png_to_png

# Full tracing with timing
ZENPIPE_TRACE=full just test-filter some_test

# Also write SVG diagram
ZENPIPE_TRACE=svg just test-filter some_test
```

### Architecture

**Layer 1 (RIAPI)**: `KvPairs::snapshot()` in zennode + `build_riapi_trace()` in bridge.
**Layer 2 (Bridge)**: `BridgeTrace` with DAG snapshots, node separation, coalescing.
**Layer 3 (Graph)**: `Tracer` facade — zero-alloc when inactive. Records per-node
  format/dims, implicit ensure_format entries, content-adaptive detection results.
**Layer 4 (Execution)**: Per-node cumulative timing via `Arc<Mutex<NodeTiming>>`.

**Key types**: `Tracer`, `TraceConfig`, `PipelineTrace`, `FullPipelineTrace`,
  `DagSnapshot` (u32 UIDs, edges), `TraceAppender`, `UpstreamMeta`.

**Output formats**: `to_text()`, `to_svg()`, `to_json()`, `to_animated_svg()`.

### Files

| File | What |
|------|------|
| `zenpipe/src/trace.rs` | All trace types, Tracer facade, output formatters, animated SVG |
| `zenpipe/src/graph.rs` | Uses Tracer for compile-time tracing |
| `zenpipe/src/sources/tracing.rs` | TracingSource with timing |
| `zenpipe/src/bridge/mod.rs` | BridgeTrace, record_snapshot, build_riapi_trace |
| `zenpipe/src/orchestrate.rs` | ProcessConfig.trace_config, trace in outputs |
| `zennode/zennode/src/kv.rs` | KvPairs::snapshot() |
| `imageflow_core/src/zen/execute.rs` | ZENPIPE_TRACE env var wiring |

## Identity Resize Bug — CONFIRMED by tracing

Trace output for `transparent_png_to_png`:
```
[  0] Source      500x500  RGBA8 sRGB +a
[  1] Resize      500x500  RGBA8 sRGB +a  -> 500x500 Robidoux
[  2] Output      500x500  RGBA8 sRGB +a
```

The Resize 500x500 → 500x500 is the identity resize — resampling filter runs on
same-size input/output, changing pixel values. This affects ~10 of the 20 failures.

**Fix**: In `compile_node_inner` NodeOp::Resize arm, skip the resize when
`upstream.width() == w && upstream.height() == h && sharpen_percent.is_none()`.
The ensure_format still needs to run (was the previous attempt's regression).

## Remaining 20 Failures

Same as previous handoff — see CONTEXT-HANDOFF.md in git history for the full table.

## Build

```bash
cargo test -p imageflow_core --features "zen-default,c-codecs" --test integration
```

**zenpipe MUST be on `main` branch** (user has `feat/serde` with breaking WIP).
