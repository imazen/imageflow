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

#[macro_export]
macro_rules! here {
    () => (
        ::CodeLocation{ line: line!(), column: column!(), file: file!(), module: module_path!()}
    );
}
#[macro_export]
macro_rules! loc {
    () => (
        concat!(file!(), ":", line!(), ":", column!(), " in ", module_path!())
    );
    ($msg:expr) => (
        concat!($msg, " at\n", file!(), ":", line!(), ":", column!(), " in ", module_path!())
    );
}
#[macro_export]
macro_rules! nerror {
    ($kind:expr) => (
        NodeError{
            kind: $kind,
            message: format!("NodeError {:?}", $kind),
            at: ::smallvec::SmallVec::new(),
            node: None
        }.at(here!())
    );
    ($kind:expr, $fmt:expr) => (
        NodeError{
            kind: $kind,
            message:  format!(concat!("NodeError {:?}: ",$fmt ), $kind,),
            at: ::smallvec::SmallVec::new(),
            node: None
        }.at(here!())
    );
    ($kind:expr, $fmt:expr, $($arg:tt)*) => (
        NodeError{
            kind: $kind,
            message:  format!(concat!("NodeError {:?}: ", $fmt), $kind, $($arg)*),
            at: ::smallvec::SmallVec::new(),
            node: None
        }.at(here!())
    );
}
#[macro_export]
macro_rules! unimpl {
    () => (
        NodeError{
            kind: ::ErrorKind::MethodNotImplemented,
            message: String::new(),
            at: ::smallvec::SmallVec::new(),
            node: None
        }.at(here!())
    );
}


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

#[derive(Debug, Clone, PartialEq)]
pub struct FlowErr {
    pub code: i32,
    pub message_and_stack: String,
}


#[derive(Debug, PartialEq, Clone)]
pub enum FlowError {
    NullArgument,
    GraphCyclic,
    ContextInvalid,
    Oom,
    Err(FlowErr),
    ErrNotImpl,
    FailedBorrow,
    NodeError(NodeError)
}

impl From<NodeError> for FlowError{
    fn from(e: NodeError) -> Self{
        FlowError::NodeError(e)
    }

}

impl std::fmt::Display for FlowError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let &FlowError::NodeError(ref e) = self {
            write!(f, "{}", e)
        } else {
            write!(f, "{:#?}", self)
        }

    }
}


impl std::error::Error for FlowError{
    fn description(&self) -> &str{
        "std::error::Error for FlowError not implemented"
    }
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
            FlowError::FailedBorrow => {
                Cow::from("Failed borrow")
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


// full path
//macro_rules! function {
//    () => {{
//        fn f() {}
//        fn type_name_of<T>(_: T) -> &'static str {
//            extern crate core;
//            unsafe { core::intrinsics::type_name::<T>() }
//        }
//        let name = type_name_of(f);
//        &name[6..name.len() - 4]
//    }}
//}


#[derive(Debug,  Clone, PartialEq)]
pub enum ErrorKind{
    NodeParamsMismatch,
    BitmapPointerNull,
    InvalidCoordinates,
    InvalidNodeParams,
    MethodNotImplemented,
    ValidationNotImplemented,
    InvalidNodeConnections,
    InvalidOperation,
    InvalidState,
    CError(FlowErr)

}
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct CodeLocation{
    pub line: u32,
    pub column: u32,
    pub file: &'static str,
    pub module: &'static str
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeError{
    pub kind: ErrorKind,
    pub message: String,
    pub at: ::smallvec::SmallVec<[CodeLocation;4]>,
    pub node: Option<::flow::definitions::NodeDebugInfo>
}


impl ::std::error::Error for NodeError {
    fn description(&self) -> &str {
        if self.message.is_empty() {
            "Node Error (no message)"
        }else{
            &self.message
        }
    }

}
impl NodeError{

    pub fn at(mut self, c: CodeLocation ) -> NodeError {
        self.at.push(c);
        self
    }
}

impl fmt::Display for NodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.message.is_empty() {
            write!(f, "Error {:?}: at\n", self.kind)?;
        }else{
            write!(f, "{} at\n", self.message)?;
        }
        let url = if::imageflow_types::build_env_info::BUILT_ON_CI{
            let repo = ::imageflow_types::build_env_info::BUILD_ENV_INFO.get("CI_REPO").unwrap_or(&Some("imazen/imageflow")).unwrap_or("imazen/imageflow");
            let commit =  ::imageflow_types::build_env_info::GIT_COMMIT;
            Some(format!("https://github.com/{}/blob/{}/", repo, commit))
        }else { None };

        for recorded_frame in &self.at{
            write!(f, "{}:{}:{} in {}\n", recorded_frame.file, recorded_frame.line, recorded_frame.column, recorded_frame.module)?;

            if let Some(ref url) = url{
                write!(f, "{}{}#L{}\n",url, recorded_frame.file, recorded_frame.line)?;
            }
        }
        if let Some(ref n) = self.node{
            write!(f, "Active node:\n{:#?}\n", n)?;
        }
        Ok(())
    }
}

pub type NResult<T> = ::std::result::Result<T, NodeError>;


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
        pub use ::{FlowError, FlowErr, Result, flow, clients,  NodeError, NResult, ErrorKind};
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
        pub use ::{FlowError, FlowErr, Result, flow, clients,  NodeError, NResult, ErrorKind};
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
