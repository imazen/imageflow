#![feature(conservative_impl_trait)]


extern crate imageflow_types;
extern crate imageflow_helpers;
//use imageflow_helpers as hlp;
//use imageflow_types as s;
//use imageflow_helpers::preludes::from_std::*;
extern crate url;
//use url::Url;


pub mod sizing;


#[cfg(test)]
mod sizing_tests;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
