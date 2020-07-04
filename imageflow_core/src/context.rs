use std;
use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::ffi;
use crate::{CError, JsonResponse, ErrorKind, FlowError, Result};
use crate::io::IoProxy;
use crate::flow::definitions::Graph;
use std::any::Any;
use imageflow_types::collections::AddRemoveSet;
use crate::ffi::ImageflowJsonResponse;
use crate::errors::{OutwardErrorBuffer, CErrorProxy};

use crate::codecs::CodecInstanceContainer;
use crate::codecs::EnabledCodecs;
use crate::ffi::IoDirection;

/// Something of a God object (which is necessary for a reasonable FFI interface).
pub struct Context {
    /// The C context object
    c_ctx: *mut ffi::ImageflowContext,
    /// Provides access to errors in the C context
    error: CErrorProxy,
    /// Buffer for errors presented to users of an FFI interface
    outward_error:  OutwardErrorBuffer,
    pub debug_job_id: i32,
    pub next_stable_node_id: i32,
    pub next_graph_version: i32,
    pub max_calc_flatten_execute_passes: i32,
    pub graph_recording: s::Build001GraphRecording,

    /// Codecs, which in turn connect to I/O instances.
    pub codecs: AddRemoveSet<CodecInstanceContainer>, // This loans out exclusive mutable references to items, bounding the ownership lifetime to Context
    /// A list of io_ids already in use
    pub io_id_list: RefCell<Vec<i32>>,

    pub enabled_codecs: EnabledCodecs,

    pub security: imageflow_types::ExecutionSecurity
}

static mut JOB_ID: i32 = 0;
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
                outward_error: OutwardErrorBuffer::new(),
                debug_job_id: unsafe{ JOB_ID },
                next_graph_version: 0,
                next_stable_node_id: 0,
                max_calc_flatten_execute_passes: 40,
                graph_recording: s::Build001GraphRecording::off(),
                codecs: AddRemoveSet::with_capacity(4),
                io_id_list: RefCell::new(Vec::with_capacity(2)),
                enabled_codecs: EnabledCodecs::default(),
                security: imageflow_types::ExecutionSecurity{
                    max_decode_size: None,
                    max_frame_size: Some(imageflow_types::FrameSizeLimit{
                        w: 10000,
                        h: 10000,
                        megapixels: 100f32
                    }),
                    max_encode_size: None
                }
            }))
        }
    }

    pub fn create_cant_panic() -> Result<Box<Context>> {
        std::panic::catch_unwind(|| {
            // Upgrade backtraces
            // Disable backtraces for debugging across the FFI boundary
            //imageflow_helpers::debug::upgrade_panic_hook_once_if_backtraces_wanted();

            Context::create_can_panic()
        }).unwrap_or_else(|_|Err(err_oom!())) //err_oom because it doesn't allocate anything.
    }


    /// Used by abi; should not panic
    pub fn abi_begin_terminate(&mut self) -> bool {
        self.codecs.mut_clear();
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
        crate::context_methods::CONTEXT_ROUTER.invoke(self, method, json)
    }


    pub fn flow_c(&self) -> *mut ffi::ImageflowContext{
        self.c_ctx
    }

    pub fn io_id_present(&self, io_id: i32) -> bool{
        self.io_id_list.borrow().iter().any(|v| *v == io_id)
    }

    fn add_io(&self, io: IoProxy, io_id: i32, direction: IoDirection) -> Result<()>{

        let codec_value = CodecInstanceContainer::create(self, io, io_id, direction).map_err(|e| e.at(here!()))?;
        let mut codec = self.codecs.add_mut(codec_value);
        if let Ok(d) = codec.get_decoder(){
            d.initialize( self).map_err(|e| e.at(here!()))?;
        }
        Ok(())
    }

    pub fn get_output_buffer_slice(&self, io_id: i32) -> Result<&[u8]> {
        let codec = self.get_codec(io_id).map_err(|e| e.at(here!()))?;
        let result = if let Some(io) = codec.get_encode_io().map_err(|e| e.at(here!()))? {
            io.map(|io| io.get_output_buffer_bytes(self).map_err(|e| e.at(here!())))
        }else{
            Err(nerror!(ErrorKind::InvalidArgument, "io_id {} is not an output buffer", io_id))
        };
        result
    }

    pub fn add_file(&mut self, io_id: i32, direction: IoDirection, path: &str) -> Result<()> {
        let io =  IoProxy::file_with_mode(self, io_id,  path, direction)
            .map_err(|e| e.at(here!()))?;
        self.add_io(io, io_id, direction).map_err(|e| e.at(here!()))
    }

    pub fn add_copied_input_buffer(&mut self, io_id: i32, bytes: &[u8]) -> Result<()> {
        let io = IoProxy::copy_slice(self, io_id,  bytes).map_err(|e| e.at(here!()))?;

        self.add_io(io, io_id, IoDirection::In).map_err(|e| e.at(here!()))
    }
    pub fn add_input_vector(&mut self, io_id: i32, bytes: Vec<u8>) -> Result<()> {
        let io = IoProxy::read_vec(self, io_id,  bytes).map_err(|e| e.at(here!()))?;

        self.add_io(io, io_id, IoDirection::In).map_err(|e| e.at(here!()))
    }

    pub fn add_input_bytes<'b>(&'b mut self, io_id: i32, bytes: &'b [u8]) -> Result<()> {
        self.add_input_buffer(io_id, bytes)
    }
    pub fn add_input_buffer<'b>(&'b mut self, io_id: i32, bytes: &'b [u8]) -> Result<()> {
        let io = unsafe { IoProxy::read_slice(self, io_id,  bytes)}.map_err(|e| e.at(here!()))?;

        self.add_io(io, io_id, IoDirection::In).map_err(|e| e.at(here!()))
    }

    pub fn add_output_buffer(&mut self, io_id: i32) -> Result<()> {
        let io = IoProxy::create_output_buffer(self, io_id).map_err(|e| e.at(here!()))?;

        self.add_io(io, io_id, IoDirection::Out).map_err(|e| e.at(here!()))
    }


    pub fn get_unscaled_image_info(&mut self, io_id: i32) -> Result<s::ImageInfo> {
        self.get_codec(io_id).map_err(|e| e.at(here!()))?.get_decoder().map_err(|e| e.at(here!()))?.get_unscaled_image_info(self).map_err(|e| e.at(here!()))
    }
    pub fn get_scaled_image_info(&mut self, io_id: i32) -> Result<s::ImageInfo> {
        self.get_codec(io_id).map_err(|e| e.at(here!()))?.get_decoder().map_err(|e| e.at(here!()))?.get_scaled_image_info(self).map_err(|e| e.at(here!()))
    }
    pub fn tell_decoder(&mut self, io_id: i32, tell: s::DecoderCommand) -> Result<()> {
        self.get_codec(io_id).map_err(|e| e.at(here!()))?.get_decoder().map_err(|e| e.at(here!()))?.tell_decoder(self,  tell).map_err(|e| e.at(here!()))
    }

    pub fn get_exif_rotation_flag(&mut self, io_id: i32) -> Result<Option<i32>>{
        self.get_codec(io_id)
            .map_err(|e| e.at(here!()))?
            .get_decoder()
            .map_err(|e| e.at(here!()))?
            .get_exif_rotation_flag( self)
            .map_err(|e| e.at(here!()))

    }

    pub fn get_codec(&self, io_id: i32) -> Result<RefMut<CodecInstanceContainer>> {
        let mut borrow_errors = 0;
        for item_result in self.codecs.iter_mut() {
            if let Ok(container) = item_result{
                if container.io_id == io_id {
                    return Ok(container);
                }
            }else{
                borrow_errors+=1;
            }
        }
        if borrow_errors > 0 {
            Err(nerror!(ErrorKind::FailedBorrow, "Could not locate codec by io_id {}; some codecs were exclusively borrowed by another scope.", io_id))
        } else {
            Err(nerror!(ErrorKind::IoIdNotFound, "No codec with io_id {}; all codecs searched.", io_id))
        }
    }



    /// For executing a complete job
    pub fn build_1(&mut self, parsed: s::Build001) -> Result<s::ResponsePayload> {
        let g = crate::parsing::GraphTranslator::new().translate_framewise(parsed.framewise).map_err(|e| e.at(here!())) ?;


        if let Some(s::Build001Config { graph_recording, security, .. }) = parsed.builder_config {
            if let Some(r) = graph_recording {
                self.configure_graph_recording(r);
            }
            if let Some(s) = security{
                self.configure_security(s);
            }
        }



        crate::parsing::IoTranslator{}.add_all( self, parsed.io.clone())?;

        let mut engine = crate::flow::execution_engine::Engine::create(self, g);

        let perf = engine.execute_many().map_err(|e| e.at(here!())) ?;


        Ok(s::ResponsePayload::BuildResult(s::JobResult { encodes: engine.collect_augmented_encode_results(&parsed.io), performance: Some(perf) }))
    }

    pub fn configure_graph_recording(&mut self, recording: s::Build001GraphRecording) {
        let r = if std::env::var("CI").and_then(|s| Ok(s.to_uppercase())) ==
            Ok("TRUE".to_owned()) {
            s::Build001GraphRecording::off()
        } else {
            recording
        };
        self.graph_recording = r;
    }

    pub fn configure_security(&mut self, s: s::ExecutionSecurity) {
        if let Some(decode) = s.max_decode_size{
            self.security.max_decode_size = Some(decode);
        }
        if let Some(frame) = s.max_frame_size{
            self.security.max_frame_size = Some(frame);
        }
        if let Some(encode) = s.max_encode_size{
            self.security.max_encode_size = Some(encode);
        }
    }

    /// For executing an operation graph (assumes you have already configured the context with IO sources/destinations as needed)
    pub fn execute_1(&mut self, what: s::Execute001) -> Result<s::ResponsePayload>{
        let g = crate::parsing::GraphTranslator::new().translate_framewise(what.framewise).map_err(|e| e.at(here!()))?;
        if let Some(r) = what.graph_recording {
            self.configure_graph_recording(r);
        }
        if let Some(s) = what.security{
            self.configure_security(s);
        }

        let mut engine = crate::flow::execution_engine::Engine::create(self, g);

        let perf = engine.execute_many().map_err(|e| e.at(here!()))?;

        Ok(s::ResponsePayload::JobResult(s::JobResult { encodes: engine.collect_encode_results(), performance: Some(perf) }))
    }

    pub fn get_version_info(&self) -> Result<s::VersionInfo>{
        Ok(s::VersionInfo{
            long_version_string: imageflow_types::version::one_line_version().to_string(),
            last_git_commit: imageflow_types::version::last_commit().to_string(),
            dirty_working_tree: imageflow_types::version::dirty(),
            build_date: imageflow_types::version::get_build_date().to_string()
        })
    }
}

#[cfg(test)]
fn test_get_output_buffer_slice_wrong_type_error(){

    let mut context = Context::create().unwrap();
    context.add_input_bytes(0, b"abcdef").unwrap();

    assert_eq!(ErrorKind::InvalidArgument, context.get_output_buffer_slice(0).err().unwrap().kind);

}

impl Drop for Context {
    /// Used by abi; should not panic
    fn drop(&mut self) {
        if let Err(e) = self.codecs.clear(){
            //TODO: log issue somewhere?
        }
        self.codecs.mut_clear(); // Dangerous, because there's no prohibition on dangling references.
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
