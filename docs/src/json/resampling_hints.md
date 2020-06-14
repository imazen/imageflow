# Resampling Hints

Resampling hints can be specified in constraint commands, scale commands, watermarking, and for compositing. They offer control over image sharpness, resampling color space, background color, and more. 


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
