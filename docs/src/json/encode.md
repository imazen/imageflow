# Encode Command

When encoding, you'll need the `io_id` of the file you're encoding to, and a encoding preset. 

```json
{
    "encode": {
        "io_id": 1,
        "preset": "gif"
    }
}
```
### Jpeg (MozJpeg encoder)

* `quality: 0..100` controls the image quality. Consider 80 as a good starting point. 
* `progressive: true` enables progressive jpeg encoding, which takes more CPU time.  
```json
{
    "mozjpeg": {
      "quality": 90,
      "progressive": false
    }
}
```
### Gif

```json
"gif"
```

### Lossless PNG

```json
{
  "lodepng": {
    "maximum_deflate": false
  }
}
```

### Lossy PNG

* `quality: 0..100` specifies the target quality to aim for. 
* `minimum_quality: 0.100` specifies the actual quality below which to switch to lossless PNG. 
* `speed: 1..10` controls the speed/quality tradeoff for encoding. 
* `maximum_deflate: true` gains 1-2% in file size reduction at the expense of a tenfold increase in CPU time. 

```json
{
    "pngquant": {
      "quality": 90,
      "minimum_quality": 20,
      "speed": null,
      "maximum_deflate": null
    }
}
```

### WebP (Lossy)

* `quality: 0..100` determines the encoding quality. 80 is a good starting point. 
```json
{
"webplossy": {
  "quality": 80
}
}
```
### WebP (Lossless)

```json
"webplossless"
```

## Deprecated Presets

```json
{
  "libjpegturbo": {
    "quality": 90,
    "progressive": false,
    "optimize_huffman_coding": true
  }
}
```
```json
{
    "libpng": {
      "depth": "png_24",
      "matte": {
        "srgb": {
          "hex": "9922FF"
        }
      },
      "zlib_compression": 7
    }
}
```
