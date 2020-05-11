//! # Purpose
//!
//! This module contains the functions exported for use by other languages.
//!
//!
//! If you're writing bindings, you're in the right place. Don't use `imageflow_core::ffi`
//!
//! Don't call functions against the same job context from multiple threads. You can create contexts
//! from as many threads as you like, but you are responsible for synchronizing API calls
//! on a per-context basis if you want to use one context from multiple threads. No use
//! case for multithreaded Context access has been presented, so it is out of scope for API design.
//!
//! Don't worry about the performance of creating/destroying contexts.
//! A context weighs less than 2kb: (384 + 1400) as of 2017-8-29.
//!
//! # Memory Lifetimes
//!
//! In order to prevent dangling pointers, we must be correct about memory lifetimes.
//!
//! ## ... when allocated by Imageflow, assume the lifetime of the `context`
//!
//! **In Imageflow, by default, all things created with a context will be destroyed when the
//! context is destroyed.** Don't try to access ANYTHING imageflow has provided after the context is gone.
//!
//! This is very nice, as it means that a client's failure to clean up
//! will have limited impact on the process as a whole - as long as the client at minimum
//! calls `flow_context_destroy` at the end of all possible code paths.
//!
//!
//! ### Destroying things
//!
//! * An `imageflow_context` should ALWAYS be destroyed with `imageflow_context_destroy`
//! * `JsonResponse` structures can be released early with `imageflow_json_response_destroy`
//!
//! ## ... when allocated by the client, Imageflow only borrows it for the `invocation`
//!
//! **Imageflow assumes that, at minimum, all pointers that you provide to it will, at minimum,
//! remain valid for the duration of the API call.** We'll call this 'borrowing'. Imageflow is
//! just borrowing it for a bit; not taking ownership of the thing.
//!
//! This may seem obvious, but it is not, in fact, guaranteed by garbage-collected languages. They
//! are oblivious to pointers, and cannot track what data is and is not referenced.
//! Therefore, we suggest that you ensure every allocation made (and handed to Imageflow) is
//! referenced *after* the imageflow API call, preferably in a way that will not be optimized away
//! at runtime. Many languages and FFI libraries offer a utility method just for this purpose.
//!
//! ## ... and it should be very clear when Imageflow is taking ownership of something you created!
//!
//! When Imageflow needs continued access to data that is NOT highly likely to be static, it
//! will be documented.
//!
//! * If you give Imageflow a buffer to read an image from, it will need to access that buffer
//!   until the context is disposed (unless otherwise specified)
//!
//! ## What if I need something to outlive the `context`?
//!
//! Copy it before the context is destroyed.
//!
//! # Data types
//!
//! Reference for those creating bindings in other languages
//!
//! Two types are platform-specific - use the corresponding pointer or size type that varies with
//! your platform.
//!
//! * `libc::c_void` (or anything *mut or *const): Platform-sized pointer. 32 or 64 bits.
//! * The above includes *mut Context an *mut `JsonResponse`
//! * `libc::size_t` (or usize): Unsigned integer, platform-sized. 32 or 64 bits.
//!
//!
//! Treat *mut `Context` and *mut `JsonResponse` as opaque pointers.
//!
//! ## Strings
//!
//! ASCII is a safe subset of UTF-8; therefore wherever Imageflow asks for UTF-8 encoded bytes, you may provide ASCII instead.
//!
//! You will provide Imageflow with strings in one of 3 ways:
//! * UTF-8 null-terminated. You'll see something like `libc::char`, but no length parameter. Short and likely static strings are usually transmitted this way.
//! * Operating system null-terminated. Only applicable to `imageflow_io_create_for_file`.
//! * UTF-8 buffer with length. You'll usually see *const u8 and a length parameter. This is common for buffers of UTF-8 encoded json.
//!
//! filename: *const `libc::c_char`
//! `function_name`: *const `libc::c_char`
//!
//! Fixed size
//!
//! * u8 (1 unsigned byte)
//! * bool (C99 style, 1 byte, value 0 or 1)
//! * The rest seem self-explanatory.
//! * `i` prefixes signed ints
//! * `u` prefixes unsigned ints.
//! * `f` prefixes floating point
//!
//! Structs
//!
//! Consider all structures to be opaque. Do not attempt to access fields by offsets; rather,
//! use the accessor functions provided.
//!
//!
//! ## Failure behavior
//!
//! If you provide a null pointer for `imageflow_context`, then the process will terminate.
//! This "fail fast" behavior offers the best opportunity for a useful stacktrace, and it's not a
//! recoverable error.
//!
//! If you try to continue using an errored `imageflow_context`, the process will terminate.
//! Some errors can be recovered from, but you *must* do that before trying to use the context again.
//!
//! For all APIS: You'll likely segfault the process if you provide a `context` pointer that is dangling or invalid.
//!
#![crate_type = "cdylib"]
#![cfg_attr(feature = "nightly", feature(core_intrinsics))]

// These functions are not for use from Rust, so marking them unsafe just reduces compile-time verification and safety
#![cfg_attr(feature = "cargo-clippy", allow(not_unsafe_ptr_arg_deref))]



#[macro_use]
extern crate imageflow_core as c;

extern crate libc;
extern crate smallvec;
extern crate backtrace;
use crate::c::ffi;

pub use crate::c::{Context, ErrorCategory};
pub use crate::c::ffi::ImageflowJsonResponse as JsonResponse;
//use c::IoDirection;
use crate::c::{ErrorKind, CodeLocation, FlowError};
use std::ptr;
use std::panic::{catch_unwind, AssertUnwindSafe};
#[cfg(test)]
use std::str;


//
// What is possible with the IO object
//#[repr(C)]
//pub enum IoMode {
//    None = 0,
//    ReadSequential = 1,
//    WriteSequential = 2,
//    ReadSeekable = 5, // 1 | 4,
//    WriteSeekable = 6, // 2 | 4,
//    ReadWriteSeekable = 15, // 1 | 2 | 4 | 8
//}

// Input or output?
//#[repr(C)]
//#[derive(Copy,Clone)]
//pub enum Direction {
//    Out = 8,
//    In = 4,
//}

///
/// How long the provided pointer/buffer will remain valid.
/// Callers must prevent the memory from being freed or moved until this contract expires.
///
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Lifetime{
    /// Pointer will outlive function call. If the host language has a garbage collector, call the appropriate method to ensure the object pointed to will not be collected or moved until the call returns. You may think host languages do this automatically in their FFI system. Most do not.
    OutlivesFunctionCall = 0,
    /// Pointer will outlive context. If the host language has a GC, ensure that you are using a data type guaranteed to neither be moved or collected automatically.
    OutlivesContext = 1,
}

/// Creates a static, null-terminated Rust string, and
/// returns a ` *const libc::c_char` pointer to it.
///
/// Useful for API invocations that require a static C string
macro_rules! static_char {
    ($lit:expr) => {
        concat!($lit, "\0").as_ptr() as *const libc::c_char
    }
}


fn type_name_of<T>(_: T) -> &'static str {
    extern crate core;
    std::any::type_name::<T>()
}

fn parent_function_name<T>(f: T) -> &'static str {
    let name = type_name_of(f);
    &name[..name.len() - 3].rsplit_terminator(":").next().unwrap_or("[function name not found]")
}

macro_rules! context {
    ($ptr:ident) => {{
        if $ptr.is_null() {
            fn f() {}
            let name = parent_function_name(f);
            eprintln!("Null context pointer provided to {}. Terminating process.", name);
            let bt = ::backtrace::Backtrace::new();
            eprintln!("{:?}", bt);
            ::std::process::abort();
        }
        (unsafe{&mut *$ptr})
    }}
}

macro_rules! context_ready {
    ($ptr:ident) => {{
        if $ptr.is_null() {
            fn f() {}
            let name = parent_function_name(f);
            eprintln!("Null context pointer provided to {}. Terminating process.", name);
            let bt = ::backtrace::Backtrace::new();
            eprintln!("{:?}", bt);
            ::std::process::abort();
        }else if unsafe{(&*$ptr)}.outward_error().has_error(){
            fn f() {}
            let name = parent_function_name(f);
            eprintln!("The Context passed to {} is in an error state and cannot be used. Terminating process.", name);
            eprintln!("{}",unsafe{(&*$ptr)}.outward_error());

            let bt = ::backtrace::Backtrace::new();
            eprintln!("{} was invoked by: \n{:?}", name, bt);
            ::std::process::abort();
        }
        (unsafe{&mut *$ptr})
    }}
}
macro_rules! handle_result {
    ($context:ident, $result:expr, $failure_value:expr) => {{
        match $result{
            Ok(Ok(v)) => v,
            Err(p) => {
                $context.outward_error_mut().try_set_panic_error(p); $failure_value
            },
            Ok(Err(error)) => {
                $context.outward_error_mut().try_set_error(error); $failure_value
            }
        }
        }}
}

include!("abi_version.rs");

///
/// Call this method before doing anything else to ensure that your header or FFI bindings are compatible
/// with the libimageflow that is currently loaded.
///
/// Provide the values `IMAGEFLOW_ABI_VER_MAJOR` and `IMAGEFLOW_ABI_VER_MINOR` to this function.
///
/// False means that
///
#[no_mangle]
#[cfg_attr(feature = "cargo-clippy", allow(absurd_extreme_comparisons))]
pub extern "C" fn imageflow_abi_compatible(imageflow_abi_ver_major: u32, imageflow_abi_ver_minor: u32) -> bool {
    imageflow_abi_ver_major == IMAGEFLOW_ABI_VER_MAJOR && imageflow_abi_ver_minor <= IMAGEFLOW_ABI_VER_MINOR
}
#[no_mangle]
pub extern "C" fn imageflow_abi_version_major() -> u32 {
    IMAGEFLOW_ABI_VER_MAJOR
}
#[no_mangle]
pub extern "C" fn imageflow_abi_version_minor() -> u32 {
    IMAGEFLOW_ABI_VER_MINOR
}

/// Creates and returns an imageflow context.
/// An imageflow context is required for all other imageflow API calls.
///
/// An imageflow context tracks
/// * error state
/// * error messages
/// * stack traces for errors (in C land, at least)
/// * context-managed memory allocations
/// * performance profiling information
///
/// **Contexts are not thread-safe!** Once you create a context, *you* are responsible for ensuring that it is never involved in two overlapping API calls.
///
/// Returns a null pointer if allocation fails or the provided interface version is incompatible
#[no_mangle]
pub extern "C" fn imageflow_context_create(imageflow_abi_ver_major: u32, imageflow_abi_ver_minor: u32) -> *mut Context {
    if imageflow_abi_compatible(imageflow_abi_ver_major, imageflow_abi_ver_minor) {
        Context::create_cant_panic().map(Box::into_raw).unwrap_or(std::ptr::null_mut())
    }else{
        ptr::null_mut()
    }
}

/// Begins the process of destroying the context, yet leaves error information intact
/// so that any errors in the tear-down process can be
/// debugged with `imageflow_context_error_write_to_buffer`.
///
/// Returns true if no errors occurred. Returns false if there were tear-down issues.
#[no_mangle]
pub extern "C" fn imageflow_context_begin_terminate(context: *mut Context) -> bool {
    let c: &mut Context = context!(context);
    c.abi_begin_terminate()
}

/// Destroys the imageflow context and frees the context object.
/// Only use this with contexts created using `imageflow_context_create`
#[no_mangle]
pub extern "C" fn imageflow_context_destroy(context: *mut Context) {
    if !context.is_null() {
        unsafe {
            let _ = Box::from_raw(context);
        }
    }
}


#[test]
fn test_create_destroy() {
    exercise_create_destroy();
}



pub fn exercise_create_destroy() {
    let c = imageflow_context_create(IMAGEFLOW_ABI_VER_MAJOR, IMAGEFLOW_ABI_VER_MINOR);
    assert!(!c.is_null());
    assert!(imageflow_context_begin_terminate(c));
    imageflow_context_destroy(c);
}

/// Returns true if the context is in an error state. You must immediately deal with the error,
/// as subsequent API calls will fail or cause undefined behavior until the error state is cleared
#[no_mangle]
pub extern "C" fn imageflow_context_has_error(context: *mut Context) -> bool {
    context!(context).outward_error_mut().has_error()
}

/// Returns true if the context is "ok" or in an error state that is recoverable.
/// You must immediately deal with the error,
/// as subsequent API calls will fail or cause undefined behavior until the error state is cleared
#[no_mangle]
pub extern "C" fn imageflow_context_error_recoverable(context: *mut Context) -> bool {
    context!(context).outward_error_mut().recoverable()
}

/// Returns true if the context is "ok" or in an error state that is recoverable.
/// You must immediately deal with the error,
/// as subsequent API calls will fail or cause undefined behavior until the error state is cleared
#[no_mangle]
pub extern "C" fn imageflow_context_error_try_clear(context: *mut Context) -> bool {
    context!(context).outward_error_mut().try_clear()
}


/// Returns the numeric code associated with the error category. 0 means no error.
///
/// These will be stabilized after 1.0, once error categories have passed rigorous real-world testing
/// `imageflow_context_error_as_exit_code` and `imageflow_context_error_as_http_status` are suggested in the meantime.
///
#[no_mangle]
pub extern "C" fn imageflow_context_error_code(context: *mut Context) -> i32 {
    context!(context).outward_error_mut().category().to_outward_error_code()
}

/// Converts the error (or lack thereof) into an unix process exit code
///
/// ## Values
///
/// * 0 - No error
/// * 64 - Invalid usage (graph invalid, node argument invalid, action not supported)
/// * 65 - Invalid Json, Image malformed, Image type not supported
/// * 66 - Primary or secondary file or resource not found.
/// * 69 - Upstream server errored or timed out
/// * 70 - Possible bug: internal error, custom error, unknown error, or no graph solution found
/// * 71 - Out Of Memory condition (malloc/calloc/realloc failed).
/// * 74 - I/O Error
/// * 77 - Action forbidden under imageflow security policy
/// * 402 - License error
/// * 401 - Imageflow server authorization required
///
#[no_mangle]
pub extern "C" fn imageflow_context_error_as_exit_code(context: *mut Context) -> i32 {
    context!(context).outward_error_mut().category().process_exit_code()
}

/// Converts the error (or lack thereof) into an equivalent http status code
///
/// ## Values
///
/// * 200 - No error
/// * 400 - Bad argument/node parameters/graph/json/image/image type
/// * 401 - Authorization to imageflow server required
/// * 402 - License error
/// * 403 - Action forbidden under imageflow security policy
/// * 404 - Primary resource/file not found
/// * 500 - Secondary resource/file not found, IO error, no solution error, unknown error, custom error, internal error
/// * 502 - Upstream server error
/// * 503 - Out Of Memory condition (malloc/calloc/realloc failed).
/// * 504 - Upstream timeout
#[no_mangle]
pub extern "C" fn imageflow_context_error_as_http_code(context: *mut Context) -> i32 {
    context!(context).outward_error_mut().category().http_status_code()
}

/// Prints the error messages (and optional stack frames) to the given buffer in UTF-8 form; writes a null
/// character to terminate the string, and *ALSO* provides the number of bytes written (excluding the null terminator)
///
/// Returns false if the buffer was too small (or null) and the output was truncated.
/// Returns true if all data was written OR if there was a bug in error serialization (that gets written, too).
///
/// If the data is truncated, "\n[truncated]\n" is written to the buffer
///
/// Please be accurate with the buffer length, or a buffer overflow will occur.
#[no_mangle]
pub extern "C" fn imageflow_context_error_write_to_buffer(context: *mut Context,
                                                                buffer: *mut libc::c_char,
                                                                buffer_length: libc::size_t,
                                                                bytes_written: *mut libc::size_t) -> bool {
    if buffer.is_null(){
        false
    }else {
        use crate::c::errors::writing_to_slices::WriteResult;
        let c = context!(context);

        if buffer_length.leading_zeros() == 0{
            c.outward_error_mut().try_set_error(nerror!(ErrorKind::InvalidArgument, "Argument `buffer_length` likely came from a negative integer. Imageflow prohibits having the leading bit set on unsigned integers (this reduces the maximum value to 2^31 or 2^63)."));
            return false;
        }

        let result = unsafe {
            c.outward_error_mut().get_buffer_writer().write_and_write_errors_to_cstring(buffer as *mut u8, buffer_length, Some("\n[truncated]\n"))
        };
        if !bytes_written.is_null(){
            unsafe {
                *bytes_written = result.bytes_written();
            }
        }
        match result {
            WriteResult::AllWritten(_) |
            WriteResult::Error { .. } => true,
            WriteResult::TruncatedAt(_) => false,
        }
    }
}


/// Prints the error to stderr and exits the process if an error has been raised on the context.
/// If no error is present, the function returns false.
///
/// THIS PRINTS DIRECTLY TO STDERR! Do not use in any kind of service! Command-line usage only!
#[no_mangle]
pub extern "C" fn imageflow_context_print_and_exit_if_error(context: *mut Context) -> bool {
    let e = context!(context).outward_error();
    if e.has_error(){
        eprintln!("{}",e);
        std::process::exit(e.category().process_exit_code())
    }else{
        false
    }

}



///
/// Writes fields from the given `imageflow_json_response` to the locations referenced.
/// The buffer pointer sent out will be a UTF-8 byte array of the given length (not null-terminated). It will
/// also become invalid if the `JsonResponse` associated is freed, or if the context is destroyed.
///
/// See `imageflow_context_error_as_http_code` for just the http status code equivalent.
///
/// Most errors are not recoverable; you must destroy the context and retry.
///
#[no_mangle]
pub extern fn imageflow_json_response_read(context: *mut Context,
                                                  response_in: *const JsonResponse,
                                                  status_as_http_code_out: *mut i64,
                                                  buffer_utf8_no_nulls_out: *mut *const u8,
                                                  buffer_size_out: *mut libc::size_t) -> bool {
    let c = context!(context); // Must be readable in error state

    if response_in.is_null() {
        c.outward_error_mut().try_set_error(nerror!(ErrorKind::NullArgument, "The argument response_in (* JsonResponse) is null."));
        return false;
    }
    unsafe {
        if !status_as_http_code_out.is_null() {
            *status_as_http_code_out = (*response_in).status_code;
        }
        if !buffer_utf8_no_nulls_out.is_null() {
            *buffer_utf8_no_nulls_out = (*response_in).buffer_utf8_no_nulls;
        }
        if !buffer_size_out.is_null() {
            *buffer_size_out = (*response_in).buffer_size;
        }
    }
    true
}

/// Frees memory associated with the given object (and owned objects) after
/// running any owned or attached destructors. Returns false if something went wrong during tear-down.
///
/// Returns true if the object to destroy is a null pointer, or if tear-down was successful.
///
/// Behavior is undefined if the pointer is dangling or not a valid memory reference.
/// Although certain implementations catch
/// some kinds of invalid pointers, a segfault is likely in future revisions).
///
/// Behavior is undefined if the context provided does not match the context with which the
/// object was created.
///
#[no_mangle]
pub extern "C" fn imageflow_json_response_destroy(context: *mut Context,
                                                         response: *mut JsonResponse)
                                                         -> bool {
    imageflow_context_memory_free(context, response as *mut libc::c_void, ptr::null(), 0)
}

///
/// Sends a JSON message to the `imageflow_context` using endpoint `method`.
///
/// ## Endpoints
///
/// * 'v1/build`
///
/// For endpoints supported by the latest nightly build, see
/// `https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/context_json_api.txt`
///
/// ## Notes
///
/// * `method` and `json_buffer` are only borrowed for the duration of the function call. You are
///    responsible for their cleanup (if necessary - static strings are handy for things like
///    `method`).
/// * `method` should be a UTF-8 null-terminated string.
///   `json_buffer` should be a UTF-8 encoded buffer (not null terminated) of length `json_buffer_size`.
///
/// You should call `imageflow_context_has_error()` to see if this succeeded.
///
/// A `JsonResponse` is returned for success and most error conditions.
/// Call `imageflow_json_response_destroy` when you're done with it (or dispose the context).
#[no_mangle]
pub extern "C" fn imageflow_context_send_json(context: *mut Context,
                                                     method: *const libc::c_char,
                                                     json_buffer: *const u8,
                                                     json_buffer_size: libc::size_t)
                                                     -> *const JsonResponse {

    let c: &mut Context = context_ready!(context);
    if method.is_null() {
        c.outward_error_mut().try_set_error(nerror!(ErrorKind::NullArgument, "The argument 'method' is null."));
        return ptr::null();
    }
    if json_buffer.is_null() {
        c.outward_error_mut().try_set_error(nerror!(ErrorKind::NullArgument, "The argument 'json_buffer' is null."));
        return ptr::null();
    }
    if json_buffer_size.leading_zeros() == 0{
        c.outward_error_mut().try_set_error(nerror!(ErrorKind::InvalidArgument, "Argument `json_buffer_size` likely came from a negative integer. Imageflow prohibits having the leading bit set on unsigned integers (this reduces the maximum value to 2^31 or 2^63)."));
        return ptr::null();
    }

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let method_str = if let Ok(str) = unsafe { ::std::ffi::CStr::from_ptr(method as *const i8)}.to_str() {
            str
        } else {
            return (ptr::null(), Err(nerror!(ErrorKind::InvalidArgument, "The argument 'method' is invalid UTF-8.")));
        };

        let json_bytes = unsafe{ std::slice::from_raw_parts(json_buffer, json_buffer_size) };

        // Segfault early
        let _ = (json_bytes.first(), json_bytes.last());


        let (json, result) = c.message(method_str, json_bytes);

        // An unfortunate copy occurs here
        (create_abi_json_response(c, &json.response_json, json.status_code), result)
    }));

    match panic_result{
        Ok((json, Ok(_))) => json,
        Ok((json, Err(e))) => {
            c.outward_error_mut().try_set_error(e);
            json
        }
        Err(p) => {
         c.outward_error_mut().try_set_panic_error(p); ptr::null_mut()
        },
    }
}


pub fn create_abi_json_response(c: &mut Context,
                                json_bytes: &[u8],
                                status_code: i64)
                                -> *const JsonResponse {
    unsafe {
        let sizeof_struct = std::mem::size_of::<JsonResponse>();
        let alloc_size = sizeof_struct + json_bytes.len();

        let pointer = crate::ffi::flow_context_calloc(c.flow_c(),
                                                 1,
                                                 alloc_size,
                                                 ptr::null(),
                                                 c.flow_c() as *mut libc::c_void,
                                                 ptr::null(),
                                                 line!() as i32) as *mut u8;
        if pointer.is_null() {
            c.outward_error_mut().try_set_error(nerror!(ErrorKind::AllocationFailed, "Failed to allocate JsonResponse ({} bytes)", alloc_size));
            return ptr::null();
        }
        if json_bytes.len().leading_zeros() == 0{
            c.outward_error_mut().try_set_error(nerror!(ErrorKind::Category(ErrorCategory::InternalError), "Error in creating JSON structure; length overflow prevented (most significant bit set)."));
            return ptr::null();
        }

        let pointer_to_final_buffer =
            pointer.offset(sizeof_struct as isize) as *mut u8;
        let imageflow_response = &mut (*(pointer as *mut JsonResponse));
        imageflow_response.buffer_utf8_no_nulls = pointer_to_final_buffer;
        imageflow_response.buffer_size = json_bytes.len();
        imageflow_response.status_code = status_code;

        let out_json_bytes = std::slice::from_raw_parts_mut(pointer_to_final_buffer,
                                                                json_bytes.len());

        out_json_bytes.clone_from_slice(json_bytes);

        imageflow_response as *const JsonResponse
    }
}


//
/////
///// Adds a file input or output to the job context
/////
///// The filename should be a null-terminated string that is valid utf-8. It should be written in codepage used by your operating system for handling `fopen` calls.
///// https://msdn.microsoft.com/en-us/library/yeby3zcb.aspx
/////
///// If the filename is fopen compatible, you're probably OK.
/////
///// As always, `mode` is not enforced except for the file open flags.
/////
//#[no_mangle]
//pub extern "C" fn imageflow_context_add_file(context: *mut Context, io_id: i32, direction: Direction,
//                                                      mode: IoMode,
//                                                      filename: *const libc::c_char)
//                                                      -> bool {
//    let mut c = context_ready!(context);
//    if filename.is_null() {
//        c.outward_error_mut().try_set_error(nerror!(ErrorKind::NullArgument, "The argument 'filename' is null."));
//        return false;
//    }
//    if c.io_id_present(io_id){
//        c.outward_error_mut().try_set_error(nerror!(ErrorKind::DuplicateIoId, "The io_id provided is already in use."));
//        return false;
//    }
//
//    let result = catch_unwind(AssertUnwindSafe(|| {
//        let filename_str = if let Ok(str) = unsafe{ ::std::ffi::CStr::from_ptr(filename)}.to_str() {
//            str
//        } else {
//            return Err(nerror!(ErrorKind::InvalidArgument, "The argument 'filename' is invalid UTF-8."));
//        };
//        let dir = match direction{
//            Direction::In => IoDirection::In,
//            Direction::Out => IoDirection::Out
//        };
//        c.add_file_with_mode( io_id, dir, filename_str, unsafe {std::mem::transmute(mode)}).map_err(|e| e.at(here!()))?;
//        Ok(true)
//    }));
//
//    handle_result!(c, result, false)
//}


///
/// Adds an input buffer to the job context.
/// You are ALWAYS responsible for freeing the memory provided (at the time specified by Lifetime).
/// If you specify `OutlivesFunctionCall`, then the buffer will be copied.
///
///
#[no_mangle]
pub extern "C" fn imageflow_context_add_input_buffer(context: *mut Context,
                                                         io_id: i32,
                                                         buffer: *const u8,
                                                         buffer_byte_count: libc::size_t,
                                                            lifetime: Lifetime)
                                                         -> bool {

    let c: &mut Context = context_ready!(context);
    if buffer.is_null() {
        c.outward_error_mut().try_set_error(nerror!(ErrorKind::NullArgument, "The argument 'buffer' is null."));
        return false;
    }
    if buffer_byte_count.leading_zeros() == 0{
        c.outward_error_mut().try_set_error(nerror!(ErrorKind::InvalidArgument, "Argument `buffer_byte_count` likely came from a negative integer. Imageflow prohibits having the leading bit set on unsigned integers (this reduces the maximum value to 2^31 or 2^63)."));
        return false;
    }
    if c.io_id_present(io_id){
        c.outward_error_mut().try_set_error(nerror!(ErrorKind::DuplicateIoId, "The io_id provided is already in use."));
        return false;
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        let bytes = unsafe {
            std::slice::from_raw_parts(buffer, buffer_byte_count)
        };

        if lifetime == Lifetime::OutlivesFunctionCall {
            c.add_copied_input_buffer(io_id,bytes).map_err(|e| e.at(here!()))?;
        }else {
            c.add_input_buffer(io_id,bytes).map_err(|e| e.at(here!()))?;
        }
        Ok(true)
    }));
    handle_result!(c, result, false)
}


///
/// Adds an output buffer to the job context.
/// The  buffer will be freed with the context.
///
///
/// Returns null if allocation failed; check the context for error details.
#[no_mangle]
pub extern "C" fn imageflow_context_add_output_buffer(context: *mut Context, io_id: i32)
                                                               -> bool {
    let c = context_ready!(context);
    let result = catch_unwind(AssertUnwindSafe(|| {
        c.add_output_buffer(io_id).map_err(|e| e.at(here!()))?;
        Ok(true)
    }));
    handle_result!(c, result, false)

}




///
/// Provides access to the underlying buffer for the given io id
///
#[no_mangle]
pub extern "C" fn imageflow_context_get_output_buffer_by_id(context: *mut Context,
                                                               io_id: i32,
                                                               result_buffer: *mut *const u8,
                                                               result_buffer_length: *mut libc::size_t)
                                                               -> bool {
    let c: &mut Context = context_ready!(context);
    if result_buffer.is_null() {
        c.outward_error_mut().try_set_error(nerror!(ErrorKind::NullArgument, "The argument 'result_buffer' is null."));
        return false;
    }

    if result_buffer_length.is_null() {
        c.outward_error_mut().try_set_error(nerror!(ErrorKind::NullArgument, "The argument 'result_buffer_length' is null."));
        return false;
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        let s = c.get_output_buffer_slice(io_id).map_err(|e| e.at(here!()))?;

        if s.len().leading_zeros() == 0 {
            Err(nerror!(ErrorKind::Category(ErrorCategory::InternalError), "Error retrieving output buffer; length overflow prevented (most significant bit set)."))
        } else {
            unsafe {
                (*result_buffer) = s.as_ptr();
                (*result_buffer_length) = s.len();
            }
            Ok(true)
        }
    }));
    handle_result!(c, result, false)
}




///
/// Allocates zeroed memory that will be freed with the context.
///
/// * filename/line may be used for debugging purposes. They are optional. Provide null/-1 to skip.
/// * If provided, `filename` should be an null-terminated UTF-8 or ASCII string which will outlive the context.
///
/// Returns null(0) on failure.
///
#[no_mangle]
pub extern "C" fn imageflow_context_memory_allocate(context: *mut Context,
                                                    bytes: libc::size_t,
                                                    filename: *const libc::c_char,
                                                    line: i32) -> *mut libc::c_void {

    let c = context_ready!(context);

    if bytes.leading_zeros() == 0{
        c.outward_error_mut().try_set_error(nerror!(ErrorKind::InvalidArgument, "Argument `bytes` likely came from a negative integer. Imageflow prohibits having the leading bit set on unsigned integers (this reduces the maximum value to 2^31 or 2^63)."));
        return ptr::null_mut();
    }
    unsafe {
        ffi::flow_context_calloc(c.flow_c(), 1, bytes, ptr::null(), c.flow_c() as *const libc::c_void, filename, line)
    }
}

///
/// Frees memory allocated with `imageflow_context_memory_allocate` early.
///
/// * filename/line may be used for debugging purposes. They are optional. Provide null/-1 to skip.
/// * If provided, `filename` should be an null-terminated UTF-8 or ASCII string which will outlive the context.
///
/// Returns false on failure. Returns true on success, or if `pointer` is null.
///
#[no_mangle]
pub  extern "C" fn imageflow_context_memory_free(context: *mut Context,
                                                       pointer: *mut libc::c_void,
                                                       filename: *const libc::c_char,
                                                       line: i32) -> bool {
    let c = context!(context); // We must be able to free in an errored state
    if !pointer.is_null() {
        unsafe {
            ffi::flow_destroy(c.flow_c(), pointer, filename, line)
        }
    }else {
        true
    }
}


#[test]
fn test_message() {
    exercise_json_message();
}

pub fn exercise_json_message() {
    unsafe {
        let c = imageflow_context_create(IMAGEFLOW_ABI_VER_MAJOR, IMAGEFLOW_ABI_VER_MINOR);
        assert!(!c.is_null());

        let method_in = static_char!("brew_coffee");
        let json_in = "{}";
        let expected_response = c::JsonResponse::teapot();
        let expected_json_out = ::std::str::from_utf8(
            expected_response.response_json.as_ref()).unwrap();
        let expected_response_status = expected_response.status_code;

        let response = imageflow_context_send_json(c,

                                           method_in,
                                           json_in.as_ptr(),
                                           json_in.len());

        assert_ne!(response, ptr::null());

        let mut json_out_ptr: *const u8 = ptr::null_mut();
        let mut json_out_size: usize = 0;
        let mut json_status_code: i64 = 0;

        assert!(imageflow_json_response_read(c,
                                             response,
                                             &mut json_status_code,
                                             &mut json_out_ptr,
                                             &mut json_out_size));


        let json_out_str =
            ::std::str::from_utf8(std::slice::from_raw_parts(json_out_ptr, json_out_size)).unwrap();
        assert_eq!(json_out_str, expected_json_out);

        assert_eq!(json_status_code, expected_response_status);

        imageflow_context_destroy(c);
    }
}


#[test]
fn test_allocate_free() {
    unsafe{
        let c = imageflow_context_create(IMAGEFLOW_ABI_VER_MAJOR, IMAGEFLOW_ABI_VER_MINOR);
        let bytes = 100;
        let ptr = imageflow_context_memory_allocate(c, bytes, static_char!(file!()),
                                                    line!() as i32) as *mut u8;
        assert!(ptr != ptr::null_mut());

        for x in 0..bytes{
            assert_eq!(*ptr.offset(x as isize), 0);
        }
        assert!(imageflow_context_memory_free(c, ptr as *mut libc::c_void, static_char!(file!()),
                                              line!() as i32));

        imageflow_context_destroy(c);
        //imageflow_context_destroy(c);
    }
}

#[cfg(test)]
extern crate base64;


#[test]
fn test_job_with_buffers() {
    {
        let c = imageflow_context_create(IMAGEFLOW_ABI_VER_MAJOR, IMAGEFLOW_ABI_VER_MINOR);
        assert!(!c.is_null());

        let input_bytes = base64::decode(&b"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABAQMAAAAl21bKAAAAA1BMVEX/TQBcNTh/AAAAAXRSTlPM0jRW/QAAAApJREFUeJxjYgAAAAYAAzY3fKgAAAAASUVORK5CYII=".to_vec()).unwrap();



        let res = imageflow_context_add_input_buffer(c, 0, input_bytes.as_ptr(), input_bytes.len(), Lifetime::OutlivesContext);
        imageflow_context_print_and_exit_if_error(c);
        assert!(res);

        let res = imageflow_context_add_output_buffer(c, 1);
        imageflow_context_print_and_exit_if_error(c);
        assert!(res);

        let method_in = static_char!("v1/execute");
        let json_in = r#"{"framewise":{"steps":[{"decode":{"io_id":0}},{"flip_h":null},{"rotate_90":null},{"resample_2d":{"w":30,"h":20,"hints":{"sharpen_percent":null}}},{"constrain":{ "mode" :"within", "w": 5,"h": 5}},{"encode":{"io_id":1,"preset":{"gif":null}}}]}}"#;

        let response = imageflow_context_send_json(c,
                                                   method_in,
                                                   json_in.as_ptr(),
                                                   json_in.len());

        assert!(!response.is_null());

        let mut json_out_ptr: *const u8 = ptr::null_mut();
        let mut json_out_size: usize = 0;
        let mut json_status_code: i64 = 0;

        assert!(imageflow_json_response_read(c,
                                             response,
                                             &mut json_status_code,
                                             &mut json_out_ptr,
                                             &mut json_out_size));


        imageflow_context_print_and_exit_if_error(c);


        let mut buf: *const u8 = ptr::null();
        let mut buf_len: usize = 0;
        let res = imageflow_context_get_output_buffer_by_id(c, 1, &mut buf as *mut *const u8, &mut buf_len as *mut usize);
        imageflow_context_print_and_exit_if_error(c);
        assert!(res);

        let expected_response_status = 200;
        assert_eq!(json_status_code, expected_response_status);

        imageflow_context_destroy(c);
    }
}


#[test]
fn test_job_with_bad_json() {
    {
        let c = imageflow_context_create(IMAGEFLOW_ABI_VER_MAJOR, IMAGEFLOW_ABI_VER_MINOR);
        assert!(!c.is_null());

        let input_bytes = base64::decode(&b"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABAQMAAAAl21bKAAAAA1BMVEX/TQBcNTh/AAAAAXRSTlPM0jRW/QAAAApJREFUeJxjYgAAAAYAAzY3fKgAAAAASUVORK5CYII=".to_vec()).unwrap();



        let res = imageflow_context_add_input_buffer(c, 0, input_bytes.as_ptr(), input_bytes.len(), Lifetime::OutlivesContext);
        imageflow_context_print_and_exit_if_error(c);
        assert!(res);

        let res = imageflow_context_add_output_buffer(c, 1);
        imageflow_context_print_and_exit_if_error(c);
        assert!(res);

        let method_in = static_char!("v1/execute");
        let json_in = r#"{"framewise":{"steps":[{"decode":{"io_id":0}},{"flip_h":null},{"rotate_90":null},{"resample_2d":{"w":30,"h":20,"down_filter":null,"up_filter":null,"hints":{"sharpen_percent":null}}},{"constrain":{"within":{"w":5,"h":5}}},{"encode":{"io_id":1,"preset":{"gif":null}}}]}}"#;

        let response = imageflow_context_send_json(c,
                                                   method_in,
                                                   json_in.as_ptr(),
                                                   json_in.len());

        assert!(!response.is_null());


        let mut json_out_ptr: *const u8 = ptr::null_mut();
        let mut json_out_size: usize = 0;
        let mut json_status_code: i64 = 0;

        assert!(imageflow_json_response_read(c,
                                             response,
                                             &mut json_status_code,
                                             &mut json_out_ptr,
                                             &mut json_out_size));
        assert!(imageflow_context_has_error(c));

        let expected_response_status = 400; //bad request
        assert_eq!(json_status_code, expected_response_status);

        imageflow_context_destroy(c);
    }
}

#[test]
fn test_file_macro_for_this_build(){
    assert!(file!().starts_with(env!("CARGO_PKG_NAME")))
}


