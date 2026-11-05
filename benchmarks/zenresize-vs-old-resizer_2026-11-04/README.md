# Why the visual outputs changed at the zenresize move (bc752d9f)

Measured 2026-11-04 on x86_64 (Ryzen 7950X, AVX-512, WSL2), on commit 78b89eac (the
`visual-checks-authoritative-reference` branch, moxcms 0.8.1) with a scratch copy of zenresize v0.3.1.

## Result

moxcms is 0.8.1 on both sides of bc752d9f. 57 of 143 visual checks changed output between the old resizer
(1f3ff939) and today's code. Putting the old resizer's stages back into zenresize makes 52 of those 57 bit-exact
with the old output. The other 5 all go through watermark compositing, which a5e0de2c rewrote; what remains in them
was not isolated.

| Stage | Old resizer | zenresize 0.3.1 | Checks that need it swapped to match |
|---|---|---|---|
| Row cache before the vertical filter | f32 | f16 | 52 |
| Linear to sRGB u8 | 16384-entry table, truncated index | 4096-entry table (linear-srgb) | 50 |
| sRGB u8 to linear | f32 `powf` table | linear-srgb table | 49 |
| Weights | f32, one normalization | f64, then f32 renormalized | 17 |
| Filter kernels | plain f32 sums | SIMD with FMA | 13 |
| Unpremultiply | any alpha above 0 | alpha above 1/1024 | 5 |

The last column is 52 minus the checks restored with that one stage left out (`results_stage_swaps.txt`).
Separately, zenresize sends opaque sRGB images through a 12-bit integer path when the filter needs 14 taps or fewer,
which for Robidoux means less than 3x downscale. 11 of the 52 also need that path turned off to match.

The old sRGB encoder is the stage that matters for tolerances. With only that stage swapped, 11 of the 25 failing
checks pass. Forcing the float path alone changes 13 outputs and fixes none.

## Accuracy against an exact reference

`accuracy.py` computes each resize in f64 with one rounding at the end, and counts the channels each path gets wrong.
The maximum error is 1 level for every path and input (`results_accuracy.txt`). Bias is the mean signed error in levels.

| 2.56x downscale, Robidoux, linear light | Photo, % off | Photo, bias | Dark ramp, % off | Dark ramp, bias |
|---|---|---|---|---|
| Old resizer | 2.37 | -0.0237 | 9.77 | -0.0977 |
| zenresize as imageflow calls it (12-bit path) | 4.60 | -0.0068 | 20.57 | -0.0048 |
| zenresize float path | 4.47 | +0.0021 | 17.47 | +0.0118 |
| Float path, old 16K encoder | 2.48 | -0.0205 | 9.67 | -0.0967 |
| Float path, exact encoder | 0.53 | +0.0033 | 0.13 | +0.0011 |
| Float path, f32 cache, exact encoder | 0.09 | +0.0009 | 0.13 | +0.0013 |

The photo is `waterhouse.jpg` (2048x1153 to 800x450). The dark ramp is 1024x1024, levels 0-24 with noise.

A comment in zenresize's path selection says the 12-bit path can be off by up to 43 levels at 2x. That did not
reproduce: the path stayed within 1 level on all 6 opaque inputs at 2x and 2.56x, including black and white noise, a
2px checker and thin white lines on near-black. These inputs do not prove there is no worse case.

The exact encoder here is a per-channel `powf`, so it says nothing about the speed of a real fix. It is also not wired
into the fused alpha path, so the "exact encoder" rows for the transparent PNG in `results_accuracy.txt` do not apply.

## Reproducing

```
git -C ~/work/zen/zenresize archive v0.3.1 | tar -x -C ~/tmp/resize-why/zenresize
cd ~/tmp/resize-why/zenresize && patch -p1 < .../stage-switches.patch
cp imageflow_core/src/graphics/lut.rs ~/tmp/resize-why/zenresize/src/old_lut.rs
```

Then add `zenresize = { path = ".../zenresize" }` to `[patch.crates-io]` in a workspace at 78b89eac. `run.sh` runs the
suite as is and with the float path forced. `matrix.sh` runs the stage swaps, and `matrix.py` compares them with a manifest
from 1f3ff939. `accuracy.py` needs `dump/` built in release mode. Each switch is an environment variable, such as
`ZR_OLD_ENCODE=1` or `ZENRESIZE_FORCE_F32=1`. `output_hashes.tsv` holds the hashes behind the counts.

The harness saves a reference after a match, so later runs in a workspace can compare checks that have no S3 reference.
That is why the second matrix pass counts 14 failures with the old encoder where a first pass counted 17.
