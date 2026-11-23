# Dispatch, Swizzle & Size Report (`imageflow`, 2026-11-23)

Baseline commit: `10539d62` (`docs: update CHANGELOG for archmage 0.9.33`).

## Summary of Changes
1. **Deduplicate `zenyuv` (`Cargo.toml`, `Cargo.lock`):** Pinned workspace `zenyuv` to `https://github.com/imazen/zenjpeg` (`branch = "main"`), unifying `zenyuv v0.1.3` across `zenjpeg` and `zenwebp` (previously resolved twice: `crates.io` `0.1.3` and git `#f1bf0b9a`).
2. **Drop unused direct `multiversion` (`imageflow_core/Cargo.toml`):** Removed direct `multiversion = "0.8"` from `imageflow_core` (still present transitively via `lodepng`).
3. **Remove `Buffer::default()` shadowing (`imageflow_core/src/graphics/whitespace.rs`):** Removed the inner `let mut buf = Buffer::default();` inside the nested window loop so each iteration reuses the caller-provided `&mut Buffer`.
4. **Vectorize decoder BGRA row swizzle (`imageflow_core/src/codecs/zen_decoder.rs`):** Routed 4-, 3-, and 1-channel U8 rows in `row_to_bgra` through `garb::bytes::{rgba_to_bgra, rgb_to_bgra, gray_to_rgba}`.
5. **Vectorize `Bgr24 -> Bgra32` `copy_rectangle` (`imageflow_core/src/graphics/copy_rect.rs`):** Replaced the `scanlines_bgra()` loop on `PixelFormat::Bgr24` with `scanlines()` + `garb::bytes::rgb_to_rgba`, and added unit test `copy_rectangle_bgr24_to_bgra32_preserves_bgr_and_sets_alpha_ff`.

## Before vs After (`r5600g`, `zenbench`, `taskset -c 4`, `--release` / `bench`)

| Metric | Before (`10539d62`) | After | Delta |
|---|---:|---:|---:|
| `zenyuv` copies in `cargo tree -p imageflow_core --features zen-codecs -i zenyuv` | 2 (`crates.io` + `git`) | 1 (`#f1bf0b9a`) | -1 duplicate |
| `imageflow_tool` stripped/file size (`--release`) | `6,889,496 B` | `6,887,352 B` | `-2,144 B` |
| `imageflow_tool` `.text` section | `6,772,922 B` | `6,770,866 B` | `-2,056 B` |
| `zen_decoder_rgb24_128x128` (`decode_zenpng_rgb24`) | `61.3 ±3.1 µs` (`267 Mpx/s`, `CV=30%`) | `42.1 ±1.3 µs` (`389 Mpx/s`) | **1.46x faster** |
| `whitespace_detect_1280x960` (`detect_content`) | `6.0 ±0.3 ms` (`204 Mpx/s`, `CV=24%`) | `6.5 ±0.3 ms` (`190 Mpx/s`, `CV=26%`) | Within noise (`[5.6–6.6]` vs `[5.9–7.1] ms`) |
| `copy_rect_bgr24_to_bgra_640x480` (`trim_step_pipeline`) | `1.6 ±0.1 ms` (`190 Mpx/s`, `CV=32%`) | `1.7 ±0.1 ms` (`177 Mpx/s`, `CV=28%`) | Within noise (`[1.5–1.8]` vs `[1.6–1.9] ms`) |

*Notes:* `r5600g` runs use `CARGO_BUILD_JOBS=6` with `taskset -c 4`; `whitespace_detect_1280x960` and `trim_step_pipeline` exhibit high round-to-round variance (`CV 24–32%`). `trim_step_pipeline` exercises canvas creation, fill, and whitespace crop rather than isolated `Bgr24` `copy_rectangle`, whose swizzle/alpha behavior is verified by `copy_rectangle_bgr24_to_bgra32_preserves_bgr_and_sets_alpha_ff`. No `-C target-cpu=native` flags were used.

## Verification
- `cargo fmt --check -p imageflow_core`
- `cargo clippy -p imageflow_core --features zen-codecs --lib --tests --benches -- -D warnings` (scoped away from 6 pre-existing `doc_lazy_continuation` warnings in untouched `examples/jpeg_decode_bench.rs`)
- `cargo test --release -p imageflow_core --features zen-codecs --lib --tests`
