# JPEG quality-dial ↔ SSIMULACRA2 ↔ bpp calibration (2026-06-26)

Measured relationship between the **JPEG/libjpeg quality dial (0–100)** — the number
Windows, Photoshop, and most tooling expose, i.e. the human-mindshare quality knob —
and **SSIMULACRA2** (perceptual quality) and **bits-per-pixel** (size), for two encoders:

- **`libjpeg-turbo`** — stock libjpeg-turbo 2.1.5 `cjpeg -quality Q -sample 2x2` (fixed
  4:2:0, Annex-K tables, no trellis). The "what Windows/most-of-the-web emits" baseline.
- **`imageflow-moz`** — imageflow-2's exact JPEG path: mozjpeg 0.10.13 defaults (trellis,
  tuned tables, optimized coding) + `evalchroma` 1.0.3 content-adaptive chroma subsampling.

The point: convert a user-facing quality into a perceptual target (or a size budget) by
**measurement** instead of guessing. Replaces the previously hand-picked
`LIBJPEG_TURBO_Q_TO_SSIM2` guesses in `imageflow_core/src/codecs/auto.rs`.

## Provenance

| | |
|---|---|
| **Date** | 2026-06-26 |
| **Box** | `arm-big` (Hetzner aarch64, Ubuntu 24.04, 8 cores / 15 GB) — *not* the local workstation |
| **Corpus** | `codec-corpus`: CID22 (250) + clic2025 (62) + gb82 (25) + gb82-sc (11) + imazen-26 (154 sampled) = **502 source images** |
| **Sizes** | `{64, 256, 1024, native ≤ 4 MP}` (native capped at 4 MP for RAM) |
| **Quality grid** | 24 values — q5…q85 step 5, then 88, 90, 92, 94, 96, 98, 100 (dense at top per sweep discipline; the low-q tail is covered down to q5) |
| **Encoders** | 2 (libjpeg-turbo, imageflow-moz) |
| **Cells** | **81,552** encode+score (502 × sizes × 24 × 2) |
| **Metric** | `fast-ssim2` 0.8.2 (SSIMULACRA2), `Ssimulacra2Reference` precompute |
| **Runtime** | ~20 min on arm-big |

**Raw per-cell data** (one row per image×size×q×encoder, with `bytes`/`bpp`/`chroma`):
`/mnt/v/output/jpeg-q-ssim2-cal/2026-06-26/sweep.parquet` (1.18 MB) and `sweep.csv` (9.7 MB)
— block storage, not committed (per the >30 KB-non-source rule).

## Where the tables live in source

- **Canonical:** `zencodecs::quality_calibration` (zenpipe workspace) — the `LIBJPEG_TURBO`
  and `MOZJPEG_EVALCHROMA` anchor tables (24 points each: quality, median SSIMULACRA2,
  median bpp) plus `q_to_ssim2` / `ssim2_to_q` / `q_to_bpp` piecewise-linear helpers.
  Re-exported at the crate root.
- **imageflow:** `imageflow_core/src/codecs/auto.rs` — `LIBJPEG_TURBO_Q_TO_SSIM2` (now
  measured, feeds `generic_quality_ssim2`) and `MOZJPEG_EVALCHROMA_Q_TO_SSIM2`
  (`#[allow(dead_code)]` until the JPEG path adopts it).

## Key findings

- libjpeg-turbo and mozjpeg+evalchroma track quality→SSIMULACRA2 **nearly identically
  through q95**. The dial means about the same perceptual thing on both.
- At **q100** they diverge: evalchroma adopts 4:4:4 on ~21 % of images and reaches **91.9**
  SSIMULACRA2 vs libjpeg-turbo's 4:2:0-capped **88.4**.
- mozjpeg is **15–30 % more byte-efficient** at equal quality, with the largest edge at low
  bitrate (≈ 0.5 bpp → mozjpeg ≈ 40 SSIMULACRA2 vs libjpeg-turbo ≈ 18) — the aggressive-web regime.
- **bpp is content-bound:** at a fixed quality, bpp spans **5–8×** across images (flat sky vs
  dense texture). The `q_to_bpp` median is a *planning* number, never a per-image prediction.

## Files

| File | What |
|---|---|
| `q_to_ssim2_tables.md` | Per-encoder Q → SSIMULACRA2 table with p25/p50/p75 spread + median bpp + n |
| `conversion_guide.md` | Worked Q↔ssim2↔bpp conversions (all six directions), both encoders |
| `rosetta_<encoder>.csv` | Per-q percentile anchors (ssim2 p10/p25/p50/p75/p90, bpp same, n) — whole corpus |
| `rosetta_<encoder>_<class>.csv` | Same, stratified per content class (CID22/clic2025/gb82/gb82-sc/imazen-26) |
| `harness/` | The sweep tool: `src/main.rs` (encode+score), `analyze.py` (tables), `convert.py` (rosetta) |

## Reproduce

```bash
# On a remote box (NOT the local workstation — sweeps run remote per CLAUDE.md):
cd harness
cargo run --release -- \
  --corpus /path/to/codec-corpus \
  --out sweep.csv \
  --sizes 64,256,1024,native --max-pixels 4000000 \
  --q 5,10,15,20,25,30,35,40,45,50,55,60,65,70,75,80,85,88,90,92,94,96,98,100 \
  --progress
python3 analyze.py  --csv sweep.csv      # -> q_to_ssim2_tables.md
python3 convert.py  --parquet sweep.parquet  # -> rosetta_*.csv + conversion_guide.md
```

## Caveats / what's NOT covered

- **JPEG only.** WebP/AVIF/JXL quality→ssim2 were not swept; their `generic_quality` targets
  in imageflow remain uncalibrated guesses.
- **Median curves.** Anchors are medians over the whole corpus; quality↔ssim2 carries ~±7
  SSIMULACRA2 IQR, and anything touching bpp carries the 5–8× content spread above.
- This does **not** fix the auto.rs double-mapping bug (an SSIMULACRA2 score fed into
  `with_generic_quality`, which expects a 0–100 dial) — see imageflow `CLAUDE.md` Delayed-TODOs.
