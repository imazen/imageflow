## ![imageflow](https://www.imageflow.io/images/imageflow.svg) optimal images at incredible speeds

[![travis-master](https://img.shields.io/travis/imazen/imageflow/master.svg?label=master%3A%20MacOS,%20Ubuntu%2016.04%2018.04)](https://travis-ci.org/imazen/imageflow/builds) [![AppVeyor build status](https://ci.appveyor.com/api/projects/status/0356x95fa312m3wy/branch/master?svg=true&passingText=master%3A%20win32%20win64%20-%20passing&failingText=master%3A%20win32%20win64%20-%20failed)](https://ci.appveyor.com/project/imazen/imageflow/branch/master) [![Coverity Scan Build Status](https://scan.coverity.com/projects/8403/badge.svg)](https://scan.coverity.com/projects/imazen-imageflow) [![state: technical preview](https://img.shields.io/badge/state-release%E2%80%93candidate-yellow.svg)](#flaws)

[![Docker Pulls](https://img.shields.io/docker/pulls/imazen/imageflow_tool.svg)](https://hub.docker.com/r/imazen/imageflow_tool/)
[![view releases](https://img.shields.io/badge/-download%20binaries%20for%20windows,%20mac,%20or%20linux-green.svg)](https://github.com/imazen/imageflow/releases) [![license: Choose AGPLv3 or Commercial](https://img.shields.io/badge/license-Choose%20AGPLv3%20or%20Commercial-green.svg)](https://imageresizing.net/pricing)


[Download](https://github.com/imazen/imageflow/releases) blazing fast and [safer](https://cve.mitre.org/cgi-bin/cvekey.cgi?keyword=imagemagick) tools for a modern image workflow.


* **imageflow_tool** is a command-line tool for experimenting, running batch jobs,
or when you want process isolation. Up to 17x faster than ImageMagick. Also produces smaller files at higher quality.
* **imageflow_server** can run JSON jobs or manipulate images in-flight (e.g.`/bucket/img.jpg?w=200`) for direct use from
HTML. Source images can reside in blob storage, on another server, or on the filesystem.
* **libimageflow** is for direct (in-process) use from *your* programming language. [**Node bindings available here**](https://github.com/imazen/imageflow-node), and  [**.NET Standard bindings here**](https://github.com/imazen/imageflow-dotnet). If we don't already have bindings for your language, consider spending a day to add them. Imageflow has a simple
C-compatible ABI, of which only 4 methods are needed to implement bindings. 

**[Open an issue](https://github.com/imazen/imageflow/issues/new) to have us write example code for your use case. We believe in feedback-driven design, and streamlining real-world usage is the fastest way to a great product.**

[Querystring API Documentation](https://docs.imageflow.io/querystring/introduction.html)

[JSON API Documentation](https://docs.imageflow.io/json/introduction.html) 

libimageflow, imageflow_tool, and imageflow_server are available as
[self-contained binaries](https://github.com/imazen/imageflow/releases) for Windows, Ubuntu, and Mac. We also offer [Docker images](https://hub.docker.com/r/imazen/imageflow_tool/) for Linux (where glibc and OpenSSL are required). 

[We thank our backers on Kickstarter](https://www.kickstarter.com/projects/njones/imageflow-respect-the-pixels-a-secure-alt-to-image/posts/1616122)
and [the many supporters of ImageResizer](https://imageresizing.net) for making this project a reality.
Visit [Imageresizing.net](https://imageresizing.net/pricing) if you need an AGPLv3 exception for commercial use.


## Start with imageflow_tool (recommended)

`imageflow_tool examples --generate` - creates an *examples* directory with JSON jobs and invocation scripts.

You can use command strings that are compatible with [ImageResizer 4 querystrings](https://imageresizing.net/docs/basics):

`imageflow_tool v1/querystring --in source.jpg  --out thumb.jpg --command "width=50&height=50&mode=crop&format=jpg" `

Or submit a JSON job file. JSON jobs can have multiple inputs and outputs, and can represent any kind of operation graph.

The following generates multiple sizes of an image from an example job file:

```
imageflow_tool v1/build --json examples/export_4_sizes/export_4_sizes.json
        --in waterhouse.jpg
        --out 1 waterhouse_w1600.jpg
              2 waterhouse_w1200.jpg
              3 waterhouse_w800.jpg
              4 waterhouse_w400.jpg
        --response operation_result.json
```

By default, imageflow_tool prints a JSON response to stdout. You write this to disk with `--response`.

`--debug-package` will create a .zip file to reproduce problematic behavior with both `v1/build` and `v1/querystring`. Please submit bug reports; we try to make it easy.

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

You'll want to mount various image source locations to prefixes. The `--mount` command parses a colon (`:`) delimited list of arguments. The first is the prefix you'll use in the URL (like `http://localhost:39876/prefix/`. The second is the engine name. Remaining arguments are sent to the engine.

#### Examples

* `--mount "/img/:ir4_local:C:\inetpub\wwwroot\images"`
* `--mount "/proxyimg/:ir4_http:https:://myotherserver.com/imagefolder/"` (note the double escaping of the colon)
* `--mount "/cachedstaticproxy/:permacache_proxy:https:://othersite.com"`
* `--mount "/githubproxy/:permacache_proxy_guess_content_types:https:://raw.github.com/because/it/doesnt/support/content/types"`
* `--mount "/static/":static:./assets"`


![](https://www.imageflow.io/images/imageflow-server-advanced.svg)

## Using libimageflow from your language

![](https://www.imageflow.io/images/libimageflow-direct.svg)

* .NET Standard bindings can be found at https://github.com/imazen/imageflow-dotnet
* Node bindings available  at https://github.com/imazen/imageflow-node
* Ruby - Basic bindings can be found in [bindings/ruby/](https://github.com/imazen/imageflow/tree/master/bindings/ruby)
* C and C++ interface is stable - use [bindings/headers/imageflow_default.h](https://github.com/imazen/imageflow/blob/master/bindings/headers/imageflow_default.h) or one of the many alternate conventions provided with each release.
* Rust - Imageflow is written in Rust, so you can use the `imageflow_core` crate.
* other languages - Use an [FFI](https://en.wikipedia.org/wiki/Foreign_function_interface) binding-generation tool for your language, and feed it whichever [header file it likes best](https://github.com/imazen/imageflow/tree/master/bindings/headers).

You also may find that `imageflow_tool` is quite fast enough for your needs.

### Crates within this project

* imageflow_abi - The stable API of libimageflow/imageflow.dll.
  Headers for libimageflow are located in `bindings/headers`
* imageflow_tool - The command-line tool
* imageflow_server - The HTTP server
* c_components - A rust crate containing C source
* c_components/tests - Tests for the C components
* imageflow_types - Shared types used by most crates, with JSON serialization
* imageflow_helpers - Common helper functions and utilities
* imageflow_riapi - RIAPI and ImageResizer4 compatibility parsing/layout
* imageflow_core - The main library and execution engine


### Known flaws and missing features (as of May 2020)

#### Flaws

- [ ] imageflow_server doesn't expose the JSON API yet.
- [ ] No fuzz testing or third-party auditing yet.

#### Missing features

- [ ] Blurring.

#### Delayed features

- [ ] Job cost prediction (delayed - no interest from community)

# Building from Source without Docker

You'll need more than just Rust to compile Imageflow, as it has a couple C dependencies.

1. **Install platform-specific prerequisites (find the right section below).**
2. Run `cargo install dssim`
3. Clone and cd into this repository
   E.g., `git clone git@github.com:imazen/imageflow.git && cd imageflow`)

If you are using `bash` on any platform, you should be able to use `build.sh`
* `./build.sh clean` - to clean
* `./build.sh test` - run all tests
* `./build.sh debug` - generate slow debug binaries
* `./build.sh release` - generate release binaries
* `./build.sh install` - install release binaries to `/usr/local` (must run `./build.sh release first)
* `./build.sh uninstall` - uninstall release binaries

`build.sh` places binaries in the `./artifacts/ directory`

If you are on Windows, only run build commands in the window created by `win_enter_env.bat`.

You can also build using `cargo` directly, although this will place binaries in `./target/release` instead.
    * `cargo test --all` to test Imageflow in debug (slooow) mode
    * `cargo build --package imageflow_abi --release` to compile `libimageflow/imageflow.dll`
    * `cargo build --package imageflow_tool --release` to compile `imageflow_tool(.exe)`
    * `cargo build --package imageflow_server --release` to compile `imageflow_server(.exe)`
    * `cargo build --all --release` to compile everything in release mode
    * `cargo doc --no-deps --all --release` to generate documentation.


## Building from Source with Docker
If you want to replicate the Imageflow CI environment:
1. [Install Docker](https://docs.docker.com/install/)
2. Run from a bash session ([Docker + Windows WSL](https://nickjanetakis.com/blog/setting-up-docker-for-windows-and-wsl-to-work-flawlessly), macOS, or linux)
3. ```bash
   git clone git@github.com:imazen/imageflow.git
   cd imageflow
   ./build_via_docker.sh debug
   ```

This will create caches within `~/.docker_imageflow_caches` specific to the docker image used. Instances will be ephemeral; the only state will be in the caches.

The [official Dockerfiles](https://github.com/imazen/dockerfiles_imageflow) are also a great place to get more detailed environment setup steps, as we don't list steps for setting up:
* Valgrind (common versions break openssl; you may need to build from source)
* Code coverage
* Bindings.

## Linux Pre-requisites

(tested on Ubuntu 16.04 and 18.04.)

```bash
#Install Rust 1.41+ by running
`curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain beta`
#Ensure build tools are installed (git, curl, wget, gcc, g++, nasm, pkg-config, openssl, ca-certificates)
`sudo apt-get install git wget curl build-essential pkg-config libssl-dev libpng-dev nasm `
```

## Mac OS Pre-requisites

1. Install [XCode Command-Line Tools](http://railsapps.github.io/xcode-command-line-tools.html) if you haven't already
2. Install [Homebrew](https://brew.sh/) if you haven't already.
3. Install nasm, pkg-config, and wget
   `brew install nasm pkg-config wget`
4. Install [Rust](https://www.rust-lang.org/en-US/install.html)


## Windows WSL (Ubuntu Bionic Subsystem) Pre-requisites

1. Install [Ubuntu 18.04 from the Windows Store](https://www.microsoft.com/store/productId/9N9TNGVNDL3Q)
2. Run Ubuntu 18.04 and create your username/password
3. `sudo apt-get update` to update available packages.
4. Install Rust 1.28+ by running
  `curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain beta`
5. Ensure build tools are installed (git, curl, wget, gcc, g++, nasm, pkg-config, openssl, ca-certificates)
    `sudo apt-get install git wget curl build-essential pkg-config libssl-dev libpng-dev nasm `
6. (optional) To use a graphical text editor, you'll need to download imageflow to a "Windows" directory, then map it to a location in Ubuntu.
   For example, if you cloned imageflow to Documents/imageflow, you would run:
   `ln -s /mnt/c/Users/[YourWindowsUserName]/Documents/imageflow ~/win_imageflow`
7. Close and re-open Ubuntu


## Windows 10 Pre-requisites

1. Install Visual Studio 2017 Build Tools ([separately](https://www.visualstudio.com/thank-you-downloading-visual-studio/?sku=BuildTools&rel=15) or as a VS component)
2. Install [Git 64-bit](https://git-scm.com/download/win).
3. `Run As Administrator` the [NASM 64-bit](https://www.nasm.us/pub/nasm/releasebuilds/2.14.02/win64/nasm-2.14.02-installer-x64.exe) installer - it will not prompt.
4. Install [Rust 64-bit](https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe) if you want 64-bit Imageflow or [Rust 32-bit](https://static.rust-lang.org/rustup/dist/i686-pc-windows-msvc/rustup-init.exe) if you don't.
Install toolchain `beta` as the default, and confirm adding it to `PATH`.
5. Open the command line and switch to this repository's root directory
5. Edit `ci/wintools/SETUP_PATH.bat` to ensure that rust/cargo, nasm, git, and Git/mingw64/bin are all in `%PATH%`.
7. Run `win_enter_env.bat` to start a sub-shell (edit it if you want a 32-bit build)
8. All build commands should be run in the sub-shell. Run `cmd.exe /c "ci\wintools\win_verify_tools.bat"` to check tools are present.

## How does one learn image processing for the web?

First, [read High Performance Images](http://shop.oreilly.com/product/0636920039730.do) for context.

There are not many great textbooks on the subject. Here are some from my personal bookshelf. Between them (and Wikipedia) I was able to put together about 60% of the knowledge I needed; the rest I found by reading the source code to [many popular image processing libraries](https://github.com/lilith/imaging-wiki?files=1).

I would start by reading [Principles of Digital Image Processing: Core Algorithms](http://www.amazon.com/gp/product/1848001940?psc=1&redirect=true&ref_=oh_aui_search_detailpage) front-to-back, then [Digital Image Warping](http://www.amazon.com/gp/product/0818689447?psc=1&redirect=true&ref_=oh_aui_search_detailpage).  Wikipedia is also a useful reference, although the relevant pages are not linked or categorized together - use specific search terms, like ["bilinear interpolation"](https://en.wikipedia.org/wiki/Bilinear_interpolation) and ["Lab color space"](https://en.wikipedia.org/wiki/Lab_color_space).

* [Digital Image Warping](http://www.amazon.com/gp/product/0818689447?psc=1&redirect=true&ref_=oh_aui_search_detailpage)
* [Computer Graphics: Principles and Practice in C (2nd Edition)](http://www.amazon.com/gp/product/0201848406?psc=1&redirect=true&ref_=oh_aui_search_detailpage)
* [Principles of Digital Image Processing: Fundamental Techniques](http://www.amazon.com/gp/product/1848001908?psc=1&redirect=true&ref_=oh_aui_search_detailpage)
* [Principles of Digital Image Processing: Core Algorithms](http://www.amazon.com/gp/product/1848001940?psc=1&redirect=true&ref_=oh_aui_search_detailpage)
* [Principles of Digital Image Processing: Advanced Methods](http://www.amazon.com/gp/product/1848829183?psc=1&redirect=true&ref_=oh_aui_search_detailpage)

I have found the source code for OpenCV, LibGD, FreeImage, Libvips, Pixman, Cairo, ImageMagick, stb_image, Skia, and FrameWave is very useful for understanding real-world implementations and considerations. Most textbooks assume an infinite plane, ignore off-by-one errors, floating-point limitations, color space accuracy, and operational symmetry within a bounded region. I cannot recommend any textbook  as an accurate reference, only as a conceptual starting point. [I made some notes regarding issues to be aware of when creating an imaging library](https://github.com/imazen/Graphics-vNext/blob/master/aware.md).

Also, keep in mind that computer vision is very different from image creation. In computer vision, resampling accuracy matters very little, for example. But in image creation, you are serving images to photographers, people with far keener visual perception than the average developer. The images produced will be rendered side-by-side with other CSS and images, and the least significant bit of inaccuracy is quite visible. You are competing with Lightroom; with offline tools that produce visually perfect results. End-user software will be discarded if photographers feel it is corrupting their work.
