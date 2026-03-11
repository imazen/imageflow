# zencodec-types Migration Plan

Migration plan for adopting zencodec-types traits in imageflow's zen codec adapters,
replacing 8 per-codec adapter files with 2 unified adapters.

## Current State

- 5 zen crates (zenjpeg, zenwebp, zengif, zenjxl, zenavif) + zenpng have `zencodec`
  feature-gated trait implementations with `FrameDecode`/`FrameEncoder` support
- zencodec-types has `'static` bounds on `FrameDec`/`FrameEnc`, so frame decoders
  and encoders can be stored in structs without lifetime issues
- imageflow has per-codec adapter structs (~100-200 lines each) that manually handle
  pixel format conversion, stride copying, error mapping, and animation state
- C-codec adapters (mozjpeg, libwebp, libpng) are unaffected by this migration
- garb crate provides SIMD-accelerated strided swizzle (RGBA↔BGRA, fill alpha, etc.)

## Goals

1. Replace 4 zen encoder adapters + 4 zen decoder adapters with 2 unified files
2. Zero-copy decode via DecodeRowSink directly into imageflow bitmaps
3. Streaming animation: decode frame → process → encode frame, one at a time
4. No frame accumulation in the adapter — codecs handle internal buffering
5. Format-agnostic animation metadata (delay, loop count) without `as_any()` downcast
6. Post-decode BGRA swizzle via garb when codec can't produce BGRA natively
7. Keep C-codec adapters, auto.rs, CMS, and NamedDecoders/NamedEncoders unchanged

## Non-Goals

- Replacing auto.rs quality profile logic (imageflow-specific business logic)
- Replacing NamedDecoders/NamedEncoders priority/enable/disable system
- Migrating C-codec adapters (mozjpeg, libwebp, libpng, lcms2)
- Changing the public imageflow JSON API
- Changing the Decoder/Encoder trait signatures (yet)

---

## Architecture

### Unified Decoder: `ZenDecoder`

```rust
struct ZenDecoder {
    config: Box<dyn DynDecoderConfig>,
    io: IoProxy,
    data: Option<Vec<u8>>,
    cached_info: Option<zencodec_types::ImageInfo>,
    // Persistent frame decoder for animation (created on first read_frame)
    frame_dec: Option<Box<dyn DynFrameDecoder>>,  // 'static — no lifetime issues
    frame_index: u32,
    // Animation metadata from last decoded frame
    last_delay_ms: u32,
    loop_count: Option<u32>,
    // Decoder options (set via tell_decoder)
    ignore_color_profile: bool,
    ignore_color_profile_errors: bool,
    // Format metadata
    preferred_extension: &'static str,
    preferred_mime_type: &'static str,
}
```

**Single-frame decode flow:**
1. Buffer data from IoProxy (lazy, once)
2. `probe()` for `get_image_info()`
3. Allocate bitmap from probed dimensions
4. `push_decoder(data, &mut BitmapSink, &[BGRA8])` — codec writes directly into bitmap
5. If codec produced RGBA: `garb::bytes::rgba_to_bgra_inplace_strided()` — one SIMD pass
6. Extract ICC from `ImageInfo.source_color`, apply CMS if needed

**Animation decode flow:**
1. Buffer data from IoProxy (lazy, once)
2. `probe()` for `get_image_info()` (same)
3. First `read_frame()`: create `DynFrameDecoder` via `dyn_job().into_frame_decoder(data, &[BGRA8])`, store in struct
4. Each `read_frame()`: allocate bitmap, `frame_dec.next_frame_to_sink(&mut BitmapSink)` — zero copy into bitmap
5. Stash `delay_ms` from frame, `loop_count()` from frame decoder
6. garb swizzle + CMS as above

**Why this works:** `DynFrameDecoder` is `'static` (zencodec-types added the bound).
All zen codec FrameDecode implementations own their data internally
(GIF: `Cursor<Vec<u8>>`, WebP/AVIF: eager pre-decode, PNG: owned `file_data`).

### Unified Encoder: `ZenEncoder`

```rust
struct ZenEncoder {
    config: Box<dyn DynEncoderConfig>,
    io: IoProxy,
    matte: Option<Color>,
    io_id: i32,
    // Persistent frame encoder for animation (created on first write_frame)
    frame_enc: Option<Box<dyn DynFrameEncoder>>,  // 'static
    is_animation: bool,
    // Format metadata
    preferred_extension: &'static str,
    preferred_mime_type: &'static str,
}
```

**Single-frame encode flow:**
1. Get bitmap window from BitmapKey
2. Apply matte if set
3. Swizzle BGRA→RGBA via garb (in-place on bitmap)
4. Wrap as `PixelSlice` with appropriate descriptor
5. Create `DynEncoder` via `config.dyn_job().into_encoder()`
6. `encoder.encode(pixel_slice)` → `EncodeOutput`
7. Write output bytes to IoProxy

**Animation encode flow (streaming):**
1. First `write_frame()`: create `DynFrameEncoder` via `config.dyn_job().into_frame_encoder()`, store in struct
2. Each `write_frame()`: get bitmap, swizzle, wrap as PixelSlice, `frame_enc.push_frame(pixels, delay_ms)`
3. `into_io()`: `frame_enc.take().finish()` → `EncodeOutput` → write to IoProxy → return IoProxy

**No frame accumulation.** The codec's frame encoder handles internal buffering
(GIF shared palette, WebP delta compression, etc.). The adapter is a pipe.

### BitmapSink (DecodeRowSink for imageflow bitmaps)

```rust
pub(crate) struct BitmapSink<'a> {
    slice: &'a mut [u8],
    stride: usize,
}

impl DecodeRowSink for BitmapSink<'_> {
    fn demand(&mut self, y: u32, height: u32, width: u32, descriptor: PixelDescriptor)
        -> PixelSliceMut<'_>
    {
        let offset = y as usize * self.stride;
        let bpp = descriptor.bytes_per_pixel();
        let needed = if height > 0 {
            (height as usize - 1) * self.stride + width as usize * bpp
        } else { 0 };
        PixelSliceMut::new(
            &mut self.slice[offset..offset + needed],
            width, height, self.stride, descriptor,
        ).expect("bitmap sink dimensions match")
    }
}
```

The sink wraps the bitmap's `&mut [u8]` with its stride. The codec writes rows
directly — no intermediate allocation. The sink controls stride alignment.

### Frame Metadata Flow

Current GIF encoder uses `as_any()` downcast to extract delay and loop count from
the decoder. This is format-specific and doesn't scale to animated WebP/JXL/AVIF/PNG.

With the unified adapter, frame metadata flows generically:
- `ZenDecoder` stores `last_delay_ms` and `loop_count` after each `next_frame()`
- `ZenEncoder` reads these via the existing `Decoder` trait (new optional methods)
  or via `CodecInstanceContainer` metadata

Options for exposing frame metadata from decoder to encoder:
- **Option A**: Add optional methods to `Decoder` trait: `last_frame_delay_ms()`, `loop_count()`
- **Option B**: Store metadata on `CodecInstanceContainer` so encoder queries it directly
- **Option C**: Keep `as_any()` downcast but on `ZenDecoder` (one downcast for all formats)

Option A is cleanest — backward-compatible default impls returning `None`.

### Pixel Format Strategy

1. Request BGRA8 via `preferred: &[PixelDescriptor::BGRA8]` in all decode calls
2. Most zen decoders support BGRA natively (zenjpeg, zenjxl)
3. If codec returns RGBA: `garb::bytes::rgba_to_bgra_inplace_strided()` — one SIMD pass
4. For encode: swizzle BGRA→RGBA in-place before creating PixelSlice, or pass BGRA
   directly if the encoder supports it

garb handles all strided swizzle operations with SIMD acceleration:
- `rgba_to_bgra_inplace_strided` / `bgra_to_rgba_inplace_strided`
- `fill_alpha_rgba_strided` / `fill_alpha_bgra_strided`
- `rgb_to_bgra_strided`, `gray_to_bgra_strided`, `gray_alpha_to_bgra_strided`

---

## Phase 0: Verify zen crate zencodec implementations

**Scope:** zen crate repos (not imageflow)

### 0a. Verify all zen crates compile with zencodec feature
```bash
cd ~/work/zenjpeg && cargo test --features zencodec
cd ~/work/zenwebp && cargo test --features zencodec
cd ~/work/zengif  && cargo test --features zencodec
cd ~/work/zenjxl  && cargo test --features zencodec
cd ~/work/zenavif && cargo test --features zencodec
cd ~/work/zenpng  && cargo test --features zencodec
```

### 0b. Verify BGRA8 preference is honored
- Each crate's `push_decoder()` with `preferred: &[BGRA8]` should produce BGRA output
  (or the closest format the codec supports)
- Codecs that output RGBA: verify garb swizzle works on the output

### 0c. Verify ImageInfo carries ICC/EXIF/CICP metadata
- `probe()` must populate `ImageInfo.source_color` with ICC profile when present
- JPEG: ICC, EXIF. WebP: ICC. JXL: ICC, CICP. AVIF: ICC, CICP. GIF: none (correct).

### 0d. Verify FrameDecode implementations are 'static
- Confirm all `FrameDec` types own their data (already verified by inspection)
- Confirm `'static` bound on `type FrameDec` compiles for all crates

**Exit criteria:** `cargo test --features zencodec` passes in all zen crate repos.

---

## Phase 1: Add zencodec-types to imageflow

**Scope:** imageflow repo only. No functional changes.

### 1a. Add dependencies
```toml
# imageflow_core/Cargo.toml
zencodec-types = { path = "../../zencodec-types", optional = true }

# Enable zencodec feature on zen crate deps
zenjpeg = { ..., features = ["decoder", "parallel", "zencodec"], optional = true }
zenwebp = { ..., features = ["zencodec"], optional = true }
zengif  = { ..., features = ["color_quant", "zencodec"], optional = true }
zenjxl  = { ..., features = ["zencodec"], optional = true }
```

### 1b. Gate behind zen-codecs feature
```toml
zen-codecs = [
    "dep:zenjpeg", "dep:zengif", "dep:zenwebp", "dep:zenjxl",
    "dep:zenpixels", "dep:zencodec-types",
]
```

### 1c. Verify clean build
```bash
cargo build --features zen-codecs
cargo build --features c-codecs
cargo build --features "zen-codecs,c-codecs"
```

**Exit criteria:** imageflow builds with zencodec-types available. No behavior changes.

---

## Phase 2: Create unified adapter infrastructure

**Scope:** New files in `imageflow_core/src/codecs/`

### 2a. BitmapSink (`bitmap_sink.rs`)
- DecodeRowSink implementation wrapping imageflow bitmap window
- Respects bitmap's stride alignment
- Unit test with mock pixel data

### 2b. Helpers (`zen_helpers.rs`)
- `map_zen_info_to_imageflow()` — convert zencodec-types ImageInfo to imageflow ImageInfo
- `bitmap_to_pixel_slice()` — wrap bitmap window as PixelSlice for encoding
- `swizzle_if_needed()` — garb RGBA↔BGRA based on what the codec produced vs what imageflow needs

**Exit criteria:** Helper code compiles. BitmapSink has tests.

---

## Phase 3: Implement ZenDecoder (unified)

**File:** `imageflow_core/src/codecs/zen_decoder.rs`

### 3a. Implement basic struct and single-frame decode
- `initialize()`, `get_unscaled_image_info()`, `get_scaled_image_info()`
- `tell_decoder()` handling (DiscardColorProfile, IgnoreColorProfileErrors, JpegDownscaleHints)
- `read_frame()` for single-frame: probe → allocate bitmap → push_decoder with BitmapSink
- CMS integration: extract ICC from ImageInfo.source_color, apply transform
- `has_more_frames()` returns false for single-frame

### 3b. Add animation decode
- Create DynFrameDecoder on first `read_frame()` when format supports animation
- `next_frame_to_sink()` for each subsequent frame
- Store delay_ms and loop_count from frame decoder
- `has_more_frames()` delegates to frame count / peek

### 3c. Wire into NamedDecoders and EnabledCodecs
- Add factory function: `ZenDecoder::create_jpeg()`, `create_webp()`, etc.
  Each constructs the appropriate DynDecoderConfig and passes format metadata.
- Wire into `NamedDecoders::create()` match arms
- Keep existing NamedDecoders variants (ZenJpegDecoder, ZenWebPDecoder, etc.)
  pointing to the unified ZenDecoder

### 3d. EXIF rotation
- `get_exif_rotation_flag()`: extract from `ImageInfo.orientation` or EXIF data
- JPEG: parse EXIF orientation from ImageInfo
- JXL: get from JxlInfo.orientation
- WebP/GIF: return None (no EXIF rotation support)

### 3e. Test each format
```bash
just test-filter jpeg   # verify JPEG decode unchanged
just test-filter webp   # verify WebP decode unchanged
just test-filter gif    # verify GIF decode unchanged (including animation)
just test-filter jxl    # verify JXL decode unchanged
```

**Exit criteria:** All existing visual checksum tests pass. CMS behavior unchanged.

---

## Phase 4: Implement ZenEncoder (unified)

**File:** `imageflow_core/src/codecs/zen_encoder.rs`

### 4a. Implement single-frame encode
- `write_frame()`: get bitmap → apply matte → swizzle if needed → encode via DynEncoder
- Write EncodeOutput bytes to IoProxy
- Return EncodeResult with dimensions, mime type, extension

### 4b. Add streaming animation encode
- First `write_frame()`: create DynFrameEncoder, store in struct
- Each subsequent `write_frame()`: push_frame(PixelSlice, delay_ms)
- `into_io()`: finish(), write output, return IoProxy
- No frame accumulation — codec handles internal buffering

### 4c. Wire into auto.rs
- Modify `create_jpeg_auto()`, `create_webp_auto()`, etc. to construct
  `ZenEncoder` with the appropriate `DynEncoderConfig`
- Codec-specific config (quality, chroma subsampling, lossless) is set on
  the concrete config type before boxing as `dyn DynEncoderConfig`

### 4d. Handle animation metadata transfer
- When encoding animation, encoder needs delay_ms per frame
- Query the source decoder for `last_frame_delay_ms()` and `loop_count()`
- Wire via Decoder trait extension or CodecInstanceContainer

### 4e. Test each format
```bash
just test-filter jpeg   # verify JPEG encode unchanged
just test-filter webp   # verify WebP encode unchanged
just test-filter gif    # verify GIF encode unchanged (including animation roundtrip)
just test-filter jxl    # verify JXL encode unchanged
```

**Exit criteria:** All existing visual checksum tests pass. Output quality unchanged.

---

## Phase 5: Cleanup

- Delete old per-codec adapter files:
  - `zenjpeg_decoder.rs`, `zenjpeg_encoder.rs`
  - `zenwebp_codec.rs`
  - `zengif_codec.rs`
  - `zenjxl_codec.rs`
- Remove dead imports from `mod.rs`
- Update `mod.rs` to `mod zen_decoder; mod zen_encoder; mod bitmap_sink; mod zen_helpers;`
- Run full test suite: `just test`
- Verify no performance regression via codec_profile example
- Update MEMORY.md with new architecture notes

---

## File Inventory

### New files (3)
- `imageflow_core/src/codecs/zen_decoder.rs` — unified decoder (~200 lines)
- `imageflow_core/src/codecs/zen_encoder.rs` — unified encoder (~150 lines)
- `imageflow_core/src/codecs/bitmap_sink.rs` — DecodeRowSink impl (~40 lines)

### Deleted files (5)
- `zenjpeg_decoder.rs` (220 lines)
- `zenjpeg_encoder.rs` (137 lines)
- `zenwebp_codec.rs` (260 lines)
- `zengif_codec.rs` (420 lines)
- `zenjxl_codec.rs` (304 lines)

### Modified files (2)
- `mod.rs` — swap module declarations, update NamedDecoders::create()
- `auto.rs` — update encoder creation functions to produce ZenEncoder

### Net change
- **Before:** ~1,341 lines across 5 files
- **After:** ~390 lines across 3 files
- **Reduction:** ~950 lines eliminated, plus unified animation support for all formats

---

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| zen crate zencodec impls are stale | Phase 0 validates before imageflow changes |
| BGRA negotiation doesn't work | garb fallback swizzle; test with each format |
| Performance regression from dyn dispatch | One vtable call per frame — negligible vs codec work |
| CMS integration breaks | Preserve ICC extraction from ImageInfo.source_color |
| GIF animation roundtrip breaks | Test with animated GIF encode/decode |
| Animation for WebP/JXL/AVIF not yet wired | Unified adapter supports it; just needs NamedDecoders entries |
| Frame encoder lifetime issues | Resolved: 'static bound on FrameEnc in zencodec-types |
| Frame decoder lifetime issues | Resolved: 'static bound on FrameDec in zencodec-types |

## Dependency Graph

```
Phase 0 (zen crates) → Phase 1 (add dep) → Phase 2 (infrastructure) → Phase 3 (decoder)
                                                                      → Phase 4 (encoder)
                                                                      → Phase 5 (cleanup)
```

Phases 3 and 4 are independent. Phase 5 requires both 3 and 4.
