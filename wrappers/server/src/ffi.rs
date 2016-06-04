extern crate libc;

pub enum Context {}

#[link(name = "imageflow")]
extern {
    pub fn flow_context_create() -> *mut Context;
}
