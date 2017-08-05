use ::std;
use ::for_other_imageflow_crates::preludes::external_without_std::*;
use ::ffi;
use ::job::Job;
use ::{FlowErr,FlowError, Result, JsonResponse};
use io::IoProxy;

use ::imageflow_types::collections::AddRemoveSet;
use ::ffi::ImageflowJsonResponse;



pub struct Context {
    //version: u64, (so different context types can be differentiated)
    c_ctx: *mut ffi::ImageflowContext,
    jobs: AddRemoveSet<Job>,
    io_proxies: AddRemoveSet<IoProxy>,
    error: RefCell<ErrorBuffer>,
}

#[derive(Copy,Clone,Debug)]
pub struct ErrorBuffer{
    c_ctx: *mut ffi::ImageflowContext,
}
impl Context {

    pub fn create() -> Result<Box<Context>>{
        Context::abi_create_boxed()
    }

    /// Used by abi; should not panic
    pub fn abi_create_boxed() -> Result<Box<Context>> {
        std::panic::catch_unwind(|| {
            // Upgrade backtraces
            imageflow_helpers::debug::upgrade_panic_hook_once_if_backtraces_wanted();

            let inner = unsafe { ffi::flow_context_create() };
            if inner.is_null() {
                Err(FlowError::Oom)
            } else {
                Ok(Box::new(Context {
                    c_ctx: inner,
                    error: RefCell::new(ErrorBuffer {c_ctx: inner}),
                    jobs: AddRemoveSet::with_capacity(2),
                    io_proxies: AddRemoveSet::with_capacity(2)
                }))
            }
        }).unwrap_or(Err(FlowError::Oom))
    }

    /// Used by abi; should not panic
    pub fn abi_begin_terminate(&mut self) -> bool {
        self.jobs.mut_clear();
        self.io_proxies.mut_clear();
        unsafe {
            ffi::flow_context_begin_terminate(self.c_ctx)
        }
    }

    pub fn error_mut(&self) -> RefMut<ErrorBuffer>{
        self.error.borrow_mut()
    }
    pub fn error(&self) -> Ref<ErrorBuffer>{
        self.error.try_borrow().expect("Another scope has mutably borrowed the ErrorBuffer; readonly access failed.")
    }

    pub unsafe fn unsafe_c_context_pointer(&self) -> *mut ffi::ImageflowContext{
        self.c_ctx
    }

    pub fn message(&mut self, method: &str, json: &[u8]) -> Result<JsonResponse> {
        ::context_methods::CONTEXT_ROUTER.invoke(self, method, json)
    }

    pub fn create_job(&self) -> RefMut<Job>{
        self.jobs.add_mut(Job::internal_use_only_create(self))
    }

    pub fn create_io_proxy(&self) -> RefMut<IoProxy>{
        self.io_proxies.add_mut(IoProxy::internal_use_only_create(self))
    }


    pub fn abi_try_remove_job(&self, job: *const Job) -> bool{
        self.jobs.try_remove(job).unwrap_or(false)
    }


    pub fn flow_c(&self) -> *mut ffi::ImageflowContext{
        self.c_ctx
    }

    pub fn c_error(&self) -> Option<FlowError>{
        self.error().get_error_copy()
    }


    pub fn get_proxy_mut_by_pointer(&self, proxy: *const IoProxy) -> Result<RefMut<IoProxy>> {
        // TODO: fix the many issues in this method. Runtime borrowing errors, etc.
        self.io_proxies.try_get_reference_mut(proxy).map_err(|e| FlowError::ErrNotImpl).and_then(|v| v.ok_or(FlowError::ErrNotImpl))
    }

    pub fn get_proxy_mut(&self, uuid: ::uuid::Uuid) -> Result<RefMut<IoProxy>> {
        // TODO: fix the many issues in this method. Runtime borrowing errors, etc.
        Ok(self.io_proxies.iter_mut().filter(|r| r.is_ok()).map(|r| r.unwrap()).find(|c| c.uuid == uuid).ok_or(FlowError::ErrNotImpl).unwrap())
    }

    pub fn create_io_from_copy_of_slice<'a, 'b>(&'a self, bytes: &'b [u8]) -> Result<RefMut<'a, IoProxy>> {
        IoProxy::copy_slice(self, bytes)
    }
    pub fn create_io_from_slice<'a>(&'a self, bytes: &'a [u8]) -> Result<RefMut<IoProxy>> {
        IoProxy::read_slice(self, bytes)
    }

    pub fn create_io_from_filename(&self, path: &str, dir: ::IoDirection) -> Result<RefMut<IoProxy>> {
        IoProxy::file(self, path, dir)
    }
    pub fn create_io_from_filename_with_mode(&self, path: &str, mode: ::IoMode) -> Result<RefMut<IoProxy>> {
        IoProxy::file_with_mode(self, path, mode)
    }

    pub fn create_io_output_buffer(&self) -> Result<RefMut<IoProxy>> {
        IoProxy::create_output_buffer(self)
    }


    pub fn todo_remove_get_floatspace(&self) -> ::ffi::Floatspace{
        unsafe {
            ::ffi::flow_context_get_floatspace(self.flow_c())
        }
    }

    pub fn todo_remove_set_floatspace(&self, b: ::ffi::Floatspace){
        unsafe {
            ::ffi::flow_context_set_floatspace(self.flow_c(),
                                               b,
                                               0f32,
                                               0f32,
                                               0f32)
        }
    }

    pub fn create_abi_json_response(&mut self,
                                    json_bytes: std::borrow::Cow<[u8]>,
                                    status_code: i64)
                                    -> *const ImageflowJsonResponse {
        unsafe {
            let sizeof_struct = std::mem::size_of::<ImageflowJsonResponse>();

            let pointer = ::ffi::flow_context_calloc(self.flow_c(),
                                                     1,
                                                     sizeof_struct + json_bytes.len(),
                                                     ptr::null(),
                                                     self.flow_c() as *mut libc::c_void,
                                                     ptr::null(),
                                                     0) as *mut u8;
            // Return null on OOM
            if pointer.is_null() {
                return ::std::ptr::null();
            }
            let pointer_to_final_buffer =
            pointer.offset(sizeof_struct as isize) as *mut libc::uint8_t;
            let imageflow_response = &mut (*(pointer as *mut ImageflowJsonResponse));
            imageflow_response.buffer_utf8_no_nulls = pointer_to_final_buffer;
            imageflow_response.buffer_size = json_bytes.len();
            imageflow_response.status_code = status_code;

            let mut out_json_bytes = std::slice::from_raw_parts_mut(pointer_to_final_buffer,
                                                                    json_bytes.len());

            out_json_bytes.clone_from_slice(&json_bytes);

            imageflow_response as *const ImageflowJsonResponse
        }
    }




    pub fn destroy_allowing_panics(mut self) {
        if !self.abi_begin_terminate(){
            self.c_error().unwrap().panic_with("Error during context shutdown");
        }
    }



    pub fn abi_destroy(mut self) -> bool{
        self.jobs.mut_clear();
        true
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
    pub fn abi_has_error(&self) -> bool{
        unsafe{
            ffi::flow_context_has_error(self.c_ctx)
        }
    }
    /// Used by abi; should not panic
    pub fn abi_clear_error(&mut self){
        unsafe {
            ffi::flow_context_clear_error(self.c_ctx)
        }
    }
    /// Used by abi; should not panic
    pub fn abi_error_code(&self) -> ErrorCode{
        unsafe {
            ffi::flow_context_error_reason(self.c_ctx)
        }
    }

    /// Used by abi; should not panic
    pub fn abi_abort_and_print_on_error(&self) -> bool{
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
    pub fn abi_raise_error_c_style(&mut self,
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
    pub fn abi_add_to_callstack_c_style(&mut self, filename: Option<&'static CStr>,
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

    pub fn assert_ok(&self) {
        if let Some(e) = self.get_error_copy() {
            e.panic_time();
        }
    }


    pub fn get_error_copy(&self) -> Option<FlowError> {
        if self.abi_has_error(){
            match self.abi_error_code(){
                0 => panic!("Inconsistent errors"),
                10 => Some(FlowError::Oom),
                _ => Some(FlowError::Err(unsafe { ErrorBuffer::get_flow_err(self.c_ctx) })),
            }
        }else{
            None
        }
    }
    unsafe fn get_flow_err(c: *mut ::ffi::ImageflowContext) -> FlowErr {


        let code = ::ffi::flow_context_error_reason(c);
        let mut buf = vec![0u8; 2048];


        let chars_written =
        ::ffi::flow_context_error_and_stacktrace(c, buf.as_mut_ptr(), buf.len(), false);

        if chars_written < 0 {
            panic!("Error msg doesn't fit in 2kb");
        } else {
            buf.resize(chars_written as usize, 0u8);
        }

        FlowErr {
            code: code,
            message_and_stack: String::from_utf8(buf).unwrap(),
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
        self.error.borrow_mut().c_ctx = ptr::null_mut();
    }
}
