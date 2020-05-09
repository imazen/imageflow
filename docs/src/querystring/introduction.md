# Querystring API

*Also called RIAPI (RESTful Image API)*

This API doesn't care about the order in which you specify commands; they're executed in a standard order regardless.

`trim whitespace` -> `srotate` -> `sflip` -> `crop` -> `scale` -> `filter` -> `pad` -> `rotate` -> `flip`

### Executing with imageflow_tool


```bash
imageflow_tool v1/querystring --in a.jpg --out b.jpg --command "w=100&h=100&mode=max" --quiet
```

### URLs with demo server

http://localhost:39876/demo_images/tulip-leaf.jpg?w=300&h=300&mode=max

## Common examples

* `width=100&height=100&mode=max&scale=down` ensures the image is downscaled to 100x100 or less,
but does not upscale the image if it is already smaller than that. Aspect ratio is maintained. 
* `width=200&height=200&mode=max&scale=both` ensures the image is downscaled or upscaled to fit within 200x200, 
maintaining aspect ratio. 
* `width=200&height=200&mode=pad&scale=both` ensures the image is downscaled or upscaled to fit within 200x200, 
  maintaining aspect ratio, then is padded to make the result always 200x200.
* `width=300&height=300&mode=crop&scale=both` ensures the image is downscaled or upscaled to fit around 300x300,
then minimally cropped to meet the aspect ratio. `scale=both` ensures the image is upscaled if smaller so the result 
is always 300x300. 





