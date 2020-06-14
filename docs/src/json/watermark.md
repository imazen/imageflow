
### Watermark

* `io_id` (**required**) specifies which input image to use as a watermark. 
* `gravity` determines how the image is placed within the `fit_box`. 
`{x: 0, y: 0}` represents top-left, `{x: 50, y: 50}` represents center, 
`{x:100, y:100}` represents bottom-right. *Default: `center`*
* `fit_mode` is one of `distort`, `within`, `fit`, `within_crop`, or `fit_crop`. 
 Meanings are the same as for [constraint modes](constrain.md#constraint-modes). *Default: `within`*
* `fit_box` can be either `image_percentage` (a box represented by percentages of target image width/height) or 
`image_margins` (a box represented by pixels from the edge of the image). *Default `image_margins` 0*
* `min_canvas_width` sets a minimum canvas width below which the watermark will be hidden. 
* `min_canvas_height` sets a minimum canvas height below which the watermark will be hidden. 
* `opacity` (0..1) How opaque to draw the image. *Default 1.0*
* `hints` See [resampling hints](resampling_hints.md)
#### Example with fit_box: image_percentage
This will align the watermark to 10% from the bottom and right edges of the image, 
scaling the watermark down if it takes more than 80% of the image space,
drawing it at 80% opacity and applying 15% sharpening. It will not display on images smaller than 50x50px in either dimension.
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
    "min_canvas_width": 50,
    "min_canvas_height": 50,
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
