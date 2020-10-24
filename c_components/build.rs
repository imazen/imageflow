extern crate cc;
extern crate glob;
use std::env;
use std::path::PathBuf;

fn main() {
    let mut cc = cc::Build::new();

    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    cc.include(root.join("lib"));
    cc.include(root);

    for path in env::split_paths(&env::var_os("DEP_JPEG_INCLUDE").expect("include paths from mozjpeg-sys")) {
        cc.include(path);
    }
    for path in env::split_paths(&env::var_os("DEP_PNG_INCLUDE").expect("include paths from libpng-sys")) {
        cc.include(path);
    }
    for path in env::split_paths(&env::var_os("DEP_LCMS2_INCLUDE").expect("include paths from lcms2-sys")) {
        cc.include(path);
    }

    cc.warnings_into_errors(false);  // fastapprox.h and bitmap_formats.c have warnings

    cc.define("imageflow_c_BUILD_STATIC", Some("1")); // -Dimageflow_c_BUILD_SHARED for DLL

    let target_cpu = env::var("TARGET_CPU");
    let comp = cc.get_compiler();
    if comp.is_like_msvc() {
        if let Ok(arch) = target_cpu {
            if arch == "haswell" { // opt
                cc.flag("/arch:AVX2");
            }
            if arch == "sandybridge" { // opt
                cc.flag("/arch:AVX");
            }
        }
        cc.flag("/fp:fast"); // opt
    } else {
        if let Ok(cpu) = target_cpu { // opt
            cc.flag_if_supported(&format!("-march={}", cpu));

            cc.flag("-funroll-loops");
            cc.flag("-ffast-math");
            cc.flag("-mfpmath=sse");
            cc.flag("-msse2");
            if cpu == "haswell" {
                cc.flag("-mavx2");
            }
            if cpu == "sandybridge" {
                cc.flag("-mavx");
            }
        }

        cc.flag("-std=gnu11");
        cc.flag_if_supported("-pipe");

        cc.flag("-Wpointer-arith");
        cc.flag("-Wcast-qual");
        // cc.flag("-Wpedantic"); // jpeglib.h doesn't pass
        cc.flag("-Wno-unused-parameter");
        cc.flag("-Wuninitialized");
        cc.flag("-Wredundant-decls");
        cc.flag("-Wno-error=unused-function");
        cc.flag("-Wno-parentheses");
        cc.flag("-Wstrict-prototypes");
        cc.flag("-Wmissing-prototypes");
        cc.flag("-Wshadow");
        cc.flag("-Wc++-compat");
    }

    //let skipped = PathBuf::from("lib/graphics.c");
    for file in glob::glob("lib/*.c").unwrap() {
        let path = file.unwrap();
        //if path != skipped {
            cc.file(path);
        //}
    }

    if cfg!(feature = "coverage") {
        cc.flag("--coverage");
        cc.debug(true);
        cc.opt_level(0);
    }

    if cfg!(feature = "profiling") {
        cc.flag("-pg");
        cc.file("tests/profile_imageflow.c");
        cc.file("tests/helpers.c");
    }

    cc.compile("imageflow_c");
}
