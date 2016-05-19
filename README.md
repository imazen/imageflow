imageflow - Real-time image processing for the web.
=========

## Elevator Pitch

The 2016 releases of ImageMagick and Vips offer remarkable image quality and support thousands of different operations on hundreds of image formats. These are fantastic pieces of software maintained by wonderful developers; and they are building blocks of today's web applications.  

But Vips and ImageMagick are intended to be used in a sandbox. In practice, they are run in-process by privileged user accounts that often have administrative access to application databases. ImageMagick executes shell scripts within SVG files, so one can own a server by uploading a simple file with any image extension. A recent string of vulnerabilities in ImageMagick were given the [ImageTragick](https://imagetragick.com) moniker to raise awareness.

Despite comprising over a million lines of code (counting codecs), ImageMagick's primary usage is straightforward: **transform and re-encode web images with optimal quality**. A sizable task, but one that merits only a fraction of that codebase.

Focusing exclusively on the transform and re-encode use case permits Imageflow to have a small, auditable codebase. It also frees developer time to spend on mathematical correctness and performance. Our early prototype of Imageflow is 2.5x faster than tuned copies of Vips or ImageMagick; and has equivalent or higher scaling quality. Most users will see an order of magnitude increase in server throughput. 

Most visual artefacts you see in images today are entirely avoidable. Decades of hacky approximations, bad mathematical and color space reasoning, and legacy compression behaviors combine to establish a very low bar for image quality. 
**There's no valid reason your web server can't deliver image quality on par with Adobe Lightroom, and do so in 8 to 200 milliseconds.**

**Imageflow enables that scenario. We don't need to shy away from on-the-fly image processing; we just need to focus on it and invest in our tools.**

**Interested in our Kickstarter?** [Give us your e-mail and we'll ping you when it goes live](https://www.surveymonkey.com/r/5M6CSFJ). You use the same link to [vote for your favorite language bindings](https://www.surveymonkey.com/r/5M6CSFJ).

----

Imageflow is AGPLv3 licensed, with commercial licenses planned to ensure sustainability.

> About the author: Nathanael Jones has a decade of experience in low-latency image server development. He created ImageResizer Server in 2006. ImageResizer v3 and v4 have since reached over one million downloads on NuGet, despite only supporting the IIS for Windows.

ImageFlow will bring world-class image quality and performance to all languages through a C-compatible API and an evolvable operation graph, with initial focus on Node and Ruby. Linux, Mac, and Windows support is planned.  

----


[![travis-master](https://img.shields.io/travis/imazen/imageflow/master.svg?label=travis%20master%20mac%20linux)
[![AppVeyor build status](https://ci.appveyor.com/api/projects/status/0356x95fa312m3wy/branch/master?svg=true)](https://ci.appveyor.com/project/imazen/imageflow/branch/master)
](https://travis-ci.org/imazen/imageflow/builds) 
[![Coverage Status](https://coveralls.io/repos/github/imazen/imageflow/badge.svg?branch=master)](https://coveralls.io/github/imazen/imageflow?branch=master)
[![Coverity Scan Build Status](https://scan.coverity.com/projects/8403/badge.svg)](https://scan.coverity.com/projects/imazen-imageflow)



### How to download, build, and run tests. 

### Universal steps for all platforms - get Conan, Cmake, and Git

Don't try to open anything in any IDE until you've run `conan install`, as cmake won't be complete.

1. [Install Conan](https://www.conan.io/downloads) - installers available for download (all platforms).
2. Install cmake (`cinst -y cmake.portable` on windows w/ chocolatey, `sudo apt-get install cmake` elsewhere)
3. Install git (`cinst -y git.install` or `sudo apt-get install git`) 
3. Install build tools for dependencies if they aren't pre-build: `sudo apt-get install build-essential binutils libtool autoconf nasm`


```bash
     git clone git@github.com:imazen/imageflow.git
     cd imageflow
     mkdir build
     cd build 
     conan install -u --file ../conanfile.py -o build_tests=True --build missing  -s build_type=Release
     cd ..
     conan build
```

### Visual Studio 2015 Update 1


Windows: build/Imageflow.sln will be created during 'conan build', but is only set up for Release mode compilation by default. Switch configuration to Release to get a build.
 
Install nasm (`cinst -y nasm` on windows, followed by `set PATH=%PATH%;%ProgramFiles(x86)%\nasm`) (only if you have to build dependencies from source).
 
 
    cd build
    conan install -u --file ../conanfile.py -o build_tests=True --build missing  -s build_type=Release -s arch=x86_64
    cd ..
    conan build
    

### Starting the demo ruby image server (complete Universal steps above first!)

    cd wrappers/ruby
    bundle install
    bundle exec thin start
    #Browse to localhost:3000/ri/7s.jpg?width=800
    #Backed by the same S3 buckets as z.zr.io demo server
    #X-CPU-Time HTTP header lets you know how much time was spent inside the imageflow library

    

======

**imageflow is still in the prototype phase. It is neither API-stable nor secure.**


![ImageFlow diagram](https://rawgit.com/imazen/imageflow/master/docs/ImageFlow_Core.svg)


## Using the long-form graph API (with proper error handling)

    
```c++
uint8_t image_bytes_literal[]
    = { 0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00,
        0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
        0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82 };

bool create_operation_graph(flow_c* c, struct flow_graph** graph_ref, int32_t input_placeholder,
                            int32_t output_placeholder, struct flow_decoder_info* info)
{

    // We'll create a simple operation graph that scales the image up 200%
    struct flow_graph* g = flow_graph_create(c, 10, 10, 200, 2.0);
    if (g == NULL) {
        FLOW_add_to_callstack(c);
        return false;
    }
    int32_t last = flow_node_create_decoder(c, &g, -1, input_placeholder);
    // Double the original width/height
    last = flow_node_create_scale(c, &g, last, info->frame0_width * 2, info->frame0_height * 2);
    // Keep the original format if png or jpeg
    size_t encoder_id = info->codec_id == flow_codec_type_decode_jpeg ? flow_codec_type_encode_jpeg
                                                                      : flow_codec_type_encode_png;
    last = flow_node_create_encoder(c, &g, last, output_placeholder, encoder_id);

    if (flow_context_has_error(c)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    *graph_ref = g;
    return true;
}

bool scale_image_inner(flow_c* c, flow_io* input, flow_io* output)
{
    // We associate codecs and nodes using integer IDs that you select
    int32_t input_placeholder = 42;
    int32_t output_placeholder = 0xbad1dea;

    // We create a job to create I/O resources and attach them to our abstract graph above
    struct flow_job* job = flow_job_create(c);
    if (job == NULL) {
        FLOW_add_to_callstack(c);
        return false;
    }
    // Uncomment to make an animation. Requires sudo apt-get install libav-tools graphviz gifsicle
    // flow_job_configure_recording(c, job, true, true, true, true, true);

    // Add I/O to the job. First bytes are read here
    if (!flow_job_add_io(c, job, input, input_placeholder, FLOW_INPUT)
        || !flow_job_add_io(c, job, output, output_placeholder, FLOW_OUTPUT)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    // Let's read information about the input file
    struct flow_decoder_info info;
    if (!flow_job_get_decoder_info(c, job, input_placeholder, &info)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    // And give it to the operation graph designer
    struct flow_graph* g;
    if (!create_operation_graph(c, &g, input_placeholder, output_placeholder, &info)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    // Execute the graph we created
    if (!flow_job_execute(c, job, &g)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    return true;
}

bool scale_image_to_disk()
{
    // flow_context provides error tracking and memory management
    flow_c* c = flow_context_create();
    if (c == NULL) {
        return false;
    }
    // We're going to use an embedded image, but you could get bytes from anywhere
    struct flow_io* input = flow_io_create_from_memory(c, flow_io_mode_read_seekable, &image_bytes_literal[0],
                                                       sizeof(image_bytes_literal), c, NULL);
    // Output to an in-memory expanding buffer. This could be a stream or file instead.
    struct flow_io* output = flow_io_create_for_output_buffer(c, c);

    // Using an inner function makes it easier to deal with errors
    if (input == NULL || output == NULL || !scale_image_inner(c, input, output)) {
        FLOW_add_to_callstack(c);
        flow_context_print_error_to(c, stderr); // prints the callstack, too
        flow_context_destroy(c);
        return false;
    }
    // Write the output to file. We could use flow_io_get_output_buffer to get the bytes directly if we wanted them
    if (!flow_io_write_output_buffer_to_file(c, output, "graph_scaled_png.png")) {

        FLOW_add_to_callstack(c);
        flow_context_print_error_to(c, stderr);
        flow_context_destroy(c);
        return false;
    }
    // This will destroy the input/output objects, but if there are underlying streams that need to be
    // closed, you would do that here after flow_context_destroy
    flow_context_destroy(c);
    return true;
}
```

## The Problem - Why we need imageflow

Image processing is a ubiquitous requirement. All popular CMSes, many CDNs, and most asset pipelines implement at least image cropping, scaling, and recoding. The need for mobile-friendly websites (and consequently responsive images) makes manual asset creation methods time-prohibitive. Batch asset generation is error-prone, highly latent (affecting UX), and severely restricts web development agility.

Existing [implementations](https://github.com/nathanaeljones/imaging-wiki) lack tests and are either (a) incorrect, and cause visual artifacts or (b) so slow that they've created industry cargo-cult assumptions about "architectural needs"; I.e, *always* use a queue and workers, because we can gzip large files on the fly but not jpeg encode them (which makes no sense from big O standpoint). This creates artificial infrastructure needs for many small/medium websites, and makes it expensive to offer image processing as part of a CDN or optimization layer. **We can eliminate this problem, and make the web faster for all users.**  There is also a high probability that (if back-ported to c89 and BSD licensed), [LibGD](https://github.com/libgd/libgd) will adopt our routines and therefore make them available within the PHP runtime, and the CMSes that build upon it. We have a great chance at reducing the 20MB homepage epidemic.

Image resampling is difficult to do correctly, and hard to do efficiently. Few attempts have been made at both. Our algorithm can [resample a 16MP image in 84ms using just one core](http://imageresizing.net/docs/v4/plugins/fastscaling). On a 16-core server, we can resample *15* such images in 262ms. Modern performance on huge matrices is all about cache-friendliness and memory latency. Compare this to 2+ seconds for FreeImage to do the same operation on 1 image with inferior accuracy. ImageMagick must be compiled in (much slower) HDRI to prevent artifacts, and even with OpenMP enabled, using all cores, is still more than an order of magnitude slower (two orders of magnitude without perf tuning).

In addition, it rarely took me more than 45 minutes to discover a vulnerability in the imaging libraries I worked with. Nearly all imaging libraries were designed as offline toolkits for processing trusted image data, accumulating years of features and attack surface area before being moved to the server. Image codecs have an even worse security record than image processing libraries, yet released toolkit binaries often include outdated and vulnerable versions.   

@jcupitt, author of the excellent [libvips](https://github.com/jcupitt/libvips) has this advice for using any imaging library:

> I would say the solution is layered security. 

> * Only enable the load libraries you really need. For example, libvips will open microscope slide images, which most websites will not require.
* Keep all the image load libraries patched and updated daily.
* Keep the image handling part of a site in a sandbox: a separate process, or even a separate machine, running as a low-privilege user.
* Kill and reset the image handling system regularly, perhaps every few images. 

**This accurate advice should be applied to any use of ImageMagick, GraphicsMagick, LibGD, FreeImage, or OpenCV.**

Also, make sure that whichever library you choose has good test coverage and automatic Valgrind and Coverity scanning set up. Also, *read* the Coverity and valgrind reports. 

Unfortunately, in-process or priviledged exeuction is the default in every CMS or image server whose code I've reviewed. 

Given the unlikelyhood of software developers learning correct sandboxing in masse (which isn't even possible to do securely on windows), it seems imperative that we create an imaging library that is safe for in-process use. 

**The proposed solution**: Create a test-covered library that is safe for use with malicious data, and says NO to any of the following

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
* [giflib](http://giflib.sourceforge.net/)
* [LittleCMS](https://github.com/mm2/Little-CMS)
* [ImageResizer - FastScaling](https://github.com/imazen/resizer/tree/develop/Plugins/FastScaling) for optimized, single-pass rendering.
* [ImageResizer](https://github.com/imazen/resizer) (From which we will port most of the domain logic, if not the image decoding/encoding portions)
* OpenCV or CCV for separate plugin to address face-aware auto-cropping.

All of the "hard" problems have been solved individually; we have proven performant implementations to all the expensive parts of image processing.

We also have room for more optimizations - by integrating with the codecs at the block and scan-line level, we can greatly reduce RAM and resource needs when downsampling large images. Libvips has proven that this approach can be incredibly fast.

A generic graph-based representation of an image processing workflow enables advanced optimizations and potentially lets us pick the fastest or best backend depending upon image format/resolution and desired workflow. Given how easily most operations compose, this could easily make the average workflow 3-8x faster, particularly when we can compose decoding and scaling for certain codecs. 

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
* Scaling at the jpeg block level may introduce a small amount of error on the right/bottom col/row of pixels if the image size is not a multiple of 8. As we only 'halve' down to 3x the target image size before resampling, this would present as a very slight weighting error. Using floating-point image dimensions could permit this to be solved, or having "subpixel_left" "subpixel_right" inputs to Render1D that describe the remainder to crop from the input.

## Node types

```c++
typedef enum flow_ntype {
    flow_ntype_Null = 0,
    flow_ntype_primitive_Flip_Vertical_Mutate = 1,
    flow_ntype_primitive_Flip_Horizontal_Mutate = 1,
    flow_ntype_primitive_Crop_Mutate_Alias = 2,
    flow_ntype_primitive_CopyRectToCanvas = 3, //Overwrite only, no compositing
    flow_ntype_Create_Canvas = 4,
    flow_ntype_primitive_RenderToCanvas1D = 5,
    flow_ntype_primitive_Scale2D_RenderToCanvas1D = 6,
    flow_ntype_primitive_bitmap_bgra_pointer,
    flow_ntype_primitive_decoder,
    flow_ntype_primitive_encoder,


    flow_ntype_non_primitive_nodes_begin = 256,

    flow_ntype_Transpose,
    flow_ntype_Flip_Vertical,
    flow_ntype_Flip_Horizontal,
    flow_ntype_Render1D,
    flow_ntype_Crop,
    flow_ntype_non_optimizable_nodes_begin = 512,


    flow_ntype_Clone,
    flow_ntype_decoder,
    flow_ntype_encoder,


    flow_ntype_Rotate_90,
    flow_ntype_Rotate_180,
    flow_ntype_Rotate_270,
    flow_ntype_Scale, //(preserve colorspace), interpolation filter


    //Not implemented below here:
    flow_ntype_Rotate_Flip_Per_Orientation,
    flow_ntype_Crop_Percentage,
    flow_ntype_Crop_Percentage_Infinite_Canvas, //canvas_color
    flow_ntype_Crop_Rectangle,
    flow_ntype_Constrain, //(mode=pad|max|crop|stretch) (width, height) (scale=down|up|both|canvas) (anchor=9 points)
    flow_ntype_Matte,
    flow_ntype_EnlargeCanvas,
    flow_ntype_Sharpen,
    flow_ntype_Blur,
    flow_ntype_Convolve_Custom,
    flow_ntype_AdjustContrast,
    flow_ntype_AdjustSaturation,
    flow_ntype_AdjustBrightness,
    flow_ntype_CropWhitespace, //tolerances and padding
    flow_ntype_Opacity,
    flow_ntype_Sepia,
    flow_ntype_Grayscale, //true|y|ry|ntsc|bt709|flat
    flow_ntype_DrawImage,
    flow_ntype_RemoveNoise,
    flow_ntype_ColorMatrixsRGB,
    flow_ntype__FORCE_ENUM_SIZE_INT32 = 2147483647
} flow_ntype;


typedef enum flow_node_state{
    flow_node_state_Blank = 0,
    flow_node_state_InputDimensionsKnown = 1,
    flow_node_state_ReadyForPreOptimizeFlatten = 1,
    flow_node_state_PreOptimizeFlattened = 2,
    flow_node_state_ReadyForOptimize = 3,
    flow_node_state_Optimized = 4,
    flow_node_state_ReadyForPostOptimizeFlatten = 7,
    flow_node_state_PostOptimizeFlattened = 8,
    flow_node_state_InputsExecuted = 16,
    flow_node_state_ReadyForExecution = 31,
    flow_node_state_Executed = 32,
    flow_node_state_Done = 63
} flow_node_state;

typedef enum flow_edgetype {
    flow_edgetype_null,
    flow_edgetype_input,
    flow_edgetype_canvas,
    flow_edgetype_info,
    flow_edgetype_FORCE_ENUM_SIZE_INT32 = 2147483647
} flow_edgetype;
         `                                                                                                                                                                                                         `
```

## Operation equivalency and composition

| Action | Equivalent action | Notes |
| ------------- |------------- | ----- |
| Rotate 270  | Transpose, Vflip |
| Rotate 90   | Vflip, Transpose |
| Rotate 180  | Vflip, Hflip |
| Transpose, Vflip | HFlip, Transpose | Slower |
| Hflip, Transpose | Transpose, VFlip | VFlip is faster than HFlip |
| Hflip, Hflip | nop |
| Vflip, Vflip | nop |
| Crop, VFlip | VFlip, Crop(adjusted) | ..etc, for HFlip, Transpose, Scale |
| Scale (new_width,new_height) | CreateCanvas(width=old_height, height=new_width), RenderToCanvas1D(new_width, Copy, transpose=true), CreateCanvas(width=new_width,height=new_height), RenderToCanvas1D(new_height,Copy,transpose=true) |

