# Create Canvas Command.

The following node creates a 200x200 transparent canvas. 
```json
{
  "create_canvas": {
    "format": "bgra_32",
    "w": 200,
    "h": 200,
    "color": "transparent"
  }
}
```

The following node creates a 200x200 black canvas. Transparency operations will not work as the canvas doesn't support an alpha channel.  
```json
{
  "create_canvas": {
    "format": "bgr_32",
    "w": 200,
    "h": 200,
    "color": "black"
  }
}
```
