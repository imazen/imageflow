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

pub mod timeywimey{
    pub fn time_bucket(seconds_per_bucket: u64, bucket_count: u64) -> u64{
        ::std::time::SystemTime::now().duration_since(::std::time::UNIX_EPOCH).unwrap().as_secs() / seconds_per_bucket % bucket_count
    }
    pub use chrono::UTC;
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
