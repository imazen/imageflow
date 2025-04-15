## RFC: Quality profiles (&qp=0..100|medium|high|etc), &format=auto, &format=lossy, &format=lossless

@jeremy-farrance @iJungleboy @HRasch @wilz05 Could I get a quick sanity check on these new commands I want to add to Imageflow this week? 

*Sorry for the delay, I've been rewriting Imageflow Server to run on Lambda and Azure Functions etc to cut infrastructure costs, be cloud native with multiple cache layers and replication, and it's been a **lot**. And making configuration easy across all platforms has been a wrestle, I wrote a DSL for routing that should be intuitive and capable enough, but C# is not as easy as Rust for parsers.*

## The suggested features

*note: avif and jxl are not yet supported, so these features will fall back to supported formats in the meantime*

```
qp=lowest|low|medium|good|high|highest|lossless - Tunings for every codec at specific perceptual quality levels.
qp=quality - Pulls numeric value from `&quality=`, if present. Users can set this as a site default to improve usability, and also set &amp;quality as a site default.
qp=0..100 - Algorithmic mapping for each codec that tries to be a perceptually smooth quality/size curve that is consistent across all codecs.
qp-dpr=1/1.5/2/3/decimal - Tweaks the qp level to account for different physical screen ppi values. 150ppi is a good modern assumption, and 3 dppx is the default. Lower values will increase quality and higher values will decrease quality.
qp-dppx as an alias for qp-dpr (new, tell me if a bad idea, I'm trying to add CSS nomenclature)
qp-dpr=dpr - pulls numeric value from &amp;dpr= or &amp;zoom=. Useful as a site default.
format=keep - will represent current format=null behavior, just explicitly, so that users can set format=auto as a default, yet override for specific images`
format=auto - will upgrade the format based on &amp;accept.webp=1 and &amp;accept.avif=true and &amp;accept.jxl=true. Suggested site default.
lossless=keep|true|false(default) - Lets users enforce lossless encoding regardless of output format. False: will force lossy encoding even if the source format is 24-bit PNG or lossless webp. Alpha will be preserved (webp/png8). True: will force lossless encoding via webp, jxl, or png. 
accept.webp=1 - will allow webp encoding when format=auto|lossy|lossless is used. &format=keep will always produce webp if the source format is webp.
accept.avif=1 - will allow avif encoding when format=auto|lossy is used. &format=keep will always produce avif if the source format is avif.
accept.jxl=1 - will allow jxl encoding when format=auto|lossy|lossless is used. &format=keep will always produce jxl if the source format is jxl.
accept.custom_color_profiles=1 - will allow custom color profiles rather than just srgb (not implemented yet)
jpeg.quality=0..100 - Gives a special knob that is jpeg-specific, to mirror webp.quality, png.quality, jxl.quality, and avif.quality.
```

`dppx as a new alias for dpr which is an alias for zoom (new, tell me if a bad idea, I'm trying to add CSS nomenclature) currently just multiplies `w/h` with the intent of simplifying user calculations for srcset or picture/img where you want to increase intrinsic pixels relative to CSS pixels. (see my new site [srcset.tips](https://srcset.tips)).

Note: a future `encoding.speedlimit` mode will be needed to limit avif encoding for larger images since avif can take 20x longer, and we probably don't want that running for real-time. Practically, servers will need to be able to check the cache for a version without the speedlimit first, and run a replacement task in some kind of background queue during idle periods to progressively improve compression. Aaand... that's a massive can of worms for future me. Which might be why I've been dragging my feet on avif. Also, I still think *decoding* avif add security risk and probably will make it an encode-only format.

## Automatic format selection goals

1. We want to preserve animation, above all
2. We want to preserve alpha
3. We want to respect losslessness, but this may conflict with animation, if webp animation is disabled or not implemented we use gif.
4. We want to provide the best quality/size tradeoff for the requested quality level or preset.

## Automatic format selection algorithm

1. If animation is needed, use  webp or gif. Browsers don't support animated jxl yet, and avif animation support is mixed and CPU intensive. If webp is not supported, use gif even if lossless is requested.
2. If alpha is needed, we exclude jpeg.
3. For losslessness, we choose jxl, webp, and png in that order.
4. If JXL is available and accept.jxl=1, use it. It's better than all other formats for all use cases.
5. The best size/quality tradeoff between jpegli, avif, and webp depends on the specific image (graphics or photo, alpha, bit depth). jpegli is very good. 
.

## Side note on cropping / focal point

I'm planning to add some crop and focal point commands too, the ones that we seem to have consensus on:
1. `c=x1-percent,y1-percent,x2-percent,y2-percent` to allow users to specify a crop area as a percentage of the width and height.
2. `c.gravity=x-percent,y-percent` to allow users to specify a center point for cropping to meet aspect ratio

The interactions of multiple crops, faces, etc become a lot more complex and the spreadsheet needs to be updated. I want AI upscaling as a future feature and to not hamstring that.

## Usage

```html
<!-- note the ratio of image pixels to CSS pixels is 600 to 300, 
 so the user lets the server know via qp-dpr=2, and it raises the 
 quality a bit since on average that means fewer pixels than device pixels -->
<img width=300 src="img.jpg?w=600&qp=low&qp-dpr=2" />`
```

1. Without adjusting the site defaults, no behavior will change. But a default of `&qp=quality&qp-dpr=dpr&format=auto` will improve usability.
2. A new Imageflow Server setting will be provided to set `accept.webp=1` and `accept.avif=true` and `accept.jxl=true` based on the presence of `image/webp`, `image/avif`, or `image/jxl` in the HTTP `Accept` header. You can do this now with a rewrite event handler in both Imageflow and ImageResizer.
3. We add documentation on how to configure a CDN to do the same thing via rules, since otherwise it will deliver the wrong formats to the wrong clients. Most CDNs do not vary the cache key based on the `Accept` header by default.



## The problems

1. `&quality=0..100 is not useful across codecs, because the perceptual quality varies widely depending on codec - especially between winjpeg (ImageResizer 4), mozjpeg (Imageflow 2), and jpegli (Imageflow vNext), webp, jxl, png, and gif. Furthermore, quality/size tradeoff is never a curve, but a spiky shape.
2. User analyis paralysis: Nobody has the patience to test thousands of permutations of codec tuning parameters - and arguably the ~20 or so that are available are insufficient and poorly documented. The defaults are not useful across different use cases.
3. This gets harder to reason about with srcset/sizes. The average dppx - CSS-pixel-to-intrinsic-pixel-ratio is 3, at 150dpi physically. At 150dpi, you want to target a low or medium quality. But if you're using vanilla &ltimg.. and providing fewer pixels than device pixels, you want higher quality. We can offer `&qp-dpr= to compensate. With enough algorithmic testing, we can be precise on this eventually.
3. Picking the best format is tricky, and depends on the source image and how it has been transformed. Imageflow could handle that complexity for the user. 



## Two common sets of site-wide defaults I would expect:

1. New sites: `&format=auto&qp=quality&qp-dpr=dpr&f.sharpen=23&down.filter=mitchell&ignore_icc_errors=true` - This would reinterpret &quality values (`&qp=quality`) to be uniform and automatically upgrade which image formats are produced (`&format=auto`).
2. Legacy: `&f.sharpen=23&down.filter=mitchell&ignore_icc_errors=true&down.colorspace=srgb` - Existing urls would evaluate the same. 

I think a bit of magic could/should happen if `&qp=` (or `&lossless`) is provided and there is no &format= command - I think &format=auto should be implied. It could be that simple, or we could do API versioning (&api=1|2) with implicit choice based on which commands are used? 


## Sharpening

Sharpening is the top factor in perceived quality, so I think quality profiles might want to influence it. 

Browsers apply sharpening when resizing images down, but tend to make them blurry when upscaling (which is most of the time). 
Sharpening an image can increase file size 20-35% easily, but is quite helpful when there are fewer image pixels than device pixels (this is almost always the case).

I think sharpness should probably go up when qp-dpr is low, and vice versa, but I'll need to run tests. 

I could see influencing sharpening being too magical, though, even if sharpness is 30% of image bytes and a chunk of appeal factor.

## Questions

1. Is qp-dpr too magical?
2. Should we drop format=lossy|lossless and only have &lossless=keep|true|false? Should we expect people to remember to set &format=auto&lossless=true? Would it be more error prone? (&format=keep(default) on a jpeg would ignore &lossless=true.) 
3. Should we magically infer &format=auto if &lossless or &qp is used?
4. Should quality profiles influence sharpening level? 
5. Is this too complicated?