#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]
#![cfg_attr(feature = "nightly", feature(portable_simd))]

// Required to force linkage of native C library — the crate has no Rust API,
// only provides C object files. `extern crate` is needed to prevent the linker
// from stripping the unused (from Rust's perspective) native library.
extern crate imageflow_c_components;

#[macro_use]
pub mod errors;
pub use crate::errors::*;

mod codecs;
mod context;
mod flow;
pub mod graphics;
mod io;
pub mod json;

pub use crate::codecs::NamedDecoders;
pub use crate::context::Context;
pub use crate::context::ThreadSafeContext;
pub use crate::flow::definitions::Graph;
pub use crate::io::IoProxy;
pub use crate::json::JsonResponse;
pub use imageflow_types::IoDirection;
mod allocation_container;
pub mod clients;
pub mod ffi;
pub mod parsing;
pub mod test_helpers;

use petgraph::graph::NodeIndex;
use std::borrow::Cow;
use std::fmt;

pub use crate::graphics::bitmaps::BitmapKey;
pub use enough::{Stop, StopReason, Unstoppable};

pub mod helpers {
    pub use crate::codecs::write_png;
}
#[doc(hidden)]
mod internal_prelude {
    #[doc(hidden)]
    pub mod external_without_std {
        pub extern crate imageflow_helpers;

        pub use core::ffi::c_void;
        pub use daggy::{Dag, EdgeIndex, NodeIndex};
        pub use imageflow_helpers::preludes::from_std::*;
        pub extern crate daggy;
        pub extern crate imageflow_types as s;
        pub extern crate petgraph;
        pub extern crate serde;
        pub extern crate serde_json;
    }
    #[doc(hidden)]
    pub mod imageflow_core_all {
        #[doc(no_inline)]
        pub use crate::clients::fluent;
        #[doc(no_inline)]
        pub use crate::{clients, CodeLocation, ErrorKind, FlowError, Result};
        #[doc(no_inline)]
        pub use crate::{Context, Graph, JsonResponse};
    }
    #[doc(hidden)]
    pub mod external {
        #[doc(no_inline)]
        pub use crate::internal_prelude::external_without_std::*;
        pub extern crate std;
    }
    #[doc(hidden)]
    pub mod works_everywhere {
        #[doc(no_inline)]
        pub use crate::internal_prelude::external::*;
        #[doc(no_inline)]
        pub use crate::{clients, ErrorKind, FlowError, Result};
    }
    #[doc(hidden)]
    pub mod default {
        #[doc(no_inline)]
        pub use crate::internal_prelude::works_everywhere::*;
        #[doc(no_inline)]
        pub use crate::{Context, Graph, JsonResponse};
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
            pub use crate::internal_prelude::external_without_std::*;
        }
        #[doc(hidden)]
        pub mod default {
            #[doc(no_inline)]
            pub use crate::internal_prelude::external_without_std::*;
            #[doc(no_inline)]
            pub use crate::internal_prelude::imageflow_core_all::*;
        }
    }
}
