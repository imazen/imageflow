# C-vs-Zen codec parity benchmark — 2026-04-15

## Run metadata

- **Commit:** `cc3cf88c` (`feat(zen): request BGRA8_SRGB direct from codecs, skip swizzles`)
- **CPU:** AMD Ryzen 9 7950X (16C/32T, water-cooled)
- **Kernel:** Linux 6.6.87.2-microsoft-standard-WSL2
- **Toolchain:** stable Rust (workspace edition 2024, MSRV 1.93)
- **Build profile:** `[profile.release]` `lto=true`, `opt-level=3` (default), `debug=true`, `strip=true`. `[profile.bench]` inherits.
- **`-C target-cpu`:** *not* set (runtime SIMD dispatch via archmage).
- **Features:** `--features zen-codecs` (default `c-codecs` already on → both compiled in)
- **Bench harness:** zenbench 0.1.7 (interleaved paired execution, 16-30 rounds per group)
- **Bench binary:** `target/release/deps/bench_codecs-942ed4d94874a756`
- **Sizes:** 256×256, 1024×1024, 4096×4096 BGRA gradients
- **Two runs:** default (Rayon threadpool, 32 threads) vs `RAYON_NUM_THREADS=1`

## Codec compilation flags (current lockfile)

### C side (`c-codecs` feature)

| Crate | Version | Features | Source |
|-------|---------|----------|--------|
| `mozjpeg` | 0.10 | (default) | crates.io |
| `mozjpeg-sys` | 2 | `nasm_simd` | crates.io |
| `jpeg-decoder` | 0.3.1 | (default) | crates.io |
| `libpng-sys` | 1.1.9 | `static`, `static-libz`, `libz-sys` | git: `imazen/rust-libpng-sys` (patched) |
| `libwebp-sys` | 0.14.1 | (default) | crates.io |
| `lcms2` | 6 | (default) | crates.io |
| `lcms2-sys` | 4 | (default) | git: `imazen/rust-lcms2-sys#update-lcms2-2.18` (patched) |
| `libz-sys` | 1 | `static` | crates.io |
| `imageflow_c_components` | path | — | local C SIMD shims |

### Zen side (`zen-codecs` feature)

| Crate | Version | Features | Source |
|-------|---------|----------|--------|
| `zenjpeg` | 0.8.4 | `decoder`, `parallel`, `zencodec`, `trellis` | git: `imazen/zenjpeg#main` |
| `zenpng` | 0.1.3 | `zencodec` | crates.io |
| `zenwebp` | 0.4.3 | `zencodec` | git: `imazen/zenwebp#main` |
| `zengif` | 0.7.2 | `color_quant`, `zencodec` | crates.io |
| `zenavif` | 0.1.4 | `zencodec`, `encode` | crates.io |
| `zenjxl` | 0.1.1 | `zencodec` | crates.io |
| `zenbitmaps` | 0.1.4 | `bmp`, `zencodec` | crates.io |
| `zenpixels` | 0.2.8 | `default-features = false` | git: `imazen/zenpixels#main` |
| `zencodec` | 0.1.18 | (default) | crates.io |
| `mozjpeg-rs` | 0.9.1 | `default-features = false`, `zencodec` | crates.io |
| `archmage` | 0.9.19 | `macros` | crates.io |

### Rayon

`zenjpeg/parallel` enables `maybe-rayon`, which uses the global Rayon pool. `RAYON_NUM_THREADS=1` collapses zenjpeg's parallel `to_pixels_fast_i16_*_parallel` decode path back to sequential `ScanlineReader`. None of the C codecs use Rayon.
