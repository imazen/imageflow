
## Image Transform Commands

* `width` constrains the image width. `w` is an alias for `width`
* `height` constrains the image height. `h` is an alias for `height`
* `dpr` is a multiplier for `width`/`height` to make responsive image usage easier. 
* `mode` determines how to handle aspect ratio differences.
    * `stretch` distorts the image to be exactly the given dimensions, if `scale=both`. If `scale=down` (the default), the image is only scaled if `width` and `height` are smaller than the image. 
    * `pad` scales the image to fit within `width` and `height`, then pads 2 edges (`bgcolor`) to make it.
    * `crop` scales the image to fit above `width` and `height`, then minimally crops to meet aspect ratio. 
    * `max` scales the image to fit within `width` and `height`
* `scale` controls whether images are upsampled or not.
    *  `down` - Never upscale an image - return at original size instead
    *  `both` - Downscale or upscale to meet size requirements. Image enlargement causes blurriness and should be avoided.
    *  `canvas` - Add padding instead of upscaling to meet size requirements.
    *  `up` - Never downscale, only upscale to meet requirements. Rarely used. 
* `anchor` determines how the image is aligned when you use `mode=crop`, `mode=pad` or `scale=canvas`. 
The default is `middlecenter`
    * values are `topleft`, `topcenter`, `topright`, `middleleft`, `middlecenter`, `middleright`, 
    `bottomleft`, `bottomcenter`, and `bottomright`.
* `sflip` flips the source image in the `x`, `y`, or `xy` dimensions. 
* `flip` flips the result image in the `x`, `y`, or `xy` dimensions. 
* `srotate` rotates the source image `90`, `180`, or `270` degrees. 
* `rotate` rotates the result image `90`, `180`, or `270` degrees. 
* `crop=x1,y1,x2,y2` crops the source image to the given coordinates. If x2 or y2 are negative, they are relative to 
the bottom-right corner of the image. `crop=10,10,-10,-10` removes 10 pixels from the edge of the image. 
* `cropxunits=100&cropyunits=100` makes the `crop` coordinates percentages of the image instead of pixels. 
* `trim.threshold=80` specifies a threshold to use for trimming whitespace.
* `trim.percentpadding=0.5` specifies percentage of padding to restore after trimming.
* `bgcolor` must be in the form RGB, RGBA, RRGGBBAA, RRGGBB, or be a named color.
`bgcolor` determines the color of padding added with `mode=pad` or `scale=canvas`. 