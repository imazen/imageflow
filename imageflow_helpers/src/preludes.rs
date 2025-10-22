#[doc(hidden)]
pub mod from_std {
    pub use ::std::cmp::Ordering;
    pub use std::borrow::Cow;
    pub use std::cell::{Cell, Ref, RefCell, RefMut};
    pub use std::collections::{HashMap, HashSet};
    pub use std::ffi::{CStr, CString};
    pub use std::fs::{create_dir_all, File, OpenOptions};
    pub use std::io::prelude::*;
    pub use std::io::BufWriter;
    pub use std::path::{Path, PathBuf};
    pub use std::str::FromStr;
    pub use std::{cell, cmp, fmt, io, marker, mem, ptr, slice, str, string};
}
