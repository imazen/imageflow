#![feature(concat_idents)]
#![feature(alloc)]
#![feature(oom)]
#![feature(alloc_system)]
#![feature(conservative_impl_trait)]
#![feature(proc_macro)]
#![feature(integer_atomics)]
#![feature(as_c_str)]
#![feature(core_intrinsics)]



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
extern crate imageflow_riapi;
extern crate num;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate serde;
extern crate libc;
extern crate alloc;
extern crate rustc_serialize;
extern crate url;
extern crate uuid;
extern crate gif;
extern crate gif_dispose;
extern crate smallvec;
extern crate core;

#[macro_use]
pub mod errors;
#[macro_use]
pub use errors::*;


mod json;
pub mod flow;
mod job;
mod context_methods;
mod job_methods;
mod context;
mod codecs;
mod io;

pub use context::{Context};
pub use job::Job;
pub use io::IoProxy;
pub use ::ffi::{IoDirection, IoMode};
pub use ::flow::definitions::Graph;
pub use json::JsonResponse;
pub use json::MethodRouter;
// use std::ops::DerefMut;
pub mod clients;
pub mod ffi;
pub mod parsing;
pub mod test_helpers;
use std::fmt;
use std::borrow::Cow;
use ::petgraph::graph::NodeIndex;


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
        pub use ::{CError, flow, clients, FlowError, Result, ErrorKind};
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
        pub use ::{CError, flow, clients, FlowError, Result, ErrorKind};
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
