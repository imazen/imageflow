//! # Purpose
//!
//! This module contains the functions exported for use by other languages.
//!
//!
//! If you're writing bindings, you're in the right place. Don't use `imageflow_core::ffi`
//!
//! As of ABI 3.1, calls are thread-safe.
//!
//! Don't worry about the performance of creating/destroying contexts.
//! A context weighs only 1100 bytes (6 allocations) as of Oct 2025.
//!
//! # Thread Safety
//!
//! **All ABI functions are thread-safe as of ABI version 3.1.**
//!
//! A single `imageflow_context` may be safely accessed from multiple threads concurrently.
//! Internal locks ensure that operations are serialized and data races cannot occur.
//!
//! ## Concurrency Behavior
//!
//! * **Serialization:** Multiple threads calling functions on the same context will serialize
//!   (block) due to internal write locks. This is intentional and prevents data corruption.
//! * **Error State:** Error state is protected by locks. The first error/panic is preserved;
//!   subsequent errors on the same context are ignored (fail-fast on first error principle).
//! * **Lock Ordering:** Internal locks are always acquired in a consistent order to prevent deadlocks.
//! * **Performance:** For best performance with multiple threads, use separate contexts per thread.
//!
//! ## Important: Not Reentrant
//!
//! Do NOT call imageflow functions from within a callback or signal handler that might be triggered
//! during an imageflow operation on the same context. This could lead to deadlock as the context
//! locks are not reentrant.
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
//! * The above includes *mut ThreadSafeContext an *mut `JsonResponse`
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
//! If you try to continue using an errored `imageflow_context`, some operations may fail.
//! Check `imageflow_context_has_error()` before proceeding with additional operations.
//! Some errors can be recovered from using `imageflow_context_error_try_clear()`, but you *must*
//! check if the error is recoverable first using `imageflow_context_error_recoverable()`.
//!
//! For all APIS: You'll likely segfault the process if you provide a `context` pointer that is dangling or invalid.
//!
//! # Safety Guarantees for Binding Authors
//!
//! ## Panic Safety
//!
//! **All panics are caught at the FFI boundary.** Internal Rust panics will never unwind across
//! the FFI boundary into your code. Instead:
//!
//! * Panics are caught using `catch_unwind`
//! * Panic information is stored in the context's error state
//! * The function returns a failure value (false, null, or error code)
//! * You can retrieve panic details via `imageflow_context_error_write_to_buffer`
//!
//! ## Lock Poisoning
//!
//! In the rare case where a panic occurs while holding internal locks:
//!
//! * The context enters a "degraded" state
//! * Subsequent operations will fail with `ErrorKind::FailedBorrow`
//! * The context remains memory-safe and can be destroyed normally
//! * **This should not happen in practice** - all panics are caught before they can poison locks
//!
//! ## Memory Safety Rules for Bindings
//!
//! To maintain memory safety, your bindings MUST enforce these rules:
//!
//! ### 1. Context Lifetime
//!
//! * A context pointer becomes invalid after `imageflow_context_destroy` is called
//! * **Never** use a context pointer after destroying it (use-after-free)
//! * **Never** destroy a context twice (double-free)
//! * Tip: Set the pointer to NULL after destroy to catch bugs early
//!
//! ### 2. Buffer Lifetimes (Critical!)
//!
//! When you pass a buffer to imageflow with `Lifetime::OutlivesContext`:
//!
//! * The buffer MUST remain valid and unmodified until context destruction
//! * **Never** free, move, or reallocate the buffer while the context exists
//! * In garbage-collected languages, pin the buffer to prevent GC movement
//! * Violation causes undefined behavior (use-after-free, data corruption)
//!
//! Use `Lifetime::OutlivesFunctionCall` if you cannot guarantee buffer lifetime - imageflow will
//! copy the data (at a performance cost).
//!
//! ### 3. Returned Pointers
//!
//! Pointers returned by imageflow (JsonResponse, output buffers, etc.):
//!
//! * Are valid until the context is destroyed OR until explicitly freed
//! * Become invalid if the context is destroyed
//! * **Must not** be freed by your allocator (use imageflow functions to free)
//! * Are read-only unless documented otherwise
//!
//! ### 4. Thread Safety Requirements
//!
//! * You MAY call imageflow functions from multiple threads on the same context
//! * You MUST NOT call imageflow from within callbacks/signals during an operation
//! * You MUST NOT destroy a context while another thread is using it
//!
//! ### 5. Error Handling Requirements
//!
//! * ALWAYS check for errors after operations that can fail
//! * Do NOT ignore errors - they indicate the context may be in an invalid state
//! * Handle errors before attempting more operations (or they may fail)
//! * You MAY retry operations after clearing recoverable errors
//!
//! ## Undefined Behavior
//!
//! The following will cause undefined behavior (crashes, corruption, security vulnerabilities):
//!
//! * Passing a null context pointer (process will abort with diagnostic)
//! * Passing a dangling pointer (freed, moved, or never valid)
//! * Use-after-free (using context/buffers after destroying them)
//! * Buffer lifetime violations (freeing buffers while context uses them)
//! * Concurrent destroy (destroying context while another thread uses it)
//! * Invalid buffer pointers or sizes (will attempt early detection but not guaranteed)
//! * Calling imageflow recursively from the same thread (deadlock)
//!
//! ## Safe Patterns
//!
//! ```c
//! // Example safe usage pattern in C
//! imageflow_context* ctx = imageflow_context_create(3, 1);
//! if (!ctx) {
//!     // Handle allocation failure
//!     return NULL;
//! }
//!
//! // Perform operations
//! bool success = imageflow_context_add_output_buffer(ctx, 1);
//! if (!success || imageflow_context_has_error(ctx)) {
//!     // Handle error
//!     char buffer[1024];
//!     size_t bytes_written;
//!     imageflow_context_error_write_to_buffer(ctx, buffer, sizeof(buffer), &bytes_written);
//!     fprintf(stderr, "Error: %s\n", buffer);
//!
//!     imageflow_context_destroy(ctx);
//!     return NULL;
//! }
//!
//! // ... more operations ...
//!
//! // Always destroy context
//! imageflow_context_destroy(ctx);
//! ctx = NULL;  // Good practice to prevent use-after-free
//! ```
//!
#![crate_type = "cdylib"]
#![cfg_attr(feature = "nightly", feature(core_intrinsics))]

// These functions are not for use from Rust, so marking them unsafe just reduces compile-time verification and safety
//#![cfg_attr(feature = "cargo-clippy", allow(not_unsafe_ptr_arg_deref))]

#[macro_use]
extern crate imageflow_core as c;

extern crate backtrace;
extern crate libc;
extern crate smallvec;

pub use crate::c::ffi::ImageflowJsonResponse as JsonResponse;
pub use crate::c::{Context, ErrorCategory, FlowError, ThreadSafeContext};
//use c::IoDirection;
use crate::c::ErrorKind;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;
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
pub enum Lifetime {
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
    };
}

fn type_name_of<T>(_: T) -> &'static str {
    extern crate core;
    std::any::type_name::<T>()
}

fn parent_function_name<T>(f: T) -> &'static str {
    let name = type_name_of(f);
    name[..name.len() - 3].rsplit_terminator(":").next().unwrap_or("[function name not found]")
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
        (unsafe { &mut *$ptr })
    }};
}

macro_rules! handle_result {
    ($outward_error:ident, $result:expr, $failure_value:expr) => {{
        match $result {
            Ok(Ok(v)) => v,
            Err(p) => {
                $outward_error.try_set_panic_error(p);
                $failure_value
            }
            Ok(Err(error)) => {
                $outward_error.try_set_error(error);
                $failure_value
            }
        }
    }};
}

macro_rules! lock_context_mut_and_error_or_return {
    ($ctx:ident,$failure_value:expr) => {{
        match $ctx.context_mut_and_error_or_poisoned() {
            (outward_error, Ok(guard)) => (outward_error, guard),
            (mut outward_error, Err(_)) => {
                fn f() {}
                let name = parent_function_name(f);
                outward_error.try_set_error(nerror!(
                    ErrorKind::FailedBorrow,
                    "Context previously panicked: {} cannot be called",
                    name
                ));
                return $failure_value;
            }
        }
    }};
}
macro_rules! lock_context_mut_or_return {
    ($ctx:ident,$failure_value:expr) => {{
        match $ctx.context_mut_and_error_or_poisoned() {
            (_, Ok(guard)) => guard,
            (mut outward_error, Err(_)) => {
                fn f() {}
                let name = parent_function_name(f);
                outward_error.try_set_error(nerror!(
                    ErrorKind::FailedBorrow,
                    "Context previously panicked: {} cannot be called",
                    name
                ));
                return $failure_value;
            }
        }
    }};
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
#[allow(clippy::absurd_extreme_comparisons)]
pub extern "C" fn imageflow_abi_compatible(
    imageflow_abi_ver_major: u32,
    imageflow_abi_ver_minor: u32,
) -> bool {
    imageflow_abi_ver_major == IMAGEFLOW_ABI_VER_MAJOR
        && imageflow_abi_ver_minor <= IMAGEFLOW_ABI_VER_MINOR
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
/// **As of ABI 3.1, contexts ARE thread-safe!** A single context may be accessed from multiple
/// threads concurrently. Operations will serialize internally to prevent data races.
/// For best performance with multiple threads, use separate contexts per thread.
///
/// Returns a null pointer if allocation fails or the provided interface version is incompatible.
///
/// # Panics
///
/// Cannot panic - all panics during creation are caught and result in returning null.
///
/// # Thread Safety
///
/// This function is safe to call from multiple threads simultaneously. Each call creates an
/// independent context.
#[no_mangle]
pub extern "C" fn imageflow_context_create(
    imageflow_abi_ver_major: u32,
    imageflow_abi_ver_minor: u32,
) -> *mut ThreadSafeContext {
    if imageflow_abi_compatible(imageflow_abi_ver_major, imageflow_abi_ver_minor) {
        ThreadSafeContext::create_cant_panic().map(Box::into_raw).unwrap_or(std::ptr::null_mut())
    } else {
        ptr::null_mut()
    }
}

/// Begins the process of destroying the context, yet leaves error information intact
/// so that any errors in the tear-down process can be
/// debugged with `imageflow_context_error_write_to_buffer`.
///
/// Returns true if no errors occurred. Returns false if there were tear-down issues.
///
/// # Safety
///
/// * `context` must be a valid pointer returned from `imageflow_context_create`
/// * `context` must not have been previously destroyed
/// * No other threads may be accessing this context during or after this call
/// * After calling this function, only error-retrieval functions and `imageflow_context_destroy`
///   may be called on this context
///
/// # Panics
///
/// Cannot panic - designed to be panic-safe for cleanup paths.
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_begin_terminate(
    context: *mut ThreadSafeContext,
) -> bool {
    let c = context!(context);
    c.abi_begin_terminate()
}

/// Destroys the imageflow context and frees the context object.
/// Only use this with contexts created using `imageflow_context_create`
///
/// All memory associated with the context is freed, including:
/// * The context itself
/// * All buffers created by imageflow for this context
/// * All JsonResponse objects not explicitly freed
/// * All internal allocations
///
/// # Safety
///
/// * `context` must be either NULL (no-op) or a valid pointer from `imageflow_context_create`
/// * `context` must not have been previously destroyed (double-free)
/// * No other threads may be accessing this context during or after this call
/// * After this call, the pointer is invalid - do not use it (use-after-free)
/// * All pointers to data owned by this context become invalid (output buffers, JsonResponse, etc.)
///
/// # Panics
///
/// Cannot panic - destruction is panic-safe and will attempt cleanup even in degraded states.
///
/// # Thread Safety
///
/// This function is NOT safe to call while other threads are using the context.
/// Ensure all threads have finished using the context before destroying it.
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_destroy(context: *mut ThreadSafeContext) {
    if !context.is_null() {
        unsafe {
            // Let it drop, the drop iml will sync locks.
            let _ = Box::from_raw(context);
        }
    }
}

#[test]
fn test_create_destroy() {
    exercise_create_destroy();
}

pub fn exercise_create_destroy() {
    unsafe {
        let c = imageflow_context_create(IMAGEFLOW_ABI_VER_MAJOR, IMAGEFLOW_ABI_VER_MINOR);
        assert!(!c.is_null());
        assert!(imageflow_context_begin_terminate(c));
        imageflow_context_destroy(c);
    }
}

/// Returns true if the context is in an error state.
///
/// When true, you should retrieve error details and handle the error before attempting more operations.
/// Some operations will fail or return incorrect results if the context is in an error state.
///
/// # Safety
///
/// * `context` must be a valid pointer from `imageflow_context_create`
/// * `context` must not be NULL (will abort process)
/// * `context` must not have been destroyed
///
/// # Thread Safety
///
/// Safe to call from multiple threads on the same context (acquires read lock).
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_has_error(context: *mut ThreadSafeContext) -> bool {
    context!(context).outward_error().has_error()
}

/// Returns true if the context is "ok" or in an error state that is recoverable.
///
/// A recoverable error is one that can be cleared with `imageflow_context_error_try_clear()`.
/// Panics and some critical errors are NOT recoverable.
///
/// # Safety
///
/// * `context` must be a valid pointer from `imageflow_context_create`
/// * `context` must not be NULL (will abort process)
/// * `context` must not have been destroyed
///
/// # Thread Safety
///
/// Safe to call from multiple threads on the same context (acquires read lock).
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_error_recoverable(
    context: *mut ThreadSafeContext,
) -> bool {
    context!(context).outward_error().recoverable()
}

/// Attempts to clear a recoverable error from the context.
///
/// Returns true if the error was cleared (or if there was no error).
/// Returns false if the error is not recoverable (panic, critical error).
///
/// You MUST check `imageflow_context_error_recoverable()` before calling this.
///
/// # Safety
///
/// * `context` must be a valid pointer from `imageflow_context_create`
/// * `context` must not be NULL (will abort process)
/// * `context` must not have been destroyed
///
/// # Thread Safety
///
/// Safe to call from multiple threads on the same context (acquires write lock).
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_error_try_clear(
    context: *mut ThreadSafeContext,
) -> bool {
    context!(context).outward_error_mut().try_clear()
}

/// Returns the numeric code associated with the error category. 0 means no error.
///
/// These will be stabilized after 1.0, once error categories have passed rigorous real-world testing
/// `imageflow_context_error_as_exit_code` and `imageflow_context_error_as_http_status` are suggested in the meantime.
/// Unstable, do not use.
///
/// # Safety
///
/// * `context` must be a valid pointer from `imageflow_context_create`
/// * `context` must not be NULL (will abort process)
/// * `context` must not have been destroyed
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_error_code(context: *mut ThreadSafeContext) -> i32 {
    context!(context).outward_error().category().to_outward_error_code()
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
/// # Safety
///
/// * `context` must be a valid pointer from `imageflow_context_create`
/// * `context` must not be NULL (will abort process)
/// * `context` must not have been destroyed
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_error_as_exit_code(
    context: *mut ThreadSafeContext,
) -> i32 {
    context!(context).outward_error().category().process_exit_code()
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
/// # Safety
///
/// * `context` must be a valid pointer from `imageflow_context_create`
/// * `context` must not be NULL (will abort process)
/// * `context` must not have been destroyed
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_error_as_http_code(
    context: *mut ThreadSafeContext,
) -> i32 {
    context!(context).outward_error().category().http_status_code()
}

/// Writes error messages (and stack frames) to the provided buffer in UTF-8 format.
///
/// The output is null-terminated. The number of bytes written (excluding null terminator)
/// is written to `bytes_written` if it's not NULL.
///
/// ## Return Value
///
/// * Returns **true** if all error data was written successfully
/// * Returns **false** if the buffer was too small and output was truncated
/// * Returns **false** if buffer is NULL
///
/// When truncated, the buffer will contain "\n[truncated]\n" at the end (before the null terminator).
///
/// ## Usage
///
/// ```c
/// char error_buffer[1024];
/// size_t bytes_written;
/// if (!imageflow_context_error_write_to_buffer(ctx, error_buffer, sizeof(error_buffer), &bytes_written)) {
///     // Buffer was too small, error message truncated
/// }
/// printf("Error: %s\n", error_buffer);
/// ```
///
/// # Safety
///
/// * `context` must be a valid pointer from `imageflow_context_create`
/// * `context` must not be NULL (will abort process)
/// * `buffer` must be either NULL or a valid pointer to at least `buffer_length` writable bytes
/// * `buffer_length` must accurately reflect the buffer size - incorrect size causes buffer overflow
/// * `buffer_length` must not have the most significant bit set (max 2^31 or 2^63)
/// * `bytes_written`, if not NULL, must be a valid pointer to write a size_t
///
/// # Thread Safety
///
/// Safe to call from multiple threads on the same context (acquires read lock on error state).
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_error_write_to_buffer(
    context: *mut ThreadSafeContext,
    buffer: *mut libc::c_char,
    buffer_length: libc::size_t,
    bytes_written: *mut libc::size_t,
) -> bool {
    if buffer.is_null() {
        false
    } else {
        use crate::c::errors::writing_to_slices::WriteResult;
        let c = context!(context);

        if buffer_length.leading_zeros() == 0 {
            c.outward_error_mut().try_set_error(nerror!(ErrorKind::InvalidArgument, "Argument `buffer_length` likely came from a negative integer. Imageflow prohibits having the leading bit set on unsigned integers (this reduces the maximum value to 2^31 or 2^63)."));
            return false;
        }

        let result = unsafe {
            c.outward_error().get_buffer_writer().write_and_write_errors_to_cstring(
                buffer as *mut u8,
                buffer_length,
                Some("\n[truncated]\n"),
            )
        };
        if !bytes_written.is_null() {
            unsafe {
                *bytes_written = result.bytes_written();
            }
        }
        match result {
            WriteResult::AllWritten(_) | WriteResult::Error { .. } => true,
            WriteResult::TruncatedAt(_) => false,
        }
    }
}

/// Prints the error to stderr and exits the process if an error has been raised on the context.
///
/// **WARNING: THIS FUNCTION CALLS exit() AND TERMINATES THE PROCESS!**
///
/// If an error is present:
/// * Prints error details to stderr
/// * Calls `exit()` with an appropriate exit code (see `imageflow_context_error_as_exit_code`)
/// * **DOES NOT RETURN**
///
/// If no error is present:
/// * Returns false
/// * Does not exit
///
/// **DO NOT USE IN SERVICES, LIBRARIES, OR LONG-RUNNING PROCESSES!**
/// This is only appropriate for command-line utilities where process termination is acceptable.
///
/// # Safety
///
/// * `context` must be a valid pointer from `imageflow_context_create`
/// * `context` must not be NULL (will abort process)
///
/// # Thread Safety
///
/// If called while other threads are running, those threads will be terminated without cleanup.
/// Only use this in single-threaded command-line programs.
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_print_and_exit_if_error(
    context: *mut ThreadSafeContext,
) -> bool {
    let e = context!(context).outward_error();
    if e.has_error() {
        eprintln!("{}", e);
        std::process::exit(e.category().process_exit_code())
    } else {
        false
    }
}

/// Reads fields from a JsonResponse and writes them to the provided output parameters.
///
/// The buffer pointer will be a UTF-8 byte array (NOT null-terminated).
///
/// **Important:** The buffer pointer is only valid until:
/// * The JsonResponse is freed with `imageflow_json_response_destroy`, OR
/// * The context is destroyed
///
/// Any of the output parameters may be NULL if you don't need that field.
///
/// Returns false if `response_in` is NULL.
///
/// # Safety
///
/// * `context` must be a valid pointer from `imageflow_context_create`
/// * `context` must not be NULL (will abort process)
/// * `response_in` must be either NULL or a valid pointer from `imageflow_context_send_json`
/// * `status_as_http_code_out`, if not NULL, must be a valid pointer to write an i64
/// * `buffer_utf8_no_nulls_out`, if not NULL, must be a valid pointer to write a *const u8
/// * `buffer_size_out`, if not NULL, must be a valid pointer to write a size_t
/// * The returned buffer pointer is only valid until response/context destruction
///
/// # Thread Safety
///
/// The response object is not protected by locks. Do not read a response while another thread
/// might be freeing it or destroying the context.
#[no_mangle]
pub unsafe extern "C" fn imageflow_json_response_read(
    context: *mut ThreadSafeContext,
    response_in: *const JsonResponse,
    status_as_http_code_out: *mut i64,
    buffer_utf8_no_nulls_out: *mut *const u8,
    buffer_size_out: *mut libc::size_t,
) -> bool {
    let c = context!(context); // Must be readable in error state

    // JsonResponse is stored in the context, so we need to lock it. It won't be locked for the duration of the client's use
    // So essentially this is not thread safe in the end.
    //
    if response_in.is_null() {
        c.outward_error_mut().try_set_error(nerror!(
            ErrorKind::NullArgument,
            "The argument response_in (* JsonResponse) is null."
        ));
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

/// Frees a JsonResponse object early.
///
/// This is optional - JsonResponse objects are automatically freed when the context is destroyed.
/// Use this to reduce memory usage if you're done with a response before destroying the context.
///
/// Returns true if successful or if `response` is NULL.
/// Returns false if the pointer was not found in the allocation list.
///
/// # Safety
///
/// * `context` must be a valid pointer from `imageflow_context_create`
/// * `context` must not be NULL (will abort process)
/// * `context` must be the same context that created the response
/// * `response` must be either NULL or a valid pointer from `imageflow_context_send_json` on this context
/// * `response` must not have been previously freed (double-free)
/// * After this call, `response` and its buffer are invalid - do not use them (use-after-free)
///
/// # Panics
///
/// Cannot panic - safe to use in cleanup paths.
///
/// # Thread Safety
///
/// Safe to call from multiple threads on the same context, but ensure no other thread is reading
/// this specific response object.
#[no_mangle]
pub unsafe extern "C" fn imageflow_json_response_destroy(
    context: *mut ThreadSafeContext,
    response: *mut JsonResponse,
) -> bool {
    let context = context!(context);
    imageflow_context_memory_free(context, response as *mut libc::c_void, ptr::null(), 0)
}

/// Requests cancellation of any running or future operations on this context.
///
/// This sets an atomic cancellation flag that operations check periodically.
/// When an operation detects the cancellation flag, it will abort and return an error.
///
/// **Important:** Cancellation is not guaranteed to be immediate. Some operations may complete
/// before detecting the flag. Once cancellation is requested, the context enters an error state.
///
/// Error details: `ErrorKind::OperationCancelled`, error code 21, HTTP 499, exit code 130
///
/// No further operations can be attempted after cancellation - the context is in an errored state.
///
/// # Safety
///
/// * `context` must be a valid pointer from `imageflow_context_create`
/// * `context` must not be NULL (will abort process)
///
/// # Panics
///
/// Cannot panic.
///
/// # Thread Safety
///
/// **This is the only function specifically designed to be called from another thread while an
/// operation is running.** Use this to implement timeouts or user cancellation.
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_request_cancellation(context: *mut ThreadSafeContext) {
    let c: &mut ThreadSafeContext = context!(context);
    c.request_cancellation();
}

/// Sends a JSON message to the imageflow context using the specified endpoint method.
///
/// This is the primary API for invoking imageflow operations.
///
/// ## Endpoints
///
/// * `v1/build` - Build and execute an image processing job
/// * `v1/execute` - Execute a pre-configured operation graph
/// * `v1/get_version_info` - Get version and build information
///
/// For the latest endpoints, see:
/// `https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/context_json_api.txt`
///
/// ## Parameters
///
/// * `method` - UTF-8 null-terminated string specifying the endpoint (e.g., "v1/build")
/// * `json_buffer` - UTF-8 encoded JSON (NOT null-terminated)
/// * `json_buffer_size` - Length of json_buffer in bytes
///
/// ## Return Value
///
/// Returns a pointer to a `JsonResponse` on success or error. The response contains:
/// * HTTP-style status code (200 for success, 4xx/5xx for errors)
/// * UTF-8 JSON response buffer
///
/// Returns NULL only if:
/// * A panic occurred and could not be converted to a JSON error
/// * Invalid arguments were provided
///
/// **Always check `imageflow_context_has_error()` after calling this function.**
///
/// The returned JsonResponse is owned by the context and remains valid until:
/// * You call `imageflow_json_response_destroy()`, OR
/// * You destroy the context
///
/// ## Memory Lifetime
///
/// * `method` and `json_buffer` are only borrowed during this function call
/// * You remain responsible for freeing them if dynamically allocated
/// * Static strings are ideal for `method`
///
/// # Safety
///
/// * `context` must be a valid pointer from `imageflow_context_create`
/// * `context` must not be NULL (will abort process)
/// * `method` must be a valid null-terminated UTF-8 string
/// * `method` must not be NULL
/// * `json_buffer` must be a valid pointer to at least `json_buffer_size` readable bytes
/// * `json_buffer` must not be NULL
/// * `json_buffer_size` must not have the most significant bit set (max 2^31 or 2^63)
/// * `json_buffer` and `method` must remain valid for the duration of this call
///
/// # Panics
///
/// Internal panics are caught and converted to error responses. Check the response status code.
///
/// # Thread Safety
///
/// Safe to call from multiple threads on the same context (acquires write lock).
/// Operations will serialize - only one operation executes at a time per context.
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_send_json(
    context: *mut ThreadSafeContext,
    method: *const libc::c_char,
    json_buffer: *const u8,
    json_buffer_size: libc::size_t,
) -> *const JsonResponse {
    let c = context!(context);
    if c.outward_error().has_error() {
        let json_error = c.outward_error().get_json_response_for_error();
        if let Some(json_error) = json_error {
            return create_abi_json_response(c, &json_error.response_json, json_error.status_code);
        }
        panic!("Internal error: error flag set but no error object available")
    }
    if method.is_null() {
        c.outward_error_mut()
            .try_set_error(nerror!(ErrorKind::NullArgument, "The argument 'method' is null."));
        return ptr::null();
    }
    if json_buffer.is_null() {
        c.outward_error_mut()
            .try_set_error(nerror!(ErrorKind::NullArgument, "The argument 'json_buffer' is null."));
        return ptr::null();
    }
    if json_buffer_size.leading_zeros() == 0 {
        c.outward_error_mut().try_set_error(nerror!(ErrorKind::InvalidArgument, "Argument `json_buffer_size` likely came from a negative integer. Imageflow prohibits having the leading bit set on unsigned integers (this reduces the maximum value to 2^31 or 2^63)."));
        return ptr::null();
    }

    let panic_result = {
        let mut inner_context_guard = lock_context_mut_or_return!(c, ptr::null());
        catch_unwind(AssertUnwindSafe(|| {
            let method_str = if let Ok(str) = unsafe { ::std::ffi::CStr::from_ptr(method) }.to_str()
            {
                str
            } else {
                return (
                    None,
                    Err(nerror!(
                        ErrorKind::InvalidArgument,
                        "The argument 'method' is invalid UTF-8."
                    )),
                );
            };

            let json_bytes = unsafe { std::slice::from_raw_parts(json_buffer, json_buffer_size) };

            // Segfault early
            let _ = (json_bytes.first(), json_bytes.last());

            let (json, call_result) = inner_context_guard.message(method_str, json_bytes);
            (Some(json), call_result)
        }))
    };

    match panic_result {
        Ok((json, Ok(_))) => {
            let json = json.unwrap();
            // An unfortunate copy occurs here
            create_abi_json_response(c, &json.response_json, json.status_code)
        }
        Ok((json, Err(e))) => {
            c.outward_error_mut().try_set_error(e);
            if let Some(json) = json {
                create_abi_json_response(c, &json.response_json, json.status_code)
            } else {
                ptr::null_mut()
            }
        }
        Err(p) => {
            c.outward_error_mut().try_set_panic_error(p);
            ptr::null_mut()
        }
    }
}

pub fn create_abi_json_response(
    c: &mut ThreadSafeContext,
    json_bytes: &[u8],
    status_code: i64,
) -> *const JsonResponse {
    unsafe {
        let sizeof_struct = std::mem::size_of::<JsonResponse>();
        let alloc_size = sizeof_struct + json_bytes.len();

        if json_bytes.len().leading_zeros() == 0 {
            c.outward_error_mut().try_set_error(nerror!(ErrorKind::Category(ErrorCategory::InternalError), "Error in creating JSON structure; length overflow prevented (most significant bit set)."));
            return ptr::null();
        }

        let pointer = match c.mem_calloc(alloc_size, 16, ptr::null(), -1) {
            Err(e) => {
                c.outward_error_mut().try_set_error(e);
                return ptr::null();
            }
            Ok(v) => v,
        };

        let pointer_to_final_buffer = pointer.add(sizeof_struct);
        let imageflow_response = &mut (*(pointer as *mut JsonResponse));
        imageflow_response.buffer_utf8_no_nulls = pointer_to_final_buffer;
        imageflow_response.buffer_size = json_bytes.len();
        imageflow_response.status_code = status_code;

        let out_json_bytes =
            std::slice::from_raw_parts_mut(pointer_to_final_buffer, json_bytes.len());

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
//pub extern "C" fn imageflow_context_add_file(context: *mut ThreadSafeContext, io_id: i32, direction: Direction,
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

/// Adds an input buffer to the job context.
///
/// The buffer lifetime semantics depend on the `lifetime` parameter:
///
/// * `Lifetime::OutlivesFunctionCall` - Imageflow copies the buffer immediately.
///   You may free the buffer as soon as this function returns.
///
/// * `Lifetime::OutlivesContext` - Imageflow borrows the buffer (zero-copy).
///   **CRITICAL:** The buffer MUST remain valid and unmodified until the context is destroyed.
///   Do NOT free, move, or modify the buffer while the context exists.
///   In GC languages, pin the buffer to prevent garbage collection or movement.
///
/// Returns false if the operation fails. Check error state with `imageflow_context_has_error()`.
///
/// # Safety
///
/// * `context` must be a valid pointer from `imageflow_context_create`
/// * `context` must not be NULL (will abort process)
/// * `buffer` must be a valid pointer to at least `buffer_byte_count` readable bytes
/// * `buffer` must not be NULL
/// * If `lifetime` is `OutlivesContext`, buffer must remain valid until context destruction
/// * `io_id` must be unique (not previously used for this context)
/// * `buffer_byte_count` must not have the most significant bit set (max 2^31 or 2^63)
///
/// # Panics
///
/// Internal panics are caught and converted to errors. Returns false on panic.
///
/// # Thread Safety
///
/// Safe to call from multiple threads on the same context (acquires write lock).
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_add_input_buffer(
    context: *mut ThreadSafeContext,
    io_id: i32,
    buffer: *const u8,
    buffer_byte_count: libc::size_t,
    lifetime: Lifetime,
) -> bool {
    let c = context!(context);
    if c.outward_error().has_error() {
        return false;
    }

    if buffer.is_null() {
        c.outward_error_mut()
            .try_set_error(nerror!(ErrorKind::NullArgument, "The argument 'buffer' is null."));
        return false;
    }
    if buffer_byte_count.leading_zeros() == 0 {
        c.outward_error_mut().try_set_error(nerror!(ErrorKind::InvalidArgument, "Argument `buffer_byte_count` likely came from a negative integer. Imageflow prohibits having the leading bit set on unsigned integers (this reduces the maximum value to 2^31 or 2^63)."));
        return false;
    }
    let (mut outward_error, mut inner_context_guard) =
        lock_context_mut_and_error_or_return!(c, false);

    let result = {
        if inner_context_guard.io_id_present(io_id) {
            outward_error.try_set_error(nerror!(
                ErrorKind::DuplicateIoId,
                "The io_id provided is already in use."
            ));
            return false;
        }
        catch_unwind(AssertUnwindSafe(|| {
            let bytes = unsafe { std::slice::from_raw_parts(buffer, buffer_byte_count) };

            if lifetime == Lifetime::OutlivesFunctionCall {
                inner_context_guard
                    .add_copied_input_buffer(io_id, bytes)
                    .map_err(|e| e.at(here!()))?;
            } else {
                inner_context_guard.add_input_buffer(io_id, bytes).map_err(|e| e.at(here!()))?;
            }
            Ok(true)
        }))
    };
    match result {
        Ok(Ok(v)) => v,
        Err(p) => {
            outward_error.try_set_panic_error(p);
            false
        }
        Ok(Err(error)) => {
            outward_error.try_set_error(error);
            false
        }
    }
}

/// Adds an output buffer to the job context.
///
/// Imageflow will allocate and manage a growable output buffer internally.
/// After processing, retrieve the buffer contents with `imageflow_context_get_output_buffer_by_id()`.
///
/// The output buffer is freed automatically when the context is destroyed.
///
/// Returns false if allocation failed or the context is in an error state.
///
/// # Safety
///
/// * `context` must be a valid pointer from `imageflow_context_create`
/// * `context` must not be NULL (will abort process)
/// * `io_id` must be unique (not previously used for this context)
///
/// # Panics
///
/// Internal panics are caught and converted to errors. Returns false on panic.
///
/// # Thread Safety
///
/// Safe to call from multiple threads on the same context (acquires write lock).
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_add_output_buffer(
    context: *mut ThreadSafeContext,
    io_id: i32,
) -> bool {
    let c = context!(context);
    if c.outward_error().has_error() {
        return false;
    }
    let (mut outward_error, mut inner_context_guard) =
        lock_context_mut_and_error_or_return!(c, false);
    let result = catch_unwind(AssertUnwindSafe(|| {
        inner_context_guard.add_output_buffer(io_id).map_err(|e| e.at(here!()))?;
        Ok(true)
    }));
    handle_result!(outward_error, result, false)
}

/// Provides access to the underlying buffer for the given output io_id.
///
/// This function writes the buffer pointer and length to the provided output parameters.
/// The buffer pointer remains valid until the context is destroyed.
///
/// **Important:** The returned buffer is read-only and owned by the context.
/// Do NOT modify or free the buffer.
///
/// Returns false and sets an error if:
/// * The io_id is invalid or not an output buffer
/// * The result pointers are null
/// * The context is in an error state
///
/// # Safety
///
/// * `context` must be a valid pointer from `imageflow_context_create`
/// * `context` must not be NULL (will abort process)
/// * `result_buffer` must be a valid pointer to write a `*const u8` to (not NULL)
/// * `result_buffer_length` must be a valid pointer to write a `size_t` to (not NULL)
/// * The returned buffer pointer is only valid until context destruction
/// * The returned buffer must NOT be modified or freed by the caller
///
/// # Panics
///
/// Internal panics are caught and converted to errors. Returns false on panic.
///
/// # Thread Safety
///
/// Safe to call from multiple threads on the same context (acquires write lock).
/// Note: The buffer pointer becomes shared - ensure no thread is writing to outputs while reading.
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_get_output_buffer_by_id(
    context: *mut ThreadSafeContext,
    io_id: i32,
    result_buffer: *mut *const u8,
    result_buffer_length: *mut libc::size_t,
) -> bool {
    let c = context!(context);
    if result_buffer.is_null() {
        c.outward_error_mut().try_set_error(nerror!(
            ErrorKind::NullArgument,
            "The argument 'result_buffer' is null."
        ));
        return false;
    }

    if result_buffer_length.is_null() {
        c.outward_error_mut().try_set_error(nerror!(
            ErrorKind::NullArgument,
            "The argument 'result_buffer_length' is null."
        ));
        return false;
    }
    let (mut outward_error, inner_context_guard) = lock_context_mut_and_error_or_return!(c, false);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let s = inner_context_guard.get_output_buffer_slice(io_id).map_err(|e| e.at(here!()))?;

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
    handle_result!(outward_error, result, false)
}

/// Allocates zeroed memory that will be freed automatically with the context.
///
/// The allocated memory is zeroed and aligned to 16 bytes.
/// The memory will be automatically freed when the context is destroyed.
///
/// You may free the memory early using `imageflow_context_memory_free()`.
///
/// * `filename`/`line` are optional debug parameters. Pass NULL/-1 to skip.
/// * If provided, `filename` should be a null-terminated UTF-8 string with static lifetime.
///
/// Returns NULL on allocation failure or if the context is in an error state.
///
/// # Safety
///
/// * `context` must be a valid pointer from `imageflow_context_create`
/// * `context` must not be NULL (will abort process)
/// * `bytes` must not have the most significant bit set (max 2^31 or 2^63)
/// * `filename`, if not NULL, must be a valid null-terminated string with static lifetime
/// * The returned pointer is valid until freed or until context destruction
///
/// # Panics
///
/// Cannot panic - allocation failures result in returning NULL with error set.
///
/// # Thread Safety
///
/// Safe to call from multiple threads on the same context (uses separate allocation lock).
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_memory_allocate(
    context: *mut ThreadSafeContext,
    bytes: libc::size_t,
    filename: *const libc::c_char,
    line: i32,
) -> *mut libc::c_void {
    let c = context!(context);
    if c.outward_error_mut().has_error() {
        return ptr::null_mut();
    }

    if bytes.leading_zeros() == 0 {
        c.outward_error_mut().try_set_error(nerror!(ErrorKind::InvalidArgument, "Argument `bytes` likely came from a negative integer. Imageflow prohibits having the leading bit set on unsigned integers (this reduces the maximum value to 2^31 or 2^63)."));
        return ptr::null_mut();
    }
    let pointer = match unsafe { c.mem_calloc(bytes, 16, filename, line) } {
        Err(e) => {
            c.outward_error_mut().try_set_error(e);
            return ptr::null_mut();
        }
        Ok(v) => v,
    };
    pointer as *mut libc::c_void
}

/// Frees memory allocated with `imageflow_context_memory_allocate` early.
///
/// This is optional - all context-allocated memory is automatically freed when the context is destroyed.
/// Use this to reduce memory usage during long-running operations.
///
/// * `filename`/`line` are reserved for future debugging. Pass NULL/-1.
///
/// Returns true on success or if `pointer` is NULL.
/// Returns false if the pointer was not found in the allocation list.
///
/// # Safety
///
/// * `context` must be a valid pointer from `imageflow_context_create`
/// * `context` must not be NULL (will abort process)
/// * `pointer` must be either NULL or a pointer returned from `imageflow_context_memory_allocate` on this context
/// * `pointer` must not have been previously freed (double-free)
/// * After this call, `pointer` is invalid - do not use it (use-after-free)
///
/// # Panics
///
/// Cannot panic - safe to use in cleanup paths even if context is in error state.
///
/// # Thread Safety
///
/// Safe to call from multiple threads on the same context (uses separate allocation lock).
#[no_mangle]
pub unsafe extern "C" fn imageflow_context_memory_free(
    context: *mut ThreadSafeContext,
    pointer: *mut libc::c_void,
    _filename: *const libc::c_char,
    _line: i32,
) -> bool {
    let c = context!(context); // We must be able to free in an errored state
    if !pointer.is_null() {
        unsafe { c.mem_free(pointer as *const u8) }
    } else {
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
        let expected_json_out =
            ::std::str::from_utf8(expected_response.response_json.as_ref()).unwrap();
        let expected_response_status = expected_response.status_code;

        let response = imageflow_context_send_json(c, method_in, json_in.as_ptr(), json_in.len());

        assert_ne!(response, ptr::null());

        let mut json_out_ptr: *const u8 = ptr::null_mut();
        let mut json_out_size: usize = 0;
        let mut json_status_code: i64 = 0;

        assert!(imageflow_json_response_read(
            c,
            response,
            &mut json_status_code,
            &mut json_out_ptr,
            &mut json_out_size
        ));

        let json_out_str =
            ::std::str::from_utf8(std::slice::from_raw_parts(json_out_ptr, json_out_size)).unwrap();
        assert_eq!(json_out_str, expected_json_out);

        assert_eq!(json_status_code, expected_response_status);

        imageflow_context_destroy(c);
    }
}

#[test]
fn test_allocate_free() {
    unsafe {
        let c = imageflow_context_create(IMAGEFLOW_ABI_VER_MAJOR, IMAGEFLOW_ABI_VER_MINOR);
        let bytes = 100;
        let ptr = imageflow_context_memory_allocate(c, bytes, static_char!(file!()), line!() as i32)
            as *mut u8;
        assert!(!ptr.is_null());

        for x in 0..bytes as isize {
            assert_eq!(*ptr.offset(x), 0);
        }
        assert!(imageflow_context_memory_free(
            c,
            ptr as *mut libc::c_void,
            static_char!(file!()),
            line!() as i32
        ));

        imageflow_context_destroy(c);
        //imageflow_context_destroy(c);
    }
}

#[cfg(test)]
extern crate base64;

#[test]
fn test_job_with_buffers() {
    unsafe {
        let c = imageflow_context_create(IMAGEFLOW_ABI_VER_MAJOR, IMAGEFLOW_ABI_VER_MINOR);
        assert!(!c.is_null());

        use base64::Engine;

        let input_bytes = base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABAQMAAAAl21bKAAAAA1BMVEX/TQBcNTh/AAAAAXRSTlPM0jRW/QAAAApJREFUeJxjYgAAAAYAAzY3fKgAAAAASUVORK5CYII=").unwrap();

        let res = imageflow_context_add_input_buffer(
            c,
            0,
            input_bytes.as_ptr(),
            input_bytes.len(),
            Lifetime::OutlivesContext,
        );
        imageflow_context_print_and_exit_if_error(c);
        assert!(res);

        let res = imageflow_context_add_output_buffer(c, 1);
        imageflow_context_print_and_exit_if_error(c);
        assert!(res);

        let method_in = static_char!("v1/execute");
        let json_in = r#"{"framewise":{"steps":[{"decode":{"io_id":0}},{"flip_h":null},{"rotate_90":null},{"resample_2d":{"w":30,"h":20,"hints":{"sharpen_percent":null}}},{"constrain":{ "mode" :"within", "w": 5,"h": 5}},{"encode":{"io_id":1,"preset":{"gif":null}}}]}}"#;

        let response = imageflow_context_send_json(c, method_in, json_in.as_ptr(), json_in.len());

        assert!(!response.is_null());

        let mut json_out_ptr: *const u8 = ptr::null_mut();
        let mut json_out_size: usize = 0;
        let mut json_status_code: i64 = 0;

        assert!(imageflow_json_response_read(
            c,
            response,
            &mut json_status_code,
            &mut json_out_ptr,
            &mut json_out_size
        ));

        imageflow_context_print_and_exit_if_error(c);

        let mut buf: *const u8 = ptr::null();
        let mut buf_len: usize = 0;
        let res = imageflow_context_get_output_buffer_by_id(
            c,
            1,
            &mut buf as *mut *const u8,
            &mut buf_len as *mut usize,
        );
        imageflow_context_print_and_exit_if_error(c);
        assert!(res);

        let expected_response_status = 200;
        assert_eq!(json_status_code, expected_response_status);

        imageflow_context_destroy(c);
    }
}

#[test]
fn test_job_with_cancellation() {
    unsafe {
        let c = imageflow_context_create(IMAGEFLOW_ABI_VER_MAJOR, IMAGEFLOW_ABI_VER_MINOR);
        assert!(!c.is_null());

        use base64::Engine;

        let input_bytes = base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABAQMAAAAl21bKAAAAA1BMVEX/TQBcNTh/AAAAAXRSTlPM0jRW/QAAAApJREFUeJxjYgAAAAYAAzY3fKgAAAAASUVORK5CYII=").unwrap();

        let res = imageflow_context_add_input_buffer(
            c,
            0,
            input_bytes.as_ptr(),
            input_bytes.len(),
            Lifetime::OutlivesContext,
        );
        imageflow_context_print_and_exit_if_error(c);
        assert!(res);

        let res = imageflow_context_add_output_buffer(c, 1);
        imageflow_context_print_and_exit_if_error(c);
        assert!(res);

        let method_in = static_char!("v1/execute");
        let json_in = r#"{"framewise":{"steps":[{"decode":{"io_id":0}},{"flip_h":null},{"rotate_90":null},{"resample_2d":{"w":30,"h":20,"hints":{"sharpen_percent":null}}},{"constrain":{ "mode" :"within", "w": 5,"h": 5}},{"encode":{"io_id":1,"preset":{"gif":null}}}]}}"#;

        imageflow_context_request_cancellation(c);

        let response = imageflow_context_send_json(c, method_in, json_in.as_ptr(), json_in.len());

        assert!(!response.is_null());

        assert!(imageflow_context_error_code(c) == 21);

        imageflow_context_destroy(c);
    }
}

#[test]
fn test_job_with_bad_json() {
    unsafe {
        let c = imageflow_context_create(IMAGEFLOW_ABI_VER_MAJOR, IMAGEFLOW_ABI_VER_MINOR);
        assert!(!c.is_null());

        use base64::Engine;

        let input_bytes = base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABAQMAAAAl21bKAAAAA1BMVEX/TQBcNTh/AAAAAXRSTlPM0jRW/QAAAApJREFUeJxjYgAAAAYAAzY3fKgAAAAASUVORK5CYII=").unwrap();

        let res = imageflow_context_add_input_buffer(
            c,
            0,
            input_bytes.as_ptr(),
            input_bytes.len(),
            Lifetime::OutlivesContext,
        );
        imageflow_context_print_and_exit_if_error(c);
        assert!(res);

        let res = imageflow_context_add_output_buffer(c, 1);
        imageflow_context_print_and_exit_if_error(c);
        assert!(res);

        let method_in = static_char!("v1/execute");
        let json_in = r#"{"framewise":{"steps":[{"decode":{"io_id":0}},{"flip_h":null},{"rotate_90":null},{"resample_2d":{"w":30,"h":20,"down_filter":null,"up_filter":null,"hints":{"sharpen_percent":null}}},{"constrain":{"within":{"w":5,"h":5}}},{"encode":{"io_id":1,"preset":{"gif":null}}}]}}"#;

        let response = imageflow_context_send_json(c, method_in, json_in.as_ptr(), json_in.len());

        assert!(!response.is_null());

        let mut json_out_ptr: *const u8 = ptr::null_mut();
        let mut json_out_size: usize = 0;
        let mut json_status_code: i64 = 0;

        assert!(imageflow_json_response_read(
            c,
            response,
            &mut json_status_code,
            &mut json_out_ptr,
            &mut json_out_size
        ));
        assert!(imageflow_context_has_error(c));

        let expected_response_status = 400; //bad request
        assert_eq!(json_status_code, expected_response_status);

        imageflow_context_destroy(c);
    }
}

#[test]
fn test_get_version_info() {
    unsafe {
        let c = imageflow_context_create(IMAGEFLOW_ABI_VER_MAJOR, IMAGEFLOW_ABI_VER_MINOR);
        assert!(!c.is_null());

        let method_in = static_char!("v1/get_version_info");
        let json_in = "{}";

        let response = imageflow_context_send_json(c, method_in, json_in.as_ptr(), json_in.len());

        assert!(!response.is_null());

        let mut json_out_ptr: *const u8 = ptr::null_mut();
        let mut json_out_size: usize = 0;
        let mut json_status_code: i64 = 0;

        assert!(imageflow_json_response_read(
            c,
            response,
            &mut json_status_code,
            &mut json_out_ptr,
            &mut json_out_size
        ));
        assert!(!imageflow_context_has_error(c));

        let expected_response_status = 200; //bad request
        assert_eq!(json_status_code, expected_response_status);

        imageflow_context_destroy(c);
    }
}

#[test]
fn test_file_macro_for_this_build() {
    assert!(file!().starts_with(env!("CARGO_PKG_NAME")))
}
