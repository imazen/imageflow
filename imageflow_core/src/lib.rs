#![feature(alloc)]
#![feature(oom)]
#![feature(alloc_system)]
#![feature(conservative_impl_trait)]
#![feature(proc_macro)]
#![feature(integer_atomics)]

//intellij-rust flags this anyway
//#![feature(field_init_shorthand)]

#![allow(unused_features)]
#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]

extern crate alloc_system;
extern crate petgraph;
extern crate daggy;
extern crate time;
extern crate imageflow_types;
#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate serde;
extern crate libc;
extern crate alloc;
extern crate rustc_serialize;

mod json;
pub mod flow;
mod context;
mod context_methods;
mod job_methods;

pub use json::JsonResponse;
pub use json::MethodRouter;
pub use context::{Context, ContextPtr, Job, JobPtr, JobIo, JobIoPtr, SelfDisposingContextPtr};
pub use ::ffi::{IoDirection, IoMode};
pub use ::flow::definitions::Graph;
//use std::ops::DerefMut;
pub mod clients;
pub mod ffi;
pub mod boring;
pub mod parsing;




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


mod internal_prelude{
    pub mod external{
        pub use std::path::{PathBuf,Path};
        pub use std::fs::File;
        pub use std::io::prelude::*;
        pub use std::cell::RefCell;
        pub use std::borrow::Cow;
        pub use std::ffi::{CString,CStr};
        pub use std::str::FromStr;
        pub use std::ascii::AsciiExt;
        pub use std::collections::{HashSet,HashMap};
        pub use daggy::{Dag, EdgeIndex, NodeIndex};
        pub use std::{ptr,marker,slice,cell,io,string,fmt,mem};
        pub use libc::{c_void, c_float, int32_t, int64_t, size_t, uint32_t};
        pub extern crate std;
        pub extern crate daggy;
        pub extern crate petgraph;
        pub extern crate serde;
        pub extern crate serde_json;
        pub extern crate time;
        pub extern crate libc;
        pub extern crate imageflow_types as s;
    }
    pub mod works_everywhere{
        pub use ::internal_prelude::external::*;
        pub use ::{FlowError,FlowErr,Result,flow,clients};
    }
    pub mod default{
        pub use ::internal_prelude::works_everywhere::*;
        pub use ::{Graph,ContextPtr,JobPtr,JobIoPtr,SelfDisposingContextPtr, JsonResponse, MethodRouter};
    }
    pub mod c_components{

    }
}