# Focus Rects, Smart Crop & Face Detection — Implementation Plan

Addresses GitHub issues #594 (focus rect / anchor improvement) and #602 (focal point support).

## Architecture Overview

Three layers, each independently useful:

```
Layer 1: Focus rects in URL/JSON API  →  deterministic crop alignment
Layer 2: Analysis engine (saliency + faces)  →  produces focus rects as JSON
Layer 3: On-the-fly smart crop  →  runs analysis at request time, feeds into Layer 1
```

Users can:
- Supply focus rects manually via URL (`&c.focus=...`) or JSON → Layer 1
- Pre-analyze images and cache JSON results → Layer 2 output fed to Layer 1
- Request auto-analysis at request time (`&c.focus=auto`) → Layer 3

---

## Layer 1: Focus Rects in the Crop Pipeline

### New URL Parameters

```
&c.focus=x1,y1,x2,y2           # single focus rect (percentages 0-100)
&c.focus=x1,y1,x2,y2;...       # multiple rects, semicolon-separated, priority order
&c.focus=auto                   # trigger on-the-fly analysis (Layer 3)
&c.focus=faces                  # on-the-fly face detection only
```

The existing `&c.gravity=x,y` (single point) continues to work unchanged. `c.focus` is the rectangle-aware upgrade.

### New JSON Types

```rust
/// A region of interest, in percentage coordinates (0-100)
pub struct FocusRect {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub weight: f32,        // priority: higher = more important. default 1.0
    pub kind: FocusKind,    // face, text, saliency, user, etc.
}

pub enum FocusKind {
    User,       // manually specified
    Face,       // from face detection
    Saliency,   // from saliency analysis
    Text,       // from text detection (future)
}

// Extended Constraint
pub struct Constraint {
    // ... existing fields ...
    pub gravity: Option<ConstraintGravity>,
    pub focus: Option<Vec<FocusRect>>,  // NEW
}
```

### Crop Alignment Logic

When `focus` is present and mode involves cropping:

1. **Compute focus centroid**: weighted center of all focus rects
2. **Use centroid as gravity**: replaces the fixed anchor/gravity point
3. **Validate**: if the crop window can't contain the highest-priority focus rect,
   shift the crop to maximize coverage of highest-weight rect first
4. **Fallback**: if impossible (focus rect larger than crop window), center on
   the focus rect's center (same behavior users requested in #602)

This plugs into the existing `align_gravity()` call in `Ir4Layout::process_constraint()`
and `get_crop_and_layout()` — replacing the `IdentityCropProvider` with a focus-aware one.

### RIAPI Layout Validation

Add validation to `Ir4Layout` that catches nonsensical parameter combinations:
- `c.focus` without a crop mode → warning/ignore
- Focus rect coordinates outside 0-100 → clamp with warning
- Multiple conflicting gravity sources (anchor + c.gravity + c.focus) → priority: c.focus > c.gravity > anchor

---

## Layer 2: Image Analysis Engine (new crate: `imageflow_focus`)

A new crate providing detection/analysis that outputs `Vec<FocusRect>`.

### Face Detection (feature-gated: `faces`)

**Approach**: Fork/vendor rustface, clean up unsafe, or wrap it behind a feature gate.

Rustface assessment:
- Pure Rust, BSD-2-Clause, 1.2MB model file (embeddable via include_bytes!)
- Takes grayscale &[u8], returns bounding boxes + scores
- 29 unsafe blocks (C++ port) — must be behind feature gate since imageflow uses forbid(unsafe)
- Dormant but stable (last substantive change 2021, contributed to by kornelski)
- Only frontal face detection, no side/profile

**Integration plan**:
- Add `rustface` as optional dependency behind `faces` feature
- Embed model via `include_bytes!` (1.2MB, compressed ~800KB in binary)
- Convert imageflow's BGRA bitmap to grayscale, run detection, map pixel coords to percentages
- Default min face size: 3% of image smaller dimension
- Output: `Vec<FocusRect>` with `kind: FocusKind::Face`

### Saliency Detection

**Approach**: Frequency-tuned saliency (Achanta et al.) — simplest, fastest, full-resolution output.

Algorithm (~40 lines of core code):
1. Compute mean R, G, B of entire image
2. Gaussian blur the image (sigma ≈ image_width / 2.75)
3. Per-pixel: saliency = squared Euclidean distance from mean to blurred pixel
4. Normalize to [0, 1]
5. Threshold + find bounding box of salient region(s)

Can operate at reduced resolution (256x256) and map back. No FFT needed, no model files,
no external crates beyond what imageflow already has.

**Output**: 1-3 `FocusRect` entries with `kind: FocusKind::Saliency`, derived by thresholding
the saliency map and finding connected component bounding boxes.

### Edge/Detail Density (supplementary signal)

Simple Laplacian on luminance → score map. Used to penalize crop candidates that cut through
detailed areas (the smartcrop.js "edge penalty" concept). Not exposed as focus rects but used
internally for crop scoring in Layer 3.

### Analysis JSON Response

New JSON API endpoint / node type that returns analysis results without modifying the image:

```json
{
  "focus_regions": [
    {"x1": 22.5, "y1": 30.0, "x2": 45.0, "y2": 80.0, "weight": 10.0, "kind": "face"},
    {"x1": 10.0, "y1": 15.0, "x2": 70.0, "y2": 85.0, "weight": 1.0, "kind": "saliency"}
  ],
  "image_width": 4000,
  "image_height": 3000,
  "analysis_ms": 23
}
```

This JSON can be cached (CDN, database, sidecar file) and later plugged into URLs:
```
/image.jpg?w=400&h=300&mode=crop&c.focus=22.5,30,45,80;10,15,70,85
```

### URL endpoint for analysis

```
/image.jpg?analyze=focus          → returns JSON, no image
/image.jpg?analyze=faces          → faces only
/image.jpg?analyze=saliency       → saliency only
/image.jpg?analyze=all            → everything available
```

---

## Layer 3: On-the-fly Smart Crop

When `&c.focus=auto` or `&c.focus=faces`:

1. Decode image (or use IDCT-scaled preview for JPEG)
2. Run requested detectors (faces, saliency, or both)
3. Produce `Vec<FocusRect>`
4. Feed into Layer 1's crop alignment logic
5. Continue normal resize pipeline

Performance budget: <50ms on a 1024x1024 image for saliency, <100ms including face detection.

Optimization: For JPEG sources, use the 1/8 IDCT downscale (~128x128) for saliency analysis
and 1/2 IDCT (~512x512) for face detection, avoiding full decode.

---

## Implementation Order

### Phase 1: Focus rects in crop pipeline (Layer 1)
1. Add `FocusRect`, `FocusKind` types to `imageflow_types`
2. Add `focus: Option<Vec<FocusRect>>` to `Constraint`
3. Parse `&c.focus=...` in RIAPI parser (`ir4/parsing.rs`)
4. Implement focus-aware crop alignment in `ir4/layout.rs` (replace IdentityCropProvider)
5. Add RIAPI validation for conflicting/invalid parameters
6. Tests with known focus rects → verify crop placement

### Phase 2: Saliency detection (Layer 2, safe Rust)
7. Create `imageflow_focus` crate (or module in imageflow_core)
8. Implement frequency-tuned saliency (~40 lines)
9. Implement saliency map → bounding box extraction
10. Wire into JSON API as analysis endpoint
11. Visual regression tests with known images

### Phase 3: Face detection (Layer 2, feature-gated)
12. Add rustface dependency behind `faces` feature flag
13. Embed model file
14. Grayscale conversion + detection wrapper
15. Map pixel bboxes to percentage FocusRects
16. Wire into analysis endpoint and on-the-fly path

### Phase 4: On-the-fly integration (Layer 3)
17. `&c.focus=auto` / `&c.focus=faces` parsing
18. Hook analysis into the decode → constrain pipeline
19. JPEG IDCT-scale optimization for faster analysis
20. Performance benchmarks

---

## Crate/Feature Structure

```
imageflow_types/         # FocusRect, FocusKind types (no new deps)
imageflow_riapi/         # c.focus parsing, validation, focus-aware layout
imageflow_focus/         # NEW crate: saliency + face detection
  default features: [saliency]
  optional features: [faces]  ← pulls in rustface, allows unsafe in this crate only
imageflow_core/          # wires focus analysis into pipeline, JSON endpoints
```

The `faces` feature is opt-in. Users who don't need face detection get no extra binary size
or unsafe code. Saliency is pure safe Rust with no external dependencies.

## Key Design Decisions

1. **Percentages (0-100) everywhere** — per user consensus in #602
2. **Focus rects, not points** — single points are a degenerate rect (x,y,x,y)
3. **Priority by weight, not position** — highest-weight rect wins when conflict
4. **No padding to preserve focus** — crop through focus rect if aspect ratio forces it (per HRasch's preference in #602)
5. **Analysis JSON is reusable** — compute once, use in many URL variants
6. **Feature-gated unsafe** — face detection behind `faces` flag, saliency is pure safe Rust
