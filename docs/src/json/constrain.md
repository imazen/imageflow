
### Constrain

* `w` The width constraint in pixels
* `h` The height constraint in pixels
* `mode` A [constraint mode](#constraint-modes)
* `gravity` determines how the image is anchored when cropped or padded. 
`{x: 0, y: 0}` represents top-left, `{x: 50, y: 50}` represents center, 
`{x:100, y:100}` represents bottom-right. *Default: `center`*
* `hints` See [resampling hints](#resampling-hints)
* `canvas_color` See [Color](#colors). The color of padding added to the image. 

```json
{ 
  "constrain": {
    "mode": "within",
    "w": 800,
    "h": 600,
    "hints": {
      "sharpen_percent": 7 
    },
    "gravity": { "percentage":  { "x":  50, "y":  50}},
    "canvas_color": "transparent"
  }
}
```


### Constraint Modes

* `distort` Distort the image to exactly the given dimensions.
If only one dimension is specified, behaves like `fit`.
* `within`
Ensure the result fits within the provided dimensions. No upscaling.
* `fit`
Fit the image within the dimensions, upscaling if needed
* `larger_than`
Ensure the image is larger than the given dimensions
* `within_crop`
Crop to desired aspect ratio if image is larger than requested, then downscale. Ignores smaller images.
If only one dimension is specified, behaves like `within`.
* `fit_crop`
Crop to desired aspect ratio, then downscale or upscale to fit.
If only one dimension is specified, behaves like `fit`.
* `aspect_crop`
Crop to desired aspect ratio, no upscaling or downscaling. If only one dimension is specified, behaves like Fit.
* `within_pad`
Pad to desired aspect ratio if image is larger than requested, then downscale. Ignores smaller images.
If only one dimension is specified, behaves like `within`
* `fit_pad`
Pad to desired aspect ratio, then downscale or upscale to fit
If only one dimension is specified, behaves like `fit`.
