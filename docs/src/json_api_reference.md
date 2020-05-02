# JSON API Reference


## Node Reference

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

### Resampling Filters
 
* `robidoux` - The default and recommended downsampling filter
* `robidoux_sharp` - A sharper version of the above
* `robidoux_fast` - A faster, less accurate version of robidoux
* `ginseng` - The default and suggested upsampling filter
* `ginseng_sharp` 
* `lanczos` 
* `lanczos_sharp`   
* `lanczos_2` 
* `lanczos_2_sharp`  
* `cubic` 
* `cubic_sharp` 
* `catmull_rom` 
* `mitchell` 
* `cubic_b_spline` 
* `hermite` 
* `jinc` 
* `triangle` 
* `linear` 
* `box` 
* `fastest` 
* `n_cubic` 
* `n_cubic_sharp` 

### Colors

* `transparent`
* `black`
* `{ "srgb": { "hex" : "ffffff" } }`

### Resampling Hints

* `sharpen_percent` (0..100) The amount of sharpening to apply during resampling
* `up_filter` The resampling filter to use if upscaling in one or more directions
* `down_filter` The resampling filter to use if downscaling in both directions.
* `scaling_colorspace` Use `linear` for the best results, or `srgb` to mimick poorly-written software. `srgb` can destroy image highlights.
* `background_color` The background color to apply. 
* `resample_when` One of `size_differs`, `size_differs_or_sharpening_requested`, or `always`.
* `sharpen_when` One of `downscaling`, `upscaling`, `size_differs`, or `always`

```json
{ 
  "sharpen_percent": 15,
  "down_filter":  "robidoux",
  "up_filter": "ginseng",
  "scaling_colorspace": "linear",
  "background_color": "transparent",
  "resample_when": "size_differs_or_sharpening_requested",
  "sharpen_when": "downscaling"
}
```

### Watermark

* `io_id` (**required**) specifies which input image to use as a watermark. 
* `gravity` determines how the image is placed within the `fit_box`. 
`{x: 0, y: 0}` represents top-left, `{x: 50, y: 50}` represents center, 
`{x:100, y:100}` represents bottom-right. *Default: `center`*
* `fit_mode` is one of `distort`, `within`, `fit`, `within_crop`, or `fit_crop`. 
 Meanings are the same as for [constraint modes](#constraint-modes). *Default: `within`*
* `fit_box` can be either `image_percentage` (a box represented by percentages of target image width/height) or 
`image_margins` (a box represented by pixels from the edge of the image). *Default `image_margins` 0*
* `opacity` (0..1) How opaque to draw the image. *Default 1.0*
* `hints` See [resampling hints](#resampling-hints)
#### Example with fit_box: image_percentage
This will align the watermark to 10% from the bottom and right edges of the image, 
scaling the watermark down if it takes more than 80% of the image space,
drawing it at 80% opacity and applying 15% sharpening.  
```json
{
  "watermark": {
    "io_id": 1,
    "gravity": { 
      "percentage" : {
        "x": 100,
        "y": 100 
      }
    },
    "fit_mode": "within",
    "fit_box": { 
      "image_percentage": {
        "x1": 10,
        "y1": 10,
        "x2": 90,
        "y2": 90
      } 
    },
    "opacity": 0.8,
    "hints": {
      "sharpen_percent": 15
    }
  }
}
```

#### Example with fit_box: image_margins
This will stretch/distort the watermark to fill the image except for a 5px margin.
```json
{
  "watermark": {
    "io_id": 1,
    "gravity": { "center": null },
    "fit_mode": "distort",
    "fit_box": { 
      "image_margins": {
        "left": 5,
        "top": 5,
        "right": 5,
        "bottom": 5
      } 
    }
  }
}
```

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
