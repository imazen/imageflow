# Imageflow3 Context Handoff

## State: 141/188 tests passing (47 failures, 4 skipped)

Branch `imageflow3`. Both v2 and zen backends run against shared checksums.

## Test commands

```bash
just test                        # both backends, shared checksums
just test-filter NAME            # filter by test name
ZENPIPE_TRACE=1 just test-filter NAME  # with pipeline trace
```

justfile passes `--features "zen-default,c-codecs"`.

## Proven facts (with tests)

### JPEG decoder parity (test: `jpeg_decoder_parity.rs`)
- sRGB JPEG (Canon 5D): **delta=0** between mozjpeg and zenjpeg. Pixel-identical.
- Rec.2020 PQ JPEG: **delta=122**, 100% pixels differ. Not IDCT — color matrix.
- Cause unverified. Needs investigation in zenjpeg's YCbCr→RGB path for wide-gamut.

### CMS transform parity (test: `moxcms/tests/rgb_vs_rgba_layout.rs`)
- Layout::Rgb vs Layout::Rgba: **identical** output from moxcms.
- Direct `new_srgb()` vs ICC-roundtripped sRGB destination: **identical**.
- CICP vs no-CICP on destination: **identical**.
- Same ICC bytes (hash verified) go to both v2 and zen CMS paths.

### ICC profile extraction (zenjpeg `tests/icc_extraction.rs`)
- `extract_icc_profile()` works correctly on all test images.
- `or_else` fallback fix (commit `0355bf1d`) resolved extras/parser priority.

### Linear matte compositing (zenpixels-convert)
- `matte_composite()` now blends in linear light using LUT-based sRGB↔linear.
- All 3 matte compositing tests pass (hardcoded pixel checks match v2).

## 47 failures by category

### Wide-gamut JPEG decoder difference (15 tests)
Rec.2020, P3, AdobeRGB, ProPhoto JPEGs. Delta=122 before CMS.
sRGB JPEGs have delta=0 — decoders agree on sRGB, disagree on wide-gamut.

Tests: `icc_rec2020_decode_{1,2}`, `icc_display_p3_decode_{1,2,3}`,
`icc_adobe_rgb_decode_{1,2}`, `icc_prophoto_decode`, `icc_gray_gamma22_decode`,
`icc_repro_{imagemagick,libvips,pillow,sharp}_icc`, `jpeg_icc2_color_profile`

### sRGB JPEG + CMS path difference (4 tests)
sRGB JPEG decoder output is identical (delta=0), but something in the
CMS skip/apply path differs. Delta=49-56 in final output.

Tests: `icc_srgb_canon_5d`, `icc_srgb_sony_a7rv`,
`icc_display_p3_resize_filter`, `icc_p3_crop_and_resize`

### Round corners (9 tests)
V2 uses volumetric_offset=0.56419 + quadrant-based rendering.
Zen matches standard corners (sim 85+) but circle mode on non-square
canvases needs v2's quadrant offset logic. Per-corner radii unsupported.

Tests: `round_corners_{small,large,custom_pixels,custom_percent,
excessive_radius,circle_wide_canvas,circle_tall_canvas,command_string}`,
`round_image_corners_transparent`

### PNG/WebP encode defaults (6 tests)
`with_generic_quality()` overrides `with_lossless()` in zenpng.
WebP lossless hint propagation fixed (preset_map order).
JPEG matte compositing added to `stream_encode`.

Tests: `transparent_png_to_png_rounded_corners`, `transparent_png_to_jpeg`,
`transparent_png_to_jpeg_constrain`, `transparent_webp_to_webp` (sim 98.9),
`matte_transparent_png`, `webp_to_webp_quality`

### WebP alpha / ExpandCanvas (3 tests)
ExpandCanvas fills with transparent [0,0,0,0] on opaque source.

Tests: `webp_{lossless,lossy}_alpha_decode_and_scale`,
`webp_lossy_noalpha_decode_and_scale`

### Other (10 tests)
- `decode_cmyk_jpeg`, `decode_rgb_with_cmyk_profile_jpeg` — CMYK path
- `jpeg_crop`, `crop_with_preshrink` — JPEG crop/IDCT
- `problematic_png_lossy` — pngquant palette
- `pngquant_command`, `pngquant_fallback_command` — pngquant hints
- `png_cicp_bt709_transfer` — CICP assertion
- `branching_crop_whitespace` — DAG mode
- `smoke_test_corrupt_jpeg` — zen decoder more tolerant
- `icc_p3_to_{jpeg_roundtrip,webp}` — re-encode quality
- `trim_whitespace` — border detection
- `rot_90_*`, `jpeg_simple_rot_90` — rotation

## Architecture

### CmsMode (on ExecutionSecurity)
- `Imageflow2Compat` (default): skip sRGB-like ICC on decode (desc heuristic)
- `SceneReferred`: strict sRGB detection, preserve wide gamut

### Backend (on Context)
- `Context.force_backend = Some(Backend::V2 | Backend::Zen)` for runtime selection
- Tests iterate both backends via `backends_to_test()`

### Zen bridge (imageflow_core/src/zen/)
- `translate.rs` (580 lines) — v2 Node → zennode via registry (no custom wrappers)
- `converter.rs` (216 lines) — ZenFilters, ExpandCanvas, Region converters
- `execute.rs` (~1500 lines) — orchestration, CMS, encode
- `preset_map.rs` (~340 lines) — v2 presets → CodecIntent

### Zen node ownership
- zenresize: resize, constrain
- zenpipe: crop_whitespace, fill_rect, remove_alpha, round_corners
- zenblend: RoundedRectMask (used by round_corners)
- zenlayout: crop, orient, flip, rotate, expand_canvas, region

### Patches (Cargo.toml [patch.crates-io])
- zenpixels, zenpixels-convert (local)
- zencodec (local — has SourceColor::is_srgb, icc_profile_is_srgb)
- zenjpeg (local — has ICC extraction fallback fix)
- moxcms (local — has PR #152 #153 fixes)

### Pipeline tracing
`ZENPIPE_TRACE=1|full|svg` — 4-layer trace (RIAPI, Bridge, Graph, Execution).
Tracer facade: zero-alloc when inactive.
