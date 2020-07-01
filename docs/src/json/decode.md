# Decode Command

Typically, you only need to specify the `io_id` of the file you're decoding. 
```json
{
    "decode": {
        "io_id": 0
    }
}
```

However, some decoders accept commands that can be used to speed up the process. 

The following causes the JPEG decoder to spatially downscale - in linear light - while decoding.

The image may not be scaled to the exact size requested, but it will be closer.
```json

{
    "decode": {
      "io_id": 0,
      "commands": [
        {
          "jpeg_downscale_hints": {
            "width": 1600,
            "height": 1600,
            "scale_luma_spatially": true,
            "gamma_correct_for_srgb_during_spatial_luma_scaling": true
          }
        }
      ]
    }
}
```

You can also do this for WebP images, although there is no support for linear light scaling:

```json

{
    "decode": {
      "io_id": 0,
      "commands": [
        {
          "webp_decoder_hints": {
            "width": 1600,
            "height": 1600
          }
        }
      ]
    }
}
```

You can force the color profile to be ignored. 
```json

{
    "decode": {
      "io_id": 0,
      "commands": [
        "discard_color_profile"
      ]
    }
}
```

Or just ignore color profile errors. 
```json

{
    "decode": {
      "io_id": 0,
      "commands": [
        "ignore_color_profile_errors"
      ]
    }
}
```
