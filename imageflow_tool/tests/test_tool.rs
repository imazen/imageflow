extern crate imageflow_tool_lib;
use std::io::Write;

#[test]
fn run_imageflow_tool_self_test(){
    let self_path = std::env::current_exe().expect("For --self-test to work, we need to know the binary's location. env::current_exe failed");
    // back out of the 'deps' directory as well
    let tool = self_path.parent().unwrap().parent().unwrap().join("imageflow_tool");
    let _  = writeln!(std::io::stdout(), "Testing binary {:?}", &tool);
    imageflow_tool_lib::self_test::run(Some(tool.clone()));
    imageflow_tool_lib::self_test::test_capture(Some(tool));
}