# Imageflow 3 Plan: v2-Compatible Zenpipe Wrapper

## Architecture Overview

Imageflow 3 is a thin orchestration layer that:
1. Preserves the imageflow v2 JSON API wire format exactly (v1/ endpoints)
2. Offers a v3 superset wire format (v3/ endpoints) with new capabilities
3. Delegates all pixel processing to zenpipe (streaming, zero-materialization)
4. Delegates all codec dispatch to zencodecs (quality calibration, format selection)
5. Delegates RIAPI parsing to imageflow_riapi (existing, well-tested)
6. Works scene-referred: sRGB normalization is opt-in, not default

## Versioning Model

**Explicit versioning from the envelope down.** No silent behavior changes.

### v2 Types (Frozen)
- `Build001`, `Execute001`, `Node`, `EncoderPreset`, `DecoderCommand`
- Never modified. The v2 wire contract.
- Used by v1/ endpoints.

### v3 Types (Superset)
- `Build003`, `Execute003`, `NodeV3`, `EncoderPresetV3`, `DecoderCommandV3`
- Every v2 variant exists in v3 with identical serde names.
- v3 adds new variants and optional fields.
- Mechanical `From<V2> for V3` conversion at the endpoint boundary.
- Used by v3/ endpoints.

### Why Two Enums
- Adding variants to `Node` would let v2 clients accidentally construct v3-only requests.
- Separate enums make the version boundary explicit at the type level.
- Internally, only `NodeV3` flows through the pipeline.

## v3 Additions (Superset of v2)

### New Node Variants
- `ColorAdjust { brightness, contrast, saturation, vibrance, exposure }` — Oklab-space
- `Sharpen { amount }` — explicit (v2 only has it via ResampleHints)
- `Blur { sigma }`
- `Orient { mode: Auto | Exif(u8) }` — cleaner than v2's `ApplyOrientation { flag: i32 }`
- `SrcsetString { value, decode, encode }` — compact RIAPI syntax (v3-only node)

### New Encoder Presets
- `Avif { quality, speed, alpha_quality, lossless }`
- `Jxl { quality, distance, effort, lossless }`
- `Heic { quality }`

### Extended Auto/Format Presets (v3 fields)
- `quality_target: Option<QualityTarget>` — MatchSource, Butteraugli, Ssimulacra2
- `output_color: Option<OutputColor>` — Preserve (default), Srgb, DisplayP3
- `ultrahdr: Option<UltraHdrEncode>`

### New Decoder Commands (v3)
- `ColorHandling { icc, profile_errors }` — explicit ICC handling
- `UltraHdrMode(UltraHdrDecodeMode)` — SDR-only, HDR reconstruct, preserve layers

### New AllowedFormats Flags
- `heic: Option<bool>`
- `ultrahdr: Option<bool>`

### Build003Config
- `optimization: Option<OptimizationLevel>` — None, Lossless, Speed

### New RIAPI Keys (via SrcsetString, not CommandString)
- `srcset=webp-70,100w,fit-crop` — compact syntax
- `output.color=preserve|srgb|p3`
- `ultrahdr=true`, `ultrahdr.mode=hdr_reconstruct`
- Per-codec: `avif.speed=4`, `jxl.distance=1.5`, `jxl.effort=7`

## Ordering Model (from zenpipe ORDERING-DESIGN.md)

Ordering is zenpipe's job. Imageflow selects the strategy.

| Input mode | Ordering | Optimization | Rationale |
|---|---|---|---|
| JSON Steps | Preserve | None (default), opt-in via config | User controls order |
| JSON DAG | Preserve topology | None (default), opt-in via config | User controls topology |
| CommandString (RIAPI) | Canonical sort | Always Speed | Keys have no order |
| SrcsetString | Canonical sort | Always Speed | Compact syntax, no order |

### Canonical sort order (RIAPI convention)
```
ExifOrient → Crop → Constrain/Resize → Filters → Sharpen → Encode
```

### Optimization levels
- **None**: No reordering. User order preserved exactly.
- **Lossless**: Commutative swaps, orient coordinate rewrites only.
- **Speed**: Nearly-lossless (crop before resize — ≤1px border difference).

### Seven reattachment points (all in zenpipe)
1. RIAPI canonical sort
2. Bridge coalescing (adjacent same-group nodes merge)
3. Geometry fusion (crop+orient+resize → single LayoutPlan)
4. Filter fusion (exposure+contrast+... → single SIMD pass)
5. Composite/blend reattachment (DAG multi-input)
6. Sidecar derivation (gain map proportional transforms)
7. Encode/decode separation (config extraction, not pixel ops)

## Pipeline Data Flow

```
v1/build request (Build001)
  → deserialize as v2 types
  → From<Node> for NodeV3 (mechanical conversion)
  → shared pipeline

v3/build request (Build003)
  → deserialize as v3 types (NodeV3 natively)
  → shared pipeline

Shared pipeline:
  1. Parse request envelope, extract IO bindings
  2. Probe source via zencodecs::probe() → ImageInfo, SourceImageInfo
  3. Check for JPEG lossless fast path (orient-only → DCT-domain)
  4. Convert NodeV3 → Vec<Box<dyn NodeInstance>> via translate.rs
  5. Convert EncoderPresetV3 → CodecIntent via preset_map.rs
  6. Resolve format+quality: zencodecs::select_format_from_intent() → FormatDecision
  7. Apply ordering strategy:
     - JSON steps: preserve (optionally optimize if config says so)
     - RIAPI/Srcset: canonical_sort + optimize(Speed)
  8. Build decoder: zencodecs::DecodeRequest → DecoderSource
  9. Process: zenpipe::orchestrate::stream(source, config) → StreamingOutput
  10. Build encoder: zencodecs::streaming_encoder(decision) → EncoderSink
  11. Execute: zenpipe::execute_with_stop(output.source, sink)
  12. Assemble response: JobResult { encodes, decodes, performance }
```

## Scene-Referred Color Model

Pipeline works in source color space by default. No automatic sRGB conversion.

- **Decode**: emits pixels in whatever space the source is (sRGB, P3, Rec.2020, ICC)
- **Processing**: operations request their preferred working space via FormatHint
  - zenfilters: Oklab f32 (gamut-agnostic perceptual)
  - zenresize: linear light (transfer stripped, primaries preserved)
  - zenpipe auto-inserts RowConverterOp when formats differ
- **Encode**: preserves source color, embeds matching ICC/CICP
  - OutputColor::Preserve (default) — keep source profile
  - OutputColor::Srgb — convert to sRGB (opt-in)
  - OutputColor::DisplayP3 — convert to P3 (opt-in)
- **RIAPI**: `color_profiles=false` → force sRGB output. `color_profiles=true` → preserve.

## File Structure

```
imageflow_types/src/
    lib.rs              shared types (Color, Filter, IoObject, PixelFormat, etc.)
    v2.rs               Node, EncoderPreset, Build001, Execute001 — FROZEN
    v3.rs               NodeV3, EncoderPresetV3, Build003, OptimizationLevel
    convert.rs          From<Node> for NodeV3, From<EncoderPreset> for EncoderPresetV3

imageflow_core/src/
    lib.rs              re-exports
    context.rs          v2 Context API surface, v1/ + v3/ endpoint routing
    json/endpoints/
        v1.rs           unchanged v1/ handlers
        v3.rs           v3/ handlers (same pipeline, v3 types)
    translate.rs        NodeV3 → Vec<NodeInstance> + CodecIntent (~500 lines)
    preset_map.rs       EncoderPresetV3 → CodecIntent (~150 lines)
    riapi.rs            CommandString → RIAPI parse (delegates to imageflow_riapi)
    srcset.rs           SrcsetString → expanded keys (from v3 branch, ~300 lines)
    lossless.rs         JPEG lossless fast path (~150 lines)

imageflow_riapi/        KEEP — v2 RIAPI parsing
imageflow_abi/          KEEP — v2 C ABI
imageflow_tool/         KEEP — CLI
```

## Deleted from v3 Branch
- `imageflow-graph/` — entire crate (replaced by zenpipe graph + bridge)
- `imageflow-commands/` — entire crate (replaced by v2/v3 types + zennode)
- `imageflow_core/src/codecs/codec_decisions.rs` — replaced by zencodecs
- `imageflow_core/src/codecs/zen_decoder.rs`, `zen_encoder.rs` — replaced by zencodecs
- `imageflow_core/src/flow/` — graph engine (replaced by zenpipe)

## Dependencies

```toml
[dependencies]
# Execution
zenpipe = { path = "../../zen/zenpipe", features = ["zennode", "std"] }

# Codecs + quality + format selection
zencodecs = { path = "../../zen/zencodecs", features = ["zennode"] }
zencodec = { path = "../../zen/zencodec" }

# Node definitions (for create_default + set_param)
zenresize = { path = "../../zen/zenresize", features = ["zennode"] }
zenlayout = { path = "../../zen/zenlayout", features = ["zennode"] }
zenfilters = { path = "../../zen/zenfilters", features = ["zennode"], optional = true }

# Zenode core
zennode = { path = "../../zen/zennode/zennode" }

# Types
imageflow_types = { path = "../imageflow_types" }

# RIAPI (existing)
imageflow_riapi = { path = "../imageflow_riapi" }

# Utilities
serde = { version = "1", features = ["derive"] }
serde_json = "1"
enough = "0.4"
```

## Changes Required in Zen Repos

### zenpipe (~160 lines)
1. Expose `optimize_node_order(level, &mut [Box<dyn NodeInstance>])` — reorder using schema metadata
2. Expose `canonical_sort(&mut [Box<dyn NodeInstance>])` — sort by NodeRole phase order
3. Harden bridge `param_*` functions to return `Result` instead of panicking
4. Fix dev-dep ImageInfo API drift (test-only)

### zencodecs (0 lines)
- No changes. Quality, format selection, policy all stable.

### zennode (0 lines)
- No changes. NodeRole 9 variants works for all crates.

### zenjpeg, zengif (~10 lines each)
- Fix stale ImageInfo field usage in their own tests.

## Migration Path

1. Save this plan (done)
2. Implement zenpipe changes (optimize_node_order, canonical_sort, param hardening)
3. Create new branch from main in this repo
4. Restructure imageflow_types (split into v2.rs + v3.rs + convert.rs)
5. Write translate.rs (NodeV3 → zennode NodeInstance mapping)
6. Write preset_map.rs (EncoderPresetV3 → CodecIntent)
7. Write v3/ endpoints
8. Rewire context.rs internals to use zenpipe
9. Port srcset.rs from v3 branch
10. Add JPEG lossless fast path
11. Adapt test corpus to test through new pipeline
12. Delete dead code (flow/, codecs/, imageflow-graph/, imageflow-commands/)
