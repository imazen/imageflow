# PNG Color Management: gAMA, cHRM, sRGB, iCCP, and cICP

## How PNG Color Metadata Works

PNG files can describe their color space through five chunk types, listed here in PNGv3 precedence order (highest to lowest):

1. **cICP** — Coding-Independent Code Points (PNG 3rd Edition, libpng 1.6.45+). Four bytes: color primaries, transfer function, matrix coefficients, full-range flag. Uses ITU-T H.273 code points. Supports HDR (PQ, HLG) and wide-gamut (BT.2020, BT.2100, Display P3).

2. **iCCP** — Embedded ICC profile. A full ICC color profile (typically 0.5–4 KB). Defines transfer function (TRC), primaries, white point, and rendering intent. The most precise color description available.

3. **sRGB** — One byte: rendering intent (perceptual, relative, saturation, absolute). Signals that the image data conforms to the sRGB specification (IEC 61966-2-1). When present, decoders must treat the image as sRGB.

4. **gAMA** — Four-byte unsigned integer: the image's encoding gamma × 100,000. For sRGB-like encoding, this is 45455 (≈1/2.2). For linear light, this is 100000 (1.0). Describes the transfer function only, not the primaries.

5. **cHRM** — Eight pairs of four-byte values: CIE 1931 xy chromaticities for white point, red, green, and blue primaries. Describes the color gamut only, not the transfer function.

### Precedence Rules (PNGv3)

When multiple chunks are present, decoders must use the highest-priority chunk:

- **cICP overrides everything.** If cICP is present, ignore iCCP, sRGB, gAMA, and cHRM.
- **iCCP overrides sRGB, gAMA, and cHRM.** The ICC profile is a complete color space description.
- **sRGB overrides gAMA and cHRM.** The sRGB specification defines both the transfer function and primaries; the gAMA and cHRM values (if present) are only fallbacks for legacy decoders.
- **gAMA and cHRM are the lowest priority.** They are only meaningful when no higher-priority chunk is present.

libpng implements this via `png_struct::chunk_gamma`: cICP sets it first (highest priority), sRGB sets it only if cICP hasn't, gAMA sets it only if nothing else has. See `pngrutil.c:1100-1319` in libpng source.

### The Encoder Side: What Gets Written

The PNG spec historically recommended that encoders writing sRGB or iCCP also write gAMA and cHRM as fallbacks for legacy decoders. **This recommendation was removed in the PNG 3rd Edition** (W3C `w3c/png` issue #151). Chris Blume (W3C PNG Working Group chair):

> "Today, the gAMA and cHRM chunks are for extremely legacy systems. We're well past that transitory phase now."

Modern encoders should write:
- **sRGB chunk** for sRGB content (optionally with gAMA+cHRM fallback, no longer recommended)
- **iCCP chunk** for ICC-profiled content
- **cICP chunk** for content described by ITU-T H.273 code points (including HDR)

Imageflow's encoder writes `png_set_sRGB_gAMA_and_cHRM()` — the sRGB chunk plus gAMA+cHRM fallback. This is still correct; the spec change only removed the *recommendation*, not the *permission*.

---

## How Applications Handle These Chunks Today

### Browsers (Best Behavior)

| Browser | gAMA | cHRM | sRGB | iCCP | cICP |
|---------|------|------|------|------|------|
| Chrome/Edge | Honored | Honored (builds ICC on the fly) | Honored | Full CMS | Shipped |
| Firefox | Honored | Honored (validated, rejects bogus) | Honored | Full CMS | Nightly 136+ |
| Safari | Honored | Ignored | Honored | Honored | Shipped |

Chrome renders PNG with only gAMA+cHRM (no sRGB, no iCCP) with slightly darker shadows compared to sRGB-tagged images, because gAMA=45455 is an approximation of the sRGB transfer function, which has a linear toe segment near black. Firefox had a historical bug where bogus cHRM chunks caused images to render excessively blue/cyan; the fix was in Little CMS's matrix validation, not by disabling cHRM support (Mozilla bug 460520).

### Desktop Applications (Mostly Ignore)

| Application | gAMA | cHRM | sRGB | iCCP | cICP |
|-------------|------|------|------|------|------|
| libvips | **Ignored** | **Ignored** | **Ignored** | Honored | No |
| Photoshop | Honored | **Ignored** | Honored | Honored | Unknown |
| GIMP | **Ignored** | **Ignored** | Unknown | Honored | No |
| XnView | **Ignored** | **Ignored** | Unknown | Partial | No |
| ImageMagick | Honored | Honored | Honored | Honored | Unknown |
| System viewers | **Ignored** | **Ignored** | **Ignored** | Passthrough | No |

### libvips Specifically

libvips completely ignores gAMA, cHRM, and sRGB chunks in both read and write paths. In its libpng backend (`vipspng.c`), it only calls `png_get_iCCP()` / `png_set_iCCP()`. In its libspng backend (`spngsave.c` / `spngload.c`), it only calls `spng_get_iccp()` / `spng_set_iccp()`. There are zero calls to any gamma, chromaticity, or sRGB APIs.

The image interpretation is hardcoded based on bit depth and band count: always `VIPS_INTERPRETATION_sRGB` for 8-bit RGB, never derived from any PNG chunk. John Cupitt (libvips maintainer) on GitHub issue #1238:

> "libvips does not support the gamma tag... It's ancient legacy stuff from before sRGB."

**Consequence:** A PNG with gAMA=1.0 (linear light) and no ICC profile is silently treated as sRGB-encoded by libvips. Midtones appear approximately 2× too dark.

---

## The gAMA=0.45455 vs sRGB TRC Problem

The sRGB transfer function is **not** a pure gamma curve:

```
sRGB:         if L <= 0.0031308: V = 12.92 * L
              else:              V = 1.055 * L^(1/2.4) - 0.055

Pure gamma:   V = L^0.45455
```

The difference peaks at approximately input level 10-11 out of 255 (about 1-2 8-bit levels in dark tones). For most photographic content this is visually negligible, but it is technically incorrect to treat gAMA=0.45455 as identical to sRGB.

### The Practical Question

When a PNG has gAMA=0.45455 (with or without cHRM), should the decoder:

**(A) Treat it as sRGB and skip the transform entirely?**
This is what Chrome and Firefox do for "neutral gamma" (gamma × 2.2 within ±0.05 of 1.0). It avoids introducing rounding error for the common case of "old encoder that meant sRGB but predated the sRGB chunk." The W3C PNG WG confirmed that essentially all decoders in use today are sRGB-aware, so gAMA=0.45455 without sRGB is a legacy encoding pattern.

**(B) Honor it literally as a pure power curve?**
This is technically correct per the PNG specification. The file says "gamma 0.45455" and we should respect that. The ≤1 level difference in dark tones is real, even if small.

**(C) Let libpng decide?**
libpng's `png_gamma_threshold()` function determines whether the gamma difference between file and screen is significant enough to warrant correction. When screen gamma is set to `PNG_DEFAULT_sRGB`, a file with gAMA=45455 will be detected as matching and no correction is applied. A file with gAMA=100000 (linear) will be corrected. This delegates the threshold decision to a well-tested, spec-maintained library.

**Our recommendation: Option C for the libpng path.** It's the most defensible position, handles edge cases correctly, and gets the 16-bit precision benefit described below.

---

## What libpng Can Do For You

libpng has a built-in gamma correction engine that applies during pixel decode. When you call:

```c
png_set_alpha_mode(png_ptr, PNG_ALPHA_PNG, PNG_DEFAULT_sRGB);
```

This tells libpng:
- Output target is sRGB (screen gamma = `PNG_GAMMA_sRGB` = 2.2)
- If the file has no gamma information, assume sRGB (default gamma = `PNG_GAMMA_sRGB_INVERSE` = 0.45455)
- Don't premultiply alpha

libpng then uses `png_resolve_file_gamma()` to determine the actual file gamma from the PNGv3 precedence chain:

```
1. png_set_gamma() explicit override (if called)
2. chunk_gamma from cICP > sRGB > gAMA (first one wins)
3. default_gamma from png_set_alpha_mode()
4. 1/screen_gamma (last resort)
```

If file gamma ≈ screen gamma, no correction is applied. If they differ, libpng builds correction tables and applies them during `png_read_row()`.

### The 16-Bit Precision Win

libpng applies gamma correction **before** 16-to-8-bit reduction in its internal transform pipeline, regardless of the order you call `png_set_strip_16()` and `png_set_gamma()`. This means:

- A 16-bit linear PNG (gAMA=1.0) gets gamma-corrected to sRGB at 16-bit precision, then stripped to 8-bit
- Shadow detail is preserved: linear values 0–2047 (the dark half of linear range) map correctly to 8-bit sRGB values 0–127
- Without this, stripping to 8-bit first collapses values 0–2047 into 0–7, destroying shadow information before the gamma curve can spread it out

### What libpng Does NOT Do

- **No chromaticity adaptation.** If cHRM specifies non-sRGB primaries, libpng does not perform a gamut mapping. It only handles the transfer function (gamma). You need a CMS (lcms2, moxcms, etc.) for primaries.
- **No ICC profile application.** libpng reads and stores ICC profiles but does not parse or apply them.
- **cICP gamma is not yet implemented.** libpng 1.6.45+ reads and stores cICP chunks, but the `chunk_gamma` extraction from cICP transfer characteristics is marked TODO. The code reserves the slot but doesn't populate it yet.

---

## Recommended Architecture

### For a libpng-based decoder

After `png_read_info()` and after inspecting color chunks:

```
1. Read all color metadata (iCCP, sRGB, gAMA, cHRM, cICP)

2. If iCCP is present:
   - Do NOT call png_set_gamma/png_set_alpha_mode
   - Let the CMS handle both gamma and primaries from the ICC profile
   - Pixel data arrives "raw" (file encoding)

3. If cICP is present with non-sRGB transfer function:
   - Do NOT call png_set_gamma/png_set_alpha_mode
     (libpng's cICP gamma extraction is not yet implemented)
   - Let the CMS handle the full CICP transform
   - Pixel data arrives "raw"

4. Otherwise (sRGB, gAMA, gAMA+cHRM, cICP-sRGB, or nothing):
   - Call png_set_alpha_mode(png_ptr, PNG_ALPHA_PNG, PNG_DEFAULT_sRGB)
   - libpng gamma-corrects to sRGB during decode (at 16-bit precision
     if input is 16-bit, before strip_16)
   - Pixel data arrives gamma-corrected to sRGB transfer function
   - If cHRM specifies non-sRGB primaries:
     - CMS does primaries-only adaptation (input has sRGB TRC,
       source primaries from cHRM, target is sRGB)
   - If cHRM is absent or matches sRGB:
     - No further transform needed
```

### For a non-libpng decoder (pure Rust, custom codec)

Without libpng's gamma engine, the decoder must handle all cases via CMS:

```
1. Parse color chunks from the PNG stream in PNGv3 precedence order

2. Priority resolution:
   a. cICP present → use CICP code points
      - If primaries=1 (BT.709) and transfer=13 (sRGB): no-op
      - Otherwise: CMS transform from CICP → sRGB
   b. iCCP present → use ICC profile
      - CMS transform from ICC → sRGB
   c. sRGB present → no-op (already sRGB)
   d. gAMA + cHRM present:
      - Validate: reject degenerate values (gamma ≤ 0, NaN, infinity,
        any y chromaticity = 0)
      - If gamma ≈ 0.45455 AND primaries ≈ sRGB: no-op
        (threshold: gamma × 2.2 within ±0.05 of 1.0, each chromaticity
         within ±0.01 of sRGB value)
      - Otherwise: build profile from gamma + primaries → CMS → sRGB
   e. gAMA only (no cHRM):
      - If gamma ≈ 0.45455: no-op (assume sRGB)
      - Otherwise: assume sRGB primaries (D65 white, BT.709),
        build profile from gamma + sRGB primaries → CMS → sRGB
   f. cHRM only (no gAMA): treat as sRGB
      (cannot do gamut mapping without knowing the transfer function)
   g. Nothing: treat as sRGB
```

### Building a CMS Profile from gAMA + cHRM

To convert a gAMA+cHRM description into a CMS-usable color profile:

```
Input:
  gamma = gAMA value (encoding gamma, e.g., 0.45455 or 1.0)
  chromaticities = cHRM values (8 xy pairs)

Steps:
  1. decoding_gamma = 1.0 / gamma
     (gAMA stores encoding gamma; CMS needs decoding gamma)
  2. TRC = pure power curve with exponent = decoding_gamma
     (applied identically to R, G, B channels)
  3. Primaries = CIE xy from cHRM (red, green, blue)
  4. White point = CIE xy from cHRM (white)
  5. Build a matrix-shaper profile:
     - RGB-to-XYZ matrix from primaries + white point
     - TRC curves from step 2
  6. Transform: source profile → sRGB profile

For gAMA-only (no cHRM), substitute sRGB/BT.709 primaries:
  White: (0.3127, 0.3290) — D65
  Red:   (0.64, 0.33)
  Green: (0.30, 0.60)
  Blue:  (0.15, 0.06)
```

---

## Decision Matrix: Every Chunk Combination

| Chunks Present | libpng Path | Non-libpng Path | CMS Needed? |
|---|---|---|---|
| cICP (sRGB: cp=1, tc=13) | `png_set_alpha_mode` → no-op | Detect sRGB → no-op | No |
| cICP (other) | Skip gamma, CMS handles all | CMS handles all | Yes (full CICP) |
| cICP + iCCP | Use cICP, ignore iCCP | Use cICP, ignore iCCP | Per cICP |
| cICP + sRGB + gAMA + cHRM | Use cICP, ignore rest | Use cICP, ignore rest | Per cICP |
| iCCP | Skip gamma, CMS uses ICC | CMS uses ICC | Yes (full ICC) |
| iCCP + sRGB | Use iCCP (PNGv3) | Use iCCP (PNGv3) | Yes (full ICC) |
| iCCP + gAMA + cHRM | Use iCCP, ignore gAMA/cHRM | Use iCCP, ignore gAMA/cHRM | Yes (full ICC) |
| sRGB | `png_set_alpha_mode` → no-op | Detect sRGB → no-op | No |
| sRGB + gAMA + cHRM | `png_set_alpha_mode` → no-op | Ignore gAMA/cHRM → no-op | No |
| gAMA + cHRM (sRGB values) | `png_set_alpha_mode` → no-op | Detect sRGB-equivalent → no-op | No |
| gAMA + cHRM (non-sRGB primaries) | libpng corrects gamma; CMS adapts primaries | CMS handles both | Yes (primaries only / full) |
| gAMA + cHRM (non-sRGB gamma) | libpng corrects gamma; CMS adapts primaries | CMS handles both | Yes (primaries only / full) |
| gAMA only (≈0.45455) | `png_set_alpha_mode` → no-op | Detect neutral → no-op | No |
| gAMA only (other, e.g. 1.0) | libpng corrects gamma | Synth sRGB primaries → CMS | No / Yes |
| cHRM only (no gAMA) | `png_set_alpha_mode` → assume sRGB | Assume sRGB → no-op | No |
| Nothing | `png_set_alpha_mode` → assume sRGB | Assume sRGB → no-op | No |

---

## PNGSuite Test Coverage Gaps

The PNGSuite (last updated July 2017, core images from 2011) has significant gaps in color metadata testing:

- **18 gamma test images** (g03–g25): gAMA chunk only, no cHRM, no sRGB, no iCCP. Six gamma values (0.35, 0.45, 0.55, 0.70, 1.00, 2.50) × three color types.
- **2 chromaticity test images** (ccwn2c08, ccwn3p08): gAMA=1.0 + cHRM with standard sRGB primaries.
- **134 other images**: gAMA=1.0 (linear), no cHRM.
- **Zero sRGB chunks.** Zero iCCP chunks. Zero cICP chunks.

The PNGSuite does not test:
- sRGB chunk alone or with fallback gAMA/cHRM
- iCCP with any ICC profile
- cICP (PNG 3rd Edition)
- The precedence chain (what happens when multiple color chunks conflict)
- Non-sRGB chromaticities (wide gamut, ACES, BT.2020)
- 16-bit linear data with gamma correction

For modern testing, supplement with:
- W3C's [PNG-ICC-tests](https://github.com/svgeesus/PNG-ICC-tests)
- The [PNG 3rd Edition implementation report](https://w3c.github.io/png/Implementation_Report_3e/) test suite
- Manually crafted PNGs with specific chunk combinations (tools: `pngcrush`, `exiftool`, or write chunks directly)

---

## Spec Status and Future Direction

### gAMA and cHRM Are Not Deprecated

The chunks remain valid in PNG 3rd Edition. What changed:
- The **recommendation to write them as fallbacks** alongside sRGB/iCCP was removed
- They remain the correct way to describe color for encoders that don't have access to ICC profiles or CICP code points
- Decoders must still honor them when no higher-priority chunk is present

### cICP Is the Future

cICP (PNG 3rd Edition) uses standardized ITU-T H.273 code points. Advantages:
- 4 bytes total (vs. hundreds of bytes for ICC profiles)
- Covers HDR: PQ (transfer=16), HLG (transfer=18)
- Covers wide gamut: BT.2020 (primaries=9), Display P3 (primaries=12)
- Shipping in Chrome, Edge, Safari, Firefox (nightly), libpng 1.6.45+, darktable, Affinity 2.4, oxipng

### libpng API Deprecations in Progress

- `png_*_cHRM_XYZ*` (4 utility functions) — agreed for deprecation
- All `*_fixed` APIs — targeted for deprecation in libpng 1.8 (moving to float internally)
- sRGB profile verification warning — may become opt-in rather than opt-out

### cHRM Was Expanded

libpng 1.6.44 relaxed cHRM validation to allow negative chromaticity values, enabling wide-gamut color spaces like ACES AP1 that have imaginary primaries outside the visible gamut.

---

## References

- [PNG Specification: Chunk Specifications](https://www.libpng.org/pub/png/spec/1.2/PNG-Chunks.html) — PNG 1.2 chunk definitions
- [PNG 3rd Edition (W3C)](https://w3c.github.io/png/) — Current spec with cICP, precedence rules
- [W3C png #151: Still recommend redundant gAMA?](https://github.com/w3c/png/issues/151) — Removal of fallback recommendation
- [libpng issue #1238: gamma handling](https://github.com/libvips/libvips/issues/1238) — libvips maintainer on ignoring gamma
- [The Sad Story of PNG Gamma "Correction"](https://hsivonen.fi/png-gamma/) — Henri Sivonen on gamma inconsistencies
- [Mozilla bug 460520: bogus cHRM chunks](https://bugzilla.mozilla.org/show_bug.cgi?id=460520) — Firefox cHRM validation fix
- [libpng source: pngrutil.c](https://github.com/pnggroup/libpng/blob/master/pngrutil.c) — Chunk handlers with PNGv3 comments
- [libpng source: pngrtran.c](https://github.com/pnggroup/libpng/blob/master/pngrtran.c) — Gamma resolution and transform engine
- [XnView forum: Gamma in PNG](https://newsgroup.xnview.com/viewtopic.php?t=24467) — XnView ignores gAMA
- [Chrome PNG color rendering](https://lr0.org/blog/p/pngchanges/) — Browser vs desktop viewer differences
- [pnggroup/libpng #587: cHRM_XYZ deprecation](https://github.com/pnggroup/libpng/issues/587) — API deprecation discussion
