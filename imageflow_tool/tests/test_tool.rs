extern crate imageflow_tool_lib;
use std::io::Write;
use std::path::PathBuf;

#[test]
fn run_imageflow_tool_self_test(){
    let tool = PathBuf::from(env!("CARGO_BIN_EXE_imageflow_tool"));
    let _  = writeln!(std::io::stdout(), "Testing binary {:?}", &tool);
    imageflow_tool_lib::self_test::run(Some(tool.clone()));
    imageflow_tool_lib::self_test::test_capture(Some(tool));
}
