imageflow - Real-time image processing for the web.
=========

Using CLion with imageflow:

    bii init -l=clion imageumbrella
    cd imageumbrella/
    git clone https://github.com/imazen/imageflow blocks/nathanaeljones/imageflow
    git clone https://github.com/nathanaeljones/theft blocks/nathanaeljones/theft
    bii test

======
**The Problem**: Image processing is a ubiquitous requirement. All popular CMSes, many CDNs, and most asset pipelines implement at least image cropping, scaling, and recoding. The need for mobile-friendly websites (and consequently responsive images) makes manual asset creation methods time-prohibitive. Batch asset generation is error-prone, highly latent (affecting UX), and severely restricts web development agility.

Existing [implementations](https://github.com/nathanaeljones/imaging-wiki) lack tests and are either (a) incorrect, and cause visual artifacts or (b) so slow that they've created industry cargo-cult assumptions about "architectural needs"; I.e, *always* use a queue and workers, because we can gzip large files on the fly but not jpeg encode them (which makes no sense from big O standpoint). This creates artificial infrastructure needs for many small/medium websites, and makes it expensive to offer image processing as part of a CDN or optimization layer. **We can eliminate this problem, and make the web faster for all users.**  There is also a high probability that (if back-ported to c89 and BSD licensed), [LibGD](https://github.com/libgd/libgd) will adopt our routines and therefore make them available within the PHP runtime, and the CMSes that build upon it. We have a great chance at reducing the 20MB homepage epidemic.

Image resampling is difficult to do correctly, and hard to do efficiently. Few attempts have been made at both. Our algorithm can [resample a 16MP image in 84ms using just one core](http://imageresizing.net/docs/v4/plugins/fastscaling). On a 16-core server, we can resample *15* such images in 262ms. Modern performance on huge matrices is all about cache-friendliness and memory latency. Compare this to 2+ seconds for FreeImage to do the same operation on 1 image with inferior accuracy. ImageMagick must be compiled in (much slower) HDRI to prevent artifacts, and even with OpenMP enabled, using all cores, is still more than an order of magnitude slower (two orders of magnitude without perf tuning).

In addition, **all the libraries that I've reviewed are insecure**. Some assume all images are trusted data (libvips). Some have complexity and dependencies that are impossible to audit (ImageMagick, FreeImage). Others (which are even more widely deployed) simply have insufficient resources to deal with the vulnerabilities found through Valgrind and Coverity. Very few imaging libraries have any kind of automated tests, which makes Valgrind analysis much less useful.

**The solution**: Create a test-covered library that is safe for use with malicious data, and says NO to any of the following

* Operations that do not have predictable resource (RAM/CPU) consumption.
* Operations that cannot be performed in under 100ms on a 16MP jpeg, on a single i7 core.
* Operations that undermine security in any way.
* Dependencies that have a questionable security track-record. LibTiff, etc.
* Optimizations that cause incorrect results (such as failing to perform color-correction, or scaling in the sRGB space instead of linear). (Or using 8-bit instead of 14-bit per channel when working in linear - this causes egregious truncation/color banding).
* Abstractions that prevent major optimizations (over 30%). Some of the most common (enforced codec agnosticism) can prevent ~3000% reductions in cost.

### Simplifying assumptions

* 32-bit sRGB is our 'common memory format'. To interoperate with other libraries (like Cairo, if users want to do text/vector/svg), we must support endian-specific layout. (BGRA on little-endian, ARGB on big-endian). Endian-agnostic layout may also be required by some libraries; this needs to be confirmed or disproven.
* We use 128-bit floating point (BGRA, linear, premultiplied) for operations that blend pixels. (Linear RGB in 32-bits causes severe truncation).
* The uncompressed 32-bit image can fit in RAM. If it can't, we don't do it. This is for web, and if a server can't allocate enough space for an bitmap, neither can a mobile phone. Also; at least 1 matrix transposition is required for downsampling an image, and this essentially requires it all to be in memory. No paging to disk, ever!
* We support jpeg, gif, and png natively. All other codecs are plugins. We only write sRGB output.

## The components

* [libjpeg-turbo](https://github.com/imazen/libjpeg-turbo) or [mozjpeg](https://github.com/mozilla/mozjpeg)
* [libpng](http://www.libpng.org/pub/png/libpng.html)
* [stb_image](https://github.com/nothings/stb) (May be useful for select functions)
* [LittleCMS](https://github.com/mm2/Little-CMS)
* [ImageResizer - FastScaling](https://github.com/imazen/resizer/tree/develop/Plugins/FastScaling) for optimized, single-pass rendering.
* [ImageResizer](https://github.com/imazen/resizer) (From which we will port most of the domain logic, if not the image decoding/encoding portions)
* OpenCV or CCV for separate plugin to address face-aware auto-cropping.

![ImageFlow diagram](https://rawgit.com/imazen/imageflow/master/docs/ImageFlow_Core.svg)

All of the "hard" problems have been solved individually; we have proven performant implementations to all the expensive parts of image processing.

We also have room for more optimizations - by integrating with the codecs at the block and scan-line level, we can greatly reduce RAM and resource needs when downsampling large images. Libvips has proven that this approach can be incredibly fast.

A generic graph-based representation of an image processing workflow is very tempting. This enables advanced optimizations and potentially lets us pick the fastest or best backend depending upon image format/resolution and desired workflow. Given how easily most operations compose, this could easily make the average workflow 3-8x faster, particularly when we can compose decoding and scaling for certain codecs. The downside to this approach is the complexity of exposing a graph API via C. I would eschew the graph for initial iterations, only introducing it once we had a naive alternative in place already.


## The languages

The pragmatic language choice for the core routines is C14. Rust is extremely attractive, and would make the solution far more secure (there are already safe Rust codecs!). However, given that we often resort to assembly or manual unrolling in C, it may be unrealistic to assume we wouldn't also periodically run into perf issues with the results of the Rust compiler. Long-term, Rust would be the ideal choice, as we get a C ABI, no runtime, yet great safety and concurrency possibilities. However, the development timeline with Rust would be nearly impossible to predict.

Given that there is a large amount of non-perf-critical domain logic required, it may be prudent to use Lua or Go for the mid and high-level APIs, particularly if Rust is not involved.


## API needs.

We should separate our high-level API needs from our low-level primitive needs.

At a high level, users will want (or end up creating) both declarative (result-descriptive) and imperative (ordered operation) APIs. People reason about images in a lot of different ways, and if the tool doesn't match their existing mental pattern, they'll create one that does.

A descriptive API is the most frequently used, and [we drafted RIAPI](https://github.com/riapi/riapi) to standardize the basics.

Among the many shiny advanced features that I've published over the years, a couple have stood out as particularly useful and popular with end-users.

* Whitespace cropping - Apply an energy filter (factoring in all 4 channels!) and then crop off most of the non-energy bounds below a threshold. This saves tremendous time for all e-commerce users.
* Face-aware cropping - Any user profile photo will need to be cropped to multiple aspect ratios, in order to meet native app and constrained space needs. Face detection can be extremely fast (particularly if your scaling algorithm is fast), and this permits the server to make smart choices about where to center the crop (or if padding is required!).

The former (whitespace cropping) doesn't require any dependencies. The latter, face rectangle detection may or may not be easily extracted from OpenCV/ccv; this might involve a dependency. The data set is also several megabytes, so it justifies a separate assembly anyway.

## Key low-level, high-performance primitives

### Color adjustments

* Convert from arbitrary color space and profile to sRGB
* sRGB<->Linear functions, on scan-line sets at a time (Operations that do any blending of pixels need to operate in linear).
* Apply gamma/adjust channels independently
* Color adjustment matrix application

### Image analysis

* Calculate histogram
* Auto white balance
* Fast octree quantizer
* Face detection (cropping heads off selfies to meet an aspect ratio need is uncool). <- tricky to do a compact implementation, data set is heavy.
* Document type detection (photograph, document, line art, etc). This would let us pick the ideal resampling filter and image codec.
* Detect boundaries (sobel filter, edges inward - can be applied locally with tiny alloc req.s)

### Operations requiring a matrix transposition

* Mathematically correct interpolation, with custom interpolation weighting callback. Cached weights mean this callback will be invoked only (w x h x scale factor) number of times.
* Generic convolution kernel applicator, with and without thresholds. Size matters; large kernels will drastically affect performance, and this needs to be clearly documented.
* Rotate 90 degree intervals
* Performance constant blur (3x box blur approximates gaussian)
* Performance constant sharpen

Scale, convolve, rotate 90 degrees, blur, and sharpen - can be all composed and require a single transposition. Separately they would require 7.

### Trivial operations

* Flip
* Crop (doesn't require a copy, just stride & pointer adjustment)
* Create canvas
* Fill rectangle with solid color
* Copy (overwrite)
* Compositing *(ok, not trivial, but easily managed if you lock down color spaces and alpha premultiplication).

You'll note that affine transform/distort is notably absent. Distortion has exponentially bad performance with image size - it's not linear. Large convolution kernels have a similar effect. Distortion is rarely needed and use should be minimized.


## How does one learn image processing?

There are not many great textbooks on the subject. Here are some from my personal bookshelf. Between them (and Wikipedia) I was able to put together about 60% of the knowledge I needed; the rest I found by reading the source code to [many popular image processing libraries](https://github.com/nathanaeljones/imaging-wiki?files=1).

I would start by reading [Principles of Digital Image Processing: Core Algorithms](http://www.amazon.com/gp/product/1848001940?psc=1&redirect=true&ref_=oh_aui_search_detailpage) front-to-back, then [Digital Image Warping](http://www.amazon.com/gp/product/0818689447?psc=1&redirect=true&ref_=oh_aui_search_detailpage).  Wikipedia is also good, although the relevant pages are not linked or categorized together - use specific search terms, like ["bilinear interpolation"](https://en.wikipedia.org/wiki/Bilinear_interpolation) and ["Lab color space"](https://en.wikipedia.org/wiki/Lab_color_space).

* [Digital Image Warping](http://www.amazon.com/gp/product/0818689447?psc=1&redirect=true&ref_=oh_aui_search_detailpage)
* [Computer Graphics: Principles and Practice in C (2nd Edition)](http://www.amazon.com/gp/product/0201848406?psc=1&redirect=true&ref_=oh_aui_search_detailpage)
* [Principles of Digital Image Processing: Fundamental Techniques](http://www.amazon.com/gp/product/1848001908?psc=1&redirect=true&ref_=oh_aui_search_detailpage)
* [Principles of Digital Image Processing: Core Algorithms](http://www.amazon.com/gp/product/1848001940?psc=1&redirect=true&ref_=oh_aui_search_detailpage)
* [Principles of Digital Image Processing: Advanced Methods](http://www.amazon.com/gp/product/1848829183?psc=1&redirect=true&ref_=oh_aui_search_detailpage)

The Graphics Gems series is great for optimization inspiration:
* [Graphics Gems](http://www.amazon.com/gp/product/0122861663?psc=1&redirect=true&ref_=oh_aui_search_detailpage)
* [Graphics Gems II](http://www.amazon.com/gp/product/0120644819?psc=1&redirect=true&ref_=oh_aui_search_detailpage)
* [Graphics Gems IV](http://www.amazon.com/gp/product/0125434553?psc=1&redirect=true&ref_=oh_aui_search_detailpage)
* [Graphics Gems V](http://www.amazon.com/gp/product/0125434553?psc=1&redirect=true&ref_=oh_aui_search_detailpage)

Also, [I made some notes regarding issues to be aware of when creating an imaging library](https://github.com/imazen/Graphics-vNext/blob/master/aware.md).

I'm not aware of any implementations of (say, resampling) that are completely correct. Very recent editions of ImageMagick are very close, though. Most offer a wide selection of 'filters', but fail to scale/truncate the input or output offsets appropriately, and the resulting error is usually greater than the difference between the filters.

### Source code to read

I have found the source code for OpenCV, LibGD, FreeImage, Libvips, Pixman, Cairo, ImageMagick, stb_image, Skia, and FrameWave is very useful for understanding real-world implementations and considerations. Most textbooks assume an infinite plane, ignore off-by-one errors, floating-point limitations, color space accuracy, and operational symmetry within a bounded region. I cannot recommend any textbook  as an accurate reference, only as a conceptual starting point.

Also, keep in mind that computer vision is very different from image creation. In computer vision, resampling accuracy matters very little, for example. But in image creation, you are serving images to photographers, people with far keener visual perception than the average developer. The images produced will be rendered side-by-side with other CSS and images, and the least significant bit of inaccuracy is quite visible. You are competing with Lightroom; with offline tools that produce visually perfect results. End-user software will be discarded if photographers feel it is corrupting their work.

# Potential problem areas

* EXIF and XMP metadata parsing to access camera orientation - this needs to be resolved (and the image rotated) before we send it to the client. Clients are notoriously inconsistent about handling rotation metadata, and they take a significant perf hit as well. We also will likely need to preserve Copyright metadata strings, which means at least rudimentary metadata APIs.
* Dealing with color management on a block or scan-line level. I haven't used littlecms yet.
* Scaling at the jpeg block level may introduce a small amount of error on the right/bottom col/row of pixels if the image size is not a multiple of 8. As we only 'halve' down to 3x the target image size before resampling, this would present as a very slight weighting error.
