extern crate imageflow_tool;

#[test]
fn run_imageflow_tool_self_test(){
    let self_path = std::env::current_exe().expect("For --self-test to work, we need to know the binary's location. env::current_exe failed");
    let tool = self_path.parent().unwrap().join("imageflow_tool");
    imageflow_tool::self_test::run(Some(tool.clone()));
    imageflow_tool::self_test::test_capture(Some(tool));
}