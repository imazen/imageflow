#![feature(conservative_impl_trait)]

#[macro_use] extern crate macro_attr;
#[macro_use] extern crate enum_derive;

extern crate imageflow_types;
extern crate imageflow_helpers;
//use imageflow_helpers as hlp;
//use imageflow_types as s;
//use imageflow_helpers::preludes::from_std::*;
extern crate url;
//use url::Url;
extern crate time;
extern crate option_filter;


pub mod ir4;
pub mod sizing;


#[cfg(test)]
mod sizing_tests;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
