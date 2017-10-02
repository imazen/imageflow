#![feature(alloc_system)]
#[allow(unused_extern_crates)]
extern crate alloc_system;
extern crate imageflow_tool_lib;
fn main() {
    let exit_code = imageflow_tool_lib::main_with_exit_code();
    std::process::exit(exit_code);
}
