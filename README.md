## ![imageflow](https://www.imageflow.io/images/imageflow.svg) optimal images at incredible speeds

[![travis-master](https://img.shields.io/travis/imazen/imageflow/master.svg?label=master%3A%20mac64%20ubuntu64%2014.04%2016.04)](https://travis-ci.org/imazen/imageflow/builds) [![AppVeyor build status](https://ci.appveyor.com/api/projects/status/0356x95fa312m3wy/branch/master?svg=true&passingText=master%3A%20win32%20win64%20-%20passing&failingText=master%3A%20win32%20win64%20-%20failed)](https://ci.appveyor.com/project/imazen/imageflow/branch/master) [![Coverity Scan Build Status](https://scan.coverity.com/projects/8403/badge.svg)](https://scan.coverity.com/projects/imazen-imageflow) [![state: technical preview](https://img.shields.io/badge/state-technical%E2%80%93preview-yellow.svg)](#flaws)

* **imageflow_server** can run jobs or manipulate images in-flight (e.g.`/bucket/img.jpg?w=200`) for direct use from HTML. Source images can reside in blob storage, on another server, or on the filesystem. 
* **libimageflow** is for direct (in-process) use from *your* programming language.  It has a simple [C-compatible ABI](https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/index.html) and [bindings](https://github.com/imazen/imageflow/tree/master/bindings). 
* **imageflow_tool** is a command-line tool for experimenting, running batch jobs, or when you want process isolation. Up to 17x faster than ImageMagick.

These all offer the JSON [`/build` API](https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/context_json_api.txt) as well as the traditional `width=300&height=200&mode=crop&format=jpg` command string form. Each is available as a self-contained binary for Windows and Mac. We offer Docker images for Linux (where glibc and OpenSSL are required). 

libimageflow offers interactive job manipulation as well [like `/tell_decoder`, `/get_image_info`, and `/execute`](https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/job_json_api.txt). Unless you are using memory buffers for I/O, it's better to use `/build`.  

[![view releases](https://img.shields.io/badge/-view%20downloads%20and%20releases-green.svg)](https://github.com/imazen/imageflow/releases) or `docker run --rm imazen/imageflow_tool`

[We thank our backers on Kickstarter](https://www.kickstarter.com/projects/njones/imageflow-respect-the-pixels-a-secure-alt-to-image/posts/1616122) and [the many supporters of ImageResizer](https://imageresizing.net) for making this project a reality.
Email support@imageflow.io if you need an AGPLv3 exception for commercial use. 

Also, please [send us 'challenging' images and tasks](https://github.com/imazen/imageflow/issues/98). We'd also appreciate it if you'd explore the JSON APIs and [review them and other topics where we are requesting feedback](https://github.com/imazen/imageflow/issues?q=is%3Aopen+is%3Aissue+label%3Arequesting-feedback). And – we need help with benchmarking on Windows.  

If we enough people beta-test Imageflow and provide feedback, we aim to publish a stable 1.0 release in August 2017 (along with Ruby and Node bindings). **See [flaws and missing features](#flaws) for project status.**

## Using imageflow_tool 

`imageflow_tool examples --generate` - creates an *examples* directory with JSON jobs and invocation scripts. 

You can use command strings that are compatible with [ImageResizer 4 querystrings](https://imageresizing.net/docs/basics):

`imageflow_tool v0.1/ir4 --in source.jpg  --out thumb.jpg --command "width=50&height=50&mode=crop&format=jpg" `

Or submit a JSON job file. JSON jobs can have multiple inputs and outputs, and can represent any kind of operation graph. 

The following generates mutiple sizes of an image from an example job file: 

```
imageflow_tool v0.1/build --json examples/export_4_sizes/export_4_sizes.json 
        --in http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg
        --out 1 waterhouse_w1600.jpg 
              2 waterhouse_w1200.jpg 
              3 waterhouse_w800.jpg 
              4 waterhouse_w400.jpg 
        --response operation_result.json
```

By default, imageflow_tool prints a JSON response to stdout. You write this to disk with `--response`.

`--debug-package` will create a .zip file to reproduce problematic behavior with both `v0.1/build` and `v0.1/ir4`. Please sumbit bug reports; we try to make it easy. 

## Using imageflow_server for dynamic imaging

`imageflow_server start --demo`

Now you can edit images from HTML... and use srcset without headache. 

```
<img src="http://localhost:39876/demo_images/u3.jpg?w=300" />

<img src="" srcset="    http://localhost:39876/demo_images/u3.jpg?w=300 300w
                        http://localhost:39876/demo_images/u3.jpg?w=800 800w
                        http://localhost:39876/demo_images/u3.jpg?w=1600 1600w" />

```

![](https://www.imageflow.io/images/imageflow-responsive.svg) ![](https://www.imageflow.io/images/edit-url.gif)

### Beyond the demo 

You'll want to mount varous image source locations to prefixes. The `--mount` command parses a colon (`:`) delimited list of arguments. The first is the prefix you'll use in the URL (like `http://localhost:39876/prefix/`. The second is the engine name. Remaining arguments are sent to the engine.  

#### Examples

* `--mount "/img/:ir4_local:C:\inetpub\wwwroot\images"`
* `--mount "/proxyimg/:ir4_http:https:://myotherserver.com/imagefolder/"` (note the double escaping of the colon)
* `--mount "/cachedstaticproxy/:permacache_proxy:https:://othersite.com"`
* `--mount "/githubproxy/:permacache_proxy_guess_content_types:https:://raw.github.com/because/it/doesnt/support/content/types"`
* `--mount "/static/":static:./assets"`


![](https://www.imageflow.io/images/imageflow-server-advanced.svg)

## Using libimageflow

![](https://www.imageflow.io/images/libimageflow-direct.svg)

* C# - @samuelenglard has volunteered to create C# bindings for Imageflow. We're tracking [design here](https://github.com/imazen/imageflow/issues/67).
* Ruby - Basic bindings can be found in [bindings/ruby/](https://github.com/imazen/imageflow/tree/master/bindings/ruby)
* Node - Not yet started. Want to help? [generate bindings from the header files](https://github.com/tjfontaine/node-ffi-generate)
* C and C++ - use [bindings/headers/imageflow_default.h](https://github.com/imazen/imageflow/blob/master/bindings/headers/imageflow_default.h) or one of the many alternate conventions provided with each release.
* Rust - Imageflow is written in Rust. Use the `imageflow_core` crate, but be warned that this interface will evolve more rapidly than the FFI `imageflow` crate.  
* other languages - Use an [FFI](https://en.wikipedia.org/wiki/Foreign_function_interface) binding-generation tool for your language, and feed it whichever [header file it likes best](https://github.com/imazen/imageflow/tree/master/bindings/headers). 

Official Ruby and Node bindings will be released by August 2017. 


# How to build Imageflow from source

We're assuming you've cloned already. 

```bash
     git clone git@github.com:imazen/imageflow.git
     cd imageflow
```

## Docker (linux/macOS/WinUbuntu)

This will create caches within `~/.docker_imageflow_caches` specific to the docker image used. Instances will be ephemeral; the only state will be in the caches.

```bash
./build_via_docker.sh debug
```


## Linux (native)

We need a few packages in order to build the C dependencies. You probably have most of these already.

* build-essential, nasm, dh-autoreconf, pkg-config 
* wget, curl, git
* libpng, libssl, ca-certificates
* python, pip, and setuptools

## For Ubuntu 14.04 and 16.04:

```bash
sudo apt-get install --no-install-recommends \
  build-essential nasm dh-autoreconf pkg-config \
  wget curl git ca-certificates \
  libpng-dev libssl-dev \
  python-minimal python-pip python-setuptools
```

After installing the above, you'll need conan, cmake 3.4.1+, dssim, and Rust Nightly.

```bash
curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly-2017-06-12
sudo pip install conan
./ci/nixtools/install_cmake.sh
./ci/nixtools/install_dssim.sh
./build.sh
```

The build will fail with `settings.target_cpu`. Edit `$HOME\.conan\settings.yml`. Append the line `target_cpu: [x86, x86-64, nehalem, sandybridge, haswell, native]`


We aren't listing dependencies needed for 

* Valgrind (common versions break openssl; you may need to build from source)
* Code coverage
* Bindings. 

Please consult the [official Dockerfiles](https://github.com/imazen/dockerfiles_imageflow) for these.

## OS X (native)

You'll need a bit less on OS X, although this may not be comprehensive:

```bash
brew install conan nasm cmake
./ci/nixtools/install_dssim.sh
./build.sh
```

The build will fail with `settings.target_cpu`. Edit `$HOME\.conan\settings.yml`. Append the line `target_cpu: [x86, x86-64, nehalem, sandybridge, haswell, native]`


## Windows

Don't use a C++ IDE until you've run `win_build_c.bat`, as CMake needs to generate files.

### Pre-requisites

1. Visual Studio 2015 (for the C++ build tools)
2. Install [Git 64-bit](https://git-scm.com/download/win).
3. Install [NASM 64-bit](http://www.nasm.us/pub/nasm/releasebuilds/2.12.02/win64/nasm-2.12.02-installer-x64.exe) Installer must be `Run as Administrator` - it will not prompt. 
4. Install [Rust 64-bit](https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe). Install toolchain `nightly-2017-06-08` and set it as default. For the moment, 32-bit builds also require [a 32-bit Rust](https://static.rust-lang.org/rustup/dist/i686-pc-windows-msvc/rustup-init.exe).
5. Install [CMake 3.7 64-bit](https://cmake.org/download/) (3.8.0-rc3 makes paths too long)
6. Install [Conan](https://www.conan.io/downloads). 

You need all of these to be in %PATH%. Edit `ci/wintools/SETUP_PATH.bat` as appropriate to ensure that cmake, conan, rust/cargo, nasm, git, and Git/mingw64/bin are all available. 

1. Run `win_enter_env.bat` to start a sub-shell with VS tools loaded and a proper PATH. Edit the file per its comments to target a 32-bit build (you may want a separate imageflow folder for each target).
2. Run `win_build_c.bat` from the shell to compile the C components
3. It will fail with `settings.target_cpu`. Edit `$HOME\.conan\settings.yml`. Append the line `target_cpu: [x86, x86-64, nehalem, sandybridge, haswell, native]`
4. `cd ..\..` back to the root and run `win_build_c.bat` again.
5. Run `win_build_rust.bat` to compile the Rust components

 `c_components/build/Imageflow.sln` will be created during 'win_build_c.bat', but is only set up for Release mode compilation by default. Switch configuration to Release to get a build. You'll need to run conan install directly if you want to change architecture to `x86`, target_cpu to `haswell`, or build_type to `Debug`, since the solutions need to be regenerated.
 
    cd build
    conan install --generator txt --scope build_tests=True -o shared=True --build missing -s build_type=Release -s arch=x86_64  -s target_cpu=sandybridge -u ../
    conan build ../

    
## How does one learn image processing for the web?

First, [read High Performance Images](http://shop.oreilly.com/product/0636920039730.do) for context.

There are not many great textbooks on the subject. Here are some from my personal bookshelf. Between them (and Wikipedia) I was able to put together about 60% of the knowledge I needed; the rest I found by reading the source code to [many popular image processing libraries](https://github.com/nathanaeljones/imaging-wiki?files=1).

I would start by reading [Principles of Digital Image Processing: Core Algorithms](http://www.amazon.com/gp/product/1848001940?psc=1&redirect=true&ref_=oh_aui_search_detailpage) front-to-back, then [Digital Image Warping](http://www.amazon.com/gp/product/0818689447?psc=1&redirect=true&ref_=oh_aui_search_detailpage).  Wikipedia is also a useful reference, although the relevant pages are not linked or categorized together - use specific search terms, like ["bilinear interpolation"](https://en.wikipedia.org/wiki/Bilinear_interpolation) and ["Lab color space"](https://en.wikipedia.org/wiki/Lab_color_space).

* [Digital Image Warping](http://www.amazon.com/gp/product/0818689447?psc=1&redirect=true&ref_=oh_aui_search_detailpage)
* [Computer Graphics: Principles and Practice in C (2nd Edition)](http://www.amazon.com/gp/product/0201848406?psc=1&redirect=true&ref_=oh_aui_search_detailpage)
* [Principles of Digital Image Processing: Fundamental Techniques](http://www.amazon.com/gp/product/1848001908?psc=1&redirect=true&ref_=oh_aui_search_detailpage)
* [Principles of Digital Image Processing: Core Algorithms](http://www.amazon.com/gp/product/1848001940?psc=1&redirect=true&ref_=oh_aui_search_detailpage)
* [Principles of Digital Image Processing: Advanced Methods](http://www.amazon.com/gp/product/1848829183?psc=1&redirect=true&ref_=oh_aui_search_detailpage)

I have found the source code for OpenCV, LibGD, FreeImage, Libvips, Pixman, Cairo, ImageMagick, stb_image, Skia, and FrameWave is very useful for understanding real-world implementations and considerations. Most textbooks assume an infinite plane, ignore off-by-one errors, floating-point limitations, color space accuracy, and operational symmetry within a bounded region. I cannot recommend any textbook  as an accurate reference, only as a conceptual starting point. [I made some notes regarding issues to be aware of when creating an imaging library](https://github.com/imazen/Graphics-vNext/blob/master/aware.md).

Also, keep in mind that computer vision is very different from image creation. In computer vision, resampling accuracy matters very little, for example. But in image creation, you are serving images to photographers, people with far keener visual perception than the average developer. The images produced will be rendered side-by-side with other CSS and images, and the least significant bit of inaccuracy is quite visible. You are competing with Lightroom; with offline tools that produce visually perfect results. End-user software will be discarded if photographers feel it is corrupting their work.

### Source organization

Rust crates

* imageflow_types - Shared types, with JSON serialization
* imageflow_helpers - Common helper functions and utilities
* imageflow_riapi - RIAPI and ImageResizer4 compatibility parsing/layout
* imageflow_core - The main library
* imageflow_abi - The C-Compatible API - exposes functionality from imageflow_core
* imageflow_tool
* imageflow_server

C source is located in ./c_componenets/lib, and ./c_components/tests

Headers for libimageflow.dll are located in `bindings/headers`

### Known flaws and missing features (as of July 2017)

#### Flaws

- [ ] JSON operations aren't yet validating their parameters. Invalid values end the process.
- [ ] imageflow_server doesn't expose the JSON API yet (due to above)
- [ ] No fuzz testing completed.

#### Missing features

- [ ] Animated GIF write support (reading individual frames is supported)
- [ ] Some advanced rendering features: Whitespace detection/cropping, watermarking, blurring.
- [ ] Automatic encoder selection and tuning.

#### Delayed features

- [ ] Job cost prediction (delayed - no interest from community)
- [ ] Node bindings (delayed - no interest from community)
