# Imageflow 4 Architecture

Imageflow 4 is a ground-up reimagination of the image processing pipeline,
built on the zen crate ecosystem. The C ABI (`imageflow_abi`) is preserved
for backward compatibility with existing bindings. Everything underneath is new.

## Design Goals

1. **ICC end-to-end** — Color profiles travel with pixels from decode to encode.
   No silent stripping. Atomic `finalize_for_output` prevents pixel/metadata mismatch.

2. **CICP-native** — First-class HDR support via PQ, HLG, BT.2020. CICP codes
   propagate through the pipeline alongside ICC profiles.

3. **UltraHDR layers** — Decode/encode gain maps. SDR and gain map are spatially
   locked via zenlayout secondary planes. Processing applies to both layers.

4. **Streaming pipeline** — Strip-based processing across the graph. Decode
   produces rows, operations transform rows, encode consumes rows. Memory is
   O(strip_height × width) not O(width × height).

5. **Trivial pipeline composition** — Operations declare their working space
   (linear, Oklab, sRGB). The pipeline auto-inserts format conversions using
   zenpixels-convert's cost model. No manual format juggling.

6. **JPEG intelligence** — Lossless transforms when possible (orient-only
   pipelines). Quality estimation from source for re-encoding. Butteraugli
   distance targeting. Shrink guarantees.

7. **Minimal code** — imageflow_core is orchestration glue. All real work
   delegated to zen crates.

## Crate Dependency Graph

```
imageflow_abi (C ABI — preserved interface)
    └── imageflow_core (pipeline orchestration)
            ├── zencodecs         (decode/encode all formats)
            ├── zenlayout         (geometry, RIAPI, constraint modes)
            ├── zenresize         (31 filters, streaming, SIMD)
            ├── zenpixels         (pixel types, descriptors, ICC/CICP)
            ├── zenpixels-convert (format conversion, CMS, negotiation)
            ├── zenjpeg           (lossless ops, quality probe, re-encode)
            ├── ultrahdr          (gain map encode/decode, streaming)
            └── imageflow_types   (JSON schema, serde types)
```

## Workspace Members

- `imageflow_types` — JSON API schema. Serde types for all operations.
- `imageflow_core` — Pipeline execution. Thin glue over zen crates.
- `imageflow_abi` — C ABI. Same public interface as imageflow 3.x.
- `imageflow` — Convenience library crate.
- `imageflow_tool` — CLI binary.

Removed from workspace (code preserved for reference):
- `imageflow_riapi` → replaced by `zenlayout` RIAPI parser
- `imageflow_helpers` → replaced by zen crate utilities
- `imageflow_http_helpers` → not needed
- `c_components` → replaced by zen codecs (pure Rust)
- `append_only_set` → not needed

## Pipeline Execution Model

### Phase 1: Plan (geometry)

```
JSON request → parse steps → zenlayout Pipeline
                                 ↓
                          IdealLayout + DecoderRequest
```

zenlayout computes all dimensions, crop rects, and canvas placement upfront.
No pixel work yet. For UltraHDR, `derive_secondary()` creates a spatially
locked gain map layout.

### Phase 2: Negotiate (formats)

Each operation declares its preferred working format via `ConvertIntent`:
- Resize → `LinearLight` (needs linear f32 for correct resampling)
- Sharpen → `Perceptual` (Oklab L-channel)
- Compose → `Blend` (premultiplied alpha)
- Encode → `Fastest` (minimize conversion cost)

zenpixels-convert's `best_match` picks optimal intermediate formats.
Conversions are inserted automatically between incompatible steps.

### Phase 3: Execute (streaming)

```
Decoder.rows() → [Step₁ → Step₂ → ... → Stepₙ] → Encoder.push_rows()
```

Each step processes strips (default 16 rows). The pipeline pulls from
the decoder and pushes to the encoder. Operations that need neighborhood
access (blur, sharpen) request fat strips with overlap.

For multi-output pipelines, the decode output is materialized once and
fanned out to independent encode branches.

### Lossless JPEG Fast Path

When the pipeline is decode → orient → encode JPEG with no pixel operations:

1. Detect at plan time: only orientation step, source is JPEG, output is JPEG
2. Skip pixel decode entirely
3. Use `zenjpeg::lossless::transform()` on DCT coefficients
4. Zero generation loss, ~10x faster

### Quality Re-estimation

When `quality_target: "match_source"`:

1. `zenjpeg::detect::probe()` identifies encoder family and quality
2. `probe.recommend_reencode(tolerance)` computes calibrated quality
3. Optional `shrink_guarantee` ensures output ≤ source size
4. Calibration accounts for encoder efficiency differences (jpegli vs IJG)

## Color Management Pipeline

```
Source file
    │
    ├── ICC profile (bytes)     ─┐
    ├── CICP codes              ─┼── ColorContext on PixelBuffer
    ├── gAMA + cHRM (PNG)       ─┘
    │
    ▼
Decode → PixelBuffer with ColorContext
    │
    ▼
Working space (auto-converted per operation)
    │  ├── Linear sRGB f32 (resize, blur)
    │  ├── Oklab f32 (sharpen, color adjust)
    │  └── sRGB u8 (fast color filters)
    │
    ▼
finalize_for_output(buffer, origin, target_profile, format, cms)
    │
    ▼
EncodeReady { pixels + OutputMetadata }
    │  ├── ICC profile to embed
    │  ├── CICP codes to signal
    │  └── Pixel values guaranteed to match metadata
    │
    ▼
Encoder writes bytes + metadata atomically
```

## UltraHDR Pipeline

### Decode
```
UltraHDR JPEG → zenjpeg extracts SDR + gain map + XMP metadata
    │
    ├── SDR: PixelBuffer (sRGB)
    ├── Gain map: PixelBuffer (grayscale)
    └── Metadata: GainMapMetadata
```

### Process
```
SDR layout → zenlayout Pipeline
Gain map layout → zenlayout derive_secondary() (spatially locked)

SDR: resize/crop/filter as normal
Gain map: resize with matching geometry (bilinear, no sharpening)
```

### Encode
```
SDR → zenjpeg JPEG encode
Gain map → zenjpeg JPEG encode (lower quality)
Metadata → XMP generation
Assembly → MPF container with both JPEGs + XMP
```

## JSON API v2

### Request Structure

```json
{
  "io": [
    {"io_id": 0, "direction": "in"},
    {"io_id": 1, "direction": "out"}
  ],
  "pipeline": [
    {"decode": {"io_id": 0}},
    {"constrain": {"mode": "fit", "w": 800, "h": 600}},
    {"encode": {
      "io_id": 1,
      "format": "jpeg",
      "quality": {"match_source": {"shrink_guarantee": true}}
    }}
  ],
  "security": {
    "max_decode_size": {"w": 10000, "h": 10000, "megapixels": 100},
    "max_encode_size": {"w": 10000, "h": 10000, "megapixels": 100}
  }
}
```

### Key Differences from v1

| Feature | v1 | v2 |
|---------|----|----|
| Color management | Strip or ignore | ICC/CICP end-to-end |
| HDR support | None | PQ, HLG, BT.2020, CICP |
| UltraHDR | None | Decode/encode with gain maps |
| JPEG lossless | None | Coefficient-domain transforms |
| Quality targeting | Fixed quality number | Match source, Butteraugli, SSIM2 |
| Pixel formats | BGRA32 only | u8/u16/f32, any channel layout |
| Filters | sRGB-space color matrix | Oklab-space perceptual adjustments |
| Streaming | Full-image materialization | Strip-based, O(strip) memory |
| Resize | Internal implementation | zenresize (31 filters, SIMD) |
| Codecs | C libraries (mozjpeg, libpng) | Pure Rust zen codecs |
| Layout | Internal sizing code | zenlayout (constraint modes, RIAPI) |
