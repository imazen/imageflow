#[cfg_attr(feature = "cargo-clippy", allow(useless_attribute))]
extern crate imageflow_tool_lib;
fn main() {
    let exit_code = imageflow_tool_lib::main_with_exit_code();
    std::process::exit(exit_code);
}
