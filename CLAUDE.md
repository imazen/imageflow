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

### CRITICAL — generic-quality double-mapping: ssim2 score fed into the libjpeg-turbo-quality knob (WebP/AVIF/JXL)
*Found & source-verified 2026-06-24 (audit of the 7-day zen-codec window).* `auto.rs` passes
`generic_quality_ssim2(qp)` — an SSIMULACRA2 **score** — into `ZenEncoder::create_{webp,avif,jxl}`
(`auto.rs:486,514,541`), which forward it to zencodec `with_generic_quality(...)`. But that trait knob is a
**calibrated 0–100 libjpeg-turbo quality** (`zencodec traits/encoding.rs:55`, "calibrated 0.0–100.0 scale"),
which each codec re-maps via its OWN calibration (`zenwebp calibrated_webp_quality` doc: "Map generic
quality (libjpeg-turbo scale) to WebP native quality"). So quality is mapped TWICE through mismatched units:
imageflow does libjpeg-q→ssim2, then the codec reads that ssim2 number as libjpeg-q→native. `High` profile:
intended webp native ≈91.3, delivered ≈85.9 (deflated); low-q profiles inflate — worst in the q5–q40 web
range. JPEG is CORRECT (uses `Quality::ApproxSsim2`, the genuinely-ssim2 zenjpeg API; `zen_encoder.rs:88`).
**Fix:** pass the libjpeg-turbo quality (`approximate_quality_profile(qp)`, the `p` value) to
`with_generic_quality` for WebP/AVIF/JXL; reserve `generic_quality_ssim2` for the JPEG `ApproxSsim2` path.
Changes encoded output → shifts checksum baselines. This is the active form of the Delayed-TODO double-map risk.

### FIXED (2026-06-25) — animated WebP/AVIF/GIF decode+encode were uncancellable (`None` at frame sites)
*Found 2026-06-24, fixed 2026-06-25.* The animation frame loop passed `None` for the per-call stop at all four
sites (`render_next_frame_owned`, `push_frame`, `finish`); zengif/zenwebp/zenavif drop the job stop and honor
only that per-call arg, so animated WebP/AVIF/GIF couldn't be interrupted mid-flight (animated JXL was fine —
zenjxl carries the job stop). **Fixed:** the four sites now thread `Some(&stop as &dyn Stop)` — from
`c.cancellation_token()` on the two decode sites and the push site, and from a persisted `stop_token` field on
the encoder for `finish()` (which runs in `into_io`, where there is no `Context`). Verified: 20 animation +
10 gif-limit + 3 webp integration tests pass. (A deterministic *mid-frame* cancellation test stays impractical
without a codec pause-hook; the pre-call `return_if_cancelled!` gate already covers between-frame cancellation.)

### FIXED (2026-06-25) — `byte_ceiling` promoted the soft avg pre-flight threshold into a hard runtime cap
*Found 2026-06-24, fixed 2026-06-25.* `MemBudgetPolicy::byte_ceiling()` min'd over ALL three thresholds
including `require_est_bytes_below` — a soft pre-flight check on the **avg** estimate (`check_estimates` →
`peak_avg`) — and that ceiling became the codec's hard `max_memory_bytes` cap, so a caller who set only the
advisory avg threshold got a hard `B − buffer` cap and a mid-flight OOM-reject despite passing pre-flight.
**Fixed:** `byte_ceiling()` now min's over only the conservative thresholds (`require_est_max_bytes_below`,
`require_tracked_bytes_below`). Regression tests `byte_ceiling_excludes_soft_avg_threshold` +
`check_estimates_gates_on_the_right_metric` added (imageflow_types). Was mostly latent (all codec estimates 0).

### LOW / not-a-clear-bug — `High`-profile JPEG `gq > 85.0` chroma boundary (ZEN-ONLY build)
*Re-traced 2026-06-25 — the 2026-06-24 "MEDIUM regression / default High JPEG" framing was WRONG.* Facts that
hold: zen `create_jpeg` sets `full_chroma = gq > 85.0`; `High` → `generic_quality_ssim2 = 85.0` exactly (the
`(91.0, 85.0)` table knot), so `85.0 > 85.0` is false → 4:2:0. BUT: (1) this fires ONLY in the **zen-only
build** — the `auto.rs` JPEG arm routes the generic-quality path under `#[cfg(all(not(c-codecs), zen-codecs))]`;
the default `c-codecs` build encodes JPEG via the C `MozjpegEncoder`, and the runtime-picked `ZenJpegEncoder`
path (`auto.rs:143`) passes `generic_quality = None` → uses the `q > 90` branch, not this boundary. (2) The C
path's chroma is decided **content-adaptively** by `evalchroma::adjust_sampling(buf, {2,2}, chroma_quality)`
per image — a fixed `gq` threshold cannot match it regardless of `>` vs `>=`. So this is NOT a default-build
regression and NOT a clear bug; it's a heuristic-threshold calibration question confined to the zen-only JPEG
path. `gq >= 85.0` would flip High → 4:4:4 there, but whether that better approximates evalchroma is
content-dependent — needs corpus measurement, not a blind 1-char change.

### MEDIUM — single-frame PNG/WebP/AVIF/JXL decode ignores `max_threads`
*Source-verified 2026-06-24.* Only JPEG takes the buffered `run_pooled` path; PNG/WebP/AVIF/JXL single-frame
decode runs `job.push_decode(...)` on the ambient global rayon pool (`zen_decoder.rs:~932`), so
`ExecutionSecurity.max_threads` is silently violated for exactly the most-parallel decoders (rav1d/jxl-rs).
**Fix:** run the single-frame `push_decode` inside `install_pooled(&self.thread_pool, …)` (mind the `&mut`
bitmap-window sink — the closure must own/move it).

### LOW / interim (2026-06-24 audit)
- `v1/estimate` ignores `data.format` (`v1.rs:301` `let _ = &data.format;`) — encode side always 0, so the
  returned `EncodeEstimate` is decode-only. Part of the #728 seam (codecs return `ResourceEstimate::unknown()`).
- `check_estimates` `peak_max = …unwrap_or(peak_avg)` collapses the conservative gate onto the avg when a
  codec sets `est` but not `max` (`ResourceEstimate::new` does exactly that). Apply a conservatism factor or
  have codecs always set `max`.
- `LIBJPEG_TURBO_Q_TO_SSIM2` (`auto.rs:644`) duplicates the `QUALITY_HINTS.ssim2` column verbatim — second
  source of truth that will drift; generate one from the other.
- Per-decode eager rayon pool (`zen_decoder.rs:239`) built even for single-frame decodes that never use it;
  one-shot encode rebuilds a pool per call (`zen_encoder.rs:672`). Build lazily / reuse `self.thread_pool`.
- Zero tests for the budgeting/estimate math (`check_estimates`/`byte_ceiling`/`v1/estimate`) and zero for
  mid-flight cancellation. The `>=` reject boundary and the cap interaction are untested.

### RESOLVED 2026-06-24 — animated non-sRGB CMS bypass (was: color corruption)
The 2026-06-22 animated-CMS-bypass bug is FIXED in `687d008d` and re-verified this audit: the animation
branch now applies `cms::transform_to_srgb` per frame before returning (`zen_decoder.rs:~861`), with the same
sRGB-skip and `ignore_color_profile`/`ignore_color_profile_errors` gating as the single-frame path, no
double-transform, no missed frames, stride-correct. (Kept here as a resolved-record; remove on next cleanup.)

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

- **ssim2↔quality table calibration — JPEG dials DONE, remaining axes still guessed.**
  - **DONE (2026-06-26):** `LIBJPEG_TURBO_Q_TO_SSIM2` in `imageflow_core/src/codecs/auto.rs` is now
    **measured** (24 anchors), and a companion `MOZJPEG_EVALCHROMA_Q_TO_SSIM2` (24 anchors,
    `#[allow(dead_code)]` until the JPEG path wires it) was added. Both are the median quality→SSIMULACRA2
    curve from an 81,552-cell sweep (codec-corpus 502 images × {64,256,1024,native≤4MP} × q5–q100 × 2
    encoders, fast-ssim2). Canonical copy + the `q_to_ssim2`/`ssim2_to_q`/`q_to_bpp` helpers + full
    provenance live in `zencodecs::quality_calibration` (zenpipe); raw Parquet at
    `/mnt/v/output/jpeg-q-ssim2-cal/2026-06-26/sweep.parquet`; docs + rosetta CSVs in
    `imageflow/benchmarks/jpeg-q-ssim2-2026-06-26/`.
  - **STILL UNCALIBRATED:** the `ssim2` column of `QUALITY_HINTS` (the DPR/quality-scalar math) has NO
    empirical backing. The measured tables cover **JPEG dials only** — WebP/AVIF/JXL quality→ssim2 were
    NOT swept, so the generic-quality target for those codecs is still a guess.
  - **STILL BROKEN (the real correctness bug, tracked separately below as the double-mapping issue):**
    auto.rs feeds an *SSIMULACRA2 score* (`generic_quality_ssim2`) into `with_generic_quality`, which
    expects a *libjpeg-turbo 0–100 dial*. Measured-vs-guessed values don't fix the type confusion. JPEG
    is correct (uses `Quality::ApproxSsim2`); the zen-only codecs (AVIF/JXL in the default build) get the
    wrong-units number. Reconcile with each zen codec's OWN internal calibration (zenwebp
    `calibrated_webp_quality`, zenavif `calibrated_avif_quality`, zenjxl `calibrated_jxl_quality`, zenjpeg
    `ssim2_to_internal`/`SSIM2_TO_JPEGLI`) so quality is not double-mapped, then sweep those codecs to
    calibrate their own quality→ssim2 curves the way JPEG now is.

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
