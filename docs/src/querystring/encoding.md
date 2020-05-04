## Image Encoding Commands

* `format=png|gif|jpeg|webp` determines the format to encode the image as. Defaults to the original format.
* `quality=0..100` determines the jpeg encoding quality. Default is `90`
* `jpeg.progressive=true` enables progressive jpeg encoding. 
* `jpeg.turbo=true` encodes files faster at the expense of file size. 
* `webp.quality=0..100` determines the webp encoding quality.
* `webp.lossless=true` enables lossless webp encoding. 
* `png.quality=0..100` determines the png quality. If absent lossless is used. 
* `png.min_quality=0..100` determines the minimum png quality that must be realized before lossless is used.

