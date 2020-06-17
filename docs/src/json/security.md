# Security

If you're getting things like source images, command strings or width/height values from untrusted sources, 
it's important to place limits on image sizes to prevent denial-of-service attacks. 

JSON jobs have a `security` key that can be filled out like this:

Note that `max_frame_size` also limits the maximum decode and encode size, 
so you don't have to specify `max_decode_size` and `max_encode_size` unless they are smaller.

```json
{
"security": {
    "max_decode_size": {
        "w": 10000,
        "h": 10000,
        "megapixels": 50
    },
    "max_frame_size": {
        "w": 10000,
        "h": 10000,
        "megapixels": 100
    },
    "max_encode_size":  {
        "w": 8000,
        "h": 8000,
        "megapixels": 20
    }
}
}
```