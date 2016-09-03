# libimageflowrs

Exposes an ABI for other languages; this is the new 'libimageflow'. 

### Testing out the interface a teeny bit

`cargo test` does the most, and tests correctness,

`ruby imageflow_cdylib/src/test.rb` (if the FFI gem is installed) will actually make FFI calls happen, in case you're skeptical of symbol name mangling et. al.

### Documentation

Run `cargo doc --no-deps` and look in `target/doc`

### Generating the dynamic library

Run `cargo build --release` and look for `target/release/libimageflowrs`


### This will eventually be renamed to libimageflow
 
Right now, 'libimageflow' is overloaded, as libimageflow.a/dll/dylib is the C portion of the library.

