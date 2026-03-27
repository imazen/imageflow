# Imageflow3 Context Handoff

## Current State (2026-03-26)

**Branch**: `imageflow3`, **139/188 passing** (49 divergences between v2 and zen backends).

## Test Architecture

Tests run **both backends** (v2 + zen) against **one shared checksum set**.
V2 runs first and sets the baseline. Zen must match within the test's tolerance.
Divergences are bugs to fix, not baselines to fork.

```bash
# Run all tests (both backends)
just test

# With auto-accept for new baselines
just test-update

# justfile passes --features "zen-default,c-codecs"
```

`Context.force_backend` controls which engine runs. `Backend::V2` or `Backend::Zen`.
`backends_to_test()` in test infrastructure returns both when `zen-pipeline` feature is on.

## Tracing

```bash
ZENPIPE_TRACE=1 just test-filter test_name    # text trace to stderr
ZENPIPE_TRACE=svg just test-filter test_name  # also write /tmp/zenpipe_trace.svg
```

## 49 Remaining Divergences (by category)

### ICC color management (18 tests)
All `format=png` no-resize tests. Zen skips the identity resize (correct behavior)
but v2 runs it, causing sRGB→linear→sRGB roundtrip that slightly alters pixels.
The zen output is MORE correct. These need the v2 baselines updated to match zen,
or the tolerance adjusted to accept the roundtrip difference.

Tests: test_icc_srgb_canon_5d, test_icc_srgb_sony_a7rv, test_icc_display_p3_decode_{1,2,3},
test_icc_adobe_rgb_decode_{1,2}, test_icc_rec2020_decode_{1,2}, test_icc_prophoto_decode,
test_icc_gray_gamma22_decode, test_icc_repro_{imagemagick,libvips,pillow,sharp}_icc,
test_icc_display_p3_resize_filter, test_icc_p3_crop_and_resize, test_icc_p3_to_jpeg_roundtrip,
test_icc_p3_to_webp

### Round corners (9 tests)
Anti-aliasing model differs: v2 uses volumetric_offset=0.56419 with quadrant-based
rendering including circle mode centering. Zen implementation matches the v2 algorithm
for standard corners (similarity 85+) but circle mode on non-square canvases needs
the v2's quadrant offset logic.

Tests: test_round_corners_{small,large,custom_pixels,custom_percent,excessive_radius,
circle_wide_canvas,circle_tall_canvas,command_string}, test_round_image_corners_transparent

### Transparent PNG/WebP format handling (7 tests)
Alpha channel handling differences between zen and v2 codec paths.

Tests: test_transparent_png_to_{png,jpeg,png_rounded_corners,jpeg_constrain},
test_transparent_webp_to_webp, test_webp_to_webp_quality, test_problematic_png_lossy

### WebP scaling with alpha (3 tests)
Alpha channel initialization differs for WebP decode → resize path.

Tests: webp_{lossless,lossy}_alpha_decode_and_scale, webp_lossy_noalpha_decode_and_scale

### CMYK JPEG decode (2 tests)
Different CMYK→RGB conversion path.

Tests: decode_cmyk_jpeg, decode_rgb_with_cmyk_profile_jpeg

### Matte compositing (2 tests)
Hardcoded pixel value checks — zen alpha compositing math differs from v2.

Tests: test_matte_compositing_{no_double_division,mixed_alpha}

### Pngquant (2 tests)
Zen encoder doesn't support pngquant-style quantized PNG yet.

Tests: test_encode_pngquant_{command,fallback_command}

### Other (6 tests)
- test_jpeg_crop: JPEG IDCT difference
- test_branching_crop_whitespace: DAG mode crop
- test_png_cicp_bt709_transfer_causes_transform: CICP color
- smoke_test_corrupt_jpeg: zen decoder more tolerant
- test_trim_whitespace: whitespace detection threshold

## Architecture (zen bridge)

### What lives where
| Crate | Owns |
|-------|------|
| zenresize | resize (forced w×h), constrain (layout-aware) |
| zenlayout | crop, orient, flip, rotate, expand_canvas, constrain, region, smart_crop |
| zenfilters | color filters (saturation, contrast, brightness, etc.) |
| zenblend | RoundedRectMask, blend modes, mask primitives |
| zenpipe | crop_whitespace, fill_rect, remove_alpha, round_corners, pipeline tracing |
| zencodecs | format selection, encode/decode dispatch, mozjpeg preset config |
| zennode | NodeInstance/NodeDef traits, KvPairs, registry |

### Imageflow zen bridge files
| File | Lines | Purpose |
|------|-------|---------|
| execute.rs | ~1500 | Pipeline orchestration, decode, encode, RIAPI expansion |
| translate.rs | ~580 | v2 Node → zennode NodeInstance (uses zen registry, no custom wrappers) |
| converter.rs | ~216 | NodeConverter for zenfilters, expand_canvas, region |
| preset_map.rs | ~332 | v2 EncoderPreset → zencodecs CodecIntent |
| riapi.rs | ~162 | Dual RIAPI parser (legacy Ir4Expand + zen-native) |
| context_bridge.rs | ~166 | v2 JSON API bridge |
| captured.rs | ~24 | Bitmap capture data |
