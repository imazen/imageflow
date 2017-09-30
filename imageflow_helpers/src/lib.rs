#![feature(alloc_system)]
#![feature(integer_atomics)]
#![feature(ascii_ctype)]
// `error_chain!` can recurse deeply
#![recursion_limit = "1024"]

extern crate alloc_system;

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate lazy_static;

extern crate reqwest;
extern crate hyper_native_tls;
extern crate regex;
extern crate hyper;
extern crate blake2_rfc;
extern crate twox_hash;
extern crate chrono;
extern crate zip;
extern crate serde;
extern crate serde_json;
extern crate libc;
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
pub mod licensing;

pub mod timeywimey{
    pub fn time_bucket(seconds_per_bucket: u64, bucket_count: u64) -> u64{
        ::std::time::SystemTime::now().duration_since(::std::time::UNIX_EPOCH).unwrap().as_secs() / seconds_per_bucket % bucket_count
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

