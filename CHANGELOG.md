# Changelog

All notable changes to Imageflow are documented here. Format follows [Keep a Changelog](https://keepachangelog.com/) with conventional categories (Added, Changed, Deprecated, Removed, Fixed, Security). Every entry is traceable to one or more short commit hashes.

## [Unreleased]

### QUEUED BREAKING CHANGES
<!-- Breaking changes staged on the `review/zencodecs-v2` branch (and the
     zencodecs lifetime-elimination plan) that will ship together in the
     next minor release. Do NOT ship piecemeal. -->
- `EncodeRequest<'a>` becomes owned `EncodeRequest` — remove the `'a` lifetime, make all fields owned/`Copy`. Deletes `zencodecs::Limits` (duplicates `zencodec::ResourceLimits`), switches `CodecConfig` to `Arc<CodecConfigs>`, makes `CodecRegistry` `Copy`, and takes `GainMapSource` owned.
- `EncoderPreset` gains AVIF/HEIC variants and a JSON-driven codec policy API; flat codec kill bits are replaced by a structured `CodecConfig` + `FormatConfig` split and a runtime `enabled_codecs` dispatch (replaces `#[cfg]`-gated codec enum variants).
- `ExecutionSecurity` gains per-format codec kill bits, process timeout, and cooperative cancellation handles; `JobOptions` is reserved (`#[non_exhaustive]`) for backend/color-management controls.
- `Build001Config`/`Execute001` grow a `JobOptions` field; `ExecutionSecurity` is already `#[non_exhaustive]` (landed in Unreleased) so external construction must go through `sane_defaults()` / `unspecified()`.
- New v2 JSON API endpoint routing via a dedicated `imageflow-graph` engine; legacy JSON endpoints are cross-checked against the graph engine but the graph engine becomes the production path.
- `mozjpeg-rs` becomes an optional encoder alongside `zenjpeg`; `DecodeMode::NativeJpeg` collapses into the `zencodec` `push_decode` path.
- AVIF/HEIC decoders and an AVIF encoder land via `zencodec-types` (adds `ImageFormat::Bmp`/`Pnm`, `zenbitmaps` codec names).
- Codec dispatch gets a pure-function `codec_decisions` module with a structured trace, `QualityIntent` system, and zen/C-aware `build_config` + `FullCodecDecision`.

### Added
- Pure-Rust codec stack: new `zen-codecs` feature provides zenjpeg/zenpng/zenwebp/zengif/zenavif decoders and zenjpeg/mozjpeg-rs/zenpng/zenwebp/zengif/zenavif encoders, fully usable without any C dependencies (1bb00db5).
- Fuzzing: four libFuzzer targets (`fuzz_decode`, `fuzz_transcode`, `fuzz_riapi`, `fuzz_json`) with clang sancov instrumentation for C decoder coverage, three format/RIAPI/JSON dictionaries (1070 entries), a CI workflow, and `just fuzz*` recipes (a8d25bf3, 2eb25593).
- Integration test suite expansion: 36 new animation, composition, and trim tests covering GIF frame selection, animated roundtrips, multi-input pyramid graphs, watermark compositing, and trim+resize flows (f6d8be58, e83f3da2).
- Grayscale ICC coverage: 42 integration tests (9 synthetic + 33 fixture-based) validating every major grayscale ICC family (gamma 1.8/2.2, sRGB TRC, linear, dot gain, newspaper) through the CMS pipeline, with 6 CC0 fixtures committed and 24 copyrighted fixtures hosted on S3 with caching download fallback (7c71ae27, 17112591, 60cc98d7, e512cc03).
- `ExecutionSecurity::max_total_file_pixels` (default 400 MP) caps cumulative decoded pixels across all animation frames and is enforced in the GIF decoder; `ExecutionSecurity` is now `#[non_exhaustive]` and gains a `JobOptions` companion struct wired through `Build001Config`/`Execute001` (a28f90a4, 1f083590).
- WebP animation encoder path: zen WebP now routes every encode through `AnimationFrameEncoder`, enabling animated-GIF→WebP animation preservation for single- and multi-frame inputs (1d774895).
- CMYK pipeline: zen CMYK JPEGs now pass raw CMYK through zenjpeg and run ICC-based CMYK→sRGB via moxcms, matching the mozjpeg C path; a `CmykHandling` enum replaces the old bool API (ee2e1dcb, 4e2a13f5).
- `LibjpegTurbo` encoder preset now routes to zenjpeg on zen-only builds (via a new `create_jpeg_libjpeg_turbo_style` helper that disables adaptive quant and defaults to Annex K Huffman + baseline) instead of hard-erroring (46e8a4b6).
- Three-way JPEG encoder bench (mozjpeg-C vs zenjpeg vs mozjpeg-rs) and a C-vs-zen decoder/encoder bench suite, plus a `zenbench_quickchart` example that emits quickchart.io URLs from zenbench JSON dumps (006bf882, d8193228, 41f88d4f).
- ROADMAP.md plus README announcements for imageflow 3 and imageflow 4 (796ce1c4).

### Changed
- Resizer rewrite: replace ~600 lines of hand-written sRGB↔linear, premultiply, V/H filter, and canvas-composite code with `zenresize` 0.3.0's streaming API. Three compositing modes (`ReplaceSelf`, `BlendWithMatte`, `BlendWithSelf`). Net −657 lines (bc752d9f).
- perf: close the zen-vs-mozjpeg JPEG decode gap from 2.5–3.5× → 1.08–1.18× (effectively parity) by switching JPEG to `into_decoder().decode()` (rayon-parallel fast i16 path) and correcting a CMYK swizzle bug in `copy_pixel_slice_to_bitmap` (c07b70ae).
- perf: zero-copy JPEG decode via `decode_into` on the fast path with a safe `decode()` fallback for progressive/arithmetic/XYB/edge cases (a447668c).
- perf: `push_decode` is now enabled for every format including JPEG, backed by upstream zenjpeg fixes (bytes_per_pixel, strided output, grayscale streaming BGRA, progressive/arithmetic guard, direct buffer indexing, decoded dims+stride) (f371bd64, ec67412b).
- perf: rolling zenjpeg uptakes — cached header (3→2 marker scans), eliminated duplicate header parse, consolidated 3→1 `read_info`, `decode_streaming_into` + eligibility short-circuit, `streaming_output_format` wiring into `Decoder::decode()` (99955460, b6d19e95, 8dc926fc, 1f3ff939, 41731cc3).
- Zen decode/encode negotiate `BGRA8_SRGB` directly with each codec, skipping intermediate RGBA→BGRA swizzles; encoders still fall back to a one-pass swizzle for codecs that only list RGB8/RGBA8/Gray8 (e.g., mozjpeg-rs) (cc3cf88c).
- CMS: `lut_transform_opts` now uses `RelativeColorimetric` for display-directed transforms (matching browsers/Skia/Photoshop), while CMYK→sRGB keeps `Perceptual` until moxcms ships Black Point Compensation (a142ff97).
- Zen JPEG/WebP auto routing honors `encoder_hints.{jpeg,webp}.{quality,progressive,lossless}`, matte, and defaults (JPEG q=90, WebP q=80); 4:2:0 subsampling kicks in at q≤90 to match mozjpeg's evalchroma (06408c98, 719c94a3).
- perf: drop the zenjpeg `IdctMethod::Libjpeg` override for decoder stability; the default Jpegli IDCT is ~37% faster and the remaining 2–4 levels of per-channel drift on three tests are absorbed by perceptual-tolerance bumps (bd545d11).
- deps: npm bindings refresh — basic-ftp 5.2.2, axios 1.15.0, `@nestjs/core` 11.1.18 (resolves Dependabot #707–#710); lodash dropped as a transitive dep, 0 vulnerabilities (2b5d3786).

### Fixed
- WebP auto-routing honored `lossless` but ignored `encoder_hints.webp.lossless`, so `format=webp&webp.lossless=true` silently produced lossy output on zen-only builds. Zen path now mirrors the c-codecs lossless resolution (066d6edd).
- GIF decoder hardened against malformed inputs: bounds-checked palette indexing, frame-bounds clipping to canvas, safe background color index lookup, and disposal clipping; ships with 5 reduced fuzz artifacts and 6 robustness tests (7f319d3f).
- `Ir4Command::parse` no longer panics on malformed URLs — `expect()` replaced with a `LayoutError::InvalidQueryString` error path; all three call sites now report the offending query (606626e3).
- Diagnose `push_decode` JPEG edge cases (grayscale→BGRA coefficient panic, CMS dual-backend, ICC P3 roundtrip), document the path forward, and stop pairing `Resample2D{1,1}` with the decode bench (35fa0fc6, 28f0b4d3).
- CI: Test/Release workflow now triggers on `pull_request` so PRs actually run integration tests (90e8d122).
- CI: zen crate `[patch.crates-io]` entries point at github `main` so CI runners without sibling checkouts can resolve `cargo metadata` (5df6ed16).
- Build: resolve warnings/errors under every feature combo (c-codecs only, zen-codecs only, both); gate `jpeg_decode_bench` example on both features (d1652405).

### Security
- `ExecutionSecurity` size limits are now configurable at runtime: `max_input_file_bytes` and `max_json_bytes` (both default 256 MB / 64 MB), plus `FetchConfig::max_response_bytes` (default 256 MB). Setting any limit to `None` disables the check (45299c73).
- Security audit hardening across all crates: PowerShell command-injection fix in `ShellFetcher`, swapped width/height canvas-creation fix in `enable_transparency`, replaced panicking `.unwrap()`s in parsing, `checked_add` on allocation sizes, 256 MB WebP input cap, graph node (2048) / edge (4096) caps, 64 MB JSON payload cap, 256 MB HTTP response cap, removed `--no-check-certificate` from `wget`, `i64` arithmetic for `ExpandCanvas` estimate, replaced panic on unsupported JPEG pixel format / GIF truncated palette / mozjpeg C-callback panic with error propagation (0e7c0385).
- Eliminated mutable-aliasing UB in mozjpeg decoder `source_fill_buffer`/`source_skip_bytes` callbacks by changing signatures to raw pointers, matching the C-side typedefs (64ef472b).
- Reject `NaN`/`Infinity` in RIAPI float parsing; remove dead `compress2` FFI (a13bce48).
- Document `IoEnum::Filename` trust model (path validation is caller's responsibility) and remove the unused `zune-bmp` dependency (46a3ab79).

### Tests (tolerance adjustments)
<!-- Tolerance bumps for real upstream zen-encoder / zen-decoder drift,
     each tracked in an imazen/zen* issue so thresholds can tighten
     when upstream parity lands. -->
- Loosen lossy-re-encode tolerances on three cross-encoder drift tests (`test_icc_p3_to_jpeg_roundtrip` → zdsim 0.30 per zenjpeg#88, `test_icc_p3_to_webp` → 0.25 per zenwebp#16, `test_transparent_png_to_jpeg` → 0.15 per zenjpeg#88); capture missing zen baseline for `test_negatives_in_command_string` (f34a6c8b).
- Enlarge four small-output zen-only tests past the zdsim-inflation threshold (transparent_webp_to_webp, jpeg_crop, crop_with_preshrink, icc_p3_crop_and_resize) and re-capture baselines under c-codecs (8e6408e2).
- Bump Rec.2020 decode tolerances to zdsim 0.03/0.04 for `test_icc_rec2020_decode_{1,2}` (e7fe6010).
- Bump `test_round_corners_command_string` and `test_rot_90_and_red_dot_command_string` to zdsim 0.08 — cross-decoder rounding noise after 17× JPEG downscale, textbook IDCT speckle (fc185e30).
- Feature-gate 3 tests that assume c-codecs behavior (`test_encode_jpeg_smoke`, `smoke_test_corrupt_jpeg`, `test_webp_to_webp_quality`) and two IDCT-scale tests on zen-only (bdb07583, 2c9dcc54).

## [2.3.1-rc01] - 2026-03-31

Release candidate focused on Rust 2024 edition migration, dual-backend CMS, and safe-code hardening.

### Added
- Rust 2024 edition + `rust-version = "1.93"` across the workspace; `time` pinned to 0.3.47 (948b9298, e63d8b98).
- CMS: `moxcms` backend with dual-backend (moxcms + lcms2) comparison mode and a centralized `lut_transform_opts`; `InterpolationMethod::Tetrahedral` support (49dc2133, f4865b9c, 1ebabe5d, 4d2f7e6a).
- Feature-gate all `unsafe` code in `imageflow_core`; remove dead `nightly` feature; add safe test infrastructure and a schema test (04aa23e4, 55701b82, eb4b5c8c).
- C ABI: `imageflow_context_take_output_buffer` + `imageflow_buffer_free` for zero-copy output handoff (174a3842).
- CI speedup via `nextest`, single-platform docs, and pre-built `regress_report` (215fc39a).

### Changed
- deps: `archmage` 0.9.1→0.9.15, `garb` 0.1→0.2.5; switch `zensim`/`zensim-regress` to published versions (61d41b6a, 5c644347).

### Fixed
- GIF frame-buffer index OOB (DoS) on crafted input (b2808b73, 605c294f).
- Bump `rustls-webpki` 0.103.9 → 0.103.10 (GHSA-pwjx-qhcg-rvj4) (51496dfa).
- Lower dual-backend CMS divergence threshold to 2/5 and downgrade divergence to a warning (fc6e3727).
- Edition-2024 compile fixes: `unsafe extern "C"` in `c_components` test, suppress `unused_unsafe` in `context!` macro expansion (f7252d02, 0635e386).

Earlier history: see git log before `v2.3.1-rc01`.
