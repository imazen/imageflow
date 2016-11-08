#![feature(alloc)]
#![feature(oom)]
#![feature(alloc_system)]
#![feature(conservative_impl_trait)]

#![allow(unused_features)]
#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]

extern crate alloc_system;
extern crate petgraph;
extern crate daggy;
extern crate time;
extern crate imageflow_serde as s;

#[macro_use]
extern crate lazy_static;

extern crate serde_json;
pub mod ffi;
pub mod boring;
pub mod parsing;
pub mod abi;
mod flow;
mod context;
pub use context::{Context, ContextPtr, Job, JobPtr, JobIo, JobIoPtr};
pub use ::ffi::{IoDirection, IoMode};

pub use parsing::JsonResponseError;
use std::ops::DerefMut;

#[macro_use]
extern crate json;
extern crate libc;
extern crate alloc;
use std::cell::RefCell;
use std::marker;
use std::borrow::Cow;
use std::ptr;

#[derive(Debug, PartialEq)]
pub struct FlowErr {
    code: i32,
    message_and_stack: String,
}

#[derive(Debug, PartialEq)]
pub enum FlowError {
    NullArgument,
    ContextInvalid,
    Oom,
    Err(FlowErr),
    ErrNotImpl,
}
pub struct JsonResponse<'a> {
    pub status_code: i64,
    pub response_json: Cow<'a,[u8]>,
}


pub type Result<T> = std::result::Result<T, FlowError>;

impl<'a> JsonResponse<'a> {

    fn from_response001(r: s::Response001) -> JsonResponse<'a> {
        JsonResponse {
            status_code: r.code,
            response_json: Cow::Owned(serde_json::to_vec_pretty(&r).unwrap())
        }
    }
    fn success_with_payload(r: s::ResponsePayload) -> JsonResponse<'a> {
        let r = s::Response001{ success: true, code: 200,
            message: Some("OK".to_owned()),
            data: r};
        JsonResponse {
            status_code: r.code,
            response_json: Cow::Owned(serde_json::to_vec_pretty(&r).unwrap())
        }
    }


    fn ok() -> JsonResponse<'a> {
        JsonResponse {
            status_code: 200,
            response_json:
            Cow::Borrowed( r#"{"success": "true","code": 200,"message": "OK"}"#
                .as_bytes())
        }
    }
    fn teapot() -> JsonResponse<'a> {
        JsonResponse {
            status_code: 418,
            response_json: /* HTTP 418 I'm a teapot per RFC 2324 */
            Cow::Borrowed(r#"{"success": "false","code": 418, "message": "I'm a little teapot, short and stout..."}"#
                .as_bytes())
        }
    }
    fn method_not_understood() -> JsonResponse<'a>{
        JsonResponse {
            status_code: 404,
            response_json: Cow::Borrowed(r#"{
                                        "success": "false",
                                        "code": 404,
                                        "message": "Endpoint name not understood"}"#
                .as_bytes())
        }
    }
}



