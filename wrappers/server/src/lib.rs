mod ffi;
#[test]
fn it_works() {
  unsafe{
    let  c = ffi::flow_context_create();
  }
}
