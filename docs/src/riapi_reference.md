# Querystring API

*Also called RIAPI (RESTful Image API)*

This API doesn't care about the order in which you specify commands; they're executed in a standard order regardless.

`srotate` -> `sflip` -> `crop` -> `scale` -> `filter` -> `pad` -> `rotate` -> `flip`

### Executing with imageflow_tool


```bash
imageflow_tool v0.1/ir4 --in a.jpg --out b.jpg --command "w=100&h=100&mode=max" --quiet
```

### URLs with demo server

http://localhost:39876/demo_images/tulip-leaf.jpg?w=300&h=300&mode=max

## Common examples

* `width=100&height=100&mode=max&scale=down` ensures the image is downscaled to 100x100 or less,
but does not upscale the image if it is already smaller than that. Aspect ratio is maintained. 
* `width=200&height=200&mode=max&scale=both` ensures the image is downscaled or upscaled to fit within 200x200, 
maintaining aspect ratio. 
* `width=200&height=200&mode=pad&scale=both` ensures the image is downscaled or upscaled to fit within 200x200, 
  maintaining aspect ratio, then is padded to make the result always 200x200.
* `width=300&height=300&mode=crop&scale=both` ensures the image is downscaled or upscaled to fit around 300x300,
then minimally cropped to meet the aspect ratio. `scale=both` ensures the image is upscaled if smaller so the result 
is always 300x300. 



## Commands - image transforms

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
* `bgcolor` must be in the form RGB, RGBA, RRGGBBAA, RRGGBB, or be a named color.
`bgcolor` determines the color of padding added with `mode=pad` or `scale=canvas`. 
* `trim.threshold=80` specifies a threshold to use for trimming whitespace.
* `trim.percentpadding=0.5` specifies percentage of padding to restore after trimming.


## Commands - image filters

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

## Commands - image encoding

* `format=png|gif|jpeg|webp` determines the format to encode the image as. Defaults to the original format.
* `quality=0..100` determines the jpeg encoding quality. Default is `90`
* `jpeg.progressive=true` enables progressive jpeg encoding. 
* `jpeg.turbo=true` encodes files faster at the expense of file size. 
* `webp.quality=0..100` determines the webp encoding quality.
* `webp.lossless=true` enables lossless webp encoding. 
* `png.quality=0..100` determines the png quality. If absent lossless is used. 
* `png.min_quality=0..100` determines the minimum png quality that must be realized before lossless is used.


## Examples

### 4 ways to grayscale

`s.grayscale`=`true|y|ry|ntsc|bt709|flat`  (true, ntsc, and y produce identical results)

The following examples use NTSC/Y/True, RY, BT709, and Flat respectively

![s.grayscale=true](https://z.zr.io/ri/utah2.jpg;width=200;s.grayscale=true)
![s.grayscale=ry](https://z.zr.io/ri/utah2.jpg;width=200;s.grayscale=ry)
![s.grayscale=bt709](https://z.zr.io/ri/utah2.jpg;width=200;s.grayscale=bt709)
![s.grayscale=flat](https://z.zr.io/ri/utah2.jpg;width=200;s.grayscale=flat)

### 1 way to sepia

![s.sepia=true](https://z.zr.io/ri/utah2.jpg;width=200;s.sepia=true)

### Inversion

![s.invert=true](https://z.zr.io/ri/utah2.jpg;width=200;s.invert=true)


### Adjust opacity/alpha

`s.alpha`= `0..1`

For true transparency, combine with `format=png`. Otherwise, the image will be blended against `bgcolor`.

![s.alpha=0.25](https://z.zr.io/ri/utah.jpg;width=200;s.alpha=0.25)
![s.alpha=0.75](https://z.zr.io/ri/utah.jpg;width=200;s.alpha=0.75)
![s.alpha=0.85](https://z.zr.io/ri/utah.jpg;width=200;s.alpha=0.85)
![s.alpha=1](https://z.zr.io/ri/utah.jpg;width=200;s.alpha=1)

### Adjust contrast


`s.contrast`= `-1..1`


![s.contrast=-0.80](https://z.zr.io/ri/utah.jpg;width=200;s.contrast=-0.99)
![s.contrast=-0.80](https://z.zr.io/ri/utah.jpg;width=200;s.contrast=-0.80)
![s.contrast=-0.40](https://z.zr.io/ri/utah.jpg;width=200;s.contrast=-0.40)
![s.contrast=-0.20](https://z.zr.io/ri/utah.jpg;width=200;s.contrast=-0.20)

![s.contrast=0](https://z.zr.io/ri/utah.jpg;width=200;s.contrast=0)


![s.contrast=0.20](https://z.zr.io/ri/utah.jpg;width=200;s.contrast=0.20)
![s.contrast=0.40](https://z.zr.io/ri/utah.jpg;width=200;s.contrast=0.40)
![s.contrast=0.80](https://z.zr.io/ri/utah.jpg;width=200;s.contrast=0.80)
![s.contrast=0.99](https://z.zr.io/ri/utah.jpg;width=200;s.contrast=0.99)



### Adjust brightness


`s.brightness`= `-1..1`


![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.brightness=-1)
![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.brightness=-0.7)
![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.brightness=-0.5)
![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.brightness=-0.2)
![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.brightness=0)
![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.brightness=0.2)
![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.brightness=0.5)
![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.brightness=0.7)
![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.brightness=1)

### Adjust saturation


`s.saturation`= `-1..1`



![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.saturation=-1)
![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.saturation=-0.9)
![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.saturation=-0.5)
![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.saturation=-0.2)
![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.saturation=0)
![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.saturation=0.2)
![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.saturation=0.5)
![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.saturation=0.9)
![](https://z.zr.io/ri/red-leaf.jpg;width=100;s.saturation=1)
