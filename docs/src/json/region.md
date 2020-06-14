# Region Command

Region is like a crop command, but you can specify coordinates outside of the image and thereby add padding. 
It's like a window.

You can specify a region as a percentage of the image's width and height:
```json
{
    "region_percent": {
        "x1": -1.0,
        "y1": -1.0,
        "x2": 101.0,
        "y2": 101.0,
        "background_color": "transparent"
    }
}
```

Or you can specify a pixel region 
```json
{
    "region": {
        "x1": -1,
        "y1": -1,
        "x2": 800,
        "y2": 800,
        "background_color": "transparent"
    }
}
```
