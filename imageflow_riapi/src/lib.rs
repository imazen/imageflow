#![forbid(unsafe_code)]

#[macro_use]
extern crate macro_attr;
#[macro_use]
extern crate enum_derive;

pub mod ir4;
pub mod sizing;

#[cfg(test)]
mod sizing_tests;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
