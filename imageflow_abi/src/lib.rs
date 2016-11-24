//! # Purpose
//!
//! This module contains the functions exported for use by other languages.
//!
//! If you're writing bindings, you're in the right place. Don't use imageflow_core::ffi, that's
//! *not* correct - those are unstable and will disappear as C is replaced with Rust.
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
//! However, waiting to free memory and run destructors until the context is destroyed is not ideal;
//! it increases our peak memory usage/needs and may cause operations
//! to fail that would otherwise succeed.
//!
//! There are two ways to mitigate this.
//!
//! 1. When creating an I/O object, request it be cleaned up when the first job it is assigned to is cleaned up (instead of the context).
//! 2. Manually invoke the corresponding destroy function when you're done with the thing.
//!
//! ### Destroying things
//!
//! * An `imageflow_context` should ALWAYS be destroyed with `imageflow_context_destroy`
//! * JsonResponse structures should be released with `imageflow_json_response_destroy`
//! * An `imageflow_job` can be destroyed early with `imageflow_job_destroy`
//!
//! ## ... when allocated by the client, Imageflow only borrows it for the `invocation`
//!
//! **Imageflow assumes that, at minimum, all pointers that you provide to it will, at minimum,
//! remain valid for the duration of the API call.** We'll call this 'borrowing'. Imageflow is
//! just borrowing it for a bit; not taking ownership of the thing.
//!
//! This may seem obvious, but it is not, in fact, guaranteed by garbage-collected languages. They
//! are often oblivious to pointers, and cannot track what data is and is not referenced.
//! Therefore, we suggest that you ensure every allocation made (and handed to Imageflow) is
//! referenced *after* the imageflow API call, preferably in a way that will not be optimized away
//! at runtime. Many languages and FFI libraries offer a utility method just for this purpose.
//!
//! ## ... although Imageflow may borrow some strings for the life of the context, yet not own them.
//!
//! This happens for strings that are usually static constants, and unlikely to be allocated on
//! the heap anyway.
//!
//! * When an Imageflow API asks for a filename, function name, or error message, it will
//!   assume that those strings are pointers that (a) Imageflow is not
//!   responsible for freeing, and (b) will (at least) outlive the `context`. For C#, GCHandle comes in handy here.
//!
//! ## ... and it should be very clear when Imageflow is taking ownership of something you created!
//!
//! When Imageflow needs continued access to data that is NOT highly likely to be static, it
//! will be documented.
//!
//! * If you give Imageflow a buffer to read an image from, it will need to access that buffer
//!   much longer than the initial io_create call.
//!
//! ## What if I need something to outlive the `context`?
//!
//! Copy it before the context is destroyed.
//!
//!
//!
//! # Data types
//!
//! Reference for those creating bindings in other languages
//!
//! Two types are platform-specific - use the corresponding pointer or size type that varies with
//! your platform.
//!
//! * libc::c_void (or anything *mut or *const): Platform-sized pointer. 32 or 64 bits.
//! * The above includes *mut Context, *mut Job, *mut JobIo, etc.
//! * libc::size_t (or usize): Unsigned integer, platform-sized. 32 or 64 bits.
//!
//!
//! Treat *mut Context, *mut Job, *mut JobIo, *mut JsonResponse ALL as opaque pointers.
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
//! filename: *const libc::c_char
//! function_name: *const libc::c_char
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
//!
//!
//!
//!
//!
//!
#![crate_type = "cdylib"]
#![feature(alloc_system)]
extern crate imageflow_core as c;
extern crate alloc_system;
extern crate libc;
use c::ffi;
pub use c::ffi::{Job, Context};
pub use c::ffi::JobIO as JobIo;
pub use c::ffi::ImageflowJsonResponse as JsonResponse;
use std::ptr;

#[cfg(test)]
#[allow(unused_imports)]
use std::ffi::CStr;

#[cfg(test)]
use std::str;

///
/// What is possible with the IO object
#[repr(C)]
pub enum IoMode {
    None = 0,
    ReadSequential = 1,
    WriteSequential = 2,
    ReadSeekable = 5, // 1 | 4,
    WriteSeekable = 6, // 2 | 4,
    ReadWriteSeekable = 15, // 1 | 2 | 4 | 8
}

///
/// Input or output?
#[repr(C)]
#[derive(Copy,Clone)]
pub enum Direction {
    Out = 8,
    In = 4,
}

///
/// When a resource should be closed/freed/cleaned up
///
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CleanupWith{
    /// When the context is destroyed
    Context = 0,
    /// When the first job that the item is associated with is destroyed. (Not yet implemented)
    FirstJob = 1
}

///
/// How long the provided pointer/buffer will remain valid.
/// Callers must prevent the memory from being freed or moved until this contract expires.
///
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Lifetime{
    /// Pointer will outlive function call. (I.e, in .NET, the memory has been pinned through the end of the call, perhaps via the 'fixed' keyword)
    OutlivesFunctionCall = 0,
    /// Pointer will outlive context (Usually a GCHandle is required to pin an object for a longer time in C#)
    OutlivesContext = 1,
}



/// Creates a static, null-terminated Rust string, and
/// returns a ` *const libc::c_char` pointer to it.
///
/// Useful for API invocations that require a static C string
#[macro_export]
macro_rules! static_char {
    ($lit:expr) => {
        concat!($lit, "\0").as_ptr() as *const libc::c_char
    }
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
/// Returns a null pointer if allocation fails.
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_create() -> *mut Context {
    ffi::flow_context_create()
}

/// Begins the process of destroying the context, yet leaves error information intact
/// so that any errors in the tear-down process can be
/// debugged with imageflow_context_error_and_stacktrace.
///
/// Returns true if no errors occurred. Returns false if there were tear-down issues.
///
/// *Behavior is undefined if context is a null or invalid ptr.*
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_begin_terminate(context: *mut Context) -> bool {
    ffi::flow_context_begin_terminate(context)
}

/// Destroys the imageflow context and frees the context object.
/// Only use this with contexts created using imageflow_context_create
///
/// Behavior is undefined if context is a null or invalid ptr; may segfault on free(NULL);
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_destroy(context: *mut Context) {
    ffi::flow_context_destroy(context)
}


#[test]
fn test_create_destroy() {
    exercise_create_destroy();
}

pub fn exercise_create_destroy() {
    unsafe {
        let c = imageflow_context_create();
        assert!(!c.is_null());
        assert!(imageflow_context_begin_terminate(c));
        imageflow_context_destroy(c);
    }
}

/// Returns true if the context is in an error state. You must immediately deal with the error,
/// as subsequent API calls will fail or cause undefined behavior until the error state is cleared
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_has_error(context: *mut Context) -> bool {
    ffi::flow_context_has_error(context)
}

/// Clear the error state. This assumes that you know which API call failed and the problem has
/// been resolved. Don't use this unless you're sure you've accounted for all possible
/// inconsistent state (and fully understand the code paths that led to the error).
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_clear_error(context: *mut Context) {
    ffi::flow_context_clear_error(context)
}

/// Prints the error messages and stacktrace to the given buffer in UTF-8 form; writes a null
/// character to terminate the string, and *ALSO* returns the number of bytes written.
///
///
/// Happy(ish) path: Returns the length of the error message written to the buffer.
/// Sad path: Returns -1 if buffer_length was too small or buffer was nullptr.
/// full_file_path, if true, will display the directory associated with the files in each stack frame.
///
/// Please be accurate with the buffer length, or a buffer overflow will occur.
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_error_and_stacktrace(context: *mut Context,
                                                                buffer: *mut u8,
                                                                buffer_length: libc::size_t,
                                                                full_file_path: bool)
                                                                -> i64 {
    ffi::flow_context_error_and_stacktrace(context, buffer, buffer_length, full_file_path)
}

/// Returns the numeric code associated with the error.
///
/// ## Error codes
///
/// * 0 - No error condition.
/// * 10 - Out Of Memory condition (malloc/calloc/realloc failed).
/// * 20 - I/O error
/// * 30 - Invalid internal state (assertion failed; you found a bug).
/// * 40 - Error: Not implemented. (Feature not implemented).
/// * 50 - Invalid argument provided
/// * 51 - Null argument provided
/// * 52 - Invalid dimensions
/// * 53 - Unsupported pixel format
/// * 54 - Item does not exist
/// * 60 - Image decoding failed
/// * 61 - Image encoding failed
/// * 70 - Graph invalid
/// * 71 - Graph is cyclic
/// * 72 - Invalid inputs to node
/// * 73 - Maximum graph passes exceeded
/// * 1024 - Other error; something else happened
/// * 1025 through 2147483647 are reserved for user-defined errors
///
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_error_code(context: *mut Context) -> i32 {
    ffi::flow_context_error_reason(context)
}

/// Prints the error to stderr and exits the process if an error has been raised on the context.
/// If no error is present, the function returns false.
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
///
/// THIS PRINTS DIRECTLY TO STDERR! Do not use in any kind of service! Command-line usage only!
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_print_and_exit_if_error(context: *mut Context) -> bool {
    ffi::flow_context_print_and_exit_if_err(context)
}

///
/// Raises an error on the context.
///
/// Returns `true` on success, `false`  if an error was already present.
///
/// Designed to be safe(ish) for use in out-of-memory scenarios; no additional allocations are made.
///
/// See `imageflow_context_error_code` for a list of error codes.
///
///
/// # Expectations
///
/// * Strings `message` and `function_name`, and `filename` should be null-terminated UTF-8 strings.
/// * The lifetime of `message` is expected to exceed the duration of this function call.
/// * The lifetime of `filename` and `function_name` (if provided), is expected to match or exceed the lifetime of `context`.
/// * You may provide a null value for `filename` or `function_name`, but for the love of puppies,
/// don't provide a dangling or invalid pointer, that will segfault... a long time later.
///
/// # Caveats
///
/// * You cannot raise a second error until the first has been cleared with
///  `imageflow_context_clear_error`. You'll be ignored, as will future
///   `imageflow_add_to_callstack` invocations.
/// * Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
/// * Behavior is undefined if `message` is an invalid ptr; immediate segfault likely.
/// * If you provide an error code of zero (why?!), a different error code will be provided.
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_raise_error(context: *mut Context,
                                                       error_code: i32,
                                                       message: *const libc::c_char,
                                                       filename: *const libc::c_char,
                                                       line: i32,
                                                       function_name: *const libc::c_char)
                                                       -> bool {
    ffi::flow_context_raise_error(context, error_code, message, filename, line, function_name)
}

///
/// Adds the given filename, line number, and function name to the call stack.
/// Strings `function_name`, and `filename` should be null-terminated UTF-8 strings who will outlive `context`
///
/// Returns `true` if add was successful.
///
/// # Will fail and return false if...
///
/// * You haven't previously called `imageflow_context_raise_error`
/// * You tried to raise a second error without clearing the first one. Call will be ignored.
/// * You've exceeded the capacity of the call stack (which, at one point, was 14). But this
///   category of failure is acceptable.
///
///
/// # Expectations
///
/// * An error has been raised.
/// * You may provide a null value for `filename` or `function_name`, but for the love of puppies,
/// don't provide a dangling or invalid pointer, that will segfault... a long time later.
/// * The lifetime of `file` and `function_name` (if provided), is expected to match
///   or exceed the lifetime of `context`.
/// * All strings must be null-terminated, C-style, valid UTF-8.
///
/// # Caveats
///
/// * Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
///
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_add_to_callstack(context: *mut Context,
                                                            filename: *const libc::c_char,
                                                            line: i32,
                                                            function_name: *const libc::c_char)
                                                            -> bool {
    ffi::flow_context_add_to_callstack(context, filename, line, function_name)
}


#[test]
pub fn test_error_handling() {
    exercise_error_handling();
}
pub fn exercise_error_handling() {
    unsafe {
        let c = imageflow_context_create();
        assert!(!c.is_null());

        assert!(!imageflow_context_has_error(c));
        assert_eq!(imageflow_context_error_code(c), 0);

        // While the strings are static, the CStrings are not.
        // But they will persist until the end of the block, which is all we need
        let message = static_char!("Test message");
        let filename = static_char!(file!());
        let function_name = static_char!("test_error_handling");

        // Let's raise a nice error
        assert!(imageflow_context_raise_error(c, 1025, message, filename, 335, function_name));

        // Check it registered
        assert!(imageflow_context_has_error(c));
        assert_eq!(imageflow_context_error_code(c), 1025);

        // Add a backtrace
        assert!(imageflow_context_add_to_callstack(c, filename, 342, ptr::null()));
        assert!(imageflow_context_add_to_callstack(c, filename, 20, ptr::null()));
        assert!(imageflow_context_add_to_callstack(c, ptr::null(), 0, ptr::null()));

        // Let's see how it handles a insufficient buffer
        let mut tiny_buf: [u8; 30] = [0; 30];
        assert_eq!(-1,
        imageflow_context_error_and_stacktrace(c, &mut tiny_buf[0], 30, true));


        // And check the output looks right
        let mut buf: [u8; 2048] = [0; 2048];
        let buf_used = imageflow_context_error_and_stacktrace(c, &mut buf[0], 2048, true);

        assert!(buf_used >= 0);
        let actual_string = ::std::str::from_utf8(&buf[0..buf_used as usize]).unwrap();


        // let expected_string = "User defined error : Test message\nsrc/lib.rs:335: in function test_error_handling\nsrc/lib.rs:342: in function (unknown)\nsrc/lib.rs:20: in function (unknown)\n(unknown):0: in function (unknown)\n";
        assert!(actual_string.starts_with("User defined error : Test message\nsrc"));

        // raising a second error should fail
        assert!(!imageflow_context_raise_error(c,
                                               1025,
                                               message,
                                               filename,
                                               line!() as i32,
                                               ptr::null()));
        // as should trying to add to the call stack
        assert!(!imageflow_context_add_to_callstack(c, filename, line!() as i32, ptr::null()));
        assert!(!imageflow_context_add_to_callstack(c, filename, line!() as i32, ptr::null()));

        // Clearing the error should work
        imageflow_context_clear_error(c);
        assert!(!imageflow_context_has_error(c));
        assert_eq!(imageflow_context_error_code(c), 0);


        // And it should be possible to report another error
        assert!(imageflow_context_raise_error(c, 1025, message, ptr::null(), 1, ptr::null()));
        imageflow_context_clear_error(c);

        // And cleanup should go smoothly
        assert!(imageflow_context_begin_terminate(c));
        imageflow_context_destroy(c);
    }
}




///
/// Writes fields from the given imageflow_json_response to the locations referenced.
/// The buffer pointer sent out will be a UTF-8 byte array of the given length (not null-terminated). It will
/// also become invalid if the JsonResponse associated is freed, or if the context is destroyed.
///
#[no_mangle]
pub unsafe extern fn imageflow_json_response_read(context: *mut Context,
                                                  response_in: *const JsonResponse,
                                                  status_code_out: *mut i64,
                                                  buffer_utf8_no_nulls_out: *mut *const u8,
                                                  buffer_size_out: *mut libc::size_t) -> bool {
    if context.is_null() {
        return false;
    }
    if response_in.is_null() {
        imageflow_context_raise_error(context,
                                      51,
                                      static_char!("response_in is null"),
                                      static_char!(file!()),
                                      line!() as i32,
                                      ptr::null());
        return false;
    }

    if !status_code_out.is_null() {
        *status_code_out = (*response_in).status_code;
    }
    if !buffer_utf8_no_nulls_out.is_null() {
        *buffer_utf8_no_nulls_out = (*response_in).buffer_utf8_no_nulls;
    }
    if !buffer_size_out.is_null() {
        *buffer_size_out = (*response_in).buffer_size;
    }
    return true;
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
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
///
#[no_mangle]
pub unsafe extern "C" fn imageflow_json_response_destroy(context: *mut Context,
                                                         response: *mut JsonResponse)
                                                         -> bool {
    ffi::flow_destroy(context, response as *mut libc::c_void, ptr::null(), 0)
}

///
/// Sends a JSON message to the imageflow_context
///
/// The context is provided `method`, which determines which code path will be used to
/// process the provided JSON data and compose a response.
///
/// * `method` and `json_buffer` are only borrowed for the duration of the function call. You are
///    responsible for their cleanup (if necessary - static strings are handy for things like
///    `method`).
/// * `method` should be a UTF-8 null-terminated string.
///   `json_buffer` should be a UTF-8 encoded buffer (not null terminated) of length json_buffer_size.
///
/// The function will return NULL if a JSON response could not be allocated (or if some other
/// bug occurred). If a null pointer is returned, consult the standard error methods of `context`
/// for more detail.
///
/// The response can be cleaned up with `imageflow_json_response_destroy`
///
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
#[no_mangle]
#[allow(unused_variables)]
pub unsafe extern "C" fn imageflow_context_send_json(context: *mut Context,
                                                     method: *const libc::c_char,
                                                     json_buffer: *const u8,
                                                     json_buffer_size: libc::size_t)
                                                     -> *const JsonResponse {
    imageflow_send_json(context, ptr::null_mut(), ptr::null_mut(), method, json_buffer, json_buffer_size)
}

///
/// Sends a JSON message to the imageflow_job
///
/// The recipient is provided `method`, which determines which code path will be used to
/// process the provided JSON data and compose a response.
///
/// * `method` and `json_buffer` are only borrowed for the duration of the function call. You are
///    responsible for their cleanup (if necessary - static strings are handy for things like
///    `method`).
/// * `method` should be a UTF-8 null-terminated string.
///   `json_buffer` should be a UTF-8 encoded buffer (not null terminated) of length json_buffer_size.
///
/// The function will return NULL if a JSON response could not be allocated (or if some other
/// bug occurred). If a null pointer is returned, consult the standard error methods of `context`
/// for more detail.
///
/// The response can be cleaned up with `imageflow_json_response_destroy`
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
#[no_mangle]
#[allow(unused_variables)]
pub unsafe extern "C" fn imageflow_job_send_json(context: *mut Context,
                                                 job: *mut Job,
                                                 method: *const libc::c_char,
                                                 json_buffer: *const libc::uint8_t,
                                                 json_buffer_size: libc::size_t)
                                                 -> *const JsonResponse {
    imageflow_send_json(context, job, ptr::null_mut(), method, json_buffer, json_buffer_size)
}

///
/// Sends a JSON message to one of 3 recipients.
///
/// 1. `imageflow_context`, If both `job` and `io` are both null. Required.
/// 2. `imageflow_job`, if only `io` is null.
/// 3. `imageflow_io`, if `io` is not null. `job` is ignored.
///
/// The recipient is then provided `method`, which determines which code path will be used to
/// process the provided JSON data and compose a response.
///
/// * `method` and `json_buffer` are only borrowed for the duration of the function call. You are
///    responsible for their cleanup (if necessary - static strings are handy for things like
///    `method`).
///
/// The function will return NULL if a JSON response could not be allocated (or if some other
/// bug occurred). If a null pointer is returned, consult the standard error methods of `context`
/// for more detail.
///
/// The response can be cleaned up with `imageflow_json_response_destroy`
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
#[allow(unused_variables)]
unsafe fn imageflow_send_json(context: *mut Context,
                              job: *mut Job,
                              io: *mut JobIo,
                              method: *const i8,
                              json_buffer: *const libc::uint8_t,
                              json_buffer_size: libc::size_t)
                              -> *const JsonResponse {
    //TODO: check for nulls in json_BUFFER AND METHOD

    // It doesn't appear that this allocates anything
    // But I'm curious how that Rust knows not to deallocate the string itself
    // Oh. It's borrowed. CStr is borrowed from the pointer. Pointers don't have lifetimes. &str is borrowed from CStr
    let method_str = ::std::ffi::CStr::from_ptr(method as *const i8).to_str().unwrap(); //TODO, throw exception if not UTF-8, argument error

    let json_bytes = std::slice::from_raw_parts(json_buffer, json_buffer_size);

    // TODO: possibly iterate access to force segfaults earlier?


    let mut ctx = c::ContextPtr::from_ptr(context);


    let response;

    if job == ptr::null_mut() && io == ptr::null_mut() {
        response = ctx.message(method_str, json_bytes).unwrap(); //Unwrap for invalid context
    } else if io == ptr::null_mut() {
        response = c::JobPtr::from_ptr(context, job).unwrap().message(method_str, json_bytes).unwrap();
    } else{
        panic!("Messaging JobIo not implemented");
    }


    let ptr = ctx.create_abi_json_response(response.response_json, response.status_code);
    //TODO: if ptr == null,
    //Handle alloc failure
    ptr
}



#[test]
fn test_message() {
    exercise_json_message();
}

pub fn exercise_json_message() {
    unsafe {
        let c = imageflow_context_create();
        assert!(!c.is_null());

        let method_in = static_char!("brew_coffee");
        let json_in = "{}";
        let expected_response = c::JsonResponse::teapot();
        let expected_json_out = ::std::str::from_utf8(
            expected_response.response_json.as_ref()).unwrap();
        let expected_reponse_status = expected_response.status_code;

        let response = imageflow_send_json(c,
                                           ptr::null_mut(),
                                           ptr::null_mut(),
                                           method_in,
                                           json_in.as_ptr(),
                                           json_in.len());

        assert!(response != ptr::null());

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

        assert_eq!(json_status_code, expected_reponse_status);

        imageflow_context_destroy(c);
    }
}

///
/// Creates an imageflow_io object to wrap a filename.
///
/// The filename should be a null-terminated string. It should be written in codepage used by your operating system for handling `fopen` calls.
/// https://msdn.microsoft.com/en-us/library/yeby3zcb.aspx
///
/// If the filename is fopen compatible, you're probably OK.
///
/// As always, `mode` is not enforced except for the file open flags.
///
#[no_mangle]
#[allow(unused_variables)]
pub unsafe extern "C" fn imageflow_io_create_for_file(context: *mut Context,
                                                      mode: IoMode,
                                                      filename: *const libc::c_char,
                                                      cleanup: CleanupWith)
                                                      -> *mut JobIo {
    // TODO: validate that 'owner' is capable of being an owner

    ffi::flow_io_create_for_file(context, std::mem::transmute(mode), filename, context as *const libc::c_void)
}


///
/// Creates an imageflow_io structure for reading from the provided buffer.
/// You are ALWAYS responsible for freeing the memory provided in accordance with the Lifetime value.
/// If you specify OutlivesFunctionCall, then the buffer will be copied.
///
///
#[no_mangle]
#[allow(unused_variables)]
pub unsafe extern "C" fn imageflow_io_create_from_buffer(context: *mut Context,
                                                         buffer: *const u8,
                                                         buffer_byte_count: libc::size_t,
                                                            lifetime: Lifetime,
                                                            cleanup: CleanupWith)
                                                         -> *mut JobIo {

    let mut final_buffer = buffer;
    if lifetime == Lifetime::OutlivesFunctionCall {
        let buf : *mut u8 = c::ffi::flow_context_calloc(context, 1, buffer_byte_count, ptr::null(), context as *const libc::c_void, ptr::null(), 0) as *mut u8 ;
        if buf.is_null() {
            //TODO: raise OOM
            return ptr::null_mut();
        }
        ptr::copy_nonoverlapping(buffer, buf, buffer_byte_count);

        final_buffer = buf;
    }
    ffi::flow_io_create_from_memory(context, std::mem::transmute(IoMode::ReadSeekable), final_buffer, buffer_byte_count, context as *mut libc::c_void, ptr::null())
}


///
/// Creates an imageflow_io structure for writing to an expanding memory buffer.
///
/// Reads/seeks, are, in theory, supported, but unless you've written, there will be nothing to read.
///
/// The I/O structure and buffer will be freed with the context.
///
///
/// Returns null if allocation failed; check the context for error details.
#[no_mangle]
#[allow(unused_variables)]
pub unsafe extern "C" fn imageflow_io_create_for_output_buffer(context: *mut Context)
                                                               -> *mut JobIo {
    // The current implementation of output buffer only sheds its actual buffer with the context.
    // No need for the shell to have an earlier lifetime for mem reasons.
    ffi::flow_io_create_for_output_buffer(context, context as *mut libc::c_void)
}


// Returns false if the flow_io struct is disposed or not an output buffer type (or for any other error)
//

///
/// Provides access to the underlying buffer for the given imageflow_io object.
///
/// Ensure your length variable always holds 64-bits.
///
#[no_mangle]
pub unsafe extern "C" fn imageflow_io_get_output_buffer(context: *mut Context,
                                                        io: *mut JobIo,
                                                        result_buffer: *mut *const u8,
                                                        result_buffer_length: *mut libc::size_t)
                                                        -> bool {

    let mut result_len: usize = 0;
    let b = ffi::flow_io_get_output_buffer(context, io, result_buffer, &mut result_len);
        (* result_buffer_length) = result_len;
    b
}

///
/// Provides access to the underlying buffer for the given imageflow_io object.
///
/// Ensure your length variable always holds 64-bits
///
#[no_mangle]
pub unsafe extern "C" fn imageflow_job_get_output_buffer_by_id(context: *mut Context,
                                                               job: *mut Job,
                                                               io_id: i32,
                                                               result_buffer: *mut *const u8,
                                                               result_buffer_length: *mut libc::size_t)
                                                               -> bool {
    let io = ffi::flow_job_get_io(context,job, io_id);
    if io.is_null(){
        return false;
    }else {
        imageflow_io_get_output_buffer(context, io, result_buffer, result_buffer_length)
    }
}


///
/// Creates an imageflow_job, which permits the association of imageflow_io instances with
/// numeric identifiers and provides a 'sub-context' for job execution
///
#[no_mangle]
pub unsafe extern "C" fn imageflow_job_create(context: *mut Context) -> *mut Job {
    ffi::flow_job_create(context)
}


///
/// Looks up the imageflow_io pointer from the provided placeholder_id
///
#[no_mangle]
pub unsafe extern "C" fn imageflow_job_get_io(context: *mut Context,
                                              job: *mut Job,
                                              placeholder_id: i32)
                                              -> *mut JobIo {
    ffi::flow_job_get_io(context, job, placeholder_id)
}

///
/// Associates the imageflow_io object with the job and the assigned placeholder_id.
///
/// The placeholder_id will correspond with io_id in the graph
///
/// direction is in or out.
#[no_mangle]
pub unsafe extern "C" fn imageflow_job_add_io(context: *mut Context,
                                              job: *mut Job,
                                              io: *mut JobIo,
                                              placeholder_id: i32,
                                              direction: Direction)
                                              -> bool {
    ffi::flow_job_add_io(context, job, io, placeholder_id, std::mem::transmute(direction))
}

///
/// Destroys the provided imageflow_job
///
#[no_mangle]
pub unsafe extern "C" fn imageflow_job_destroy(context: *mut Context, job: *mut Job) -> bool {
    ffi::flow_job_destroy(context, job)
}


///
/// Allocates zeroed memory that will be freed with the context.
///
/// * filename/line may be used for debugging purposes. They are optional. Provide null/-1 to skip.
/// * `filename` should be an null-terminated UTF-8 or ASCII string which will outlive the context.
///
/// Returns null(0) on failure.
///
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_memory_allocate(context: *mut Context,
                                                    bytes: libc::size_t,
                                                    filename: *const libc::c_char,
                                                    line: i32) -> *mut libc::c_void {

    ffi::flow_context_calloc(context, 1, bytes, ptr::null(), context as *const libc::c_void, filename, line)
}

///
/// Frees memory allocated with imageflow_context_memory_allocate early.
///
/// * filename/line may be used for debugging purposes. They are optional. Provide null/-1 to skip.
/// * `filename` should be an null-terminated UTF-8 or ASCII string which will outlive the context.
///
/// Returns false on failure.
///
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_memory_free(context: *mut Context,
                                                       pointer: *mut libc::c_void,
                                                       filename: *const libc::c_char,
                                                       line: i32) -> bool {
    ffi::flow_destroy(context, pointer, filename, line)
}

#[test]
fn test_allocate_free() {
    unsafe{
        let c = imageflow_context_create();
        let bytes = 100;
        let ptr = imageflow_context_memory_allocate(c, bytes, static_char!(file!()),
                                                    line!() as i32) as *mut u8;
        assert!(ptr != ptr::null_mut());

        for x in 0..bytes{
            assert_eq!(*ptr.offset(x as isize), 0);
        }
        assert!(imageflow_context_memory_free(c, ptr as *mut libc::c_void, static_char!(file!()),
                                              line!() as i32));
    }
}