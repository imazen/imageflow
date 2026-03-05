#![allow(non_camel_case_types)]
//! # Do not use
//! Do not use functions from this module outside of `imageflow_core`
//!
//! **Use the `imageflow_abi` crate when creating bindings**
//!
//! These aren't to be exposed, but rather to connect to `imageflow_c`/`c_components` internals.
//! Overlaps in naming are artifacts from restructuring
//!

pub use imageflow_types::EdgeKind;
pub use imageflow_types::Filter;
pub use imageflow_types::IoDirection;
pub use imageflow_types::PixelFormat;

// These are reused in the external ABI, but only as opaque pointers
///
/// `ImageflowJsonResponse` contains a buffer and buffer length (in bytes), as well as a status code
/// The status code can be used to avoid actual parsing of the response in some cases.
/// For example, you may not care about parsing an error message if you're hacking around -
/// Or, you may not care about success details if you were sending a command that doesn't imply
/// a result.
///
/// The contents of the buffer MAY NOT include any null characters.
/// The contents of the buffer MUST be a valid UTF-8 byte sequence.
/// The contents of the buffer MUST be valid JSON per RFC 7159.
///
/// The schema of the JSON response is not globally defined; consult the API methods in use.
///
/// Use `imageflow_json_response_destroy` to free (it will otherwise remain on the heap and
/// tracking list until the context is destroyed).
///
/// Use `imageflow_context_read_response` to access
#[repr(C)]
pub struct ImageflowJsonResponse {
    pub status_code: i64,
    pub buffer_utf8_no_nulls: *const u8,
    pub buffer_size: usize,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BitmapCompositingMode {
    ReplaceSelf = 0,
    BlendWithSelf = 1,
    BlendWithMatte = 2,
}

// --- Everything below requires C codec libraries ---

#[cfg(feature = "c-codecs")]
mod c_interop;

#[cfg(feature = "c-codecs")]
pub use c_interop::*;
