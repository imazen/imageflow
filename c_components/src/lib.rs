// These -sys crates are dependencies solely for native library linking.
// `extern crate` is required to force the linker to include their native libraries,
// since no Rust API is used from them (edition 2021 only auto-links crates that are referenced).
extern crate lcms2_sys;
extern crate libc;
extern crate libpng_sys;
extern crate libz_sys;
extern crate mozjpeg_sys;
