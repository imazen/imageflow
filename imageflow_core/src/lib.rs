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

#[macro_use]
extern crate lazy_static;

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
use std::ptr;

#[derive(Debug, PartialEq)]
pub struct FlowErr {
    code: i32,
    message_and_stack: String,
}

#[derive(Debug, PartialEq)]
pub enum FlowError {
    ContextInvalid,
    Oom,
    Err(FlowErr),
    ErrNotImpl,
}
pub struct JsonResponse<'a> {
    pub status_code: i64,
    pub response_json: &'a [u8],
}


pub type Result<T> = std::result::Result<T, FlowError>;




