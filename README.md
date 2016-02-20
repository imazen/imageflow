imageflow - Real-time image processing for the web.
=========

[![travis-master](https://img.shields.io/travis/imazen/imageflow/master.svg?label=travis%20master)
](https://travis-ci.org/imazen/imageflow/builds) 

Using CLion with imageflow:

    bii init -l=clion imageumbrella
    cd imageumbrella/
    git clone git@github.com:imazen/imageflow.git blocks/nathanaeljones/imageflow
    git clone git@github.com:nathanaeljones/theft.git blocks/nathanaeljones/theft
    bii test


    bii test  -T memcheck

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
* The uncompressed 32-bit image can fit in RAM. If it can't, we don't do it. This is for web output use, not scientific or mapping applications. Also; at least 1 matrix transposition is required for downsampling an image, and this essentially requires it all to be in memory. No paging to disk, ever!
* We support jpeg, gif, and png natively. All other codecs are plugins. We only write sRGB output.

## The components

* [libjpeg-turbo](https://github.com/imazen/libjpeg-turbo) or [mozjpeg](https://github.com/mozilla/mozjpeg)
* [libpng](http://www.libpng.org/pub/png/libpng.html)
* [stb_image](https://github.com/nothings/stb) (May be useful for select functions, but use is unlikely)
* [LittleCMS](https://github.com/mm2/Little-CMS)
* [ImageResizer - FastScaling](https://github.com/imazen/resizer/tree/develop/Plugins/FastScaling) for optimized, single-pass rendering.
* [ImageResizer](https://github.com/imazen/resizer) (From which we will port most of the domain logic, if not the image decoding/encoding portions)
* OpenCV or CCV for separate plugin to address face-aware auto-cropping.

![ImageFlow diagram](https://rawgit.com/imazen/imageflow/master/docs/ImageFlow_Core.svg)

All of the "hard" problems have been solved individually; we have proven performant implementations to all the expensive parts of image processing.

We also have room for more optimizations - by integrating with the codecs at the block and scan-line level, we can greatly reduce RAM and resource needs when downsampling large images. Libvips has proven that this approach can be incredibly fast.

A generic graph-based representation of an image processing workflow is very tempting. This enables advanced optimizations and potentially lets us pick the fastest or best backend depending upon image format/resolution and desired workflow. Given how easily most operations compose, this could easily make the average workflow 3-8x faster, particularly when we can compose decoding and scaling for certain codecs. The downside to this approach is the complexity of exposing a graph API via C. I would eschew the graph for initial iterations, only introducing it once we had a naive alternative in place already.


## The languages

The pragmatic language choice for the core routines is C11. Rust is extremely attractive, and would make the solution far more secure (there are already safe Rust codecs!). However, given that we often resort to assembly or manual unrolling in C, it may be unrealistic to assume we wouldn't also periodically run into perf issues with the results of the Rust compiler. Long-term, Rust would be the ideal choice, as we get a C ABI, no runtime, yet great safety and concurrency possibilities. However, the development timeline with Rust would be nearly impossible to predict.

Given that there is a large amount of non-perf-critical domain logic required, it may be prudent to use Lua or Go for the mid and high-level APIs, particularly if Rust is not involved. This, however, would make it difficult to expose a high-level cross platform API.

For the present, we intend to use C11 for the entire set of APIs.


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

## Potential problem areas

* EXIF and XMP metadata parsing to access camera orientation - this needs to be resolved (and the image rotated) before we send it to the client. Clients are notoriously inconsistent about handling rotation metadata, and they take a significant perf hit as well. We also will likely need to preserve Copyright metadata strings, which means at least rudimentary metadata APIs.
* Dealing with color management on a block or scan-line level. I haven't used littlecms yet.
* Scaling at the jpeg block level may introduce a small amount of error on the right/bottom col/row of pixels if the image size is not a multiple of 8. As we only 'halve' down to 3x the target image size before resampling, this would present as a very slight weighting error. Using floating-point image dimensions could permit this to be solved.

## Problems

* If a node has two outbound edges and both try to re-use the canvas, how do we solve the problem?


## Operation equivalency and composition

| Action | Equivalent action | Notes
| Rotate 270  | Transpose, Vflip |
| Rotate 90   | Vflip, Transpose |
| Rotate 180  | Vflip, Hflip |
| Transpose, Vflip | HFlip, Transpose | Slower |
| Hflip, Transpose | Transpose, VFlip | VFlip is faster than HFlip |
| Hflip, Hflip | nop |
| Vflip, Vflip | nop |
| Crop, VFlip | VFlip, Crop(adjusted) | ..etc, for HFlip, Transpose, Scale |
| Scale (new_width,new_height) | CreateCanvas(width=old_height, height=new_width), RenderToCanvas1D(new_width, Copy, transpose=true), CreateCanvas(width=new_width,height=new_height), RenderToCanvas1D(new_height,Copy,transpose=true) |

## Concrete Frame operations

| VFlip | Format agnostic | In Place
| Crop  | Format agnostic | In Place
| CopyRect  | Format agnostic | New Frame
| CreateCanvas | 
| RenderToCanvas1D (scale (InterpolationDetails), compose (InPlace, Copy, Blende, Matte[color]), bool transpose, [list of convolution & pixel filters], working_floatspace)

Resize: 
CreateCanvas
Render1D(scale, Copy, transpose=true)
Render1D(scale, Copy, transpose=true)


## Sample API use

```

//TODO: Adapt these function signatures to deal with error reporting (or are we expecting the host language to panic/throw exception?)
//TODO: Add dispose hooks?

//ImageSourceBufferReader
size_t get_length(void * token, Context * c){
    //Get size of image from storage based on token.
}
size_t copy_to(uint8_t * buffer, size_t buffer_size, void * token, Context * c){
    //Copy image bytes to destination buffer, returning actual number of bytes copied (in case get_length overestimated)
    //May be called with a smaller buffer if only the header is required. May be called multiple times; caching is suggested.
}


//ImageSourceSequentialReader
size_t get_length(void * token, Context * c){
    //Get size of image from storage based on token.
}
size_t read_bytes(uint8_t * buffer, size_t buffer_size, void * token, Context * c){
    //Copy next set of image bytes to destination buffer, returning actual number of bytes copied (in case get_length overestimated)
    //May be called many times.
}

//ImageSourceIO
size_t custom_read(void *buffer, size_t size, void * token) {
    return fread(buffer, 1, size, (FILE *)token);
}
size_t custom_write(void *buffer, size_t size, void * token) {
    return (size_t)fwrite(buffer, 1, size, (FILE *)token);
}
int custom_seek(long offset, int origin, void * token) {
    return fseek((FILE *)handle, offset, origin);
}
long int custom_position(void * token) {
    return ftell((FILE *)token);
}
size_t custom_length(void * token){

}

//ImageSourcePeek
size_t peek_bytes(void *buffer, size_t requested_byte_count, int32_t * more_bytes_exist, void * token, Context * c){
//Returns actual byte count, which may be less than requested, either because fewer header bytes were cached by the host,
//or because the file is shorter. Check the more_bytes_exist flag  (0 - all file bytes sent, 1 - partial file sent)
}

// ImageSourceWriter
int write_bytes(void *buffer, size_t size, void * token, Context * c){
}


// initialize your own IO functions
ImageSourceIO io;
io.read_proc = custom_read;
io.write_proc = custom_write;
io.seek_proc = custom_seek;
io.tell_proc = custom_position;
io.length_proc = custom_length;

uint8_t * image_a_buffer = malloc(200);
size_t image_a_bytes = 200;

char * image_b_uuid = "124-515215-15251";

ImageSourceBufferReader image_b_buffer;
image_b_buffer.get_length = get_length;
image_b_buffer.copy_to = copy_to;

//Source complicates
// Color profile is orthogonal to orientation data
//

Context * c = Context_create(); if (c == NULL) return 1;


//We construct a frame graph using numeric placeholders for input and output. 
FrameGraph * g = FrameGraph_create(c, 4096); //initial allocation size. failure to allocate enough space will ? cause a panic. ?
if (g == NULL){ return 2 }; //OOM 

int last = FrameNode_create(c, g,  NULL, FrameNode_Input_Placeholder, 0 );
last = FrameNode_create(c,g, last, FrameNode_Constrain, 300,200, Constrain_max, Upscale_canvas);
last = FrameNode_create(c,g, last, FrameNode_Output_Placeholder,0);




//Once the context is created, we rely on the fact that calls should fail via result code
ImageSource * image_a = ImageSource_create_and_copy_from_buffer(c, image_a_buffer, image_a_bytes); 
ImageSource_add_peek_function(c,image_a, peek_bytes, NULL);

ImageSource * image_b = ImageSource_create_empty(c); 
ImageSource_add_buffer_reader(c, image_b, image_b_buffer, image_b_uuid);

ImageSource * image_c = ImageSource_create_empty(c);
ImageSource_add_io(c, image_c, io, /* file ptr */);


//Wait, is it easier to run a binary search over input image sizes, or to implement constraint algebra over the graph? Or can the former solve for more than 1 variable?
ImageSource * image_simulation = ImageSource_create_with_dimensions(c, 200,100, Bgra32);

ImageJob * sim = ImageJob_create(c);
int useful_width;
int useful_height;
ImageJob_find_maximum_useful_dimensions(c, sim, g, &useful_width, &useful_height); 

ImageJob * job = ImageJob_create(c);

//coder and decoder instances are local to the image jobs

if (Context_has_error(c)){
    //TODO: propagate error details
    Context_destroy(c);
    return 1;
}

ImageJob_add_primary_source(c, job, image_a); //These can be called with null ImageSource, 
ImageJob_add_secondary_source(c, job, image_b);
ImageJob_add_target(c, job, image_c);

StatusCode result = ImageJob_read_sources_info(c, job);
if (result == Ok){
    ImageSource_get_frame_count(c,image_a);
    ImageSource_get_page_count(c,image_a);
    ImageSource_get_dimensions(c,image_a, &w, &h);
    ImageJob_set_target_format(c, job, image_c, Jpeg, 90);
    ImageJob_autoset_target_format( c, job, image_c) ; //perhaps based on the source images?
    
    
    
    do {
    FrameGraph  * frame0 = FrameGraph_copy(c,g);
        ImageJob_complete_frame_graph(c, job, frame0);
        ImageGraph_flatten(c, job, frame0);
        ImageGraph_optimize(c, job, frame0);
        
    ImageJob_execute_all_targets(c, job, frame0);
    }while(ImageJob_next_frame(c,job));
    
        
}else{
    //Deal with error
    ImageJob_destroy(c, job);
    Context_destroy(c); //Destroying the context should ensure any ImageSource caches are freed. 
    return 2;
}
 

//If using a managed language, make sure you pin your reader/writer structs & functions.
ImageJob_destroy(c, job);
Context_destroy(c); //Destroying the context should ensure any ImageSource caches are freed. 
return 0;

```



## Generating animated gifs of graph progression.

1. Switch to the directory holding the generated graph_version_XXX.dot files.
2. Ensure you have graphviz, gifsicle and avtools:  sudo apt-get install libav-tools graphviz gifsicle
3. Convert them to .png: `find . -type f -name '*.dot' -execdir dot -Tpng -Gsize=5,9\! -Gdpi=100  -O {} \;`
4. Assemble .gif: `avconv -i job_2_graph_version_%d.dot.png -pix_fmt rgb24 -y output.gif`
5: Add delay to frames, optimize: `gifsicle -d 200 output.gif -l=2 -O -o optimized.gif`
