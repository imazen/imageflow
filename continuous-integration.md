# Designing continuous integration for Imageflow

### category A platforms

These are the platforms we build on. All are 64-bit, but may or may not compile/test in 32-bit compatibility as well.

* Windows 2012 R2 with VS2015 (As configured on [AppVeyor](https://www.appveyor.com/docs/installed-software) when using `os: Visual Studio 2015`). We cross-compile 32-bit, and run 32-bit tests as well, but as a separate job. 
* OS X 10.10 with xcode 7.1.1, OS X 10.11 with xcode 7.3.1, and OS X 10.11 with xcode 8beta4. These correspond to `xcode7.1`, `xcode7.3`, and `xcode8` on [travis's OS X environments](https://docs.travis-ci.com/user/osx-ci-environment/#OS-X-Version). 
* 64-bit Ubuntu Xenial/GCC5.3, wily/gcc5.2, trusty/gcc4.8, vivid/gcc4.9, currently using [these docker images](https://github.com/lasote/conan-docker-tools). 

### category B platforms

These are the platforms we build for.

Linux, Mac, and Windows
...todo: be more specific as data allows...

### Primary goals

* Run all tests in both C and Rust code, on all category A platforms, providing clear failure detail for local repro. 
* Create binaries for Linux, Mac, and Windows, upload them somewhere, and update something to link to them. 
* Binaries include (flow-proto1, imageflow-tool\*, imageflow-server\*, libimageflow\* (cdylib), libimageflow\* (static lib))
* Our binaries should be fast. Released binaries should target the best instruction sets possible.
* We should test all code paths with valgrind
* We should calculate test coverage for both Rust and C (coveralls/lcov) parts of the code.
* We should compile using both Stable and Nightly rust


### Secondary goals

* Generate documentation and publish to two locations/channels - stable and nightly. 
* Versioned stable releases and documentation sets should be accessible indefinitely. 
* We should push our Rust packages to Crates.io
* We should push our cdylibs to Conan.io for C/C++ consumers, and NuGet\* for .NET consumers. (both stable and nightly channels)

Items with a star are blocked, but are expected to be unblocked. 


### Constraints

* In order to use Valgrind on Rust executables or tests (as jemalloc [dropped valgrind support](https://github.com/jemalloc/jemalloc/issues/369)) , one must use the [nightly version of the Rust compiler (possibly built with custom flags)](https://github.com/rust-lang/rust/issues/28224#issuecomment-138725566), or expose the tests as a static library (system allocator) which is invoked by a C wrapper executable.  
* Docker can only be used on x64 Travis. 
* We must test against both Rust nightly and Rust stable. 

### Acceptable compromises

* We can target a base assumption of AVX & SSE4.2 instruction support for public binary releases. Old (2008-era) and cheap < $50 CPUs may not support these, but we can also say that.
* No 32-bit artifacts for linux
* No 32-bit artifacts for mac
* No ARM builds
* No testing of mingw/gcc on Windows; MS CRT only.
* Security through obscurity for binaries during the Kickstarter early access period. 
* To reduce permutations, we can skip testing against the Rust Beta channel, although we shouldn't.
* We can move particularly slow things (like valgrind testing) to be conditional upon branch name, and create a branch 'slow-tests' which we push to manually, periodically.


### Resources

* AppVeyor has described a method for [deploy.yml](https://www.appveyor.com/blog/2015/11/04/deployment-projects) in order to separate test and deploy jobs. This means a separate status badge for 'successfully deployed' can be used, since you use a separate branch to trigger said deployment. 

