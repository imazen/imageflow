# JSON API

JSON is primarily used by libimageflow and imageflow_tool. 

You can specify a series of `steps` to take (the easiest), or you can specify a [graph](graph.md) with 
nodes and edges (which allows for multiple inputs and outputs).  Note that you can watermark with a series of steps. 

JSON jobs have the keys `io` and `framewise`, which refer to your inputs/outputs and steps/graph to apply to each image frame. 

JSON jobs also have a `security` key that you can read more about [here](security.md).

If you're using `imageflow_tool v1/build`, you'll need to specify your inputs and outputs. This isn't needed if you're using `libimageflow` and `v1/execute`, as you'll have already registered the inputs and outputs.  

The following example uses `steps` to constrain an image to 1400px or less and encodes it in 8-bit png. 
```json
{
  "io": [
    {
      "io_id": 0,
      "direction": "in",
      "io": "placeholder"
    },
    {
      "io_id": 1,
      "direction": "out",
      "io": "placeholder"
    }
  ],
  "security": {
    "max_decode_size": {
      "w": 10000,
      "h": 10000,
      "megapixels": 100
    },
    "max_frame_size": null,
    "max_encode_size": null
  },
  "framewise": {
    "steps": [
      {
        "decode": {
          "io_id": 0
        }
      },
      {
        "constrain": {
          "mode": "within",
          "w": 1400
        }
      },
      {
        "encode": {
          "io_id": 1,
          "preset": {
            "pngquant": {
              "quality": 80
            }
          }
        }
      }
    ]
  }
}
```