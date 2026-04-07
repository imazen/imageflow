# Imageflow `format=auto` — Full Feature Specification

**Status:** Partial implementation. Core format selection engine exists (`imageflow_core/src/codecs/auto.rs`). HTTP Accept header integration is the missing piece.

**Scope:** This document specifies the complete `format=auto` behavior for Imageflow, incorporating the superset of all features observed across Cloudinary, imgix, Cloudflare, ImageKit, Akamai IVM, Fastly IO, Bunny, KeyCDN, Sirv, and Uploadcare — plus Imageflow-specific capabilities none of them offer.

---

## 1. Detection: How the Server Knows What the Client Supports

### 1.1 Accept Header Parsing (Primary Signal)

The HTTP `Accept` header is the only standards-compliant signal. Parse it for image MIME types:

| MIME type | Format | Notes |
|-----------|--------|-------|
| `image/jxl` | JPEG XL | Safari 17+, limited elsewhere |
| `image/avif` | AVIF | Chrome 85+, Firefox 93+, Safari 16.4+ |
| `image/webp` | WebP | Chrome 32+, Firefox 65+, Safari 14+ (macOS 11+) |
| `image/jpeg` | JPEG | Universal |
| `image/png` | PNG | Universal |

**Parsing rules:**
- Match MIME types as substrings in the Accept header (e.g., `/image\/avif/` test). Browsers include these alongside `*/*`.
- Respect `q=` quality weights if present. A `q=0` explicitly rejects that format.
- Multiple formats may be advertised simultaneously. The server picks the best, not the first.

**What Imageflow has today:** The `accept.webp`, `accept.avif`, `accept.jxl` querystring parameters already feed into `AllowedFormats`. The missing piece is middleware that reads the `Accept` header and injects these parameters before the pipeline runs.

### 1.2 Client Hints (Secondary Signal)

Client Hints provide device context that affects quality decisions, not format decisions:

| Header | Use |
|--------|-----|
| `Save-Data: on` | Client requests reduced data. Drop quality profile by one tier (e.g., High → Good). |
| `ECT: slow-2g\|2g\|3g` | Effective connection type. Drop quality on 2g/slow-2g. |
| `RTT` | Round-trip time in ms. If > 200ms, consider reducing quality. |
| `Downlink` | Approximate bandwidth in Mbps. If < 2, consider reducing quality. |
| `DPR` | Device pixel ratio. Feed into `qp-dpr` if not already set in the URL. |
| `Width` | Layout width in CSS pixels. Could inform `w` if not set. |
| `Viewport-Width` | Viewport width. Could inform responsive sizing. |

**Opt-in required.** The server must send `Accept-CH: DPR, Width, Save-Data, ECT, Downlink` for browsers to include these headers. Only Chromium-based browsers support this today.

Cloudflare is the only CDN that implements `Save-Data` and `ECT` quality reduction (`slow-connection-quality` parameter). This is a good feature. Akamai also has RTT-based quality override (RTT >= 300ms).

### 1.3 User-Agent (Tertiary Signal — Use with Caution)

Cloudinary uses UA in addition to Accept. Everyone else relies on Accept alone.

UA parsing is brittle, but it catches two real problems:
1. **Browsers that advertise format support but have rendering bugs.** Safari 14's WebP support was buggy; imgix held back AVIF delivery to Safari until 16.4. Cloudinary maintains an internal list of UA+format combinations to block.
2. **Clients that don't send Accept headers.** Mobile apps frequently omit `Accept` for image requests. Without UA, you can't negotiate at all.

**Recommended approach:** Accept header is authoritative. UA is a veto — use a deny-list of known-broken UA+format combinations, not an allow-list. Ship the deny-list as a config file, not compiled in, so it can be updated without rebuilding.

### 1.4 Detection Priority

```
1. Explicit URL parameter (format=webp, format=avif, etc.) → use it, skip negotiation
2. format=auto or format=keep (with accept.* set) → negotiate:
   a. Parse Accept header → set of supported formats
   b. Check UA deny-list → remove formats with known bugs for this UA
   c. Intersect with AllowedFormats (server config + querystring accept.* params)
   d. Pass to format_auto_select() for content-aware selection
```

---

## 2. Format Selection: Picking the Best Output Format

This is the core algorithm. Imageflow's existing `format_auto_select()` already handles this well. Here's the complete decision tree, annotated with CDN comparisons.

### 2.1 Priority Order (Imageflow's Current Algorithm)

```
1. Animation needed?
   → WebP (animated) if supported → GIF fallback
   (No CDN supports animated AVIF reliably. Animated JXL has no browser support.)

2. JXL available and accepted?
   → JXL (always best quality/size ratio)
   (Only Cloudinary and Fastly support JXL in auto. Imageflow does too.)

3. Alpha or lossless needed?
   → WebP → PNG → AVIF
   (WebP lossless is much smaller than PNG; PNG is more compatible than AVIF for lossless.)

4. Lossy, small image (< 3 Mpx), AVIF available?
   → AVIF (10x slower than JPEG, but worth it for small images)

5. Jpegli available?
   → JPEG via jpegli (comparable to WebP, 10x faster than AVIF)

6. High quality (q > 90) or no progressive JPEG?
   → WebP (beats mozjpeg at high quality)

7. Otherwise?
   → JPEG (mozjpeg) → WebP → AVIF → PNG → GIF
```

**What this already does better than every CDN:** The selection is quality-aware. At medium quality, jpegli beats WebP on size. At high quality, WebP beats mozjpeg. The algorithm switches between them at the crossover point. No other CDN documents quality-dependent format selection.

### 2.2 Content-Aware Selection (Enhancement Opportunities)

Several CDNs analyze image content to influence format selection. Imageflow doesn't do this yet, but could.

| Signal | What to do | Who does this |
|--------|------------|---------------|
| **Transparency** | Already handled — `needs_alpha` excludes JPEG | All CDNs |
| **Animation** | Already handled — `needs_animation` triggers WebP/GIF | All CDNs |
| **Photo vs. graphic** | Graphics with flat colors and hard edges compress better as PNG/WebP lossless than lossy JPEG. Detect via color histogram entropy. | Cloudinary (with q_auto), ImageKit, Uploadcare |
| **Very small images** | Below ~5,000 pixels total, AVIF container overhead exceeds savings. Use WebP or JPEG. | Cloudinary (5K px threshold) |
| **Low source quality** | If source JPEG quality <= 60, re-encoding as WebP may increase size (Cloudflare observed this). Skip format upgrade. | Cloudflare Polish |
| **Multi-encode size comparison** | Encode in 2-3 candidate formats, serve the smallest. | Akamai (default), Sirv, KeyCDN |

**Recommendation:** Start with the small-image AVIF threshold (Cloudinary's 5K pixel rule is a good default). Photo-vs-graphic detection can come later. Multi-encode comparison is expensive and usually unnecessary when the format priority is quality-calibrated (which Imageflow's already is).

### 2.3 Format Capabilities Matrix

When the algorithm needs to decide, here's what each format can and can't do:

| Capability | JPEG | PNG | WebP | AVIF | JXL | GIF |
|------------|------|-----|------|------|-----|-----|
| Lossy | Yes | Via pngquant | Yes | Yes | Yes | — |
| Lossless | — | Yes | Yes | Yes (slow) | Yes | Yes (256 colors) |
| Alpha | — | Yes | Yes | Yes | Yes | 1-bit |
| Animation | — | APNG (limited) | Yes | Buggy/slow | Future | Yes |
| Progressive decode | Yes | Interlace | — | — | Yes | — |
| HDR / wide gamut | — | 16-bit | — | Yes (10/12-bit) | Yes | — |
| ICC profiles | Yes | Yes | Limited | Yes (CICP) | Yes | — |
| Encode speed | Fast | Fast | Medium | Slow (10x JPEG) | Medium | Fast |
| Browser support | Universal | Universal | 97%+ | 93%+ | Safari 17+ only | Universal |

### 2.4 AVIF Encoding Budget

Every CDN that supports AVIF has a fallback mechanism because AVIF encoding is slow. The approaches:

| CDN | AVIF strategy |
|-----|--------------|
| Cloudflare | Best-effort; auto-fallback to WebP on timeout |
| Cloudinary | Account-gated; 30 Mpx limit; skips below 5K px |
| Uploadcare | Hard cutoff at 2 Mpx |
| Fastly | Premium feature; billed higher |
| ImageKit | Notes "can take over a second"; mitigates with scale |
| Bunny | Refused to implement (100x slower, cache complexity) |
| imgix | No documented budget |

**Imageflow's approach (existing):** Pixel-count gating. AVIF is allowed below 3 Mpx when jpegli is available; always allowed when jpegli isn't (because WebP is worse than AVIF at most quality levels). The RFC mentions a future `encoding.speedlimit` mode for background queue processing of AVIF for larger images.

**Recommended AVIF budget strategy:**

```
let avif_allowed = match context {
    RealTime => pixel_count < avif_pixel_budget,  // default: 3 Mpx
    Background => true,                            // encoding.speedlimit mode
    PreGenerated => true,                          // eager/batch
};
```

The `avif_pixel_budget` should be configurable per-server. 3 Mpx is a reasonable default for real-time. Servers with fast CPUs or low traffic can raise it.

---

## 3. Quality Profiles: Unified Cross-Codec Quality

This is where Imageflow is genuinely ahead of every CDN. The `QualityProfileHints` table maps a single quality intent to calibrated per-codec parameters.

### 3.1 The Quality Calibration Table

Imageflow's existing table (from `auto.rs`):

| Profile | Percent | SSIM2 | mozjpeg | jpegli | WebP | WebP effort | AVIF | AVIF speed | JXL dist | JXL effort | PNG qual | PNG max |
|---------|---------|-------|---------|--------|------|-------------|------|------------|----------|------------|----------|---------|
| Lowest | 15 | 10 | 15 | 15 | 15 | 6 | 23 | 6 | 13.0 | 5 | 0 | 10 |
| Low | 20 | 30 | 20 | 20 | 20 | 6 | 34 | 6 | 7.4 | 6 | 0 | 20 |
| MediumLow | 34 | 50 | 34 | 34 | 34 | 6 | 45 | 6 | 4.3 | 5 | 0 | 35 |
| Medium | 55 | 60 | 57 | 52 | 53 | 5 | 44 | 6 | 3.92 | 5 | 0 | 55 |
| Good | 73 | 70 | 73 | 73 | 76 | 6 | 55 | 6 | 2.58 | 5 | 50 | 100 |
| High | 91 | 85 | 91 | 91 | 93 | 5 | 66 | 6 | 1.0 | 5 | 80 | 100 |
| Highest | 96 | 90 | 96 | 96 | 96 | 5 | 100 | 6 | 0.5 | 0 | 90 | 100 |
| Lossless | 100 | 100 | 100 | 100 | 100 | 6 | 100 | 5 | 0.0 | 6 | 100 | 100 |

Values between named profiles are interpolated. The `ssim2` column is the target SSIMULACRA2 score; per-codec values are calibrated to hit that target.

No CDN exposes this level of detail. Cloudinary's `q_auto` has four tiers (best/good/eco/low). Fastly uses SSIMULACRA2 internally but doesn't expose the mapping. Uploadcare has five named tiers. None of them publish their calibration tables.

### 3.2 DPR-Aware Quality Adjustment

When `qp-dpr` is set, the algorithm adjusts quality based on device pixel density:

```
quality_factor = 3.0 / dpr.clamp(0.1, 12.0)
target_ssim2 = (base_ssim2 * quality_factor).clamp(10.0, 90.0)
→ re-derive all codec params from the new SSIM2 target
```

The assumption: at 3x DPR (150 physical DPI, typical modern device), each image pixel maps to ~1 device pixel. At 1x DPR, each image pixel is upscaled 3x on screen, so you want higher quality. At 6x DPR (rare), each image pixel spans 0.5 device pixels, so you can afford lower quality.

No CDN does this. Cloudflare has `slow-connection-quality` (network-aware, not DPR-aware). Some CDNs accept a `dpr` multiplier for dimensions but don't adjust quality.

### 3.3 Save-Data Quality Reduction

When `Save-Data: on` is present in the request:

```
effective_quality = max(quality_profile - 1 tier, Lowest)
```

Cloudinary does this (drops from `q_auto:good` to `q_auto:eco`). Cloudflare does it via `slow-connection-quality`. Imageflow should too.

### 3.4 How CDN Quality Systems Compare

| CDN | Quality model | Perceptual metric | Per-codec calibration | DPR-aware |
|-----|--------------|-------------------|----------------------|-----------|
| **Imageflow** | 8 named tiers + 0-100 continuous + per-codec overrides | SSIMULACRA2 target | Yes (8 codecs calibrated) | Yes (qp-dpr) |
| Cloudinary | 4 tiers (best/good/eco/low) + 0-100 manual | Custom SSIM-derived | Undocumented | No |
| imgix | Single q= 0-100, auto=compress sets to 45 | Not documented | No | No |
| Cloudflare | Single quality 1-100 or 4 named levels | Not documented | No | No |
| Fastly | Single quality + dual-quality for WebP | SSIMULACRA2 | Undocumented | No |
| ImageKit | Single quality 0-100, default 80 | Not documented | No | No |
| Akamai | Static or perceptual quality | Perceptual (undocumented) | Undocumented | No |
| Uploadcare | 5 named tiers | Not documented | No | No |

---

## 4. Cache Key Management

This is the hardest part of f_auto. The same URL must serve different formats to different clients, and the CDN cache must handle this correctly. Every CDN solves it differently.

### 4.1 The Problem

```
GET /photo.jpg?format=auto
  → Chrome:  200 OK, Content-Type: image/avif
  → Safari:  200 OK, Content-Type: image/jxl
  → Firefox: 200 OK, Content-Type: image/webp
  → IE 11:   200 OK, Content-Type: image/jpeg
```

If a CDN caches the Chrome response and serves it to IE 11, the browser gets an AVIF it can't decode. Broken image.

### 4.2 How CDNs Solve This

| CDN | Cache key strategy |
|-----|-------------------|
| **Cloudflare** | `Vary: Accept` on the response. Separate cache entries per Accept header value. Requires "Vary for Images" setting (Pro+ plan). Without it, format=auto may serve wrong format from cache. |
| **Cloudinary** | `Vary: Accept, User-Agent`. CDN edge translates `f_auto` to concrete `f_avif`/`f_webp`/`f_jpeg` before reaching origin, so each variant has a distinct internal cache key. |
| **imgix** | Internal CDN management. Doesn't support third-party CDNs in front (warns it breaks negotiation). |
| **Fastly** | **Strips the Vary header** from responses. Handles variant caching internally at shield PoPs. |
| **Akamai** | Policy name + version in cache key. Derivatives cached separately from pristine. No standard HTTP Vary. |
| **Bunny** | Custom "Vary Cache" system. WebP variant cache auto-enabled with Optimizer. Separate toggle per variation type (WebP, UA, country). |
| **KeyCDN** | Query string becomes part of cache key. Origin Shield auto-enabled. |

### 4.3 Recommended Approach for Imageflow

Imageflow is a processing engine, not a CDN. The cache key problem belongs to whatever sits in front of Imageflow (Cloudflare, Varnish, nginx, etc.). But Imageflow needs to cooperate.

**What Imageflow should do:**

1. **Set `Vary: Accept` on every response where `format=auto` was used.** This tells any RFC-compliant cache to store separate entries per Accept header value. It's the standard mechanism.

2. **Include the resolved format in a response header.** Add `X-Imageflow-Format: avif` (or whatever was selected). This helps CDN rules, logging, and debugging.

3. **Support a `format.resolved` query parameter** that CDN edge rules can set. A CDN Worker/VCL can parse Accept, resolve the format, and rewrite `?format=auto` to `?format=avif` before hitting Imageflow. This turns the variant into a distinct URL, eliminating the Vary problem entirely. Cloudinary does exactly this at their edge.

4. **Document CDN configuration.** For each major CDN, provide the exact config needed:
   - Cloudflare: Enable Vary for Images, or use a Worker to rewrite format=auto
   - Fastly: VCL snippet to parse Accept and set format
   - Varnish: VCL example for Vary normalization
   - nginx: map directive for Accept-based format rewrite
   - Bunny: Enable WebP Vary Cache

**What Imageflow should NOT do:**

- Don't normalize or truncate the Accept header (that's the CDN's job).
- Don't try to be a CDN. Don't cache responses internally based on Accept (that's not the processing engine's concern).

### 4.4 Vary Header Normalization

Raw `Accept` headers vary wildly between browsers and versions. Caching on the raw header creates excessive cache fragmentation. CDNs normalize the header before using it as a cache key.

**Recommended normalization for CDN edge rules** (not in Imageflow itself):

```
Accept: image/avif,image/webp,image/apng,image/jxl,*/*;q=0.8
  → normalized cache key: "avif+webp+jxl"

Accept: image/webp,image/apng,*/*;q=0.8
  → normalized cache key: "webp"

Accept: */*
  → normalized cache key: "none"
```

Extract the set of `{avif, webp, jxl}` present in the Accept header, sort, join. This reduces the variant space from thousands of Accept strings to 8 combinations (2^3).

---

## 5. Animated Content Handling

### 5.1 CDN Landscape

| CDN | GIF→aWebP | GIF→aAVIF | GIF→MP4 | aWebP→aAVIF |
|-----|-----------|-----------|---------|-------------|
| Cloudinary | Yes | Yes (Jan 2023) | Via video | No |
| imgix | Yes | Firefox: static only | Premium | — |
| Cloudflare | Passthrough (<50 Mpx) | No | No | — |
| ImageKit | — | — | Yes (suffix) | — |
| Akamai | Yes | No | Yes (requires video contract) | — |
| Fastly | No | No | Yes (GIF source only) | — |
| Bunny | Yes | No | No | — |
| Sirv | Yes | No | No | — |
| KeyCDN | No | No | No | — |
| Uploadcare | No | No | gif2video op | — |

### 5.2 Imageflow's Approach

Current state: `format_auto_select()` checks `needs_animation` and picks WebP or GIF. WebP animation isn't implemented yet (`FEATURES_IMPLEMENTED.webp_animation = false`), so animated input always falls through to GIF.

**Recommended behavior when animated WebP encoding lands:**

```
Animation detected:
  1. If accept.webp=true and animated WebP implemented → animated WebP
  2. Else → GIF (preserve original)

  Do NOT attempt:
  - Animated AVIF (too slow, browser support poor, Firefox renders as static)
  - Animated JXL (no browser support)
  - GIF→MP4 (different pipeline; video transcoding is out of scope)
```

**Frame count and size limits:**
- Cloudflare: 50 Mpx total across all frames
- Fastly: 1,000 frames max

Imageflow should enforce a configurable limit (default: 50 Mpx total, 500 frames). Exceed the limit → serve original GIF without re-encoding.

### 5.3 Single-Frame Animated GIF Detection

Cloudinary detects single-frame "animated" GIFs and treats them as still images. This avoids the overhead of animation-path encoding for what's effectively a static image. Imageflow should do the same — if frame count == 1, clear the `needs_animation` flag.

---

## 6. Fallback Chains and Error Handling

### 6.1 Encoding Failure Fallback

If the selected format's encoder fails or times out:

```
JXL fails → AVIF (if budget allows) → WebP → JPEG → PNG → error
AVIF fails → WebP → JPEG → PNG → error
WebP fails → JPEG → PNG → error
JPEG fails → PNG → error
PNG fails → error
```

Cloudflare's AVIF is explicitly "best-effort" with WebP fallback. This is the right model.

### 6.2 Size Guard (Shrink Guarantee)

Imageflow already has this: if the output file is larger than the input, serve the original. Fastly does the same ("original image is delivered" when output exceeds input). KeyCDN returns `x-ip: 2` ("original returned as smaller").

This should be enabled by default with `format=auto`. Disable with `shrink_guarantee=false` for cases where format upgrade is mandatory (e.g., converting HEIC input to web-safe output).

### 6.3 Format Override vs. Auto

When explicit format parameters coexist with auto:

| Parameters | Behavior |
|-----------|----------|
| `format=auto` | Full negotiation |
| `format=webp` | WebP, period. No negotiation. |
| `format=auto&accept.avif=false` | Negotiate, but exclude AVIF |
| `format=auto&accept.webp=false&accept.avif=false` | Negotiate, but only JXL or legacy formats |
| `format=keep` | Match source format. If source is AVIF and client can't decode AVIF, this may serve an undisplayable image. Explicit user choice. |
| `format=keep&accept.webp=true` | If source is WebP, output WebP. If source is JPEG, output JPEG. `keep` means keep, regardless of accept flags. Accept flags only matter for `format=auto`. |
| `qp=good` (no format) | Implies `format=auto` (existing behavior). |

---

## 7. HTTP Response Headers

### 7.1 Required Headers

```http
Content-Type: image/avif
Vary: Accept
X-Imageflow-Format: avif
```

- `Content-Type`: The actual delivered format. Must match the encoded bytes.
- `Vary: Accept`: Tells caches to vary on the Accept header. Only include when `format=auto` was used. If the format was explicitly requested (`format=webp`), don't add Vary.
- `X-Imageflow-Format`: The format that was selected. Useful for debugging, CDN rules, and logging.

### 7.2 Informational Headers

```http
X-Imageflow-Quality: high
X-Imageflow-Format-Candidates: jxl,avif,webp,jpeg
X-Imageflow-Format-Selected: avif
X-Imageflow-Format-Reason: accept+budget
```

These help operators debug format selection. Include in development/debug mode; strip in production if header size is a concern.

### 7.3 What CDNs Return

| CDN | Format header | Vary header | Other |
|-----|--------------|-------------|-------|
| Cloudflare | Standard Content-Type | Vary: Accept (if configured) | `cf-polished`, `cf-bgj` (bytes saved) |
| KeyCDN | Standard Content-Type | — | `x-ip: 0\|1\|2`, `x-ip-info: size,dims,format` |
| Fastly | Standard Content-Type | Vary stripped | — |
| Cloudinary | Standard Content-Type | Vary: Accept, User-Agent | — |

---

## 8. Configuration Surface

### 8.1 URL Querystring Parameters (Existing)

```
format=auto|keep|jpeg|png|webp|gif|avif|jxl
accept.webp=true|false
accept.avif=true|false
accept.jxl=true|false
accept.color_profiles=true|false
qp=lowest|low|medium|good|high|highest|lossless|0..100|quality
qp-dpr=0.1..12.0|dpr
lossless=true|false|keep
quality=0..100                    (legacy, codec-specific)
jpeg.quality=0..100               (per-codec override)
webp.quality=0..100
png.quality=0..100
avif.quality=0..100
avif.speed=3..10
jxl.quality=0..100
jxl.distance=0..25
jxl.effort=0..7
```

### 8.2 Server-Level Configuration (New)

These control behavior across all requests. Set in the server config, not per-URL.

```toml
[format_auto]
# Parse Accept header and set accept.* params automatically
accept_header_negotiation = true

# UA deny-list file path (known-broken UA+format combinations)
ua_denylist = "config/ua-format-denylist.toml"

# Maximum pixel count for real-time AVIF encoding
avif_pixel_budget = 3_000_000

# Minimum pixel count for AVIF (below this, AVIF container overhead isn't worth it)
avif_min_pixels = 5_000

# Maximum pixel count for real-time JXL encoding (less of a concern; JXL is faster)
jxl_pixel_budget = 50_000_000

# Enable background queue for AVIF/JXL encoding of images above the budget
background_encode = false

# Reduce quality by one tier when Save-Data: on is present
save_data_quality_reduction = true

# Reduce quality when network hints indicate slow connection
slow_connection_quality_reduction = true
slow_connection_rtt_threshold_ms = 200
slow_connection_ect_threshold = "3g"

# Shrink guarantee: if output > input, serve original
shrink_guarantee = true

# Animation limits
max_animation_total_pixels = 50_000_000
max_animation_frames = 500

# Default quality profile when none specified
default_quality_profile = "high"

# Default format when none specified
default_format = "keep"  # or "auto" for new deployments
```

### 8.3 Site-Wide Defaults

As described in the RFC, recommended site defaults for new deployments:

```
format=auto&qp=quality&qp-dpr=dpr&f.sharpen=23&down.filter=mitchell&ignore_icc_errors=true
```

This means:
- `format=auto`: Negotiate output format via Accept header
- `qp=quality`: Reinterpret legacy `&quality=N` through the unified quality profile system
- `qp-dpr=dpr`: Read DPR from `&dpr=` parameter (if present) and adjust quality accordingly
- The rest are processing defaults (sharpening, resampling filter, ICC error tolerance)

---

## 9. CDN Integration Guides

### 9.1 Cloudflare (Workers)

```javascript
export default {
  async fetch(request) {
    const url = new URL(request.url);
    const accept = request.headers.get("Accept") || "";

    // Resolve format from Accept header
    if (url.searchParams.get("format") === "auto") {
      let format = "jpeg";
      if (/image\/jxl/.test(accept))       format = "jxl";
      else if (/image\/avif/.test(accept)) format = "avif";
      else if (/image\/webp/.test(accept)) format = "webp";

      url.searchParams.set("format", format);
      url.searchParams.delete("accept.webp");
      url.searchParams.delete("accept.avif");
      url.searchParams.delete("accept.jxl");
    }

    // Forward to Imageflow origin with resolved format
    // No Vary needed — each format is a distinct URL now
    return fetch(url.toString(), { headers: request.headers });
  }
};
```

### 9.2 Cloudflare (without Workers)

Enable "Vary for Images" in the Cloudflare dashboard. Imageflow sets `Vary: Accept` on responses. Cloudflare stores separate cache entries per Accept value. This works but creates more cache variants than the Worker approach.

### 9.3 Fastly (VCL)

```vcl
sub vcl_recv {
  if (req.url ~ "format=auto") {
    # Normalize Accept into a cache key suffix
    set req.http.X-Img-Format = "jpeg";
    if (req.http.Accept ~ "image/jxl")       { set req.http.X-Img-Format = "jxl"; }
    elsif (req.http.Accept ~ "image/avif")   { set req.http.X-Img-Format = "avif"; }
    elsif (req.http.Accept ~ "image/webp")   { set req.http.X-Img-Format = "webp"; }

    # Rewrite URL to resolved format
    set req.url = regsuball(req.url, "format=auto", "format=" + req.http.X-Img-Format);
  }
}
```

### 9.4 Varnish

```vcl
sub vcl_recv {
  if (req.url ~ "format=auto") {
    if (req.http.Accept ~ "image/jxl") {
      set req.http.X-Normalized-Accept = "jxl";
    } elsif (req.http.Accept ~ "image/avif") {
      set req.http.X-Normalized-Accept = "avif";
    } elsif (req.http.Accept ~ "image/webp") {
      set req.http.X-Normalized-Accept = "webp";
    } else {
      set req.http.X-Normalized-Accept = "jpeg";
    }
  }
}

sub vcl_hash {
  if (req.http.X-Normalized-Accept) {
    hash_data(req.http.X-Normalized-Accept);
  }
}
```

### 9.5 nginx

```nginx
map $http_accept $img_format {
  default         "jpeg";
  "~image/jxl"    "jxl";
  "~image/avif"   "avif";
  "~image/webp"   "webp";
}

server {
  location /images/ {
    # Rewrite format=auto to resolved format
    if ($args ~ "format=auto") {
      rewrite ^(.*)$ $1 break;
      set $args "${args}&accept.$img_format=true";
    }
    proxy_pass http://imageflow:3000;
    proxy_set_header Accept $http_accept;
  }
}
```

---

## 10. Edge Cases and Known Pitfalls

### 10.1 Googlebot

Googlebot sends `Accept: image/avif,image/webp,*/*`. It can index and display AVIF and WebP. Serve the best format — no special handling needed.

### 10.2 Email Clients

Email clients have limited format support. Most only handle JPEG, PNG, and GIF. If you're generating images for email, use `format=jpeg` or `format=png` explicitly. Don't rely on `format=auto` — email clients don't send useful Accept headers.

### 10.3 RSS Readers

Similar to email. Many RSS readers delegate image display to a basic rendering engine that may not support WebP/AVIF. Use explicit formats for RSS-embedded images.

### 10.4 Open Graph / Social Media Previews

Facebook, Twitter, LinkedIn, and other social platforms crawl OG images with their own scrapers. These scrapers may not send Accept headers for modern formats. Facebook's crawler supports JPEG, PNG, GIF, and WebP. Twitter's supports JPEG, PNG, GIF, and WebP.

**Recommendation:** For OG image URLs, use `format=auto` — the crawlers advertise WebP support when they have it. If they don't, they get JPEG. Works correctly with Accept-based negotiation.

### 10.5 Safari Version Fragmentation

| Safari version | WebP | AVIF | JXL |
|---------------|------|------|-----|
| < 14 | No | No | No |
| 14-15 (macOS 11+) | Yes (buggy in 14.0) | No | No |
| 16.0-16.3 | Yes | No | No |
| 16.4+ | Yes | Yes | No |
| 17.0+ | Yes | Yes | Yes |

imgix held back AVIF from Safari until 16.4 due to bugs. If using UA deny-list, consider blocking AVIF for Safari 16.0-16.3 (they don't advertise `image/avif` in Accept anyway, so this is a non-issue for Accept-based negotiation).

### 10.6 iOS WebView

iOS WebView inherits Safari's format support but may not send the same Accept headers as Safari proper. Mobile apps that use WebView for image display should set Accept headers explicitly, or use `accept.webp=true` in the URL.

### 10.7 Proxy Caches and CDN Layering

imgix explicitly warns against placing a third-party CDN in front of imgix — it breaks content negotiation. The same risk applies to Imageflow.

If a proxy cache sits between the CDN edge and Imageflow, it must either:
1. Forward the `Accept` header and respect the `Vary: Accept` response, or
2. Let the CDN edge resolve `format=auto` to a concrete format before the request reaches the proxy

Option 2 is simpler and more cache-efficient. This is why the CDN integration guides above rewrite the URL at the edge.

### 10.8 Content Credentials (C2PA)

Cloudflare preserves and extends C2PA provenance chains through image transformations. This is a differentiator no other CDN has.

Imageflow should preserve C2PA manifests when present in the source image and the output format supports them (JPEG, PNG, WebP, AVIF, JXL all support C2PA via JUMBF). If the image is re-encoded, the C2PA chain should note that a transformation occurred. This is a future feature, not blocking for f_auto.

---

## 11. Metrics and Observability

### 11.1 What to Log

For every request where `format=auto` is used:

```
format_requested: "auto"
format_resolved: "avif"
format_candidates: ["jxl", "avif", "webp", "jpeg"]
format_reason: "accept_header"
accept_header: "image/avif,image/webp,*/*"
quality_profile: "high"
quality_profile_dpr: 2.0
quality_effective: "good"  # after DPR adjustment
encode_time_ms: 142
input_bytes: 284000
output_bytes: 71000
compression_ratio: 4.0
pixel_count: 1920000
shrink_guarantee_applied: false
save_data: false
```

### 11.2 Aggregate Metrics

Track over time:
- Format distribution (what % of responses are AVIF, WebP, JXL, JPEG, PNG)
- Compression ratio by format
- Encode latency by format and pixel count (for AVIF budget tuning)
- Shrink guarantee trigger rate (if high, your default quality may be too low)
- Cache hit ratio per format variant (if CDN provides this)

---

## 12. Implementation Phases

### Phase 1: Server-Side Accept Header Integration

Wire the existing `format_auto_select()` to HTTP. The algorithm is ready; it needs a server binding.

1. In the Imageflow server HTTP handler, parse `Accept` header
2. If `format=auto` (or `format` is absent and `qp` is set):
   - Check for `image/jxl` → set `accept.jxl=true`
   - Check for `image/avif` → set `accept.avif=true`
   - Check for `image/webp` → set `accept.webp=true`
3. Set `Vary: Accept` on the response
4. Set `X-Imageflow-Format: {resolved}` on the response

### Phase 2: Save-Data and Client Hints

1. Parse `Save-Data: on` → reduce quality tier by one
2. Parse `ECT` / `RTT` / `Downlink` for further quality reduction
3. Parse `DPR` → feed into `qp-dpr` if not already set
4. Send `Accept-CH: DPR, Save-Data, ECT, Downlink` in responses

### Phase 3: UA Deny-List

1. Ship a default deny-list of known-broken UA+format combinations
2. Load from config file, hot-reloadable
3. Check UA against deny-list after Accept parsing, remove blocked formats

### Phase 4: Content-Aware Enhancement

1. AVIF minimum pixel threshold (skip below 5K px)
2. Single-frame animated GIF detection
3. Photo-vs-graphic classifier (color entropy based) for lossless format selection

### Phase 5: AVIF Background Queue

1. For images above `avif_pixel_budget`, encode JPEG/WebP immediately
2. Queue AVIF encoding as a background task
3. On completion, store AVIF variant in cache
4. Subsequent requests get the AVIF variant

### Phase 6: CDN Integration Documentation

1. Cloudflare Worker snippet (tested)
2. Fastly VCL snippet (tested)
3. Varnish config (tested)
4. nginx config (tested)
5. Bunny, KeyCDN, Akamai configuration guides
