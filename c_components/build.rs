extern crate cc;
extern crate glob;
use std::env;
use std::path::PathBuf;

fn main() {
    // Step 1: Initialize cc build and gather environment variables
    // Goal: Prepare the build, including detecting target architectures and CPU types.
    let mut cc = cc::Build::new();
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    cc.include(root.join("lib"));
    cc.include(&root);

    for path in env::split_paths(&env::var_os("DEP_JPEG_INCLUDE").expect("include paths from mozjpeg-sys")) {
        cc.include(path);
    }
    for path in env::split_paths(&env::var_os("DEP_PNG_INCLUDE").expect("include paths from libpng-sys")) {
        cc.include(path);
    }
    for path in env::split_paths(&env::var_os("DEP_LCMS2_INCLUDE").expect("include paths from lcms2-sys")) {
        cc.include(path);
    }

    // Step 2: Configure warnings, static build definition
    // Goal: Relax certain warning behaviors, set build type macros.
    // // fastapprox.h and bitmap_formats.c have warnings
    cc.warnings_into_errors(false);
    cc.define("imageflow_c_BUILD_STATIC", Some("1"));
    // -Dimageflow_c_BUILD_SHARED for DLL


    // Step 3: Retrieve target CPU and compiler, detect MSVC or GCC/Clang
    // Goal: Apply architecture-specific flags in an intelligent way.
    let target_cpu = env::var("TARGET_CPU");
    let comp = cc.get_compiler();
    let target_triple = env::var("TARGET").unwrap_or_default();

    // Step 4: Apply MSVC-specific flags if applicable
    // Goal: For MSVC, we can set SSE/AVX etc. but skip them if it's arm64.
    if comp.is_like_msvc() {
        if let Ok(arch) = target_cpu {
            // Reasoning: Skip SSE/AVX flags if architecture is arm64 or aarch64.
            if !(target_triple.contains("aarch64") || target_triple.contains("arm64")) {
                if arch == "haswell" {
                    cc.flag("/arch:AVX2");
                }
                if arch == "sandybridge" {
                    cc.flag("/arch:AVX");
                }
            }
        }
        cc.flag("/fp:fast");
    } else {
        // Step 5: Apply GCC/Clang flags
        // Goal: If the architecture indicates aarch64/arm64, skip SSE flags.
        if let Ok(cpu) = target_cpu {
            cc.flag_if_supported(format!("-march={}", cpu));
            cc.flag("-funroll-loops");
            cc.flag("-ffast-math");

            // Only apply SSE if NOT aarch64/arm64
            if !(target_triple.contains("aarch64") || target_triple.contains("arm64")) {
                cc.flag("-mfpmath=sse");
                cc.flag("-msse2");
                if cpu == "haswell" {
                    cc.flag("-mavx2");
                }
                if cpu == "sandybridge" {
                    cc.flag("-mavx");
                }
            }
        }

        // Additional common flags
        cc.flag("-std=gnu11");
        cc.flag_if_supported("-pipe");
        cc.flag("-Wpointer-arith");
        cc.flag("-Wcast-qual");
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


    // Step 7: Gather source files, conditionally excluding graphics.c if not c_rendering
    // Goal: Add each .c file to the build, except for the optional skip.
    for file in glob::glob("lib/*.c").unwrap() {
        let path = file.unwrap();
        cc.file(path);

    }

    // Step 8: Optional coverage and profiling flags
    // Goal: Provide build instrumentation if features are enabled.
    if cfg!(feature = "coverage") {
        cc.flag("--coverage");
        cc.debug(true);
        cc.opt_level(0);
    }

    // Step 9: Compile the library
    // Goal: Produce the final static library.
    cc.compile("imageflow_c");
}
