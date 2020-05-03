# RIAPI (querystring) Reference


# Basic commands

<style type="text/css">
.lineup img {vertical-align:top;}
</style>

## Width & Height

You can set bounds for an image with the `width` and/or `height` commands. How those bounds are interpreted is determined by the `mode` and `scale` commands. If only `width` *or* `height` is specified, aspect ratio is maintained.

### Fit Mode

* `stretch` distorts the image to be exactly the given dimensions, if `scale=both`. If `scale=down` (the default), the image is only scaled if `width` and `height` are smaller than the image. 
* `pad` scales the image to fit within `width` and `height`, then pads 2 edges (`bgcolor`) to make it 


## Scale=down|both|canvas

**By default, images are *not* enlarged** - the image stays its original size if you request a larger size.


To allow both reduction and enlargement, use `scale=both`. Image enlargement causes blurriness and should be avoided. `scale=canvas` is another option. The image never gets upscaled, but the canvas is expanded to fill the desired area.

Here we attempt to upscale an image using `scale=down`, `scale=both`, and `scale=canvas` respectively.

<img src="https://z.zr.io/ri/tractor-tiny.jpg;width=150;scale=down" style="border: 1px solid gray" />
<img src="https://z.zr.io/ri/tractor-tiny.jpg;width=150;scale=both" style="border: 1px solid gray"  />
<img src="https://z.zr.io/ri/tractor-tiny.jpg;width=150;scale=canvas" style="border: 1px solid gray"  />

You can [change the default behavior from `scale=down` to something else with the DefaultSettings plugin](/plugins/defaultsettings).

## Alignment

So, you don't like images being centered when you use `mode=crop`, `mode=pad`, or `scale=canvas`? You can pick the alignment type with the `anchor` command. 

Valid values are `topleft`, `topcenter`, `topright`, `middleleft`, `middlecenter`, `middleright`, `bottomleft`, `bottomcenter`, and `bottomright`.

Mode=Crop, Anchor=Topleft: ![](https://z.zr.io/ri/zermatt.jpg;w=100;h=100;mode=crop;anchor=topleft)
Anchor=bottomright: ![](https://z.zr.io/ri/zermatt.jpg;w=100;h=100;mode=crop;anchor=bottomright)

Mode=Pad, Bgcolor=gray, Anchor=Topleft: ![](https://z.zr.io/ri/zermatt.jpg;w=100;h=100;bgcolor=gray;anchor=topleft)
 Anchor=bottomright: ![](https://z.zr.io/ri/zermatt.jpg;w=100;h=100;bgcolor=gray;anchor=bottomright)

Scale=canvas, bgcolor=gray, Anchor=Topleft: ![](https://z.zr.io/ri/tractor-tiny.jpg;w=150;bgcolor=gray;scale=canvas;anchor=topleft)

## Formats & compression

Set `format=jpg`, `format=gif`, or `format=png` to force a particular output format. By default, the original format (or the closest match) is used; however, you can convert *any* format file to *any* other file, maintaining transparency, IF the [PrettyGifs plugin](/plugins/prettygifs) is installed.

Adjust jpeg compression with the `quality=0..100` command. The default is 90, an excellent tradeoff between size and perfection. 



## Background color

Dislike white? Transparent padding is added (when required) for PNGs and GIFs, but jpegs don't support transparency.

Add **bgcolor=name** or **bgcolor=33ddff** to set the background (matte) color. Named colors and hex values supported.

<img src="https://z.zr.io/ri/quality-original.jpg;w=100;h=100;bgcolor=33ddff" />

## Cropping 

The URL syntax for cropping is `&crop=x1,y1,x2,y2`. The coordinates are relative to the top-left corner of the original image - if they are positive values.

If X2 or Y2 are 0 or less, they are relative to the bottom-right corner. This allows easy trimming without knowing the size of the image.

For example, crop=0,0,0,0 leaves the image uncropped. crop=10,10,-10,-10 removes 10 pixels from all edges of the image.

In addition, you can specify `cropxunits` and `cropyunits`. Setting them to 100 allows you to crop by percentage. Example which crops 10% off each edge: `?cropxunits=100&cropyunits=100&crop=10,10,90,90`. Setting them to the width/height of the display image allows you to crop in display coordinates, without needing to know the original size of the image.

## Back to cooler stuff...

The following filters require the [SimpleFilters plugin](/plugins/simplefilters), part of the Creative edition.

![Original image](https://z.zr.io/ri/utah2.jpg;width=200)


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

# Full command reference

Full Command Reference

Rotation & flipping
autorotate=true Automatically rotates the image based on the EXIF info from the camera. autorotate.default=true will only autorotate if the image is processed.
sflip=none|x|y|xy Flips the source image prior to processing (new in V3.1).
srotate=0|90|180|270 Rotates the source image prior to processing (only 90 degree intervals) (new in V3.1).
rotate=degrees – Rotates the image any arbitrary angle (occurs after cropping).
flip=none|x|y|xy - Flips the image after everything is done.
Manual cropping
crop=(x1,y1,x2,y2) – Crop the image to the specified rectangle on the source image. You can use negative coordinates to specify bottom-right relative locations.
cropxunits The width which the x1 and x2 coordinates are relative to, e.g., use '100' to make x1 and x2 percentages. Useful when you don't know the original image size.
cropyunits The height which the y1 and y2 coordinates are relative to, e.g., use '100' to make y1 and y2 percentages. Useful when you don't know the original image size.
Sizing (and padding, autocropping, carving and stretching)
Please note that width/height/maxwidth/maxheight do NOT include border, margin, or padding widths, and do not include the extra space used by rotation. They constrain the image, not the canvas.

maxwidth, maxheight – Fit the image within the specified bounds, preserving aspect ratio.
width, height – Force the final width and/or height to certain dimensions. Whitespace will be added if the aspect ratio is different.
mode=max|pad|crop|carve|stretch - How to handle aspect-ratio conflicts between the image and width+height. 'pad' adds whitespace, 'crop' crops minimally, 'carve' uses seam carving, 'stretch' loses aspect-ratio, stretching the image. 'max' behaves like maxwidth/maxheight (new in V3.1).
anchor=topleft|topcenter|topright|middleleft|middlecenter|middleright|bottomleft|bottomcenter|bottomright How to anchor the image when padding or cropping (new in V3.1).
scale=both|upscaleonly|downscaleonly|upscalecanvas – By default, images are never upscaled. Use &scale=both to upscale images if they are smaller than width and height.
zoom=0..infinity - Scale the image by a multiplier. Useful for mobile devices and situations where you need to retain all the existing width/height/crop settings, but scale the result up or down. Defaults to 1. 0.5 produces a half-size image, 2 produces a double-size image.
Border, padding, margins and background colors
bgcolor=color name | hex code (6-char). Sets the background/whitespace color.
margin=3 or margin=5,5,10,10 Specify a universal margin or left,top,right,bottom widths (new in V3.1.
Output format
format=jpg|png|gif - The output format to use.
quality - Jpeg compression: 0-100 100=best, 90=very good balance, 0=ugly.

Watermark plugin
watermark - The name of one or more watermark layers (or layer groups) to render.

SimpleFilters plugin
&s.grayscale=true|y|ry|ntsc|bt709|flat (true, ntsc, and y produce identical results)
&s.sepia=true
&s.alpha= 0..1
&s.brightness=-1..1
&s.contrast=-1..1
&s.saturation=-1..1
&s.invert=true

trim.threshold=80 - The threshold to use for trimming whitespace.
trim.percentpadding=0.5 - The percentage of padding to restore after trimming.
