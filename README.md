## ![imageflow](https://www.imageflow.io/images/imageflow.svg) optimal images at incredible speeds

[![tests](https://github.com/imazen/imageflow/workflows/Test/badge.svg?branch=main)](https://github.com/imazen/imageflow/actions?query=workflow%3ATest)  [![state: release candidate](https://img.shields.io/badge/state-release%E2%80%93candidate-yellow.svg)](#flaws)

[![Docker Pulls](https://img.shields.io/docker/pulls/imazen/imageflow_tool.svg)](https://hub.docker.com/r/imazen/imageflow_tool/)
[![view releases](https://img.shields.io/badge/-download%20binaries%20for%20windows,%20mac,%20or%20linux-green.svg)](https://github.com/imazen/imageflow/releases) [![license: Choose AGPLv3 or Commercial](https://img.shields.io/badge/license-Choose%20AGPLv3%20or%20Commercial-green.svg)](https://imageresizing.net/pricing)


[Download](https://github.com/imazen/imageflow/releases) blazing fast and [safer](https://cve.mitre.org/cgi-bin/cvekey.cgi?keyword=imagemagick) tools for a modern image workflow.


* **`imageflow_tool`** is a command-line tool for experimenting, running batch jobs, JSON jobs,
or when you want process isolation. Up to 17x faster than ImageMagick. Also produces smaller files at higher quality. [Download](https://github.com/imazen/imageflow/releases) or `docker run imazen/imageflow_tool`.
* **`libimageflow`** is for direct (in-process) use from *your* programming language. See our [**Node bindings**](https://github.com/imazen/imageflow-node),  [**Go bindings**](https://github.com/imazen/imageflow-go), [**Scala bindings**](https://github.com/Dealermade/imageflow-scala), [**Elixir bindings**](https://github.com/naps62/imageflow_ex), or  [**.NET bindings**](https://github.com/imazen/imageflow-dotnet). If we don't already have bindings for your language, consider spending a day to add them. Imageflow has a simple
  C-compatible ABI, of which only 4 methods are needed to implement bindings. 
* **[Imageflow.Server](https://github.com/imazen/imageflow-dotnet-server)** is cross-platform and can manipulate images in-flight (e.g.`/bucket/img.jpg?w=200`) for direct use from
HTML. Source images can reside in blob storage, on another server, or on the filesystem. It's a production ready server with excellent hybrid disk caching, support for Azure and Amazon blob storage, and can be easily customized. You can deploy it easily via Docker, on a VM, or via any cloud host. It's also backwards compatible with the ImageResizer API - which is useful, as ImageResizer as been integrated into more than a thousand different CMSes and applications in the last decade.

**[Open an issue](https://github.com/imazen/imageflow/issues/new) to share ideas, feedback, or ask questions. We believe in feedback-driven design, and streamlining real-world usage is the fastest way to a great product.**

[Querystring API Documentation](https://docs.imageflow.io/querystring/introduction.html)

[JSON API Documentation](https://docs.imageflow.io/json/introduction.html) 

libimageflow and  imageflow_tool are available as
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

## Using Imageflow.Server for dynamic imaging

NOTE: imageflow_server has been removed as the underlying web framework (iron) is abandoned and no longer secure. For the last few years we have suggested moving to the production-ready [Imageflow.Server product](https://github.com/imazen/imageflow-dotnet-server), which also offers docker deployment (but we suggest your own dockerfile to permit configuration)

Now you can edit images from HTML... and use srcset without headache.

```
<img src="http://localhost:39876/demo_images/u3.jpg?w=300" />

<img src="" srcset="    http://localhost:39876/demo_images/u3.jpg?w=300 300w
                        http://localhost:39876/demo_images/u3.jpg?w=800 800w
                        http://localhost:39876/demo_images/u3.jpg?w=1600 1600w" />

```

![](https://www.imageflow.io/images/imageflow-responsive.svg) ![](https://www.imageflow.io/images/edit-url.gif)


## Using libimageflow from your language

![](https://www.imageflow.io/images/libimageflow-direct.svg)

* .NET Standard bindings can be found at https://github.com/imazen/imageflow-dotnet
* Node bindings available  at https://github.com/imazen/imageflow-node
* Ruby - Basic bindings can be found in [bindings/ruby/](https://github.com/imazen/imageflow/tree/main/bindings/ruby)
* C and C++ interface is stable - use [bindings/headers/imageflow_default.h](https://github.com/imazen/imageflow/blob/main/bindings/headers/imageflow_default.h) or one of the many alternate conventions provided with each release.
* Rust - Imageflow is written in Rust, so you can use the `imageflow_core` crate, althogh the interfaces are not stable or semver in line with tagged releases (those version numbers are for the C ABI, not the Rust API)
* other languages - Use an [FFI](https://en.wikipedia.org/wiki/Foreign_function_interface) binding-generation tool for your language, and feed it whichever [header file it likes best](https://github.com/imazen/imageflow/tree/main/bindings/headers).

You also may find that `imageflow_tool` is quite fast enough for your needs.

### Crates within this project

* imageflow_abi - The stable API of libimageflow/imageflow.dll.
  Headers for libimageflow are located in `bindings/headers`
* imageflow_tool - The command-line tool
* c_components - A rust crate containing C source
* c_components/tests - Tests for the C components
* imageflow_types - Shared types used by most crates, with JSON serialization
* imageflow_helpers - Common helper functions and utilities
* imageflow_riapi - RIAPI and ImageResizer4 compatibility parsing/layout
* imageflow_core - The main library and execution engine

# Building from Source without Docker

You'll need more than just Rust to compile Imageflow, as it has a couple C dependencies.

1. **Install platform-specific prerequisites (find the right section below).**
2. Clone and cd into this repository
   E.g., `git clone git@github.com:imazen/imageflow.git && cd imageflow`)

3. Run `cargo build --release --all`
4. Look in `./target/release` for the binaries

If you are on Windows, only run build commands in the window created by `win_enter_env.bat`.

### Build using `cargo` directly, although this will place binaries in `./target/release` instead.
    * `cargo test --all` to test Imageflow in debug (slooow) mode
    * `cargo build --package imageflow_abi --release` to compile `libimageflow/imageflow.dll`
    * `cargo build --package imageflow_tool --release` to compile `imageflow_tool(.exe)`
    * `cargo build --all --release` to compile everything in release mode
    * `cargo doc --no-deps --all --release` to generate documentation.


## Linux Pre-requisites

(tested on Ubuntu 20.04 and 22.04.)

```bash
#Install Rust by running
curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain stable
#Ensure build tools are installed (git, curl, wget, gcc, g++, nasm, pkg-config, openssl, ca-certificates)
sudo apt-get install --no-install-recommends -y \
  sudo build-essential nasm dh-autoreconf pkg-config ca-certificates \
  git zip curl libpng-dev libssl-dev wget libc6-dbg  \
  libcurl4-openssl-dev libelf-dev libdw-dev apt-transport-https
```
## Mac OS Pre-requisites

1. Install [XCode Command-Line Tools](http://railsapps.github.io/xcode-command-line-tools.html) if you haven't already
2. Install [Homebrew](https://brew.sh/) if you haven't already.
3. Install nasm, pkg-config, and wget
   `brew install nasm pkg-config wget`
4. Install [Rust](https://www.rust-lang.org/en-US/install.html)


## Windows WSL (Ubuntu) Pre-requisites

1. Install Ubuntu from the Windows Store
2. Run Ubuntu 22.04 and create your username/password
3. `sudo apt-get update` to update available packages.
4. Install Rust by running
  `curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain stable`
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
Install toolchain `stable` as the default, and confirm adding it to `PATH`.
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
