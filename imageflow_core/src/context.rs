use ::std;
use ::for_other_imageflow_crates::preludes::external_without_std::*;
use ::ffi;
use ::job::Job;
use ::{CError, JsonResponse, ErrorKind, FlowError, Result};
use io::IoProxy;
use std::any::Any;
use ::imageflow_types::collections::AddRemoveSet;
use ::ffi::ImageflowJsonResponse;
use ::errors::{OutwardErrorBuffer, CErrorProxy};


pub struct Context {
    //version: u64, (so different context types can be differentiated)
    c_ctx: *mut ffi::ImageflowContext,
    jobs: AddRemoveSet<Job>,
    io_proxies: AddRemoveSet<IoProxy>,
    error: CErrorProxy,
    outward_error:  OutwardErrorBuffer
}

impl Context {

    pub fn create() -> Result<Box<Context>>{
        Context::create_cant_panic()
    }

    pub fn create_can_panic() -> Result<Box<Context>>{
        let inner = unsafe { ffi::flow_context_create() };
        if inner.is_null() {
            Err(err_oom!())
        } else {
            Ok(Box::new(Context {
                c_ctx: inner,
                error: CErrorProxy::new(inner),
                jobs: AddRemoveSet::with_capacity(2),
                io_proxies: AddRemoveSet::with_capacity(2),
                outward_error: OutwardErrorBuffer::new()
            }))
        }
    }

    pub fn create_cant_panic() -> Result<Box<Context>> {
        std::panic::catch_unwind(|| {
            // Upgrade backtraces
            // Disable backtraces for debugging across the FFI boundary
            //imageflow_helpers::debug::upgrade_panic_hook_once_if_backtraces_wanted();

            Context::create_can_panic()
        }).unwrap_or(Err(err_oom!())) //err_oom because it doesn't allocate anything.
    }


    /// Used by abi; should not panic
    pub fn abi_begin_terminate(&mut self) -> bool {
        self.jobs.mut_clear();
        self.io_proxies.mut_clear();
        unsafe {
            ffi::flow_context_begin_terminate(self.c_ctx)
        }
    }
    pub fn destroy(mut self) -> Result<()>{
        if self.abi_begin_terminate(){
            Ok(())
        }else {
            Err(cerror!(self,"Error encountered while terminating Context"))
        }
    }

    pub fn outward_error(&self) -> &OutwardErrorBuffer{
        &self.outward_error
    }
    pub fn outward_error_mut(&mut self) -> &mut OutwardErrorBuffer{
        &mut self.outward_error
    }

    pub fn c_error_mut(&mut self) -> &mut CErrorProxy{
        &mut self.error
    }
    pub fn c_error(&self) -> &CErrorProxy{
        &self.error
    }


    pub fn message(&mut self, method: &str, json: &[u8]) -> (JsonResponse, Result<()>) {
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




    pub fn get_proxy_mut_by_pointer(&self, proxy: *const IoProxy) -> Result<RefMut<IoProxy>> {
        // TODO: fix the many issues in this method. Runtime borrowing errors, etc.
        self.io_proxies.try_get_reference_mut(proxy).map_err(|e| unimpl!()).and_then(|v| v.ok_or(unimpl!()))
    }

    pub fn get_proxy_mut(&self, uuid: ::uuid::Uuid) -> Result<RefMut<IoProxy>> {
        // TODO: fix the many issues in this method. Runtime borrowing errors, etc.
        Ok(self.io_proxies.iter_mut().filter(|r| r.is_ok()).map(|r| r.unwrap()).find(|c| c.uuid == uuid).ok_or(unimpl!()).unwrap())
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





    pub fn build_1(&self, parsed: s::Build001) -> Result<s::ResponsePayload> {
        let mut g =::parsing::GraphTranslator::new().translate_framewise(parsed.framewise) ?;

        let mut job = self.create_job();

        if let Some(s::Build001Config { graph_recording, .. }) = parsed.builder_config {
            if let Some(r) = graph_recording {
                job.configure_graph_recording(r);
            }
        }

        ::parsing::IoTranslator::new(self).add_to_job( &mut * job, parsed.io.clone());

        ::flow::execution_engine::Engine::create(self, & mut job, & mut g).execute() ?;

        Ok(s::ResponsePayload::BuildResult(s::JobResult { encodes: job.collect_augmented_encode_results( & g, &parsed.io) }))
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
        self.error = CErrorProxy::null();
    }
}

#[test]
fn test_context_size(){
    println!("std::mem::sizeof(Context) = {}", std::mem::size_of::<Context>());
    assert!(std::mem::size_of::<Context>() < 500);
}
