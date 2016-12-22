#[macro_use]
extern crate lazy_static;

extern crate regex;
extern crate hyper;
extern crate blake2_rfc;
extern crate bit_vec;
extern crate twox_hash;
extern crate os_type;
extern crate chrono;
extern crate zip;
extern crate serde;
extern crate serde_json;
extern crate libc;

pub mod identifier_styles;
pub mod preludes;
pub mod filesystem;
pub mod fetching;
pub mod caching;
pub mod hashing;
pub mod process_testing;
pub mod process_capture;


pub mod timeywimey{
    pub fn time_bucket(seconds_per_bucket: u64, bucket_count: u64) -> u64{
        ::std::time::SystemTime::now().duration_since(::std::time::UNIX_EPOCH).unwrap().as_secs() / seconds_per_bucket % bucket_count
    }
    pub use chrono::UTC;
}

pub mod detect_os{
    // NOT USED, cfg!(os_type="windows") was enough
    pub use os_type::{current_platform, OSType};
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
