//!
//! # Memory Lifetimes
//!
//! In order to prevent dangling pointers, we must be correct about memory lifetimes.
//!
//! ## ... when allocated by Imageflow, assume the lifetime of the `context`
//!
//! **In Imageflow, by default, all things created with a context will be destroyed when the
//! context is destroyed.**
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
//! 1. Schedule the destruction to occur earlier, using ownership.
//! 2. Invoke the corresponding destroy function when you're done with the thing.
//!
//! Only certain things may be owners: `context`, `job`, and `io` pointers. Setting a
//! 'shorter-lived' owner, like the job (vs. context) can help, but can be less effective
//! than directly invoking the destroy function as soon as it is possible to do so.
//!
//! ### Destroying things
//!
//! * An `imageflow_context` should ALWAYS be destroyed with `imageflow_context_destroy`
//! * ImageflowJsonResponse structures should be released with `imageflow_json_response_destroy`
//! *
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
//!   responsible for freeing, and (b) will (at least) outlive the `context`.
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
//! Then you'll need to change the owner - disassociate the thing from the context
//! , and become responsible for it,
//! and all the things it might have owned, all the destructors that will now never run.
//!
//! [TODO] Provide instructions
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
//! * The above includes *mut Context, *mut Job, *mut JobIO, etc.
//! * libc::size_t (or usize): Unsigned integer, platform-sized. 32 or 64 bits.
//!
//!
//! Treat *mut Context, *mut Job, *mut JobIO, *mut ImageflowJsonResponse ALL as opaque pointers.
//!
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

extern crate libc;


pub use ffi::{Job, JobIO, Context, IoMode, IoDirection};
use std::{ptr, mem};

#[cfg(test)]
use std::str;

#[cfg(test)]
use std::ffi::CString;

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
pub unsafe extern fn imageflow_context_create() -> *mut Context {
    ::ffi::flow_context_create()
}

/// Begins the process of destroying the context, yet leaves error information intact
/// so that any errors in the tear-down process can be
/// debugged with imageflow_context_error_and_stacktrace.
///
/// Returns true if no errors occurred. Returns false if there were tear-down issues.
///
/// *Behavior is undefined if context is a null or invalid ptr.*
#[no_mangle]
pub unsafe extern fn imageflow_context_begin_terminate(context: *mut Context) -> bool {
    ::ffi::flow_context_begin_terminate(context)
}

/// Destroys the imageflow context and frees the context object.
/// Only use this with contexts created using imageflow_context_create
///
/// Behavior is undefined if context is a null or invalid ptr; may segfault on free(NULL);
#[no_mangle]
pub unsafe extern fn imageflow_context_destroy(context: *mut Context) {
    ::ffi::flow_context_destroy(context)
}


#[test]
fn test_create_destroy() {
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
pub unsafe extern fn imageflow_context_has_error(context: *mut Context) -> bool {
    ::ffi::flow_context_has_error(context)
}

/// Clear the error state. This assumes that you know which API call failed and the problem has
/// been resolved. Don't use this unless you're sure you've accounted for all possible
/// inconsistent state (and fully understand the code paths that led to the error).
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
#[no_mangle]
pub unsafe extern fn imageflow_context_clear_error(context: *mut Context) {
    ::ffi::flow_context_clear_error(context)
}

/// Prints the error messages and stacktrace to the given buffer
/// Happy(ish) path: Returns the length of the error message written to the buffer.
/// Sad path: Returns -1 if buffer_length was too small or buffer was nullptr.
/// full_file_path, if true, will display the directory associated with the files in each stack frame.
///
/// Please be accurate with the buffer length, or a buffer overflow will occur.
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
#[no_mangle]
pub unsafe extern fn imageflow_context_error_and_stacktrace(context: *mut Context,
                                                            buffer: *mut u8,
                                                            buffer_length: libc::size_t,
                                                            full_file_path: bool)
                                                            -> i64 {
    ::ffi::flow_context_error_and_stacktrace(context,
                                             buffer,
                                             buffer_length,
                                             full_file_path)
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
pub unsafe extern fn imageflow_context_error_code(context: *mut Context) -> i32 {
    ::ffi::flow_context_error_reason(context)
}

/// Prints the error to stderr and exits the process if an error has been raised on the context.
/// If no error is present, the function returns false.
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
///
/// THIS PRINTS DIRECTLY TO STDERR! Do not use in any kind of service! Command-line usage only!
#[no_mangle]
pub unsafe extern fn imageflow_context_print_and_exit_if_error(context: *mut Context) -> bool {
    ::ffi::flow_context_print_and_exit_if_err(context)
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
/// * All strings must be null-terminated, C-style, valid UTF-8.
/// * The lifetime of `message` is expected to match or exceed the duration of this function call.
/// * The lifetime of `file` and `function_name` (if provided), is expected to match or exceed the lifetime of `context`.
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
pub unsafe extern fn imageflow_context_raise_error(context: *mut Context,
                                                   error_code: i32, message: *const libc::c_char,
                                                   file: *const libc::c_char, line: i32, function_name: *const libc::c_char) -> bool {
    ::ffi::flow_context_raise_error(context, error_code, message, file, line, function_name)
}

///
/// Adds the given filename, line number, and function name to the call stack.
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
pub unsafe extern fn imageflow_context_add_to_callstack(context: *mut Context, filename: *const libc::c_char, line: i32, function_name: *const libc::c_char) -> bool {
    ::ffi::flow_context_add_to_callstack(context, filename, line, function_name)
}


#[test]
pub fn test_error_handling() {
    unsafe {
        let c = imageflow_context_create();
        assert!(!c.is_null());

        assert!(!imageflow_context_has_error(c));
        assert_eq!(imageflow_context_error_code(c), 0);

        // While the strings are static, the CStrings are not.
        // But they will persist until the end of the block, which is all we need
        let message_str = CString::new("Test message").unwrap();
        let message = message_str.as_ptr();
        let filename_str = CString::new(file!()).unwrap();
        let filename = filename_str.as_ptr();
        let function_name_str = CString::new("test_error_handling").unwrap();
        let function_name = function_name_str.as_ptr();

        //Let's raise a nice error
        assert!(imageflow_context_raise_error(c, 1025, message, filename, 335, function_name));

        //Check it registered
        assert!(imageflow_context_has_error(c));
        assert_eq!(imageflow_context_error_code(c), 1025);

        //Add a backtrace
        assert!(imageflow_context_add_to_callstack(c, filename, 342, ptr::null()));
        assert!(imageflow_context_add_to_callstack(c, filename, 20, ptr::null()));
        assert!(imageflow_context_add_to_callstack(c, ptr::null(), 0, ptr::null()));

        //Let's see how it handles a insufficient buffer
        let mut tiny_buf: [u8; 30] = [0; 30];
        assert_eq!(-1, imageflow_context_error_and_stacktrace(c, &mut tiny_buf[0], 30, true));


        //And check the output looks right
        let mut buf: [u8; 2048] = [0; 2048];
        let buf_used = imageflow_context_error_and_stacktrace(c, &mut buf[0], 2048, true);

        assert!(buf_used >= 0);
        let actual_string = str::from_utf8(&buf[0..buf_used as usize]).unwrap();


        let expected_string = "User defined error : Test message\nsrc/abi.rs:335: in function test_error_handling\nsrc/abi.rs:342: in function (unknown)\nsrc/abi.rs:20: in function (unknown)\n(unknown):0: in function (unknown)\n";
        assert_eq!(expected_string, actual_string);

        // raising a second error should fail
        assert!(!imageflow_context_raise_error(c, 1025, message, filename,line!() as i32, ptr::null()));
        // as should trying to add to the call stack
        assert!(!imageflow_context_add_to_callstack(c,filename,line!() as i32,ptr::null()));
        assert!(!imageflow_context_add_to_callstack(c,filename,line!() as i32,ptr::null()));

        // Clearing the error should work
        imageflow_context_clear_error(c);
        assert!(!imageflow_context_has_error(c));
        assert_eq!(imageflow_context_error_code(c), 0);


        // And it should be possible to report another error
        assert!(imageflow_context_raise_error(c, 1025, message, ptr::null(),1, ptr::null()));
        imageflow_context_clear_error(c);

        //And cleanup should go smoothly
        assert!(imageflow_context_begin_terminate(c));
        imageflow_context_destroy(c);
    }
}


///
/// imageflow_response contains a buffer and buffer length (in bytes), as well as a status code
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
    pub buffer_utf8_no_nulls: *const libc::uint8_t,
    pub buffer_size: libc::size_t
}

///
/// Writes fields from the given imageflow_json_response to the locations referenced.
///
#[no_mangle]
pub unsafe extern fn imageflow_json_response_read(context: *mut Context,
                                                  response_in: *mut ImageflowJsonResponse,
                                                  status_code_out: *mut i64,
                                                  buffer_utf8_no_nulls_out: *mut *const libc::uint8_t,
                                                  buffer_size_out: *mut libc::size_t) -> bool {
    if context.is_null() {
        return false;
    }
    if response_in.is_null() {
        //[TODO]: RAISE ERROR ON CONTEXT
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
pub unsafe extern fn imageflow_json_response_destroy(context: *mut Context, response: *mut ImageflowJsonResponse) -> bool {
    ::ffi::flow_destroy(context, response as *mut libc::c_void, ptr::null(), 0)
}


///
/// Sends a JSON message to one of 3 recipients.
///
/// 1. `imageflow_context`, If both `job` and `io` are both null
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
#[no_mangle]
#[allow(unused_variables)]
pub unsafe extern fn imageflow_send_json(context: *mut Context,
                                         job: *mut Job,
                                         io: *mut JobIO,
                                         method: *const libc::uint8_t,
                                         json_buffer: *const libc::uint8_t,
                                         json_buffer_size: libc::size_t) -> *const ImageflowJsonResponse {
    let json_bytes = "{}".as_bytes();
    let sizeof_struct = mem::size_of::<ImageflowJsonResponse>();


    let pointer = ::ffi::flow_context_calloc(context, 1, sizeof_struct + json_bytes.len(),
                                             ptr::null(), context as *mut libc::c_void, ptr::null(), 0) as *mut ImageflowJsonResponse;


    let ref mut response = *pointer;
    response.buffer_utf8_no_nulls = pointer.offset(sizeof_struct as isize) as *const libc::uint8_t;
    response.buffer_size = json_bytes.len();
    response.status_code = 501;

    return (response) as *const ImageflowJsonResponse;
}


#[no_mangle]
pub unsafe extern fn imageflow_io_create_for_file(context: *mut Context,
                                                  mode: IoMode,
                                                  filename: *const libc::c_char,
                                                  owner: *mut libc::c_void)
                                                  -> *mut JobIO {
    //TODO: validate that 'owner' is capable of being an owner

    ::ffi::flow_io_create_for_file(context, mode, filename, owner)
}


///
/// This method has not been stabilized; monitor its signature for changes.
///
/// Creates an imageflow_io structure for reading from/writing to the provided memory buffer.
/// You are responsible for freeing the memory provided; ownership does not transfer to Imageflow
/// unless you provide a destructor_function (which is not yet supported).
///
///
///
///
/// Destructor functions are not yet supported.
/// Destructor functions cannot be reliably written until
/// access to flow_context error raising methods is provided. \
///
///
/// destructor_function should be
///
#[no_mangle]
pub unsafe extern fn imageflow_io_create_from_memory(context: *mut Context,
                                                     mode: IoMode,
                                                     memory: *const u8,
                                                     length: libc::size_t,
                                                     owner: *mut libc::c_void,
                                                     destructor_function: *const libc::c_void)
                                                     -> *mut JobIO {
    ::ffi::flow_io_create_from_memory(context, mode, memory, length, owner, destructor_function)
}


///
/// Creates an imageflow_io structure for writing to an expanding memory buffer.
///
/// Reads/seeks, are, in theory, supported, but unless you've written, there will be nothing to read.
///
/// The I/O structure and buffer will be freed with the context.
///
/// Early destruction is not yet available; the value of `owner`, is, for now, ignored, and the
/// value of `context` is used instead, as that is when the underlying buffer is freed.
///
/// Returns null if allocation failed; check the context for error details.
#[no_mangle]
#[allow(unused_variables)]
pub unsafe extern fn imageflow_io_create_for_output_buffer(context: *mut Context,
                                                           owner: *const libc::c_void)
                                                           -> *mut JobIO {
    // The current implementation of output buffer only sheds its actual buffer with the context.
    // No need for the shell to have an earlier lifetime for mem reasons.
    ::ffi::flow_io_create_for_output_buffer(context, context as *mut libc::c_void)
}


// Returns false if the flow_io struct is disposed or not an output buffer type (or for any other error)
//

///
/// Provides access to the underlying buffer for the given imageflow_io object.
///
#[no_mangle]
pub unsafe extern fn imageflow_io_get_output_buffer(context: *mut Context,
                                                    io: *mut JobIO,
                                                    result_buffer: *mut *mut u8,
                                                    result_buffer_length: *mut libc::size_t)
                                                    -> bool {
    ::ffi::flow_io_get_output_buffer(context, io, result_buffer, result_buffer_length)
}


#[no_mangle]
pub unsafe extern fn imageflow_job_create(context: *mut Context) -> *mut Job {
    ::ffi::flow_job_create(context)
}


#[no_mangle]
pub unsafe extern fn imageflow_job_get_io(context: *mut Context,
                                          job: *mut Job,
                                          placeholder_id: i32)
                                          -> *mut JobIO {
    ::ffi::flow_job_get_io(context, job, placeholder_id)
}


#[no_mangle]
pub unsafe extern fn imageflow_job_add_io(context: *mut Context,
                                          job: *mut Job,
                                          io: *mut JobIO,
                                          placeholder_id: i32,
                                          direction: IoDirection)
                                          -> bool {
    ::ffi::flow_job_add_io(context, job, io, placeholder_id, direction)
}

#[no_mangle]
pub unsafe extern fn imageflow_job_destroy(context: *mut Context, job: *mut Job) -> bool {
    ::ffi::flow_job_destroy(context, job)
}

//malloc/calloc/free/raiseerror/addtocallstack/

//#[no_mangle]
//pub unsafe extern fn imageflow_job_get_decoder_info(c: *mut libc::c_void,
//                                     job: *mut Job,
//                                     by_placeholder_id: i32,
//                                     info: *mut DecoderInfo)
//                                     -> bool{
//
//}
//
//#[no_mangle]
//pub unsafe extern fn imageflow_job_decoder_set_downscale_hints_by_placeholder_id(c: *mut libc::c_void,
//                                                                  job: *mut Job, placeholder_id:i32,
//                                                                  if_wider_than: i64,  or_taller_than: i64,
//                                                                  downscaled_min_width: i64,  downscaled_min_height:i64,  scale_luma_spatially:bool,
//                                                                  gamma_correct_for_srgb_during_spatial_luma_scaling:bool) -> bool{
//
//}

//#[no_mangle]
//pub unsafe extern fn imageflow_job_execute(c: *mut libc::c_void, job: *mut Job, g: *mut *mut Graph) -> bool{
//
//}
//





// Exposing the ability to report an error is a prerequisite for any i/o structure
// AND for any destructor - or any kind of callback, for that matter

//
//PUB void flow_context_raise_error(flow_c * c, flow_status_code code, char * message, const char * file, int line,
//const char * function_name);
//PUB char * flow_context_set_error_get_message_buffer(flow_c * c, flow_status_code code, const char * file, int line,
//const char * function_name);
//PUB void flow_context_add_to_callstack(flow_c * c, const char * file, int line, const char * function_name);
//





//flow context/job/io send message
// malloc/calloc/free
// flow_set_owner
// flow_set_destructor


//PUB bool flow_set_destructor(flow_c * c, void * thing, flow_destructor_function destructor);
//
//// Thing will only be automatically destroyed and freed at the time that owner is destroyed and freed
//PUB bool flow_set_owner(flow_c * c, void * thing, void * owner);
//
//////////////////////////////////////////////
//// use imageflow memory management
//
//PUB void * flow_context_calloc(flow_c * c, size_t instance_count, size_t instance_size,
//flow_destructor_function destructor, void * owner, const char * file, int line);
//PUB void * flow_context_malloc(flow_c * c, size_t byte_count, flow_destructor_function destructor, void * owner,
//const char * file, int line);
//PUB void * flow_context_realloc(flow_c * c, void * old_pointer, size_t new_byte_count, const char * file, int line);
//PUB void flow_deprecated_free(flow_c * c, void * pointer, const char * file, int line);
//PUB bool flow_destroy_by_owner(flow_c * c, void * owner, const char * file, int line);
//PUB bool flow_destroy(flow_c * c, void * pointer, const char * file, int line);
//
