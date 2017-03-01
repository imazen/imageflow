extern crate imageflow_tool;
fn main() {
    let exit_code = imageflow_tool::main_with_exit_code();
    std::process::exit(exit_code);
}
