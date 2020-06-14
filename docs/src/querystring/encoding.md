## Image Encoding Commands

* `format=png|gif|jpeg|webp` determines the format to encode the image as. Defaults to the original format.
* `jpeg.quality=0..100` determines the jpeg encoding quality. Default is `90`
* `jpeg.progressive=true` enables progressive jpeg encoding. 
* `jpeg.turbo=true` encodes files faster at the expense of file size. 
* `webp.quality=0..100` determines the webp encoding quality.
* `webp.lossless=true` enables lossless webp encoding. Default `false`
* `png.lossless=false` disables lossless PNG encoding. Default `true` unless `png.quality` is specified. 
* `png.quality=0..100` determines lossy png quality. Default `100`. 
* `png.min_quality=0..100` determines the minimum png quality that must be realized before lossless is used. Default `0`

