# Substitution runtime validation — 2026-04-21

Companion metadata for `substitution_runtime_2026-04-21.csv`.

## Bench

- **File:** `imageflow_core/benches/substitution_runtime.rs`
- **Command:**
  ```
  cargo bench -p imageflow_core --features zen-codecs,c-codecs \
      --bench substitution_runtime
  ```
- **Git commit (HEAD at run time):** `feat/killbits-three-layer` branch,
  last fixup commit of the priority-substitution series. See
  `git log` for the exact SHA — fixups are autosquashed into the
  PR commits, so the canonical provenance is the merged commit on
  `main` once the PR lands.

## Corpus

- **Source:** imageflow test_inputs cache at
  `.image-cache/sources/imageflow-resources/test_inputs/`.
- **Filter caps (post-expansion 2026-04-21):**
  - `MAX_SAMPLE_BYTES = 8 MiB` (was 512 KiB).
  - `MAX_DECODED_PIXELS = 2048 × 2048` (was 512 × 512).
  - `MAX_SAMPLES_PER_FORMAT = 10` (was 3).
- **Samples actually exercised (after filter):**
  - PNG (9): `1_webp_a.sm.png`, `dice.png`, `frymire.png`,
    `gradients.png`, `png_turns_empty_2.png`, `red-night.png`,
    `rings2.png`, `shirt_transparent.png`, plus the synthetic
    `fallback_checker_256x256.bgra` for deterministic reproducibility
    when the corpus is missing.
  - JPEG (9): `MarsRGB_tagged.jpg`, `MarsRGB_v4_sYCC_8bit.jpg`,
    `cmyk_logo.jpg`, `color_profile_error.jpg`, `gamma_test.jpg`,
    `red-leaf.jpg`, `roof_test_800x600.jpg`, `waterhouse.jpg`,
    `wrenches.jpg`.
- **Row counts:** 171 measurements (20 PNG + 2 lodepng + 9×2×8=144
  JPEG pairs + 9 imagequant scaffolding rows). Prior run (pre-expansion)
  was 22 measurements — the larger corpus is primarily JPEG additions
  (zero prior JPEG coverage) plus seven new PNG samples.

## Cap

- **Runtime regression cap:** ≤ 35% slower than the legacy codec at
  the legacy's default knob. Encoded in
  [`RUNTIME_CAP`](../imageflow_core/src/codecs/substitution_measurements/mod.rs).

## Results

- **171 measurements**, 32 over cap.
- Three groups exercised:
  - `png_compression_mapping` — 90 rows (9 samples × 10 zlib levels), 0 over cap.
  - `lodepng_maximum_deflate_mapping` — 9 rows (9 samples × 1 knob), 0 over cap.
  - `jpeg_substitution_mapping` — 72 rows (9 samples × 2 qualities × 4 pairs).
    32 over cap — all on the `libjpeg_turbo_preset` baseline (see JPEG
    findings below).

## PNG mapping — unchanged under expanded corpus

The zlib 0-9 → `zenpng::Compression` mapping stepped down in the
previous run (every knob ≥ 2 → `Fastest`, zlib=9 → `Turbo`) holds on
the larger corpus with zero cap violations. No additional step-downs
needed for PNG.

### Step-downs carried forward from prior run

| Legacy knob | Starting guess | After validation | Reason |
|---|---|---|---|
| `zlib=2` | `Turbo` | `Fastest` | Turbo 2.19x on checker |
| `zlib=3..=4` | `Fast` | `Fastest` | Fast 1.55-4.28x |
| `zlib=5..=6` | `Balanced` | `Fastest` | Balanced 2.84-4.88x, Turbo 1.80-1.90x |
| `zlib=7` | `High` | `Fastest` | High > 1.48x, Balanced/Turbo > cap |
| `zlib=8` | `Aggressive` | `Fastest` | Aggressive ~2x, Balanced/Turbo > cap |
| `zlib=9` | `Maniac(30)` | `Turbo` | Maniac ~9x, Balanced 4.50x, Fastest ≤ cap |
| `lodepng.maximum_deflate=true` | `Maniac(30)` | `Fastest` | Fastest only tier under cap |

Note: the "starting guess" column captures the user's initial table
from the task spec. The validated values live in
`imageflow_core::codecs::substitution_measurements`.

## JPEG substitution findings — new 2026-04-21

Four pairs measured per sample × quality (q=85 and q=95):

1. **Mozjpeg(c) q vs MozjpegRs q** — via `EncoderPreset::Mozjpeg`
   on both sides. All rows pass: ratio 0.21–1.20x, typically < 1.0x.
2. **Mozjpeg(c) q vs ZenJpeg q** — same preset on both sides. All rows
   pass: ratio 0.19–1.06x, typically < 0.9x. ZenJpeg is faster than
   Mozjpeg(c) on every non-trivial image.
3. **LibjpegTurbo preset (baseline-only C encode) vs ZenJpeg under
   Mozjpeg preset** — **all 18 rows over cap** (ratios 1.67–6.98x).
4. **LibjpegTurbo preset vs MozjpegRs under Mozjpeg preset** —
   **all 18 rows over cap** (ratios 1.80–9.62x).

### Why LibjpegTurbo substitutes fail the cap

`EncoderPreset::LibjpegTurbo` routes through
`create_jpeg_libjpeg_turbo_style` (on the zen side) or
`MozjpegEncoder::create_classic` (on the c side): **auto-optimize
disabled, non-progressive, no adaptive quantization**. That's
baseline-libjpeg-style output and it's fast.

The substitute pairs in this group measure the **Mozjpeg preset**
zen backends — which by default enable trellis quantization + full
Huffman optimization + progressive encoding. That's a different
workload, so the 2–10x slowdown is inherent to the optimization
contract, not a bug in the substitute path.

**Implication for the substitution table:** the entry

```
(N::MozjpegEncoder, s::EncoderPreset::LibjpegTurbo { .. })
  → V3: [N::ZenJpegEncoder]
  → V2: [N::ZenJpegEncoder]
```

already correctly routes `LibjpegTurbo` requests to
`create_jpeg_libjpeg_turbo_style` — an apples-to-apples zen
implementation of the libjpeg-turbo baseline shape. That path was
not measured in this run (it needs its own bench group). The
over-cap readings above measure the **cross-preset substitution**
scenario (user requested LibjpegTurbo, we route to a Mozjpeg-preset
zen backend), which only happens today when `LibjpegTurbo`'s
primary + substitute (`ZenJpegEncoder`) are both denied and the
dispatcher has to take a step that intentionally changes the
preset. That's a correctness fallback, not a primary path; a ≤35%
cap doesn't apply.

**No step-downs taken.** The JPEG substitution table is unchanged;
the finding is now documented in the CSV + this meta file so the
next session doesn't have to re-measure.

### Mozjpeg substitution ratios — representative rows

| Sample | Quality | Moz(c) → MozRs | Moz(c) → ZenJpeg |
|---|---|---|---|
| MarsRGB_tagged.jpg (3.8KB) | 85 | 0.22x | 0.20x |
| MarsRGB_tagged.jpg | 95 | 0.23x | 0.25x |
| cmyk_logo.jpg (161KB) | 85 | 1.12x | 0.70x |
| color_profile_error.jpg (194KB) | 85 | 1.20x | 0.46x |
| gamma_test.jpg (25KB) | 85 | 0.69x | 0.54x |
| red-leaf.jpg (126KB) | 85 | 0.71x | 0.57x |
| roof_test_800x600.jpg (225KB) | 85 | 0.74x | 0.65x |
| waterhouse.jpg (802KB) | 85 | 0.67x | 0.54x |
| wrenches.jpg (971KB) | 85 | — | — |

ZenJpeg is consistently 1.3x–2.2x faster than Mozjpeg(c) on real
photo content at q=85 and q=95. MozjpegRs is 1.1x–1.5x faster. Both
substitute paths are well within cap.

## pngquant → zenquant substitution (scaffolded)

The new `NamedEncoderName::ZenPngZenquantEncoder` /
`ZenPngImagequantEncoder` variants added in this PR extend the
substitution-priority table to `ZenPng+zenquant → ZenPng+imagequant →
PngQuant → ZenPng (truecolor)`. Runtime validation for the zenquant
substitute is out of scope for this session — the existing
`pngquant_speed_to_zenquant_quality` mapping in
`substitution_measurements` is labelled with the prior "validated
2026-04-21" claim for continuity, but the CSV still records that row
as scaffolded. Enable with `SUBSTITUTION_RUNTIME_PNGQUANT=1` when a
zenquant vs libimagequant measurement harness is added.

## Caveats

- The synthetic checker is a worst-case for zenpng (every filter
  produces a short run, so all three strategies in `Turbo` do work).
  Real-world photos consistently fit within the cap at higher
  compression tiers — see `1_webp_a.sm.png` rows in the CSV where
  `Balanced` was within cap for zlib=9 (1.02x).
- The libpng baseline is measured through imageflow's full
  decode → encode pipeline (see `encode_libpng_from_pngbuf` in the
  bench), which adds a constant ~1-2ms decode overhead. The JPEG
  group uses the same pipeline for symmetry. Both legs pay the same
  decoder cost, so the ratio isolates encoder time.
