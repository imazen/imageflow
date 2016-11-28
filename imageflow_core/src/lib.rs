#![feature(alloc)]
#![feature(oom)]
#![feature(alloc_system)]
#![feature(conservative_impl_trait)]
#![feature(proc_macro)]

#![allow(unused_features)]
#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]

extern crate alloc_system;
extern crate petgraph;
extern crate daggy;
extern crate time;
extern crate imageflow_types as s;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate serde_derive;

extern crate serde_json;
extern crate serde;
pub mod ffi;
pub mod boring;
pub mod parsing;
mod json;
mod flow;
mod context;
pub mod clients;
pub use context::{Context, ContextPtr, Job, JobPtr, JobIo, JobIoPtr, SelfDisposingContextPtr};
pub use ::ffi::{IoDirection, IoMode};

pub use parsing::JsonResponseError;
use std::ops::DerefMut;

extern crate libc;
extern crate alloc;
use std::cell::RefCell;
use std::marker;
use std::borrow::Cow;
use std::ptr;

pub use json::JsonResponse;
pub use json::MethodRouter;

#[derive(Debug, PartialEq)]
pub struct FlowErr {
    pub code: i32,
    pub message_and_stack: String,
}

#[derive(Debug, PartialEq)]
pub enum FlowError {
    NullArgument,
    ContextInvalid,
    Oom,
    Err(FlowErr),
    ErrNotImpl,
}

pub type Result<T> = std::result::Result<T, FlowError>;


