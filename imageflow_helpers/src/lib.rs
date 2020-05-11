
// `error_chain!` can recurse deeply
#![recursion_limit = "1024"]

//hexadecimal colors aren't numbers
#![cfg_attr(feature = "cargo-clippy", allow(unreadable_literal))]


//
//use std::heap::{Alloc, System, Layout, AllocErr};
//
//struct MyAllocator;
//
//unsafe impl<'a> Alloc for &'a MyAllocator {
//    unsafe fn alloc(&mut self, layout: Layout) -> Result<*mut u8, AllocErr> {
//        System.alloc(layout)
//    }
//
//    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
//        System.dealloc(ptr, layout)
//    }
//}

use std::alloc::System;

#[global_allocator]
static GLOBAL: System = System;


#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate lazy_static;

extern crate rand;
extern crate reqwest;
extern crate hyper_native_tls;
extern crate regex;
extern crate hyper;
extern crate blake2_rfc;
extern crate twox_hash;
extern crate chrono;
extern crate zip;
//extern crate serde;
//extern crate serde_json;
extern crate backtrace;
extern crate num;
extern crate base64;
extern crate sha2;
extern crate unicase;
extern crate app_dirs;
extern crate chashmap;
extern crate parking_lot;
extern crate time;
extern crate uuid;
extern crate lockless;
extern crate smallvec;
#[cfg(test)]
extern crate mockito;

extern crate digest;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
extern crate openssl;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
extern crate hyper_openssl;

pub mod identifier_styles;
pub mod preludes;
pub mod filesystem;
pub mod fetching;
pub mod caching;
pub mod hashing;
pub mod process_testing;
pub mod process_capture;
pub mod colors;
pub mod debug;
pub mod util;


pub mod timeywimey{
    pub fn time_bucket(seconds_per_bucket: u64, bucket_count: u64) -> u64{
        ::std::time::SystemTime::now().duration_since(::std::time::UNIX_EPOCH).unwrap().as_secs() / seconds_per_bucket % bucket_count
    }

    pub fn precise_time_ns() -> u64{
        use std::time::{SystemTime, UNIX_EPOCH};
        //TODO: consider if u64 is too small
        SystemTime::now().duration_since(UNIX_EPOCH)
                .expect("Time went backwards").as_nanos() as u64
    }
    pub use chrono::prelude::Utc;
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}



// We'll put our errors in an `errors` module, and other modules in
// this crate will `use errors::*;` to get access to everything
// `error_chain!` creates.
#[allow(unused_doc_comments)]
#[allow(renamed_and_removed_lints)]
mod errors {
    // Create the Error, ErrorKind, ResultExt, and Result types

    error_chain! {

        errors{
           LicenseCorrupted(msg: String) {
                description("Please verify/reinstall license; license corrupt.")
                display("Please verify/reinstall license; license corrupt: {}", msg)
           }
           RsaDecryptInputLargerThanModulus
        }

    }
}


#[test]
fn test_file_macro_for_this_build(){
    assert!(file!().starts_with(env!("CARGO_PKG_NAME")))
}

