extern crate cc;
extern crate glob;

use std::env;
use std::path::PathBuf;

fn main() {
    let mut cc = cc::Build::new();
    cc.warnings(false);

    for path in env::split_paths(&env::var_os("DEP_JPEG_INCLUDE").expect("include paths from mozjpeg-sys")) {
        cc.include(path);
    }
    for path in env::split_paths(&env::var_os("DEP_PNG_INCLUDE").expect("include paths from libpng-sys")) {
        cc.include(path);
    }
    for path in env::split_paths(&env::var_os("DEP_LCMS2_INCLUDE").expect("include paths from lcms2-sys")) {
        cc.include(path);
    }

    let test_root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let c_root = test_root.parent().unwrap();
    cc.include(c_root.join("lib"));
    cc.include(c_root);

    cc.include(&test_root);

    cc.define("imageflow_c_BUILD_STATIC", Some("1")); // -Dimageflow_c_BUILD_SHARED for DLL

    let mut cxx = cc.clone();
    cxx.cpp(true);

    if !cc.get_compiler().is_like_msvc() {
        cc.flag("-std=c11");
        cxx.flag("-std=c++11");
    }

    // C and C++ tests have to be compiled separately, otherwise C files get wrong symbols.
    cc.file(test_root.join("helpers.c"));
    cc.file(test_root.join("profile_imageflow.c"));
    cc.compile("imageflow_c_tests");

    // the C code wants __FILE__ to contain slashes

    cxx.file(test_root.join("test.cpp"));
    cxx.file(test_root.join("test_context.cpp"));
    cxx.file(test_root.join("test_error_handling.cpp"));
    cxx.file(test_root.join("test_operations.cpp"));
    cxx.file(test_root.join("test_variations.cpp"));
    cxx.file(test_root.join("test_weighting.cpp"));
    cxx.file(test_root.join("test_weighting_helpers.cpp"));
    cxx.file(test_root.join("runner.cpp"));
    cxx.compile("imageflow_cxx_tests");
}
