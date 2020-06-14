# Color Filter sRGB

This command is not ideal as it operates in the sRGB space. For alpha operations it doesn't matter, and for grayscale conversion it matches various international standards. 


```json
{ 
  "color_filter_srgb": "grayscale_ntsc"
}
```
```json
{ 
  "color_filter_srgb": "grayscale_flat"
}
```
```json
{ 
  "color_filter_srgb": "grayscale_bt709"
}
```
```json
{ 
  "color_filter_srgb": "grayscale_ry"
}
```

```json
{ 
  "color_filter_srgb": "sepia"
}
```

```json
{ 
  "color_filter_srgb": "invert"
}
```

* `alpha: 0..1`

```json
{ 
  "color_filter_srgb": {"alpha":  0.5}
}
```
* `contrast: -1..1`
```json
{ 
  "color_filter_srgb": {"contrast":  0.5}
}
```
* `brightness: -1..1`
```json
{ 
  "color_filter_srgb": {"brightness":  0.5}
}
```
* `saturation: -1...1`
```json
{ 
  "color_filter_srgb": {"saturation":  0.5}
}
```
