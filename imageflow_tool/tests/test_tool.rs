#[cfg_attr(feature = "cargo-clippy", allow(useless_attribute))]
extern crate imageflow_tool_lib;
extern crate imageflow_types as s;
use std::io::Write;
use std::path::{Path,PathBuf};


fn build_dirs() -> Vec<PathBuf>{
    let target_triple = crate::s::version::get_build_env_value("TARGET").expect("TARGET triple required");
    let profile = crate::s::version::get_build_env_value("PROFILE").expect("PROFILE (debug/release) required");


    let target_dir = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("target");

    let a = target_dir.join(target_triple).join(profile);
    let b = target_dir.join(profile);
    vec![a,b]
}
#[cfg(windows)]
fn binary_ext() -> &'static str{
    "exe"
}
#[cfg(not(windows))]
fn binary_ext() -> &'static str{
    ""
}

fn locate_binary(name: &str) -> Option<PathBuf> {
    for dir in build_dirs() {
        let file_path = dir.join(name).with_extension(binary_ext());

        if file_path.exists() {
            return Some(dir.join(name))
        }
    }
    None
}


fn tool_path() -> PathBuf {
    match locate_binary("imageflow_tool"){
        Some(v) => v,
        None => {
            panic!("Failed to locate imageflow_tool binary in {:?}", build_dirs());
        }
    }
}


#[test]
fn run_imageflow_tool_self_test(){
    let tool = tool_path();
    let _  = writeln!(std::io::stdout(), "Testing binary {:?}", &tool);
    imageflow_tool_lib::self_test::run(Some(tool.clone()));
    imageflow_tool_lib::self_test::test_capture(Some(tool));
}
