# Imageflow3 Context Handoff

## State: 132/193 tests passing (57 failures, 4 ignored)

Branch `imageflow3`. Both v2 and zen backends run against shared v2 golden checksums.

## Test commands

```bash
just test                        # both backends, shared v2 golden
just test-filter NAME            # filter by test name
ZENPIPE_TRACE=1 just test-filter NAME  # with pipeline trace
```

## Root cause of remaining 57 failures

### 1. JPEG decoder difference: delta=49 (affects ~30 tests)

zenjpeg produces different pixels than mozjpeg despite LibjpegCompat chroma upsampling being set.

**Proven facts:**
- LibjpegCompat chroma config IS reaching zenjpeg (trace confirmed `StripProcessor::new chroma_upsampling=LibjpegCompat`)
- CMS is NOT the cause (moxcms sRGB→sRGB is identity, delta=0, proven in `/home/lilith/work/moxcms/tests/srgb_roundtrip.rs`)
- No-op resize is NOT the cause (removed, trace confirms Source→Output with no Resize)
- The Canon JPEG may be progressive (buffered mode) where chroma config might not apply
- The delta is purely from JPEG decode differences

**Next step:** Run the Canon 5D JPEG through both mozjpeg and zenjpeg (LibjpegCompat mode) in isolation, compare raw pixels. If delta>0, the bug is in zenjpeg's LibjpegCompat implementation. Check if the JPEG is progressive and if buffered mode respects chroma config.

### 2. CMS/ICC differences (affects ~15 ICC decode tests on top of JPEG delta)

Wide-gamut profiles (Adobe RGB, P3, ProPhoto, Rec.2020) go through CMS on both sides. Both use moxcms. But JPEG decode differences get amplified by the CMS transform — different source pixels → different CMS output.

**Proven:** ICC profile bytes are extracted identically on both sides. Both backends apply moxcms. The delta is from decode, not CMS.

### 3. Trim detection (5 tests, score=0)

Zen uses corner-color comparison, v2 uses Sobel-Scharr edge detection. Different algorithms → different crop bounds. Fix: implement Sobel-Scharr in zenpipe.

### 4. Watermark compositing (6 tests, 5-8% differ)

Watermark compositing is pixel-identical for synthetic inputs (proven with red-on-green and red-alpha-on-blue tests, delta=0). The integration test differences come from JPEG decode differences in the watermark source image.

### 5. WebP alpha (2 tests)

zenwebp vs libwebp decoder alpha differences.

### 6. EXIF alpha normalization (1 test)

`crop_exif` — RGB identical, alpha=255 on 24% pixels. The `alpha_meaningful` flag isn't propagating correctly for Crop+Within pipeline.

## Architecture

### Zen module structure (`imageflow_core/src/zen/`)
- `execute.rs` (1202 lines) — pipeline execution, decode, encode
- `cms.rs` (521 lines) — ICC/gAMA/cICP transforms
- `translate.rs` (600 lines) — v2 Node → zennode translation
- `converter.rs` (420 lines) — NodeConverter implementations (white_balance, color_matrix, region, expand_canvas)
- `watermark.rs` (600 lines) — watermark compositing with zenresize
- `preset_map.rs` (354 lines) — EncoderPreset → CodecIntent
- `nodes.rs` (90 lines) — custom NodeInstance types
- `color.rs` (70 lines) — shared color parsing
- `context_bridge.rs` (163 lines) — v2 JSON → zen pipeline
- `riapi.rs` (153 lines) — RIAPI expansion

### Key decisions made this session
- Zen compares against v2 golden (no separate `_zen` baselines)
- `NodeOp::Materialize` has labels for pipeline tracing
- All `ColorFilterSrgb` variants use sRGB-space color matrices (not Oklab)
- `ColorMatrixSrgb` operates in sRGB gamma space
- LibjpegCompat chroma upsampling configured for JPEG decode
- No-op Resample2D nodes stripped from expanded CommandStrings

### Patches
- zenpixels, zenpixels-convert (local)
- zencodec (local — has SourceColorExt::is_srgb, icc_profile_is_srgb)
- zenjpeg (local — has ICC extraction fallback fix)
- moxcms (local — has PR #152 #153 fixes)

### Tests
- `/home/lilith/work/moxcms/tests/srgb_roundtrip.rs` — proves moxcms sRGB identity
- `zen_watermark_red_on_green` — proves watermark compositing works
- `zen_watermark_red_alpha_on_blue` — proves alpha compositing matches v2
- `zen_watermark_fullframe_resized` — proves resize+compositing matches v2
