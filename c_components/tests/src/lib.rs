extern crate imageflow_c_components;

extern "C" {
    pub fn run_c_components_tests() -> std::os::raw::c_int;
    pub fn run_c_components_test_failure() -> std::os::raw::c_int;
    pub fn profile_main();
    pub fn keep1();
    pub fn keep2();
    pub fn keep3();
    pub fn keep4();
    pub fn keep5();
    pub fn keep6();
    pub fn keep7();
    pub fn keep8();
    pub fn keep9();
    pub fn keep10();
}

#[test]
fn c_catch() {
    unsafe {
        assert_eq!(0, run_c_components_tests());
    }
}

//// Uncomment if you're not seeing C test output
//#[test]
//fn test_failure_works() {
//    unsafe {
//        // If no failures are reported, then we're getting false positives
//        assert_eq!(2, run_c_components_test_failure());
//    }
//}
#[test]
fn test_prevent_lto_stripping() {
    unsafe {
        keep1();
        keep2();
        keep3();
        keep4();
        keep5();
        keep6();
        //keep7(); skipping test_simple_fastscaling
        keep8();
        keep9();
        keep10();
    }
}
