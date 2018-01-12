extern crate cc;
use std::env;
use std::path::PathBuf;

fn main() {
    let mut cc = cc::Build::new();
    cc.warnings(false);

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

    let mut cxx = cc.clone();

    cc.flag("-std=c11");
    cc.file(test_root.join("helpers.c"));
    cc.file(test_root.join("profile_imageflow.c"));
    cc.compile("imageflow_c_tests");

    cxx.cpp(true);
    cxx.flag("-std=c++11");

    // the C code wants __FILE__ to contain slashes
    cxx.file(test_root.join("runner.cpp"));
    cxx.file(test_root.join("test.cpp"));
    cxx.file(test_root.join("test_context.cpp"));
    cxx.file(test_root.join("test_error_handling.cpp"));
    cxx.file(test_root.join("test_integration.cpp"));
    cxx.file(test_root.join("test_io.cpp"));
    cxx.file(test_root.join("test_operations.cpp"));
    cxx.file(test_root.join("test_variations.cpp"));
    cxx.file(test_root.join("test_weighting.cpp"));
    cxx.file(test_root.join("test_weighting_helpers.cpp"));
    cxx.compile("imageflow_cxx_tests");
}
