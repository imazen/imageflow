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
extern crate imageflow_helpers;

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
mod job;
mod context_methods;
mod job_methods;
mod context;

pub use context::{Context};
pub use job::Job;
pub use ::ffi::{IoDirection, IoMode};
pub use ::flow::definitions::Graph;
pub use json::JsonResponse;
pub use json::MethodRouter;
// use std::ops::DerefMut;
pub mod clients;
pub mod ffi;
pub mod boring;
pub mod parsing;
pub mod test_helpers;

use std::borrow::Cow;

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

impl FlowError{
    pub fn to_cow(&self) -> Cow<'static, str> {
        match *self {
            FlowError::Err(ref e) => {
                Cow::from(format!("Error {} {}\n", e.code, e.message_and_stack))
            }
            FlowError::Oom => {
                Cow::from("Out of memory.")
            }
            FlowError::ErrNotImpl => {
                Cow::from("Error not implemented")
            }
            FlowError::ContextInvalid => {
                Cow::from("Context pointer null")
            }
            FlowError::NullArgument => {
                Cow::from("Null argument")
            }
            ref other => {
                Cow::from(format!("{:?}", other))
            }
        }
    }
    pub fn panic_time(&self){
        panic!("{}",self.to_cow());
    }
    pub fn panic_with(&self, message: &str){
        panic!("{}\n{}", message, self.to_cow());
    }

    pub fn write_to_buf(&self, buf: &mut ::context::ErrorBuffer) -> bool{
        buf.abi_raise_error_c_style(ffi::FlowStatusCode::NotImplemented as i32, None, None, None, None)
    }
    pub unsafe fn write_to_context_ptr(&self, c: *const Context) {
        self.write_to_buf(&mut *(&*c).error_mut());
    }


}

pub type Result<T> = std::result::Result<T, FlowError>;

#[doc(hidden)]
mod internal_prelude {
    #[doc(hidden)]
    pub mod external_without_std {
        pub extern crate imageflow_helpers;

        pub use imageflow_helpers::preludes::from_std::*;
        pub use daggy::{Dag, EdgeIndex, NodeIndex};
        pub use libc::{c_void, c_float, int32_t, int64_t, size_t, uint32_t};
        pub extern crate daggy;
        pub extern crate petgraph;
        pub extern crate serde;
        pub extern crate serde_json;
        pub extern crate time;
        pub extern crate libc;
        pub extern crate imageflow_types as s;
    }
    #[doc(hidden)]
    pub mod imageflow_core_all {
        #[doc(no_inline)]
        pub use ::{Graph, Context, Job, JsonResponse,
                   MethodRouter};
        #[doc(no_inline)]
        pub use ::{FlowError, FlowErr, Result, flow, clients};
        #[doc(no_inline)]
        pub use ::clients::fluent;
    }
    #[doc(hidden)]
    pub mod external {
        #[doc(no_inline)]
        pub use ::internal_prelude::external_without_std::*;
        pub extern crate std;
    }
    #[doc(hidden)]
    pub mod works_everywhere {
        #[doc(no_inline)]
        pub use ::{FlowError, FlowErr, Result, flow, clients};
        #[doc(no_inline)]
        pub use ::internal_prelude::external::*;
    }
    #[doc(hidden)]
    pub mod default {
        #[doc(no_inline)]
        pub use ::{Graph, Context, Job, JsonResponse,
                   MethodRouter};
        #[doc(no_inline)]
        pub use ::internal_prelude::works_everywhere::*;
    }
    #[doc(hidden)]
    pub mod c_components {}
}
#[doc(hidden)]
pub mod for_other_imageflow_crates {
    #[doc(hidden)]
    pub mod preludes {
        #[doc(hidden)]
        pub mod external_without_std {
            #[doc(no_inline)]
            pub use ::internal_prelude::external_without_std::*;
        }
        #[doc(hidden)]
        pub mod default {
            #[doc(no_inline)]
            pub use ::internal_prelude::external_without_std::*;
            #[doc(no_inline)]
            pub use ::internal_prelude::imageflow_core_all::*;
        }
    }
}
