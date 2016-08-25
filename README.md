# ![imageflow](https://www.imageflow.io/images/imageflow.svg) = libimageflow + imageflow-server

Imageflow will bring world-class image quality and performance to all languages through a C-compatible API (libimageflow) and a separate RESTful turnkey HTTP server and command-line tool. Linux, Mac, and Windows are supported. [The Imageflow Kickstarter ended successfully!](https://www.kickstarter.com/projects/njones/imageflow-respect-the-pixels-a-secure-alt-to-image/posts/1616122)

----

[![travis-master](https://img.shields.io/travis/imazen/imageflow/master.svg?label=travis%20master%20mac%20linux)
[![AppVeyor build status](https://ci.appveyor.com/api/projects/status/0356x95fa312m3wy/branch/master?svg=true)](https://ci.appveyor.com/project/imazen/imageflow/branch/master)
](https://travis-ci.org/imazen/imageflow/builds) 
[![Coverage Status](https://coveralls.io/repos/github/imazen/imageflow/badge.svg?branch=HEAD)](https://coveralls.io/github/imazen/imageflow?branch=HEAD)
[![Coverity Scan Build Status](https://scan.coverity.com/projects/8403/badge.svg)](https://scan.coverity.com/projects/imazen-imageflow)

### How can I help?

#### Do you know a little Rust, or are you interested in learning?

We're starting to port large parts of Imageflow to Rust, and code review / design feedback is invaluable.

We need a GIF codec. GIFLIB is not acceptable, for both security and licensing reasons. Most image libraries write their own, as it's not a huge task. We'd like ours to be written in Rust.

There are a few Rust gif codecs out there, but none that combine all the features we need together. This is a medium-size, but reasonably self-contained task with copious amounts of prior art to reference. Existing codecs are appropriately licensed, so the largest task may be setting up a test suite.

Have you worked with directed acyclic graphs in Rust? [It would help if you could share your findings here](https://github.com/imazen/imageflow/issues/79).

#### Do you have a physical Windows box?

Virtual machines aren't great for benchmarks. Imageflow (flow-proto1, for now) could benefit from independent benchmarking on physical machines.

#### Are you comfortable using Docker? 

Our linux build enivornment is fully Dockerized. But there are parts that we haven't yet set up

* Documentation generation & upload
* Release automation
* Triggering a second CI round for long-running (i.e, 40m+) comparison tests. 
* We haven't set up a tutorial for building Imageflow via docker.

### Algorithm implementation work

- [x] Fast image resampling (scaling) with superb quality & speed
- [x] Basic operations (crop, rotate, flip, expand canvas, fill rectangle)
- [x] Support color profile, convert to sRGB
- [x] Image blending and composition (no external API yet)
- [x] Whitespace detection/cropping (no external API yet)
- [x] Ideal scaling-integrated sharpening for subpixel accuracy. (no external API yet)
- [x] Automatic white balance correction.  (no external API yet)
- [ ] Time-constant guassian approximation (%97) blur (1 bug remaining)
- [ ] Improve contrast/saturation/luma adjustment ergonomics
- [x] Integrate libjpeg-turbo (read/write)
- [x] Create correct and ooptimized IDCT downscaling for libjpeg-turbo (linear light, robidoux filter)
- [x] Integrate libpng (read/write) (32-bit only)
- [x] Integrate libgif (readonly, single frame)
- [ ] Support animated gifs
- [ ] Support metadata reading and writing
- [ ] Histogram support
- [ ] Document type detection
- [ ] Generic convolution support
- [ ] Add 128-bit color depth support for all operations (most already use 128-bit internally)
- [ ] Integrate libimagequant for optimal 8-bit png and gif file sizes.
- [ ] Build command-line tool for users to experiment with during Kickstarter. 
- [ ] Implement cost estimation for all operations
- [ ] Add subpixel cropping during scale to compensate for IDCT block scaling where subpixel accuracy can be reduced.
- [x] Auto-generate animated gifs of the operation graph evolution during execution.  
- [ ] Create face and object detection plugin for smart cropping. Not in main binary, though. 
- [ ] Reason about signal-to-noise ratio changes, decoder hints, and determine best codec tuning for optimal quality. Let's make a better photocopier (jpeg). 


### API Work
- [x] Expose an xplat API (using direct operation graph construction) and test via Ruby FFI bindings.
- [x] Validate basic functionality via simple ruby REST [RIAPI](http://riapi.org) server to wrap libimageflow
- [x] Design correct error handling protocol so all APIs report detailed stacktrace w/ line numbers and useful error messages for all API surfaces. 
- [x] Expose custom memory managment interface, so callers can use custom allocators or reusable slab allocation (great to avoid fragmentation on large allocations). This may go away if we introduce Rust, although with 1.9 we can still handle malloc failure gracefully.
- [x] Expose flexible I/O interface so a variety if I/O types can be cleanly supported from host languages (I.e, .NET Stream, FILE *, membuffer, circular buffer, etc)
- [ ] Replace direct graph maniupulation with JSON API
- [ ] Finish API design and test coverage for image composition, whitespace detection/cropping, sharpening, blurring, contrast/saturation, and white balance (algorithms already complete or well defined). 
- [ ] Expose plugin interface for codecs (70%)
- [ ] Expose custom operations API so host languages can add new algoroithms (50%)
- [ ] Create documentation
- [ ] Create .NET Full/Core bindings
- [ ] Create Node bindings 

### Refactorings


- [ ] Begin porting to Rust. 
- [x] Explicit control flow at all points.
- [x] Full debugging information by recording errors at failure point, then appending the stacktract
- [x] Give user complete control over allocation method and timing.
- [x] Use [Conan.io](http://conan.io) for package management and builds to eliminate dependency hell.
- [x] Make codecs and node definitions uniform
- [x] Establish automated code formatting rules in .clang-format
- [ ] Replace giflib
- [ ] replace zlib with zlib-ng
- [ ] Replace ruby prototype of libimageflow-server with a Rust version
- [ ] Look into replacing parts of the jpeg codec with concurrent alternatives. 
- [ ] Add fuzz testing for JSON and I/O 
- [ ] Find cleaner way to use SSE2 constructs with scalar fallbacks, it is messy in a few areas.


### How to download, build, and run tests. 

### Universal steps for all platforms - get Conan, Cmake, and Git

Don't try to open anything in any IDE until you've run `conan install`, as cmake won't be complete.

1. [Install Conan](https://www.conan.io/downloads) - installers available for download (all platforms).
2. Install cmake (`cinst -y cmake.portable` on windows w/ chocolatey, `sudo apt-get install cmake` elsewhere)
3. Install git (`cinst -y git.install` or `sudo apt-get install git`) 
3. Install build tools for dependencies: `sudo apt-get install build-essential binutils libtool autoconf nasm`


```bash
     git clone git@github.com:imazen/imageflow.git
     cd imageflow
     ./rebuild.sh
```

### Visual Studio 2015 Update 1


Windows: build/Imageflow.sln will be created during 'conan build', but is only set up for Release mode compilation by default. Switch configuration to Release to get a build. You'll need to run conan install directly if you want to change architecture, since the solutions need to be regeneterated.
 
Install nasm (`cinst -y nasm` on windows, followed by `set PATH=%PATH%;%ProgramFiles(x86)%\nasm`) (only if you have to build dependencies from source).
 
 
    cd build
    conan install -u --file ../conanfile.py --scope build_tests=True --build missing  -s build_type=Release -s arch=x86_64
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

**libimageflow is still in the prototype phase. It is neither API-stable nor secure.**

![](https://www.imageflow.io/images/imageflow-features.svg)


## The Problem - Why we need imageflow

Image processing is a ubiquitous requirement. All popular CMSes, many CDNs, and most asset pipelines implement at least image cropping, scaling, and recoding. The need for mobile-friendly websites (and consequently responsive images) makes manual asset creation methods time-prohibitive. Batch asset generation is error-prone, highly latent (affecting UX), and severely restricts web development agility.

![](https://www.imageflow.io/images/imageflow-responsive.svg) ![](https://www.imageflow.io/images/edit-url.gif)

Existing [implementations](https://github.com/nathanaeljones/imaging-wiki) lack tests and are either (a) incorrect, and cause visual artifacts or (b) so slow that they've created industry cargo-cult assumptions about "architectural needs"; I.e, *always* use a queue and workers, because we can gzip large files on the fly but not jpeg encode them (which makes no sense from big O standpoint). This creates artificial infrastructure needs for many small/medium websites, and makes it expensive to offer image processing as part of a CDN or optimization layer. **We can eliminate this problem, and make the web faster for all users.** 

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

## Integration options

![](https://www.imageflow.io/images/imageflow-server-advanced.svg)
![](https://www.imageflow.io/images/libimageflow-direct.svg)

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
