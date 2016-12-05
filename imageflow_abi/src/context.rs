extern crate imageflow_core as fc;
use ::std;
use self::fc::for_other_imageflow_crates::preludes::external_without_std::*;
use self::fc::ffi as ffi;

pub struct Context{
    c_ctx: *mut ffi::ImageflowContext,
    error: ErrorBuffer
}

pub struct ErrorBuffer{
    c_ctx: *mut ffi::ImageflowContext,
}
pub enum ContextError{
    AllocationFailed
}

type Result<T> = std::result::Result<T,ContextError>;


impl Context {
    pub fn create_boxed() -> Result<Box<Context>> {
        std::panic::catch_unwind(|| {
            let inner = unsafe { ffi::flow_context_create() };
            if inner.is_null() {
                Err(ContextError::AllocationFailed)
            } else {
                Ok(Box::new(Context {
                    c_ctx: inner,
                    error: ErrorBuffer {c_ctx: inner}
                }))
            }
        }).unwrap_or(Err(ContextError::AllocationFailed))
    }
    //pub fn begin_terminate(&mut self) -> bool {}

    pub fn error_mut(&mut self) -> &mut ErrorBuffer{
        &mut self.error
    }
    pub fn error(&self) -> &ErrorBuffer{
        &self.error
    }
    pub fn unsafe_c_ctx(&mut self) -> *mut ffi::ImageflowContext{
        self.c_ctx
    }
}

type ErrorCode = i32;
impl ErrorBuffer{

    /// Prints the error messages and stacktrace to the given buffer in UTF-8 form; writes a null
    /// character to terminate the string, and *ALSO* returns the number of bytes written.
    ///
    ///
    /// Happy(ish) path: Returns the length of the error message written to the buffer.
    /// Sad path: Returns -1 if slice was too small or buffer was nullptr.
    /// full_file_path, if true, will display the directory associated with the files in each stack frame.
//    pub fn write_to_slice_as_c_str(&self, slice: &mut [u8], prefer_full_paths: bool) -> i64{
//        // ffi::flow_context_error_and_stacktrace(context, buffer as *mut u8, buffer_length, full_file_path)
//    }
//
//
//    pub fn has_error(&self) -> bool{
//
//    }
//    pub fn clear_error(&mut self) -> bool{
//
//    }
//
//    pub fn error_code(&self) -> ErrorCode{
//
//    }
//    pub fn abort_and_print_on_error(&self) -> bool{
//
//    }

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
//    pub fn raise_error_c_style(&mut self,
//                                                           error_code: ErrorCode,
//                                                           message: &CStr,
//                                                           filename: Option<&'static CStr>,
//                                                           line: Option<i32>,
//                                                           function_name: Option<&'static CStr>)
//                                                           -> bool {
//
//        //ffi::flow_context_raise_error(context, error_code, message, filename, line, function_name)
//    }
//
//    pub fn add_to_callstack_c_style(&mut self, filename: Option<&'static CStr>,
//                                    line: Option<i32>,
//                                    function_name: Option<&'static CStr>)
//                                    -> bool {
//
//    }


    fn nah(){}
}
impl Drop for Context {
    fn drop(&mut self) {
        if !self.c_ctx.is_null() {
            unsafe {
                ffi::flow_context_destroy(self.c_ctx);
            }
        }
        self.c_ctx = ptr::null_mut();
        self.error.c_ctx = ptr::null_mut();
    }
}
