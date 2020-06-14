## Querystring Examples

## TODO

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
