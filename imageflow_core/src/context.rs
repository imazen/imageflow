use ::std;
use ::for_other_imageflow_crates::preludes::external_without_std::*;
use ::ffi;
use ::{CError, JsonResponse, ErrorKind, FlowError, Result};
use io::IoProxy;
use flow::definitions::Graph;
use std::any::Any;
use ::imageflow_types::collections::AddRemoveSet;
use ::ffi::ImageflowJsonResponse;
use ::errors::{OutwardErrorBuffer, CErrorProxy};
use ::rustc_serialize::base64;
use ::rustc_serialize::base64::ToBase64;
use codecs::CodecInstanceContainer;
use ffi::IoDirection;

pub struct Context {
    c_ctx: *mut ffi::ImageflowContext,
    error: CErrorProxy,
    outward_error:  OutwardErrorBuffer,
    pub debug_job_id: i32,
    pub next_stable_node_id: i32,
    pub next_graph_version: i32,
    pub max_calc_flatten_execute_passes: i32,
    pub graph_recording: s::Build001GraphRecording,
    pub codecs: AddRemoveSet<CodecInstanceContainer>, // This loans out exclusive mutable references to items, bounding the ownership lifetime to Context
    pub io_id_list: RefCell<Vec<i32>>
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
                io_id_list: RefCell::new(Vec::with_capacity(2))
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
        ::context_methods::CONTEXT_ROUTER.invoke(self, method, json)
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

    pub fn get_output_buffer_slice<'b>(&'b self, io_id: i32) -> Result<&'b [u8]> {
        self.get_codec(io_id).map_err(|e| e.at(here!()))?.get_encode_io()?.expect("Not an output buffer").get_output_buffer_bytes(self).map_err(|e| e.at(here!()))
    }

    pub fn add_file(&mut self, io_id: i32, direction: IoDirection, path: &str) -> Result<()> {
        let mode = match direction {
            s::IoDirection::In => ::ffi::IoMode::ReadSeekable,
            s::IoDirection::Out => ::ffi::IoMode::WriteSeekable,
        };
        self.add_file_with_mode(io_id, direction, path, mode).map_err(|e| e.at(here!()))
    }
    pub fn add_file_with_mode(&mut self, io_id: i32, direction: IoDirection, path: &str, mode: ::IoMode) -> Result<()> {
        if direction == IoDirection::In && !mode.can_read() {
            return Err(nerror!(ErrorKind::InvalidArgument, "You cannot add an input file with an IoMode that can't read"));
        }
        if direction == IoDirection::Out && !mode.can_write() {
            return Err(nerror!(ErrorKind::InvalidArgument, "You cannot add an output file with an IoMode that can't write"));
        }
        let mut io =  IoProxy::file_with_mode(self, io_id,  path, mode).map_err(|e| e.at(here!()))?;
        self.add_io(io, io_id, direction).map_err(|e| e.at(here!()))
    }


    pub fn add_copied_input_buffer(&mut self, io_id: i32, bytes: &[u8]) -> Result<()> {
        let mut io = IoProxy::copy_slice(self, io_id,  bytes).map_err(|e| e.at(here!()))?;

        self.add_io(io, io_id, IoDirection::In).map_err(|e| e.at(here!()))
    }
    pub fn add_input_bytes<'b>(&'b mut self, io_id: i32, bytes: &'b [u8]) -> Result<()> {
        self.add_input_buffer(io_id, bytes)
    }
    pub fn add_input_buffer<'b>(&'b mut self, io_id: i32, bytes: &'b [u8]) -> Result<()> {
        let mut io = IoProxy::read_slice(self, io_id,  bytes).map_err(|e| e.at(here!()))?;

        self.add_io(io, io_id, IoDirection::In).map_err(|e| e.at(here!()))
    }

    pub fn add_output_buffer(&mut self, io_id: i32) -> Result<()> {
        let mut io = IoProxy::create_output_buffer(self, io_id).map_err(|e| e.at(here!()))?;

        self.add_io(io, io_id, IoDirection::Out).map_err(|e| e.at(here!()))
    }


    pub fn get_image_info(&mut self, io_id: i32) -> Result<s::ImageInfo> {
        self.get_codec(io_id).map_err(|e| e.at(here!()))?.get_decoder().map_err(|e| e.at(here!()))?.get_image_info(self).map_err(|e| e.at(here!()))
    }

    pub fn tell_decoder(&mut self, io_id: i32, tell: s::DecoderCommand) -> Result<()> {
        self.get_codec(io_id).map_err(|e| e.at(here!()))?.get_decoder().map_err(|e| e.at(here!()))?.tell_decoder(self,  tell).map_err(|e| e.at(here!()))
    }

    pub fn get_exif_rotation_flag(&mut self, io_id: i32) -> Result<Option<i32>>{
        self.get_codec(io_id).map_err(|e| e.at(here!()))?.get_decoder().map_err(|e| e.at(here!()))?.get_exif_rotation_flag( self).map_err(|e| e.at(here!()))

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




    pub fn build_1(&mut self, parsed: s::Build001) -> Result<s::ResponsePayload> {
        let mut g =::parsing::GraphTranslator::new().translate_framewise(parsed.framewise).map_err(|e| e.at(here!())) ?;


        if let Some(s::Build001Config { graph_recording, .. }) = parsed.builder_config {
            if let Some(r) = graph_recording {
                self.configure_graph_recording(r);
            }
        }

        ::parsing::IoTranslator{}.add_all( self, parsed.io.clone())?;

        let perf = ::flow::execution_engine::Engine::create(self, & mut g).execute().map_err(|e| e.at(here!())) ?;

        Ok(s::ResponsePayload::BuildResult(s::JobResult { encodes: self.collect_augmented_encode_results( & g, &parsed.io), performance: Some(perf) }))
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

    pub fn execute_1(&mut self, what: s::Execute001) -> Result<s::ResponsePayload>{
        let mut g = ::parsing::GraphTranslator::new().translate_framewise(what.framewise).map_err(|e| e.at(here!()))?;
        if let Some(r) = what.graph_recording {
            self.configure_graph_recording(r);
        }

        let perf = ::flow::execution_engine::Engine::create(self, &mut g).execute().map_err(|e| e.at(here!()))?;

        Ok(s::ResponsePayload::JobResult(s::JobResult { encodes: Context::collect_encode_results(&g), performance: Some(perf) }))
    }

    pub fn collect_encode_results(g: &Graph) -> Vec<s::EncodeResult>{
        let mut encodes = Vec::new();
        for node in g.raw_nodes() {
            if let ::flow::definitions::NodeResult::Encoded(ref r) = node.weight.result {
                encodes.push((*r).clone());
            }
        }
        encodes
    }
    pub fn collect_augmented_encode_results(&self, g: &Graph, io: &[s::IoObject]) -> Vec<s::EncodeResult>{
        Context::collect_encode_results(g).into_iter().map(|r: s::EncodeResult|{
            if r.bytes == s::ResultBytes::Elsewhere {
                let obj: &s::IoObject = io.iter().find(|obj| obj.io_id == r.io_id).unwrap();//There's gotta be one
                let bytes = match obj.io {
                    s::IoEnum::Filename(ref str) => s::ResultBytes::PhysicalFile(str.to_owned()),
                    s::IoEnum::OutputBase64 => {
                        let slice = self.get_output_buffer_slice(r.io_id).map_err(|e| e.at(here!())).unwrap();
                        s::ResultBytes::Base64(slice.to_base64(base64::Config{char_set: base64::CharacterSet::Standard, line_length: None, newline: base64::Newline::LF, pad: true}))
                    },
                    _ => s::ResultBytes::Elsewhere
                };
                s::EncodeResult{
                    bytes: bytes,
                    .. r
                }
            }else{
                r
            }

        }).collect::<Vec<s::EncodeResult>>()
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
