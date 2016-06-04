extern crate cmake;
use cmake::Config;
use std::env;
  
fn main() {
  // OS X rpath makes dynamic linking quite terrible on my dev box. 
  // We want to statically link anyway
  // But that requires passing CMake flags through cargo so the same stdlib is used
  // everywhere

  //docs: http://alexcrichton.com/cmake-rs/cmake/index.html
  // Example
  //let dst = Config::new("libfoo")
  //              .define("FOO", "BAR")
  //              .cflag("-foo")
  //              .build_target("all")
  //              .build();
  // println!("cargo:rustc-link-search=native={}", dst.display());
  // println!("cargo:rustc-link-lib=static=foo");

  // Right now we dynamically link... and we fail, because we can't convince cargo 
  // to use a differnt rpath for 'cargo test' 

  let build_dir = env::current_dir().unwrap().join("../../build/lib").canonicalize().unwrap();
  println!("cargo:rustc-link-search=native={}", build_dir.to_str().unwrap() );
  println!("cargo:rustc-link-lib=gif");
  println!("cargo:rustc-link-lib=jpeg");
  println!("cargo:rustc-link-lib=lcms2");
  println!("cargo:rustc-link-lib=png16");
  println!("cargo:rustc-link-lib=simd");
  println!("cargo:rustc-link-lib=turbojpeg");
  println!("cargo:rustc-link-lib=z");
}
