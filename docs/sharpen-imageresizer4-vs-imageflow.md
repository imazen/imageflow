# `f.sharpen`: ImageResizer 4 (FastScaling) vs Imageflow

Investigation for [imazen/imageflow#616](https://github.com/imazen/imageflow/issues/616)
("Investigate differences between ImageResizer4 f.sharpen and Imageflow. Determine
which math is more correct"). Everything below is traced from source; the kernel
numbers were computed with the probe at the end of this page (2026-08-27,
imageflow_core `populate_weights`, which zenresize 0.3.1 declares parity with).

## TL;DR

The two implementations do not compute the same thing, and neither is a textbook
unsharp mask:

| | ImageResizer 4 FastScaling | Imageflow (current) |
|---|---|---|
| User value `f.sharpen=N` | `N/200` → internal goal 0..0.5 | `N/100` → target negative-lobe ratio 0..1 |
| Mechanism | separable 3-tap high-boost `[-n/2, 1+n, -n/2]` applied after each 1-D resample pass, `n = p/(1-p)`, `p = N/200 − natural_ratio(filter)`, skipped when `p ≤ 0.01` | rescales the resampling kernel so that `Σneg / Σpos = N/100` (one-way: only amplifies), positive weights renormalised so the kernel still sums to 1 |
| Filter-lobe stage | present in code but a no-op (`goal/100` is always below the filter's natural ratio — unit mismatch) | this *is* the mechanism |
| Filters without negative lobes (Hermite, Triangle, Box, B-spline) | still sharpened (post pass) | **no effect at all** |
| Strength growth | bounded: `N=100` → `[-0.5, 2.0, -0.5]` at most | `Σpos = 1/(1−N/100)`: N=50 → 2.0, N=75 → 4.0, N=99 → 100 (ringing), **N=100 → silently no sharpening** |
| Working colorspace (default) | `f.colorspace` default `as_is` (sRGB) | `down.colorspace` default linear |
| Kernel sum | 3-tap sums to 1; interposharpen branch (dead) did not preserve sum | always 1 (brightness preserving) |

So for a migrated URL like `f.sharpen=25` with the default Robidoux downscale:

* IR4 applied a mild 3-tap `[-0.054, 1.109, -0.054]` per axis.
* Imageflow rescales Robidoux's 8-tap kernel from neg/pos 0.03 to 0.25 (negative sum
  −0.33 instead of −0.03, peak tap 0.50 instead of 0.39) — a considerably stronger
  and wider high-boost.

Neither is "more correct" in an absolute sense, but Imageflow's mapping has two
concrete defects and one migration hazard, listed under *Recommendation*.

## ImageResizer 4 (FastScaling plugin), traced

Sources: `imazen/resizer`, `deprecated/plugins/FastScaling/`.

1. `ImageResizer.Plugins.FastScaling.cpp:169`
   `opts->SharpeningPercentGoal = f.sharpen / 200.0` — a fraction 0..0.5.
   `rendering.h:72` copies it to `details->sharpen_percent_goal` unchanged, and
   `renderer.c:52` sets `minimum_sample_window_to_interposharpen = 1.5`.
2. `renderer.c:611-617`: if the interpolation window is ≥ 1.5, the goal is also
   copied onto the interpolation details.
3. `weighting.c:370-371`: `desired_sharpen_ratio = sharpen_percent_goal / 100.0`.
   The goal is already a fraction, so this is `N/20000` (≤ 0.005) — always below the
   natural negative ratio of any filter that has negative lobes (Robidoux 0.027,
   Catmull-Rom 0.072, Ginseng 0.132, Lanczos 0.137). `weighting.c:429` only rescales
   lobes when `desired > natural`, so this stage never fires. (When it would have,
   `pos_factor = 1`, i.e. the kernel sum was not preserved.)
4. `renderer.c:315-317` (`ApplyConvolutionsFloat1D`): after each 1-D pass,
   `sharpening_applied = contrib->percent_negative` (the filter's actual negative
   ratio) and, if `goal > applied + 0.01`, `BitmapFloat_sharpen_rows(pct = goal − applied)`.
5. `convolution.c:248-253` (`SharpenBgraFloatInPlace`): `n = -pct/(pct-1)`,
   `c_o = -n/2`, `c_i = 1+n`; each row pixel becomes `c_o·left + c_i·self + c_o·right`
   (alpha included when present). Being inside the 1-D pass it runs once per axis, so
   the 2-D effect is the outer product of the 3-tap.

Consequences: `f.sharpen` below roughly `200 × natural_ratio + 2` does nothing
(Robidoux: N < ~7; Lanczos/Ginseng: N < ~30), and the maximum is a bounded
`[-0.5, 2, -0.5]` at N=100 (Hermite/Triangle, natural ratio 0). It runs in whatever
floatspace `f.colorspace` selects, default `as_is` (sRGB).

## Imageflow, traced

* `imageflow_riapi/src/ir4/layout.rs:526` → `ResampleHints.sharpen_percent = f.sharpen`.
* `imageflow_core/src/flow/nodes/scale_render.rs:51-66, 297` → `sharpen_percent_goal`
  (gated by `f.sharpen_when`, default always), default filter Robidoux (down) /
  Ginseng (up), default colorspace linear (`:277`).
* `imageflow_core/src/graphics/scaling.rs:104-105` → `zenresize` builder
  `.resize_sharpen(goal)`; zenresize 0.3.1 `pixel.rs:683` → `LobeRatio::SharpenPercent`,
  `filter.rs:433` `desired = max(natural, goal/100)`, weights normalised as in
  `imageflow_core/src/graphics/weights.rs:742-758` (zenresize's own test
  `imageflow_populate_pixel` asserts byte parity with that function).
* `weights.rs:33-38`: `desired = min(1, max(natural, N/100))`;
  `weights.rs:742-758`: if `desired < 1`, `Σpos = 1/(1−desired)`, `Σneg = −desired·Σpos`
  (so the kernel sums to 1); **if `desired == 1` the branch is skipped and the kernel
  is normalised as if no sharpening was requested.**

## Measured kernels (Robidoux, 2:1 downscale, interior output pixel)

| `f.sharpen` | Imageflow Σpos / Σneg / peak / min tap | IR4 3-tap per axis |
|---|---|---|
| 0 | 1.031 / −0.031 / 0.385 / −0.009 | none |
| 10 | 1.111 / −0.111 / 0.415 / −0.031 | `[-0.012, 1.024, -0.012]` |
| 25 | 1.333 / −0.333 / 0.499 / −0.093 | `[-0.054, 1.109, -0.054]` |
| 50 | 2.000 / −1.000 / 0.748 / −0.279 | `[-0.144, 1.287, -0.144]` |
| 75 | 4.000 / −3.000 / 1.495 / −0.836 | `[-0.267, 1.534, -0.267]` |
| 99 | 100.0 / −99.0 / 37.4 / −27.6 | — |
| 100 | 1.031 / −0.031 / 0.385 / −0.009 (**no-op**) | `[-0.449, 1.898, -0.449]` |

Same shape for Ginseng/Lanczos/Catmull-Rom (only the natural ratio, i.e. the point
where each scheme starts to bite, differs). Hermite/Triangle: Imageflow never changes
the kernel; IR4 goes up to `[-0.5, 2.0, -0.5]`.

## Recommendation

1. **Bug: `f.sharpen=100` is a silent no-op** (`weights.rs:745` guard). The docs
   already say `0..99`. Either clamp the target ratio (e.g. `min(desired, 0.75)`) or
   treat 100 as the maximum; a discontinuity between 99 (Σpos = 100) and 100 (off) is
   not defensible.
2. **Scale is far too hot at the top.** `Σpos = 1/(1−N/100)` is hyperbolic: values
   above ~50 ring badly and 90+ are unusable. A bounded curve
   (`desired = natural + (N/100)·(cap − natural)`, cap ≈ 0.5) keeps the 0..100 range
   monotonic and usable, and the sum-to-1 normalisation (which Imageflow gets right
   and IR4 did not) can stay.
3. **Migration hazard.** IR4 users tuned `f.sharpen` against a mild 3-tap in sRGB
   space; the same number in Imageflow amplifies the whole resampling kernel and runs
   in linear light, so migrated sites come out visibly sharper (and haloed) than
   before. If IR4 fidelity matters, the closest match is a post-resample 3-tap
   `[-n/2, 1+n, -n/2]` with `n` derived from `N/200 − natural_ratio` — zenresize's
   `post_sharpen` (Gaussian unsharp mask) is not that kernel, so it would be a new
   small pass, or an explicit `f.sharpen.mode=ir4` alias.
4. **Filters without negative lobes** (`down.filter=hermite|triangle|box|bspline`)
   silently ignore `f.sharpen` in Imageflow. Worth either documenting or falling back
   to the post-pass for those filters.

Which of (2)/(3) to adopt changes output for every URL that uses `f.sharpen`, so it is
a compatibility decision for the maintainer, not something to change unilaterally.
(1) is a plain bug fix.

## Reproducing the numbers

```rust
// imageflow_core/tests/zz_probe_616.rs (temporary; run with `-- --nocapture`)
use imageflow_core::graphics::weights::{populate_weights, Filter, InterpolationDetails, PixelRowWeights};

fn interior(filter: Filter, pct: f32, out: u32, inp: u32) -> Vec<f32> {
    let mut d = InterpolationDetails::create(filter);
    if pct > 0.0 { d.set_sharpen_percent_goal(pct); }
    let mut c = PixelRowWeights::new();
    populate_weights(&mut c, 64 * out, 64 * inp, &d).unwrap();
    let row = &c.contrib_row()[32 * out as usize];
    c.weights()[row.left_weight as usize..=row.right_weight as usize].to_vec()
}
// IR4: goal = N/200; post = goal - InterpolationDetails::create(filter).natural_negative_ratio();
// if post > 0.01 { n = post/(1-post); taps = [-n/2, 1+n, -n/2] }
```
