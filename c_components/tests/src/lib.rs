extern crate imageflow_c_components;

extern "C" {
    pub fn run_c_components_tests() -> std::os::raw::c_int;
    pub fn profile_main();
}

#[test]
fn c_catch() {
    unsafe {
        assert!(0 == run_c_components_tests());
    }
}
