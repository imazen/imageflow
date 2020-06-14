# Copy Rectangle Command

This node can only be used with `graph`, as it requires both a `canvas` and an `input` node. 

The following node copies (but does not blend/composite) a 100x100 square from the `input` node to `x:100, y:100` on the `canvas` node. 
```json
{
  "copy_rect_to_canvas": {
    "from_x": 0,
    "from_y": 0,
    "w": 100,
    "h": 100,
    "x": 100,
    "y": 100
  }
}
```
