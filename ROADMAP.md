# Imageflow Roadmap

The current shipping line is Imageflow 2. The next two major versions are below.

## Imageflow 3 — pure-Rust codecs and multicore

**Status:** in active development.

- A new pipeline engine with substantially faster decode, resize, and encode paths.
- Multicore encoding and decoding.
- A full set of `#![forbid(unsafe)]` pure-Rust native codecs replacing the C dependencies — JPEG, PNG, WebP, GIF, AVIF, JXL, BMP, TIFF, and more — with better compression and speed than the equivalent C libraries on most workloads.
- Backwards-compatible API: existing v1 querystrings and JSON jobs run unchanged.

Output remains sRGB in v3. Color space and HDR work lands in v4.

## Imageflow 4 — color, HDR, and streaming

**Status:** planned, follows v3.

- A new streaming pipeline that keeps peak memory bounded regardless of image dimensions.
- End-to-end HDR and wide-gamut color, preserved through every operation rather than collapsed to sRGB.
- Output to any color space or ICC profile, not just sRGB.
- UltraHDR gain map round-trip and tone-mapping for SDR fallback.
- A set of professional photographer-focused tuning filters — exposure, contrast, clarity, saturation, white balance — operating in perceptually uniform color space.

## Following along

- File ideas, requests, or questions at [imazen/imageflow/issues](https://github.com/imazen/imageflow/issues).
- The repository's `main` branch tracks v3 work; v4 work will branch from v3 once it stabilizes.
