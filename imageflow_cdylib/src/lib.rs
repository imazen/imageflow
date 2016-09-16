
#![crate_type = "cdylib"]
#![crate_name = "imageflowrs"]
#![feature(alloc_system)]
extern crate imageflow_core;
extern crate alloc_system;
pub use ::imageflow_core::abi::*;
