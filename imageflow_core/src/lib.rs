#![feature(alloc)]
#![feature(oom)]
#![feature(alloc_system)]
#![feature(conservative_impl_trait)]
#![feature(proc_macro)]
#![feature(integer_atomics)]
#![feature(plugin)]

#![plugin(clippy)]
// intellij-rust flags this anyway
// #![feature(field_init_shorthand)]


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

pub use context::{ContextPtr, JobPtr, SelfDisposingContextPtr};
pub use ::ffi::{IoDirection, IoMode};
pub use ::flow::definitions::Graph;
pub use json::JsonResponse;
pub use json::MethodRouter;
// use std::ops::DerefMut;
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
    GraphCyclic,
    InvalidConnectionsToNode{index: usize, value: ::flow::definitions::NodeParams, message: String},
    ContextInvalid,
    Oom,
    Err(FlowErr),
    ErrNotImpl,
}

pub type Result<T> = std::result::Result<T, FlowError>;


mod internal_prelude {
    pub mod external_without_std {
        pub use daggy::{Dag, EdgeIndex, NodeIndex};
        pub use libc::{c_void, c_float, int32_t, int64_t, size_t, uint32_t};
        pub use std::{ptr, marker, str, slice, cell, io, string, fmt, mem};
        pub use std::ascii::AsciiExt;
        pub use std::borrow::Cow;
        pub use std::cell::RefCell;
        pub use std::collections::{HashSet, HashMap};
        pub use std::ffi::{CString, CStr};
        pub use std::fs::{File, create_dir_all};
        pub use std::io::BufWriter;
        pub use std::io::prelude::*;
        pub use std::path::{PathBuf, Path};
        pub use std::str::FromStr;
        pub extern crate daggy;
        pub extern crate petgraph;
        pub extern crate serde;
        pub extern crate serde_json;
        pub extern crate time;
        pub extern crate libc;
        pub extern crate imageflow_types as s;
    }
    pub mod imageflow_core_all {
        pub use ::{Graph, ContextPtr, JobPtr, SelfDisposingContextPtr, JsonResponse,
                   MethodRouter};
        pub use ::{FlowError, FlowErr, Result, flow, clients};
        pub use ::clients::fluent;
    }
    pub mod external {
        pub use ::internal_prelude::external_without_std::*;
        pub extern crate std;
    }
    pub mod works_everywhere {
        pub use ::{FlowError, FlowErr, Result, flow, clients};
        pub use ::internal_prelude::external::*;
    }
    pub mod default {
        pub use ::{Graph, ContextPtr, JobPtr, SelfDisposingContextPtr, JsonResponse,
                   MethodRouter};
        pub use ::internal_prelude::works_everywhere::*;
    }
    pub mod c_components {}
}
pub mod for_other_imageflow_crates {
    pub mod preludes {
        pub mod external_without_std {
            pub use ::internal_prelude::external_without_std::*;
        }

        pub mod default {
            pub use ::internal_prelude::external_without_std::*;
            pub use ::internal_prelude::imageflow_core_all::*;
        }
    }
}
