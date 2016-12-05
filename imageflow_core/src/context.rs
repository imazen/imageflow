use ::std;
use ::for_other_imageflow_crates::preludes::external_without_std::*;
use ::ffi;


pub struct Job{
    pub debug_job_id: int32_t,
    pub next_stable_node_id: int32_t,
    pub next_graph_version: int32_t,
    pub max_calc_flatten_execute_passes: int32_t,
    // FIXME: find a safer way to store them
//    pub codecs_head: *mut CodecInstance,
//    pub codecs_tail: *mut CodecInstance,
    pub record_graph_versions: bool,
    pub record_frame_images: bool,
    pub render_graph_versions: bool,
    pub render_animated_graph: bool,
    pub render_last_graph: bool,
}

pub struct Context{
    c_ctx: *mut ffi::ImageflowContext,
    error: ErrorBuffer,
    jobs: Vec<Job>
}

pub struct ErrorBuffer{
    c_ctx: *mut ffi::ImageflowContext,
}
pub enum ContextError{
    AllocationFailed
}

type Result<T> = std::result::Result<T,ContextError>;


impl Context {
    /// Used by abi; should not panic
    pub fn create_boxed() -> Result<Box<Context>> {
        std::panic::catch_unwind(|| {
            let inner = unsafe { ffi::flow_context_create() };
            if inner.is_null() {
                Err(ContextError::AllocationFailed)
            } else {
                Ok(Box::new(Context {
                    c_ctx: inner,
                    error: ErrorBuffer {c_ctx: inner},
                    jobs: vec![]
                }))
            }
        }).unwrap_or(Err(ContextError::AllocationFailed))
    }

    /// Used by abi; should not panic
    pub fn begin_terminate(&mut self) -> bool {
        self.jobs.clear();
        unsafe {
            ffi::flow_context_begin_terminate(self.c_ctx)
        }
    }

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
    /// Used by abi; should not panic
    pub fn has_error(&self) -> bool{
        unsafe{
            ffi::flow_context_has_error(self.c_ctx)
        }
    }
    /// Used by abi; should not panic
    pub fn clear_error(&mut self){
        unsafe {
            ffi::flow_context_clear_error(self.c_ctx)
        }
    }
    /// Used by abi; should not panic
    pub fn error_code(&self) -> ErrorCode{
        unsafe {
            ffi::flow_context_error_reason(self.c_ctx)
        }
    }

    /// Used by abi; should not panic
    pub fn abort_and_print_on_error(&self) -> bool{
        unsafe {
            ffi::flow_context_print_and_exit_if_err(self.c_ctx)
        }
    }

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
    /// * If you provide an error code of zero (why?!), a different error code will be provided.
    pub fn raise_error_c_style(&mut self,
                                                           error_code: ErrorCode,
                                                           message: Option<&CStr>,
                                                           filename: Option<&'static CStr>,
                                                           line: Option<i32>,
                                                           function_name: Option<&'static CStr>)
                                                           -> bool {
        unsafe {
            ffi::flow_context_raise_error(self.c_ctx, error_code,
                                          message.map(|cstr| cstr.as_ptr()).unwrap_or(ptr::null()),
                                          filename.map(|cstr| cstr.as_ptr()).unwrap_or(ptr::null()),
                                          line.unwrap_or(-1),
                                          function_name.map(|cstr| cstr.as_ptr()).unwrap_or(ptr::null()))
        }
    }


    ///
    /// Adds the given UTF-8 filename, line number, and function name to the call stack.
    ///
    /// Returns `true` if add was successful.
    ///
    /// # Will fail and return false if...
    ///
    /// * You haven't raised an error
    /// * You tried to raise a second error without clearing the first one. Call will be ignored.
    /// * You've exceeded the capacity of the call stack (which, at one point, was 14). But this
    ///   category of failure is acceptable.
    pub fn add_to_callstack_c_style(&mut self, filename: Option<&'static CStr>,
                                    line: Option<i32>,
                                    function_name: Option<&'static CStr>)
                                    -> bool {


        unsafe {
            ffi::flow_context_add_to_callstack(self.c_ctx,
                                               filename.map(|cstr| cstr.as_ptr()).unwrap_or(ptr::null()),
                                               line.unwrap_or(-1),
                                               function_name.map(|cstr| cstr.as_ptr()).unwrap_or(ptr::null()))
        }
    }

}
impl Drop for Context {
    /// Used by abi; should not panic
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
