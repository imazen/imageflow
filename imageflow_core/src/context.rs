use std;
use std::{ptr,marker,slice,cell};
use libc;
use ::{FlowError,FlowErr, JsonResponse,JsonResponseError,Result,IoDirection};
use ::ffi::ImageflowJsonResponse;
use std::path::Path;
use std::fs::File;
use std::io::Write;

extern crate imageflow_serde as s;
extern crate serde_json;

pub struct ContextPtr {
    // TODO: Remove pub as soon as tests/visuals.rs doesn't need access
    // (i.e, unit test helpers are ported, or the helper becomes cfgtest on the struct itself)
    pub ptr: Option<*mut ::ffi::Context>,
}
pub struct Context {
    p: cell::RefCell<ContextPtr>,
}

pub struct SelfDisposingContextPtr{
    ptr: ContextPtr
}
impl SelfDisposingContextPtr{
    pub fn create() -> Result<SelfDisposingContextPtr> {
        let p = ContextPtr::create()?;
        Ok(SelfDisposingContextPtr{ptr: p})
    }
    pub fn inner(&self) -> &ContextPtr{
        &self.ptr
    }

    pub fn create_job(&self) -> Result<JobPtr> {
        JobPtr::create(self.ptr.as_ptr()?)
    }

    pub fn destroy_allowing_panics(mut self) -> () {
        self.ptr.destroy_allowing_panics()
    }

}
impl Drop for SelfDisposingContextPtr {
    fn drop(&mut self) {
        self.ptr.force_destroy();
    }
}

pub struct JobPtr {
    ptr: *mut ::ffi::Job,
    c: *mut ::ffi::Context
}

impl JobPtr {
    pub fn context_ptr(&self) -> *mut ::ffi::Context{ self.c }
    pub fn as_ptr(&self) -> *mut ::ffi::Job { self.ptr}

    pub fn from_ptr(context: *mut ::ffi::Context, job: *mut ::ffi::Job) -> Result<JobPtr> {
        if context.is_null() || job.is_null() {
            Err(FlowError::NullArgument)
        }else {
            Ok(JobPtr {
                ptr: job,
                c: context
            })
        }
    }
    pub fn create(context: *mut ::ffi::Context) -> Result<JobPtr> {
        if context.is_null() {
            return Err(FlowError::ContextInvalid)
        }
        unsafe {
            let job = ::ffi::flow_job_create(context);
            if job.is_null() {
                Err(FlowError::Oom)
            }else{
                Ok(JobPtr { ptr: job, c: context})
            }
        }
    }
    pub unsafe fn add_io_ptr(&self,
                         io: *mut ::ffi::JobIO,
                         io_id: i32,
                         direction: IoDirection)
                         -> Result<()> {
        let p = ::ffi::flow_job_add_io(self.context_ptr(),
                                       self.as_ptr(),
                                       io,
                                       io_id,
                                       direction);
        if !p {
            Err(self.ctx().get_error_copy().unwrap())
        } else {
            Ok(())
        }

    }

    pub fn add_input_bytes<'a>(&'a self, io_id: i32, bytes: &'a [u8]) -> Result<()>{
        unsafe {
            let p = ::ffi::flow_io_create_from_memory(self.context_ptr(),
                                                      ::ffi::IoMode::read_seekable,
                                                      bytes.as_ptr(),
                                                      bytes.len(),
                                                      self.context_ptr() as *const libc::c_void,
                                                      ptr::null());
            if p.is_null() {
                Err(self.ctx().get_error_copy().unwrap())
            } else {
                self.add_io_ptr(p, io_id, IoDirection::In)
            }
        }
    }

    pub fn add_output_buffer<'a>(&'a self, io_id: i32) -> Result<()>{
        unsafe {
            let p = ::ffi::flow_io_create_for_output_buffer(self.context_ptr(), self.context_ptr() as *const libc::c_void);
            if p.is_null() {
                Err(self.ctx().get_error_copy().unwrap())
            } else {
                self.add_io_ptr(p, io_id, IoDirection::Out)
            }
        }
    }



    pub fn record_graphs(&self){
        let _ = unsafe { ::ffi::flow_job_configure_recording(self.context_ptr(),
                                                             self.as_ptr(),
                                                             true,
                                                             true,
                                                             true,
                                                             false,
                                                             false) };
    }
    pub fn configure_graph_recording(&self, recording: s::Build001GraphRecording) {
        let r = if std::env::var("CI").and_then(|s| Ok(s.to_uppercase()))  == Ok("TRUE".to_owned()) {
            s::Build001GraphRecording::off()
        } else {
            recording
        };
        let _ = unsafe {
            ::ffi::flow_job_configure_recording(self.context_ptr(),
                                                self.as_ptr(),
                                                r.record_graph_versions
                                                    .unwrap_or(false),
                                                r.record_frame_images
                                                    .unwrap_or(false),
                                                r.render_last_graph
                                                    .unwrap_or(false),
                                                r.render_graph_versions
                                                    .unwrap_or(false),
                                                r.render_animated_graph
                                                    .unwrap_or(false))
        };
    }


    pub fn get_image_info(&self, io_id: i32) -> Result<s::ImageInfo> {
        unsafe {
            let mut info: ::ffi::DecoderInfo = ::ffi::DecoderInfo { ..Default::default() };

            if !::ffi::flow_job_get_decoder_info(self.context_ptr(), self.as_ptr(), 0, &mut info) {
                ContextPtr::from_ptr(self.context_ptr()).assert_ok(None);
            }
            Ok(s::ImageInfo {
                frame0_post_decode_format: s::PixelFormat::from(info.frame0_post_decode_format),
                frame0_height: info.frame0_height,
                frame0_width: info.frame0_width,
                frame_count: info.frame_count,
                current_frame_index: info.current_frame_index,
                preferred_extension: std::ffi::CStr::from_ptr(info.preferred_extension).to_owned().into_string().unwrap(),
                preferred_mime_type: std::ffi::CStr::from_ptr(info.preferred_mime_type).to_owned().into_string().unwrap(),
            })
        }

    }

    pub fn tell_decoder(&self, io_id: i32, tell: s::TellDecoderWhat ) -> Result<()> {
        unsafe {
            match tell {
                s::TellDecoderWhat::JpegDownscaleHints(hints) => {
                    if !::ffi::flow_job_decoder_set_downscale_hints_by_placeholder_id(self.context_ptr(),
                                                                                      self.as_ptr(), io_id,
                                                                                      hints.width, hints.height,
                                                                                      hints.width, hints.height,
                                                                                      hints.scale_luma_spatially.unwrap_or(false),
                                                                                      hints.gamma_correct_for_srgb_during_spatial_luma_scaling.unwrap_or(false)

                    ){
                        panic!("");
                    }
                }
            }
        }
        Ok(())

    }

    pub fn document_message() -> String {
        let mut s = String::new();
        s.reserve(8000);
        s += "JSON API - Job\n\n";
        s += "imageflow_job responds to these message methods\n\n";
        s += "## v0.0.1/get_image_info \n";
        s += "Example message body:\n";
        s += &serde_json::to_string_pretty(&s::GetImageInfo001::example_get_image_info()).unwrap();
        s += "\nExample response:\n";
        s += &serde_json::to_string_pretty(&s::Response001::example_image_info()).unwrap();
        s += "\n\n";


        s += "## v0.0.1/tell_decoder \n";
        s += "Example message body:\n";
        s += &serde_json::to_string_pretty(&s::TellDecoder001::example_hints()).unwrap();
        s += "\nExample response:\n";
        s += &serde_json::to_string_pretty(&s::Response001::example_ok()).unwrap();
        s += "\n\n";

        s += "## v0.0.1/execute \n";
        s += "Example message body (with graph):\n";
        s += &serde_json::to_string_pretty(&s::Execute001::example_graph()).unwrap();
        s += "Example message body (with linear steps):\n";
        s += &serde_json::to_string_pretty(&s::Execute001::example_steps()).unwrap();
        s += "\nExample response:\n";
        s += &serde_json::to_string_pretty(&s::Response001::example_ok()).unwrap();
        s += "\nExample failure response:\n";
        s += &serde_json::to_string_pretty(&s::Response001::example_error()).unwrap();
        s += "\n\n";

        s
    }

    pub fn message<'a, 'b, 'c>(&'a mut self,
                               method: &'b str,
                               json: &'b [u8])
                               -> Result<JsonResponse<'c>> {

        match method {
            "v0.0.1/get_image_info" => {
                let parsed_maybe: std::result::Result<s::GetImageInfo001, serde_json::Error> = serde_json::from_slice(json);
                match parsed_maybe {
                    Ok(parsed) => {
                        let info = self.get_image_info(parsed.io_id).unwrap();
                        Ok(JsonResponse::success_with_payload(s::ResponsePayload::ImageInfo(info)))
                    }
                    Err(e) => {
                        Ok(JsonResponse::from_parse_error(e,json))
                    }
                }

            }
            "v0.0.1/tell_decoder" => {
                let parsed_maybe: std::result::Result<s::TellDecoder001, serde_json::Error> = serde_json::from_slice(json);
                match parsed_maybe {
                    Ok(parsed) => {
                        self.tell_decoder(parsed.io_id, parsed.command).unwrap();
                        Ok(JsonResponse::ok())
                    }
                    Err(e) => {
                        Ok(JsonResponse::from_parse_error(e,json))
                    }
                }
            }
            "v0.0.1/execute" => {
                let parsed_maybe: std::result::Result<s::Execute001, serde_json::Error> = serde_json::from_slice(json);
                match parsed_maybe {
                    Ok(parsed) => {
                        let mut g = ::parsing::GraphTranslator::new().translate_framewise(parsed.framewise);
                        if let Some(r) = parsed.graph_recording {
                            self.configure_graph_recording(r);
                        }
                        unsafe {
                            if let Some(b) = parsed.no_gamma_correction {
                                ::ffi::flow_context_set_floatspace(self.c, match b {
                                    true => ::ffi::Floatspace::srgb,
                                    false => ::ffi::Floatspace::linear
                                }, 0f32, 0f32, 0f32)
                            }
                        }
                        if !self.execute(&mut g){
                            unsafe { self.ctx().assert_ok(Some(&mut g)); }
                        }
                        Ok(JsonResponse::ok())
                    }
                    Err(e) => {
                        Ok(JsonResponse::from_parse_error(e,json))
                    }
                }
            }
            "brew_coffee" => Ok(JsonResponse::teapot()),
            _ => Ok(JsonResponse::method_not_understood())
        }
    }



    pub fn io_get_output_buffer_copy<'a, 'b>(&'a mut self,
                                        io_id: i32)
                                        -> Result<Vec<u8>> {
        unsafe {
            let io_p = ::ffi::flow_job_get_io(self.c, self.ptr, io_id);
            if io_p.is_null() {
                Err(self.ctx().get_error_copy().unwrap())
            } else {
                let mut buf_start: *const u8 = ptr::null();
                let mut buf_len: usize = 0;
                let worked = ::ffi::flow_io_get_output_buffer(self.c,
                                                              io_p,
                                                              &mut buf_start as *mut *const u8,
                                                              &mut buf_len as *mut usize);
                if !worked {
                    Err(self.ctx().get_error_copy().unwrap())
                } else {
                    if buf_start.is_null() {
                        // Not sure how output buffer is null... no writes yet?
                        Err(FlowError::ErrNotImpl)
                    } else {
                        Ok((std::slice::from_raw_parts(buf_start, buf_len)).to_vec())
                    }
                }
            }
        }
    }



    fn ctx(&self) -> ContextPtr{
        ContextPtr::from_ptr(self.context_ptr())
    }
}

impl ContextPtr {
    pub fn create_abi_json_response(&self, json_bytes: std::borrow::Cow<[u8]>, status_code: i64) -> *const ImageflowJsonResponse{
        unsafe {
            let sizeof_struct = std::mem::size_of::<ImageflowJsonResponse>();

            let pointer = ::ffi::flow_context_calloc(self.ptr.unwrap(),
                                                   1,
                                                   sizeof_struct + json_bytes.len(),
                                                   ptr::null(),
                                                   self.ptr.unwrap() as *mut libc::c_void,
                                                   ptr::null(),
                                                   0) as *mut u8;
            //Return null on OOM
            if pointer.is_null() {
                return ::std::ptr::null();
            }
            let pointer_to_final_buffer = pointer.offset(sizeof_struct as isize) as *mut libc::uint8_t;
            let ref mut imageflow_response = *(pointer as *mut ImageflowJsonResponse);
            imageflow_response.buffer_utf8_no_nulls = pointer_to_final_buffer;
            imageflow_response.buffer_size = json_bytes.len();
            imageflow_response.status_code = status_code;

            let mut out_json_bytes = std::slice::from_raw_parts_mut(pointer_to_final_buffer,
                                                                    json_bytes.len());

            out_json_bytes.clone_from_slice(&json_bytes);

            imageflow_response as *const ImageflowJsonResponse
        }
    }

}

pub struct Job {
    pub p: cell::RefCell<JobPtr>,
}
pub struct JobIoPtr {
    pub ptr: Option<*mut ::ffi::JobIO>,
}

pub struct JobIo<'a, T: 'a> {
    pub p: cell::RefCell<JobIoPtr>,
    pub _marker: marker::PhantomData<&'a T>,
}



impl Context {
    pub fn message<'a, 'b, 'c>(&'a mut self,
                               method: &'b str,
                               json: &'b [u8])
                               -> Result<JsonResponse> {
        let ref mut b = *self.p.borrow_mut();
        b.message(method, json)
    }
}

fn get_create_doc_dir() -> std::path::PathBuf {
    let path = Path::new(file!()).parent().unwrap().join(Path::new("../../target/doc"));
    let _ = std::fs::create_dir_all(&path);
    //Error { repr: Os { code: 17, message: "File exists" } }
    //The above can happen, despite the docs.
    path
}
#[test]
fn write_context_doc(){
    let path = get_create_doc_dir().join(Path::new("context_json_api.txt"));
    File::create(&path).unwrap().write_all(ContextPtr::document_message().as_bytes()).unwrap();
}

#[test]
fn write_job_doc(){
    let path = get_create_doc_dir().join(Path::new("job_json_api.txt"));
    File::create(&path).unwrap().write_all(JobPtr::document_message().as_bytes()).unwrap();
}

impl ContextPtr {

    pub fn document_message() -> String {
        let mut s = String::new();
        s.reserve(8000);
        s += "# JSON API - Context\n\n";
        s += "imageflow_context responds to these message methods\n\n";
        s += "## v0.0.1/build \n";
        s += "Example message body:\n";
        s += &serde_json::to_string_pretty(&s::Build001::example_with_steps()).unwrap();
        s += "\n\nExample response:\n";
        s += &serde_json::to_string_pretty(&s::Response001::example_ok()).unwrap();
        s += "\n\nExample failure response:\n";
        s += &serde_json::to_string_pretty(&s::Response001::example_error()).unwrap();
        s += "\n\n";

        s
    }






    pub fn message<'a, 'b, 'c>(&'a mut self,
                               method: &'b str,
                               json: &'b [u8])
                               -> Result<JsonResponse<'c>> {
        if self.ptr.is_none() {
            return Err(FlowError::ContextInvalid);
        }
        let response = match method {
            "brew_coffee" => JsonResponse::teapot(),
            "v0.0.1/build" => unsafe {

                let handler = ::parsing::BuildRequestHandler::new();
                let response = handler.do_and_respond(&mut *self, json);
                self.assert_ok(None);

                response.unwrap()
            },
            _ => JsonResponse::method_not_understood()
        };
        Ok(response)
    }

    fn build_0_0_1<'a, 'b, 'c>(&'a mut self, json: &'b [u8]) -> Result<JsonResponse<'c>> {
        match ::parsing::BuildRequestHandler::new().do_and_respond(self, json) {
            Ok(response) => Ok(response),
            Err(original_err) => {
                Err(match original_err {
                    JsonResponseError::Oom(()) => FlowError::Oom,
                    JsonResponseError::NotImplemented(()) => FlowError::ErrNotImpl,
                    JsonResponseError::Other(e) => FlowError::ErrNotImpl,
                })
            }
        }
    }
}

impl ContextPtr {
    pub fn create() -> Result<ContextPtr> {
        unsafe {
            let ptr = ::ffi::flow_context_create();

            if ptr.is_null() {
                Err(FlowError::Oom)
            } else {
                Ok(ContextPtr { ptr: Some(ptr) })
            }
        }
    }

    fn force_destroy(&mut self) {
        unsafe {
            self.ptr = match self.ptr {
                Some(ptr) => {
                    ::ffi::flow_context_destroy(ptr);
                    None
                }
                _ => None,
            }
        }
    }

    fn destroy_allowing_panics(&mut self) {
        unsafe {
            self.ptr = match self.ptr {
                Some(ptr) => {
                    if !::ffi::flow_context_begin_terminate(ptr){
                        panic!("Error during context shutdown{:?}", self.get_error_copy().unwrap());
                    }
                    ::ffi::flow_context_destroy(ptr);
                    None
                }
                _ => None,
            }
        }
    }

    pub fn from_ptr(ptr: *mut ::ffi::Context) -> ContextPtr {
        ContextPtr {
            ptr: match ptr.is_null() {
                false => Some(ptr),
                true => None,
            },
        }
    }
    pub fn as_ptr(&self) -> Result<*mut ::ffi::Context> {
        match self.ptr {
            Some(p) if p != ptr::null_mut() => Ok(p),
            _ =>  Err(FlowError::ContextInvalid),
        }
    }
}



impl Drop for Context {
    fn drop(&mut self) {
        (*self.p.borrow_mut()).force_destroy();
    }
}
impl Context {
    pub fn create() -> Result<Context> {
        unsafe {
            let ptr = ::ffi::flow_context_create();

            if ptr.is_null() {
                Err(FlowError::Oom)
            } else {
                Ok(Context { p: cell::RefCell::new(ContextPtr { ptr: Some(ptr) }) })
            }
        }
    }

    pub fn unsafe_borrow_mut_context_pointer(&mut self) -> std::cell::RefMut<ContextPtr> {
        self.p.borrow_mut()
    }

    fn get_error_copy(&self) -> Option<FlowError> {
        (*self.p.borrow()).get_error_copy()
    }

    pub fn destroy(self) -> Result<()> {
        let ref mut b = *self.p.borrow_mut();
        match b.ptr {
            None => Ok(()),
            Some(ptr) => unsafe {
                if !::ffi::flow_context_begin_terminate(ptr) {
                    // Already borrowed; will panic!
                    // This kind of bug is only exposed at runtime, now.
                    // Code reuse will require two copies of every function
                    // One against the ContextPtr, to be reused
                    // One exposed publicly against the Context, which performs the borrowing
                    // Same scenario will occur with other types.
                    // let copy = self.get_error_copy().unwrap();

                    // So use the ContextPtr version
                    let copy = b.get_error_copy().unwrap();
                    b.force_destroy();
                    Err(copy)
                } else {
                    b.force_destroy();
                    Ok(())
                }
            },
        }
    }

    pub fn create_job(&mut self) -> Result<Job> {
        let ref b = *self.p.borrow_mut();
        match b.ptr {
            None => Err(FlowError::ContextInvalid),
            Some(ptr) => unsafe {
                let p = ::ffi::flow_job_create(ptr);
                if p.is_null() {
                    Err(b.get_error_copy().unwrap())
                } else {
                    Ok(Job { p: cell::RefCell::new(JobPtr::from_ptr(ptr, p).unwrap()) })
                }
            },
        }
    }


    pub fn create_io_from_slice<'a, 'c>(&'c mut self,
                                        bytes: &'a [u8])
                                        -> Result<JobIo<'a, &'a [u8]>> {
        let ref b = *self.p.borrow_mut();
        match b.ptr {
            None => Err(FlowError::ContextInvalid),
            Some(ptr) => unsafe {
                let p = ::ffi::flow_io_create_from_memory(ptr,
                                                          ::ffi::IoMode::read_seekable,
                                                          bytes.as_ptr(),
                                                          bytes.len(),
                                                          ptr as *const libc::c_void,
                                                          ptr::null());
                if p.is_null() {
                    Err(b.get_error_copy().unwrap())
                } else {
                    Ok(JobIo {
                        _marker: marker::PhantomData,
                        p: cell::RefCell::new(JobIoPtr { ptr: Some(p) }),
                    })
                }
            },
        }
    }


    pub fn create_io_output_buffer<'a, 'b>(&'a mut self) -> Result<JobIo<'b, ()>> {
        let ref b = *self.p.borrow_mut();
        match b.ptr {
            None => Err(FlowError::ContextInvalid),
            Some(ptr) => unsafe {
                let p = ::ffi::flow_io_create_for_output_buffer(ptr, ptr as *const libc::c_void);
                if p.is_null() {
                    Err(b.get_error_copy().unwrap())
                } else {
                    Ok(JobIo {
                        _marker: marker::PhantomData,
                        p: cell::RefCell::new(JobIoPtr { ptr: Some(p) }),
                    })
                }
            },
        }
    }

    pub fn job_add_io<T>(&mut self,
                         job: &mut Job,
                         io: JobIo<T>,
                         io_id: i32,
                         direction: IoDirection)
                         -> Result<()> {
        let ref b = *self.p.borrow_mut();
        match b.ptr {
            None => Err(FlowError::ContextInvalid),
            Some(ptr) => unsafe {
                let p = ::ffi::flow_job_add_io(ptr,
                                               (*job.p.borrow_mut()).ptr,
                                               (*io.p.borrow_mut()).ptr.unwrap(),
                                               io_id,
                                               direction);
                if !p {
                    Err(b.get_error_copy().unwrap())
                } else {
                    Ok(())
                }
            },
        }
    }


    pub fn io_get_output_buffer<'a, 'b>(&'a mut self,
                                        job: &'b Job,
                                        io_id: i32)
                                        -> Result<&'b [u8]> {
        let ref b = *self.p.borrow_mut();
        match b.ptr {
            None => Err(FlowError::ContextInvalid),
            Some(ptr) => unsafe {

                let io_p = ::ffi::flow_job_get_io(ptr, (*job.p.borrow_mut()).ptr, io_id);
                if io_p.is_null() {
                    Err(b.get_error_copy().unwrap())
                } else {
                    let mut buf_start: *const u8 = ptr::null();
                    let mut buf_len: usize = 0;
                    let worked = ::ffi::flow_io_get_output_buffer(ptr,
                                                                  io_p,
                                                                  &mut buf_start as *mut *const u8,
                                                                  &mut buf_len as *mut usize);
                    if !worked {
                        Err(b.get_error_copy().unwrap())
                    } else {
                        if buf_start.is_null() {
                            // Not sure how output buffer is null... no writes yet?
                            Err(FlowError::ErrNotImpl)
                        } else {
                            Ok((std::slice::from_raw_parts(buf_start, buf_len)))
                        }
                    }
                }



            },
        }
    }
}


impl ContextPtr {


    unsafe fn get_flow_err(&self, c: *mut ::ffi::Context) -> FlowErr {


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


    pub unsafe fn assert_ok(&self, g: Option<&::flow::graph::Graph>) {
        match self.get_error_copy() {
            Some(which_error) => {
                match which_error {
                    FlowError::Err(e) => {

                        println!("Error {} {}\n", e.code, e.message_and_stack);
                        if e.code == 72 || e.code == 73 {
                            if g.is_some() {
                                //                                let _ = ::flow::graph::print_to_stdout(
                                //                                    self.ptr.unwrap(),
                                //                                    g.unwrap() as &flow::graph::Graph);
                            }
                        }

                        panic!();
                    }
                    FlowError::Oom => {
                        panic!("Out of memory.");
                    }
                    FlowError::ErrNotImpl => {
                        panic!("Error not implemented");
                    }
                    FlowError::ContextInvalid => {
                        panic!("Context pointer null");
                    }
                    FlowError::NullArgument => {
                        panic!("Context pointer null");
                    }

                }
            }
            None => {}
        }
    }


    fn get_error_copy(&self) -> Option<FlowError> {
        unsafe {
            match self.ptr {
                Some(ptr) if ::ffi::flow_context_has_error(ptr) => {
                    match ::ffi::flow_context_error_reason(ptr) {
                        0 => panic!("Inconsistent errors"),
                        10 => Some(FlowError::Oom),
                        _ => Some(FlowError::Err(self.get_flow_err(ptr))),
                    }
                }
                None => Some(FlowError::ContextInvalid),
                Some(_) => None,
            }
        }
    }
}



#[test]
fn it_works() {
    let mut c = Context::create().unwrap();

    let mut j = c.create_job().unwrap();


    let bytes = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49,
        0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06,
        0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44,
        0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D,
        0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42,
        0x60, 0x82];

    let input = c.create_io_from_slice(&bytes).unwrap();

    let output = c.create_io_output_buffer().unwrap();

    c.job_add_io(&mut j, input, 0, IoDirection::In).unwrap();
    c.job_add_io(&mut j, output, 1, IoDirection::Out).unwrap();


    // let output_bytes = c.io_get_output_buffer(&j, 1).unwrap();

    assert_eq!(c.destroy(), Ok(()));

}