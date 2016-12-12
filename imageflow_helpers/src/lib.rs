#[macro_use]
extern crate lazy_static;

extern crate regex;
extern crate hyper;
extern crate blake2_rfc;
extern crate bit_vec;


pub mod identifier_styles;
pub mod preludes;
pub mod filesystem;
pub mod fetching;
pub mod caching;
pub mod hashing;





#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
