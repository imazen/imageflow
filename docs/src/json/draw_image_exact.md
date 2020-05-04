# Draw Image Exact Command

The following node will compose the `input` image with the top-left 100x100 square on the 
`canvas`, distorting it if the aspect ratio is different. 15% sharpening will be applied. 

This node can only be used with `graph`.

```json
{
  "draw_image_exact": {
    "x": 0,
    "y": 0,
    "w": 100,
    "h": 100,
    "blend": "compose",
    "hints": {
      "sharpen_percent": 15
    }
  }
}
