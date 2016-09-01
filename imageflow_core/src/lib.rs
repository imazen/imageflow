#![feature(alloc)]
#![feature(oom)]

#![allow(unused_features)]
#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]


pub mod ffi;
pub mod boring;
pub mod parsing;


#[macro_use]
extern crate json;
extern crate libc;
extern crate alloc;

use std::marker;
use std::ptr;
use std::cell::RefCell;


struct ContextPtr {
    ptr: Option<*mut ::ffi::Context>
}
pub struct Context{
    p: RefCell<ContextPtr>
}

struct JobPtr{
    ptr: Option<*mut ::ffi::Job>
}

pub struct Job{
    p: RefCell<JobPtr>
}

struct JobIoPtr{
    ptr: Option<*mut ::ffi::JobIO>
}

pub struct JobIo<'a, T: 'a>{
    p: RefCell<JobIoPtr>,
    _marker: marker::PhantomData<&'a T>
}

#[derive(Debug, PartialEq)]
pub enum FlowError {
    ContextInvalid,
    Oom,
    ErrNotImpl

}

pub type Result<T> = std::result::Result<T, FlowError>;

impl ContextPtr {
    fn destroy(&mut self){
        unsafe {
            self.ptr = match self.ptr{
                Some(ptr) => {
                    ::ffi::flow_context_destroy(ptr);
                    None
                }
                _ => None
            }
        }
    }
}
impl Drop for Context {
    fn drop(&mut self) {
        (*self.p.borrow_mut()).destroy();
    }
}
impl Context {
    pub fn create() -> Context {
        unsafe {
            let ptr = ::ffi::flow_context_create();

            if ptr.is_null() {
                Context {
                    p: RefCell::new(ContextPtr { ptr: None }),
                }
            } else {
                Context {
                    p: RefCell::new(ContextPtr { ptr: Some(ptr) }),
                }
            }
        }
    }

    fn get_error_copy(&self) -> Option<FlowError> {
        unsafe {
            match (*self.p.borrow()).ptr {
                Some(ptr) if ::ffi::flow_context_has_error(ptr) => Some(FlowError::ErrNotImpl),
                None => Some(FlowError::ContextInvalid),
                Some(_) => None
            }
        }
    }

    pub fn destroy(self) -> Result<()> {
        let ref mut b = *self.p.borrow_mut();
        match b.ptr {
            None => Ok(()),
            Some(ptr) => unsafe {
                if !::ffi::flow_context_begin_terminate(ptr) {
                    //Already borrowed; will panic!
                    //This kind of bug is only exposed at runtime, now.
                    //Code reuse will require two copies of every function
                    //One against the ContextPtr, to be reused
                    //One exposed publicly against the Context, which performs the borrowing
                    //Same scenario will occur with other types.
                    let copy = self.get_error_copy().unwrap();
                    b.destroy();
                    Err(copy)
                } else {
                    b.destroy();
                    Ok(())
                }
            }
        }
    }

    pub fn create_job(&mut self) -> Result<Job> {
        let ref b = *self.p.borrow_mut();
        match b.ptr {
            None => Err(FlowError::ContextInvalid),
            Some(ptr) => unsafe {
                let p = ::ffi::flow_job_create(ptr);
                if p.is_null() {
                    Err(FlowError::Oom)
                } else {
                    Ok(Job { p: RefCell::new(JobPtr { ptr: Some(p) }) })
                }
            }
        }
    }


    pub fn create_io_from_slice<'a, 'c>(&'c mut self, bytes: &'a [u8]) -> Result<JobIo<'a, &'a [u8]>> {
        let ref b = *self.p.borrow_mut();
        match b.ptr {
            None => Err(FlowError::ContextInvalid),
            Some(ptr) => unsafe {
                let p = ::ffi::flow_io_create_from_memory(ptr, ::ffi::IoMode::read_seekable, bytes.as_ptr(), bytes.len(), ptr as *const libc::c_void, ptr::null());
                if p.is_null() {
                    Err(FlowError::Oom)
                } else {
                    Ok(JobIo{ _marker: marker::PhantomData, p: RefCell::new(JobIoPtr { ptr: Some(p) }) })
                }
            }
        }
    }

}

#[test]
fn it_works() {
    let mut c = Context::create();

    let j = c.create_job().unwrap();

    let j2 = c.create_job().unwrap();


    let j3 = c.create_job().unwrap();

    let bytes: [u8;3] = [2,3,4];

    let i1 = c.create_io_from_slice(&bytes).unwrap();

    assert_eq!(c.destroy(), Ok(()));

}

//pub struct FlowIoRef{
//    ptr: *mut ::ffi::JobIO
//}






pub struct JsonResponse<'a>{
    pub status_code: i64,
    pub response_json: &'a [u8]
}

impl Context{
    pub fn message(&self, method: &str, json: &[u8]) -> JsonResponse{

        match method {
            "teapot" => JsonResponse {
                status_code: 418,
                response_json:
                r#"{"success": "false","code": 418,"message": "I'm a teapot, short and stout"}"#
                    .as_bytes()
            },
            _ => JsonResponse {
                status_code: 404,
                response_json: r#"{
                                        "success": "false",
                                        "code": 404,
                                        "message": "Method not understood"}"#.as_bytes()
            }
        }
    }
}
//
//impl SpeakJson for FlowContext{
//    fn message(&self, method: &str, json: &str) -> &JsonResponse{
//
//    }
//}


//
//pub struct FlowCtx{
//    ptr: *mut ::ffi::Context,
//
//}
//impl FlowCtx {
//    pub fn from_ptr(ptr: *mut ::ffi::Context) -> FlowCtx{
//        FlowCtx{ptr: ptr}
//    }
//}
//
//
//
//struct FlowIo{
//
//}




//#[test]
//fn test_panics(){
//    let result = ::std::panic::catch_unwind(|| {
//        panic!("oh no!");
//    });
//
//    if let Err(err) = result {
//        let str = format!("{:?}", err.downcast::<&'static str>());
//        assert_eq!(str, "");
//    }
//}

#[test]
fn test_panics2(){
   // let input_bytes = [0u8;3000000];
//    let result = ::std::panic::catch_unwind(|| {
//        let input_bytes = [2u8;10 * 1024 * 1024 * 1024];
//    });

//    if let Err(err) = result {
//        let str = format!("{:?}", err.downcast::<&'static str>());
//        assert_eq!(str, "");
//    }
}


fn new_oom_handler() -> ! {
    panic!("OOM");
}

#[allow(unused_variables)]
#[test]
fn test_panics3(){

    alloc::oom::set_oom_handler(new_oom_handler);

    // let input_bytes = [0u8;3000000];
    let result = ::std::panic::catch_unwind(|| {
        let b = vec![0;30 * 1024 * 1024 * 1024];
    });

    if let Err(err) = result {
        let str = format!("{:?}", err.downcast::<&'static str>());
        assert_eq!(str, "Ok(\"OOM\")");
    }
}