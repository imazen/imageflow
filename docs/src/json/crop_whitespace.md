# Crop Whitespace Command

* `threshold: 1..255` determines how much noise/edges to tolerate before cropping is finalized. `80` is a good starting point.
* `percent_padding` determines how much of the image to restore after cropping to provide some padding. `0.5` (half a percent) is a good starting point. 
```json
{
  "crop_whitespace": {
    "threshold": 80,
    "percent_padding" : 2
  }
}
```
