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