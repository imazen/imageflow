# Querystring API

*Also called RIAPI (RESTful Image API)*

This API doesn't care about the order in which you specify commands; they're executed in a standard order regardless.

`trim whitespace` -> `srotate` -> `sflip` -> `crop` -> `scale` -> `filter` -> `pad` -> `rotate` -> `flip`

### Executing with imageflow_tool


```bash
imageflow_tool v1/querystring --in a.jpg --out b.jpg --command "w=100&h=100&mode=max" --quiet
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

## srcset syntax

Our new, more compact comma-delimited syntax. It lets you use the familiar srcset width and density descriptors such as  `80w`, `70h`, `2.5x`.

### Notes: 

* srcset commands are comma delimited, and use `-` to separate command parameters. Ex `srcset=jpeg-100,sharp-20`
* You can also combine them, so you don't have to do math. Ex. `&srcset=100w,2x` translates to `&w=100&zoom=2` which translates to 200px wide.
* Since CSS pixels map to multiple device pixels, this reduces an error-prone task.
* The default mode is `max`, so you don't need to specify it when using both width and height.
* Quality values do not translate across encoders, a fact that is lost on many people. In this syntax, we combine the format and quality value.
* `srcset` commands expand, internally, to `&w=100&h=100&mode=max&format=[value]&zoom=[density]&quality=[x]&webp.quality=[y]&png.quality=[z]&scale=[both|down]&f.sharpen=[pct]` etc


### List of srcset values and what they affect

* `jpeg-100` - JPEG, 100% quality
* `jpeg` - JPEG, default quality configured in the server
* `png-100` - PNG, 100% quality
* `webp-100` - WebP, 100% quality
* `webp` - WebP, default quality configured in the server
* `webp-lossless` or `webp-l` - WebP, lossless
* `png-lossless` or `png-l` or `png`  - PNG, lossless
* `gif` - GIF
* `2.5x` - 2.5x density/multiplier applied to width and height
* `100w` - 100px wide (times the density multiplier, if specified)
* `100h` - 100px tall
* `fit-max` - (default) don't change the image's intrinsic aspect ratio, just constrain it within the width/height box if both are specified/
* `fit-crop` - crop to meet aspect ratio
* `fit-pad` - pad to meet aspect ratio
* `fit-distort` - distort to meet aspect ratio
* `upscale` - by default, images are only downscaled. This can cause unexpected results when using `crop` or `pad`. This option allows upscaling when needed.
* `crop-10-20-80-90` - crop to rectangle 10%,20%,80%,90% of the source image
* `crop-10-20-80-90,100w,100h,webp-90` - crop to rectangle 10%,20%,80%,90% of the source image, scale to 100px wide, 100px tall, WebP, 90% quality
* `sharpen-20` - sharpen by 20%
* `sharp-20` - sharpen by 20%

#### Examples

* `&srcset=webp-70,sharp-15,100w` - WebP, 70% quality, 15% sharpening, 100px wide
* `&srcset=jpeg-80,2x,100w,sharpen-20` - JPEG, 80% quality, 2x density, 200px wide, 20% sharpening
* `&srcset=png-90,2.5x,100w,100h,crop` - PNG, 90% quality, 250px wide, 250px tall, cropped to aspect ratio
* `&srcset=png-lossless` - PNG, lossless
* `&srcset=gif,crop-20-30-90-100,2.5x,100w,100h` - GIF, cropped to rectangle 20%,30%,90%,100%, 250px wide, 250px tall
* `&srcset=webp-l,2.5x,100w,100h,crop` - WebP, lossless, cropped to aspect ratio, resized to 250px wide, 250px tall
* `&srcset=webp-lossless,2.5x,100w,100h,upscale` - WebP, lossless, 250x250px, upscale to width & height if original image is smaller.
