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


### Resampling Hints

```json
{ 
  "sharpen_percent": 15
}
    pub sharpen_percent: Option<f32>,
    pub down_filter: Option<Filter>,
    pub up_filter: Option<Filter>,
    pub scaling_colorspace: Option<ScalingFloatspace>,
    pub background_color: Option<Color>,
    pub resample_when: Option<ResampleWhen>,
    pub sharpen_when: Option<SharpenWhen>

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
  {"watermark": {
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
  }}
```

#### Example with fit_box: image_margins
This will stretch/distort the watermark to fill the image except for a 5px margin.
```json
  {"watermark": {
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
  }}
```

### Constrain

` `
