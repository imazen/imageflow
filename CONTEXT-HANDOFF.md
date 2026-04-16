# Context Handoff — 2026-04-16

## What was accomplished this session

### imageflow (imazen/imageflow, main branch, commit f371bd64)

- **push_decode enabled for ALL formats including JPEG** — 257/257 zen-codecs tests pass
- Removed `Resample2D{1,1}` from decode bench (was adding noise)
- Added `streaming_output_format` wiring to `Decoder::decode()` and zencodec `Decode::decode()` trait
- `prefers_buffered_decode` now returns false for all formats
- Added 42 grayscale ICC integration tests (29 profiles, S3-hosted copyrighted fixtures)
- Added debug examples: `profile_imageflow_decode`, `debug_push_decode`, `debug_push_vs_buffered`
- Added `zenbench_quickchart` example for chart URL generation

### zenjpeg (imazen/zenjpeg, main branch, commit 5e6b3e77)

7 fixes to enable push_decode:
1. `bytes_per_pixel` not `num_channels` in push_decoder_direct (#90)
2. Strided output buffer support (row scatter)
3. Grayscale streaming BGRA path (16px chunk-exact unrolled)
4. Progressive/arithmetic guard on push_decoder_direct
5. Safe `cfg.decode()` fallback in push_decoder_direct
6. Decoded dimensions + stride in row copy
7. Direct buffer indexing in `read_rows_xrgb_4bpp` (avoids imgref stride*height)

Also: fused BGRA streaming kernel (057b35dd), `streaming_output_format` in `Decoder::decode()`, calloc for alloc, restart marker tests.

### zengif (imazen/zengif, main branch, commit 704880a)

- Zero-copy canvas elimination (B.2 fix)
- Palette expansion 16x unrolled with `[Rgba; 256]` LUT (B.3)
- Buffer reclaim pattern (B.1)
- Redundant fill removal (B.4)
- garb SIMD BGRA swizzle
- RGBX/BGRX encode dispatch
- wuffs-weezl LZW patch (kept)
- In-repo zenbench decode harness
- Zero-dimension frame hang fix
- Callgrind profiling example

### Upstream issues filed
- image-rs/weezl#75: LZW decoder infinite loop on zero-length output buffer
- imazen/zenjpeg#90: push_decoder_direct writes too few pixels (FIXED)
- imazen/oxcms#3: Test parsing all R2-hosted ICC profiles across all CMS backends
- awxkee/moxcms#169: Lab PCS grayscale (CLOSED, filed on wrong repo)

### RGBX/BGRX encoder support added to 5/6 zen encoders
zenpng, zenwebp, zenjxl, zenavif, mozjpeg-rs, zengif — all accept RGBX8_SRGB/BGRX8_SRGB

## Current performance (codec-level benches, not imageflow)

| Codec | vs C reference | Notes |
|-------|---------------|-------|
| zenjpeg baseline 4096² | +7% faster than mozjpeg | All sizes: +7-15% faster |
| zenjpeg progressive | +44-63% faster than mozjpeg | |
| zengif 256² | +23% faster than gif-rs | |
| zengif 1024² | +28% faster than gif-rs | |
| zengif 4096² | parity with gif-rs | Was -53%, eliminated canvas clone |

imageflow-level JPEG still shows adapter overhead (Context::create, fixture copy per iteration in bench).

## Next steps (prioritized)

### 1. Grayscale 1bpp decode path for CMS
zenjpeg already supports `GRAY8_SRGB` → 1bpp output. imageflow should request it when source is grayscale, pass 1bpp to moxcms (which wants GrayAlpha), then expand to BGRA only at bitmap write. Saves 4× bandwidth during decode + CMS.

### 2. Fix the remaining push_decode performance gap
push_decoder_direct now uses `cfg.decode()` + row copy (safe fallback). For baseline non-progressive JPEGs, switch back to `cfg.decode_into()` with proper eligibility checks to get zero-copy. The safe fallback adds one memcpy.

### 3. Publish zen crate updates
zenjpeg, zengif, and the 5 encoders have unpublished changes on main. Need version bumps + crates.io publish.

### 4. Lab PCS grayscale in moxcms/oxcms
3 profiles (Gray CIE*L, ISOnewspaper26v4, WAN-IFRAnewspaper26v5) rejected by moxcms. Filed as imazen/oxcms#3.

### 5. imageflow bench harness improvement
The bench still does `Context::create()` + `fixture.to_vec()` per iteration. Consider a lighter harness that reuses context or benchmarks the zen adapter directly.

## Files modified (uncommitted)
None — everything committed and pushed.

## Stashed work in other repos
- `~/work/zen/zenjpeg`: git stash "stash: other agent's debug eprintln in zenyuv/sharp.rs"
- imageflow: weezl patch experiment stashed (yielded no win)
