pub mod ffi;
pub mod boring;
pub mod parsing;

#[macro_use]
extern crate json;



#[test]
fn it_works() {
    unsafe {
        let c = ffi::flow_context_create();
        assert!(!c.is_null());

    }
}
