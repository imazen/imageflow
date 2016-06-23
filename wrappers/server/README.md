# imageflow/rust Playgroud

So, first thing: THESE ARE THROW-AWAY PROTOTYPES. Don't use them in production, please. Test coverage sucks and we're validating theories & testing dependencies right now. 

Feel free to have fun playing around with the examples, though. 

Remember to run `conan install` before `cargo build`, as we need it to generate `conan_cargo_build.rs`. 

1. flow-proto1 - This is a jpeg scaling tool written so users can verify quality/performance claims for Imageflow's most expensive operation (jpeg decode/scale/encode). 
2. bench.sh - This is a benchmark to compare flow-proto1 and libvips/ImageMagick. Please ping us if you can't [reproduce these benchmarks](https://gist.github.com/nathanaeljones/3c8e3600bfd5e440ecde670239d366dd). 
3. imageflow-server - This is a learning experiment; a throwaway proxy for testing the HTTP libraries we plan to use. It's a saturday afternoon exercise, not a real product. It's not safe (nor really safer than a C equivalent), and we're not using Rust idomatically or correctly. Nothing is re-entrant, and errors panic the process.  Run with `cargo run --bin imageflow-server`, then open in your browser: http://localhost:3000/proto1/scale_unsplash_jpeg/1200/1200/photo-1436891678271-9c672565d8f6
