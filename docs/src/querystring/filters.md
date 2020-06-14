## Image Filter Commands

* `f.sharpen=0..99` determines how much sharpening to apply when scaling the image.
* `f.sharpenwhen=always|downscaling|sizediffers` determines when to sharpen. 
* `down.filter`determines the down-sampling filter to use. Must be one of `robidoux`, 
`robidoux_sharp`, `robidoux_fast`, `ginseng`, `ginseng_sharp`, `lanczos`, `lanczos_sharp`
, `lanczos_2`, `lanczos_2_sharp` , `cubic`, `cubic_sharp`, `catmull_rom`, `mitchell`, 
`cubic_b_spline`, `hermite`, `jinc`, `triangle`, `linear`, `box`, `fastest`, `n_cubic`, `n_cubic_sharp`  
* `up.filter` determines the up-sampling filter to use. See `down.filter`
* `down.colorspace=srgb` downscales in the srgb color space instead of linear RGB. Mimics widespread but bad behavior; destroys image highlights. 
* `s.grayscale`=`true|y|ry|ntsc|bt709|flat` transforms the image into grayscale using various methods.
* `s.sepia=true` turns the image into sepia tone
* `s.invert=true` inverts the image colors in the srgb space
* `s.alpha=0..1` makes the image partially transparent
* `s.contrast=-1..1` adjusts the contrast in the srgb space
* `s.brightness=-1..1` adjusts brightness in the srgb space
* `s.saturation=-1..1` adjusts saturation in the srgb space
