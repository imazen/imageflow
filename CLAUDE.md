# Imageflow Project Instructions

## SIMD & Dispatch Crates

`multiversion` is allowed in this project for autovectorization dispatch on scalar loops (e.g., `scaling.rs`). Prefer the defaults provided by `multiversed` for new code — use `multiversion` only where `multiversed` doesn't fit.

For explicit SIMD intrinsics, use `archmage` (already in use for `transpose.rs`).

## f32/f64 Clamping

**Do NOT replace `min(max(...))` patterns with `.clamp()` on floats.** `f32::clamp()` propagates NaN, while `min(max(...))` suppresses it. In image processing pipelines, NaN propagation turns a single bad pixel into a full-image corruption. The `min(max(...))` pattern is intentional NaN defense.

## Git Workflow

Always commit `cargo fmt` changes as a separate commit from code changes.

## Test Commands

All integration tests live in `imageflow_core/tests/integration/` as a single binary.

```bash
just test              # run all tests with nextest
just test-filter NAME  # run tests matching NAME
just test-update       # run tests, auto-accept checksums within tolerance
just test-replace      # reset all checksum baselines to current output
just test-list         # list all test names
just test-build        # compile-check tests without running
```

Checksum TOML files: `imageflow_core/tests/visuals/checksums/`
Reference images: `imageflow_core/tests/visuals/checksums/images/`

## Known Bugs

### Animated WebP/AVIF/JXL with non-sRGB profile bypass CMS (color corruption)
*Found & source-verified 2026-06-22.* `zen_decoder.rs::read_frame` computes `source_profile`
(`imageflow_core/src/codecs/zen_decoder.rs:596`) but the animation / frame-decoder branch
(lines 609–700) `return Ok(bitmap_key)` at **line 699 without ever calling `cms::transform_to_srgb`**.
The transform runs ONLY in the single-frame fall-through path (lines 787–797). The frame bitmap is
tagged `ColorSpace::StandardRGB` (line 666) while holding *source-space* pixels. So an animated
WebP / animated AVIF / animated JXL carrying a non-sRGB ICC profile or non-sRGB CICP is encoded with
**unconverted pixels** (wrong colors). GIF is benign (always frame-path via `always_use_frame_decoder`,
line 36, but carries no ICC). Single-frame WebP/AVIF/JXL are correct. **Fix:** apply
`transform_to_srgb` per frame in the animation branch before line 699, mirroring the single-frame
gating (skip when `SourceProfile::Srgb`; honor `ignore_color_profile`/`ignore_color_profile_errors`).

### zencodec cancellation tokens not wired (except native-JPEG encode)
*Source-verified 2026-06-22.* `enough` IS a dep (`imageflow_core/Cargo.toml`, `enough = "0.4"`) and
`Context` exposes a real `Stop` token (`context.rs:372`). But only native-JPEG encode threads it:
`zen_encoder.rs:401-408` `push_packed(slice, stop)`. Every other zencodec call passes NO stop —
decode `make_job()` sites (`zen_decoder.rs:618,738,747`, animation `render_next_frame_owned(None)` at
`:643,:685`) and zencodec non-JPEG encode (`zen_encoder.rs:466,552,600,611`) never call
`job.set_stop(...)`. They only get a pre-call `return_if_cancelled!` gate (`zen_decoder.rs:590`,
`zen_encoder.rs:359`), so a long decode/encode of a large WebP/AVIF/JXL cannot be interrupted
mid-flight. zencodec 0.1.13 exposes `DynDecodeJob::set_stop`/`DynEncodeJob::set_stop` — just uncalled.
**Fix:** `job.set_stop(StopToken::from(c.cancellation_token()))` at each site before consuming the job.

## Audit Notes (2026-06-22, "since v2.3.1-rc01" review)

- **CHANGELOG.md:13 is inaccurate**: claims `ExecutionSecurity` "gains … process timeout, and
  cooperative cancellation handles." The actual struct (`imageflow_types/src/lib.rs:1127-1144`) has
  ONLY size/byte/pixel limits — no timeout field, no cancellation handle. `JobOptions`
  (`lib.rs:1476`) is an empty `#[non_exhaustive]` placeholder. No process timeout exists anywhere.
- **CHANGELOG AVIF/HEIC drift**: AVIF decoder+encoder and BMP/PNM (`zenavif`/`zenbitmaps`) already
  SHIPPED in `1bb00db5` but are listed under "QUEUED BREAKING CHANGES" as if pending; **HEIC is
  genuinely absent** (no `heic` refs in `src/`). `ZenJxlDecoder` enum variant exists with no `zenjxl`
  dep (scaffolding). `f06b478b` (moxcms widen) is uncited; `8e6f2483`→`bd545d11` IDCT churn is invisible.
- **Privacy metadata = stripped by construction** (clean result): no encoder writes EXIF/XMP/IPTC/GPS;
  EXIF orientation is applied-to-pixels-then-dropped; no preserve-metadata option exists. Source ICC is
  read for the color transform but never re-embedded — output is plain sRGB-by-convention (only the C
  libpng path writes an sRGB marker, `codec_png_wrapper.c:420`).
- **Quality units**: only a codec-agnostic 0–100 scalar (`QualityProfile`, `lib.rs:644`) → static
  per-codec tables (`auto.rs:557-716 QUALITY_HINTS`). No metric-target unit (zensim/ssim2/butteraugli);
  `jxl.distance` is the only metric-flavored knob (JXL-only passthrough). The `ssim2` column in
  `QUALITY_HINTS` is internal DPR-math only, not a request unit. `zensim` is a **dev-dependency only**.
  `QualityIntent`/`codec_decisions.rs` (the richer design) is on branch `feat/zen-codecs-v3`, NOT HEAD,
  and still collapses to one `generic_quality` float — would need structural change for metric targeting.

## Delayed TODOs

- **Empirical calibration of the ssim2↔quality tables (currently uncalibrated guesses).** The `ssim2`
  column of `QUALITY_HINTS` and the `LIBJPEG_TURBO_Q_TO_SSIM2` table in
  `imageflow_core/src/codecs/auto.rs` are hand-picked approximations with NO empirical backing — they
  map the quality dial to an SSIMULACRA2 score by guess, and they now feed every zencodec
  `with_generic_quality` decision, so a miscalibration mis-targets quality on every codec. Calibrate
  per the CLAUDE.md sweep discipline: sweep q5–q100 (dense at low q) × tiny/small/medium/large sizes ×
  photo/screen/line-art/mixed corpora, MEASURE achieved ssim2 (zensim/fast-ssim2) per (codec, quality),
  fit per-codec quality→ssim2 curves, and replace the constants with provenance-commented fits (corpus,
  date, n, validation error). Reconcile with each zen codec's OWN internal calibration (zenwebp
  `calibrated_webp_quality`, zenavif `calibrated_avif_quality`, zenjxl `calibrated_jxl_quality`, zenjpeg
  `ssim2_to_internal`/`SSIM2_TO_JPEGLI`) so quality is not double-mapped.

- **Issue #728 zencodec passthrough (currently interim heuristic).** The `target=fast|optimal` +
  balance directive and `is_optimal`/optimality-headroom annotations are implemented in imageflow with
  an INTERIM local cost/RD heuristic (`auto.rs`), because zencodec 0.1.19 exposes no encode
  resource-estimate, no candidate-`ImageFormat` selector, and no optimality API. zencodec 0.1.24 adds
  `EncoderConfig::estimate_encode_resources` plumbing but every codec returns `ResourceEstimate::unknown()`.
  Full passthrough needs: (1) imageflow on zencodec ≥ the release that ships real
  `estimate_encode_resources` impls in zenavif/zenjpeg/zenwebp/zenjxl, and (2) NEW zencodec APIs (a
  candidate-format selector + optimality/would-not-improve determination) that exist in no version yet.
  Replace the interim seams (marked `// TODO(#728): zencodec passthrough`) when those land.

- **Licensing/caching module** (`imageflow_helpers/src/unused/`): ~2300 lines of draft licensing, caching, and polling code. Currently unreferenced (no `mod` declaration). Needs review, modernization, and wiring into the build when ready to complete.
