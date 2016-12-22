#[doc(hidden)]
pub mod from_std {
    pub use std::{ptr, marker, str, slice, cell, io, string, fmt, mem, cmp};
    pub use std::ascii::AsciiExt;
    pub use std::borrow::Cow;
    pub use std::cell::{Cell, RefCell, Ref, RefMut};
    pub use std::collections::{HashSet, HashMap};
    pub use std::ffi::{CString, CStr};
    pub use std::fs::{File, create_dir_all, OpenOptions};
    pub use std::io::BufWriter;
    pub use std::io::prelude::*;
    pub use std::path::{PathBuf, Path};
    pub use std::str::FromStr;
    pub use ::std::cmp::Ordering;
}