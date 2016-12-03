use ::{JsonResponse, IoDirection, MethodRouter};
use ::ffi::ImageflowJsonResponse;
use flow::definitions::Graph;
use ::internal_prelude::works_everywhere::*;
use ::rustc_serialize::base64;
use ::rustc_serialize::base64::ToBase64;

pub struct ContextPtr {
    // TODO: Remove pub as soon as tests/visuals.rs doesn't need access
    // (i.e, unit test helpers are ported, or the helper becomes cfgtest on the struct itself)
    pub ptr: Option<*mut ::ffi::ImageflowContext>,
}
pub struct Context {
    p: cell::RefCell<ContextPtr>,
}

pub struct SelfDisposingContextPtr {
    ptr: ContextPtr,
}
impl SelfDisposingContextPtr {
    pub fn create() -> Result<SelfDisposingContextPtr> {
        let p = ContextPtr::create()?;
        Ok(SelfDisposingContextPtr { ptr: p })
    }
    pub fn inner(&self) -> &ContextPtr {
        &self.ptr
    }

    pub fn create_job(&self) -> Result<JobPtr> {
        unsafe {
            JobPtr::create(self.ptr.as_ptr()?)
        }
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
    ptr: *mut ::ffi::ImageflowJob,
    c: *mut ::ffi::ImageflowContext,
}



impl JobPtr {
    pub fn context_ptr(&self) -> *mut ::ffi::ImageflowContext {
        self.c
    }
    pub fn as_ptr(&self) -> *mut ::ffi::ImageflowJob {
        self.ptr
    }

    pub fn from_ptr(context: *mut ::ffi::ImageflowContext,
                    job: *mut ::ffi::ImageflowJob)
                    -> Result<JobPtr> {
        if context.is_null() || job.is_null() {
            Err(FlowError::NullArgument)
        } else {
            Ok(JobPtr {
                ptr: job,
                c: context,
            })
        }
    }
    pub unsafe fn create(context: *mut ::ffi::ImageflowContext) -> Result<JobPtr> {
        if context.is_null() {
            return Err(FlowError::ContextInvalid);
        }

        let job = ::ffi::flow_job_create(context);
        if job.is_null() {
            Err(FlowError::Oom)
        } else {
            Ok(JobPtr {
                ptr: job,
                c: context,
            })
        }
    }
    pub unsafe fn add_io_ptr(&self,
                             io: *mut ::ffi::ImageflowJobIo,
                             io_id: i32,
                             direction: IoDirection)
                             -> Result<()> {
        let p = ::ffi::flow_job_add_io(self.context_ptr(), self.as_ptr(), io, io_id, direction);
        if !p {
            Err(self.ctx().get_error_copy().unwrap())
        } else {
            Ok(())
        }

    }

    pub fn add_input_bytes<'a>(&'a self, io_id: i32, bytes: &'a [u8]) -> Result<()> {
        unsafe {
            let p = ::ffi::flow_io_create_from_memory(self.context_ptr(),
                                                      ::ffi::IoMode::ReadSeekable,
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

    pub fn add_output_buffer(&self, io_id: i32) -> Result<()> {
        unsafe {
            let p =
                ::ffi::flow_io_create_for_output_buffer(self.context_ptr(),
                                                        self.context_ptr() as *const libc::c_void);
            if p.is_null() {
                Err(self.ctx().get_error_copy().unwrap())
            } else {
                self.add_io_ptr(p, io_id, IoDirection::Out)
            }
        }
    }



    //    pub fn record_graphs(&self){
    //        let _ = unsafe { ::ffi::flow_job_configure_recording(self.context_ptr(),
    //                                                             self.as_ptr(),
    //                                                             true,
    //                                                             true,
    //                                                             true,
    //                                                             false,
    //                                                             false) };
    //    }
    pub fn configure_graph_recording(&self, recording: s::Build001GraphRecording) {
        let r = if std::env::var("CI").and_then(|s| Ok(s.to_uppercase())) ==
                   Ok("TRUE".to_owned()) {
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

            if !::ffi::flow_job_get_decoder_info(self.context_ptr(),
                                                 self.as_ptr(),
                                                 io_id,
                                                 &mut info) {
                ContextPtr::from_ptr(self.context_ptr()).assert_ok(None);
            }
            Ok(s::ImageInfo {
                frame_decodes_into: s::PixelFormat::from(info.frame_decodes_into),
                image_height: info.image_height,
                image_width: info.image_width,
                frame_count: info.frame_count,
                current_frame_index: info.current_frame_index,
                preferred_extension: std::ffi::CStr::from_ptr(info.preferred_extension)
                    .to_owned()
                    .into_string()
                    .unwrap(),
                preferred_mime_type: std::ffi::CStr::from_ptr(info.preferred_mime_type)
                    .to_owned()
                    .into_string()
                    .unwrap(),
            })
        }

    }

    pub fn tell_decoder(&self, io_id: i32, tell: s::DecoderCommand) -> Result<()> {
        unsafe {
            match tell {
                s::DecoderCommand::JpegDownscaleHints(hints) => {
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



    pub fn message(&mut self, method: &str, json: &[u8]) -> Result<JsonResponse> {

        ::job_methods::JOB_ROUTER.invoke(self, method, json)
    }



    pub fn io_get_output_buffer_copy(&mut self, io_id: i32) -> Result<Vec<u8>> {
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
                } else if buf_start.is_null() {
                    // Not sure how output buffer is null... no writes yet?
                    Err(FlowError::ErrNotImpl)
                } else {
                    Ok((std::slice::from_raw_parts(buf_start, buf_len)).to_vec())
                }
            }
        }
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
    pub fn collect_augmented_encode_results(&mut self, g: &Graph, io: &[s::IoObject]) -> Vec<s::EncodeResult>{
        JobPtr::collect_encode_results(g).into_iter().map(|r: s::EncodeResult|{
            if r.bytes == s::ResultBytes::Elsewhere {
                let obj: &s::IoObject = io.iter().find(|obj| obj.io_id == r.io_id).unwrap();//There's gotta be one
                let bytes = match obj.io {
                    s::IoEnum::Filename(ref str) => s::ResultBytes::PhysicalFile(str.to_owned()),
                    s::IoEnum::OutputBase64 => {
                        let vec = self.io_get_output_buffer_copy(r.io_id).unwrap();
                        s::ResultBytes::Base64(vec.as_slice().to_base64(base64::Config{char_set: base64::CharacterSet::Standard, line_length: None, newline: base64::Newline::LF, pad: true}))
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

    fn ctx(&self) -> ContextPtr {
        ContextPtr::from_ptr(self.context_ptr())
    }
}

impl ContextPtr {
    pub fn create_abi_json_response(&self,
                                    json_bytes: std::borrow::Cow<[u8]>,
                                    status_code: i64)
                                    -> *const ImageflowJsonResponse {
        unsafe {
            let sizeof_struct = std::mem::size_of::<ImageflowJsonResponse>();

            let pointer = ::ffi::flow_context_calloc(self.ptr.unwrap(),
                                                     1,
                                                     sizeof_struct + json_bytes.len(),
                                                     ptr::null(),
                                                     self.ptr.unwrap() as *mut libc::c_void,
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
}

pub struct Job {
    pub p: cell::RefCell<JobPtr>,
}
pub struct JobIoPtr {
    pub ptr: Option<*mut ::ffi::ImageflowJobIo>,
}

pub struct JobIo<'a, T: 'a> {
    pub p: cell::RefCell<JobIoPtr>,
    pub _marker: marker::PhantomData<&'a T>,
}



impl Context {
    pub fn message(&mut self, method: &str, json: &[u8]) -> Result<JsonResponse> {
        let b = &mut (*self.p.borrow_mut());
        b.message(method, json)
    }
}



impl ContextPtr {
    pub fn message(&mut self, method: &str, json: &[u8]) -> Result<JsonResponse> {

        ::context_methods::CONTEXT_ROUTER.invoke(self, method, json)
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
                    if !::ffi::flow_context_begin_terminate(ptr) {
                        panic!("Error during context shutdown{:?}",
                               self.get_error_copy().unwrap());
                    }
                    ::ffi::flow_context_destroy(ptr);
                    None
                }
                _ => None,
            }
        }
    }

    pub fn from_ptr(ptr: *mut ::ffi::ImageflowContext) -> ContextPtr {
        ContextPtr {
            ptr: if ptr.is_null() { None } else { Some(ptr) }
        }
    }
    pub fn as_ptr(&self) -> Result<*mut ::ffi::ImageflowContext> {
        match self.ptr {
            Some(p) if !p.is_null() => Ok(p),
            _ => Err(FlowError::ContextInvalid),
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
        let b = &mut (*self.p.borrow_mut());
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
        let b = &(*self.p.borrow_mut());
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
        let b = &(*self.p.borrow_mut());
        match b.ptr {
            None => Err(FlowError::ContextInvalid),
            Some(ptr) => unsafe {
                let p = ::ffi::flow_io_create_from_memory(ptr,
                                                          ::ffi::IoMode::ReadSeekable,
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
        let b = &(*self.p.borrow_mut());
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
        let b = &(*self.p.borrow_mut());
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
        let b = &(*self.p.borrow_mut());
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
                    } else if buf_start.is_null() {
                        // Not sure how output buffer is null... no writes yet?
                        Err(FlowError::ErrNotImpl)
                    } else {
                        Ok((std::slice::from_raw_parts(buf_start, buf_len)))
                    }
                }



            },
        }
    }
}


impl ContextPtr {
    unsafe fn get_flow_err(&self, c: *mut ::ffi::ImageflowContext) -> FlowErr {


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
    pub unsafe fn err_maybe(&self) -> Result<()> {
        match self.get_error_copy() {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    pub unsafe fn assert_ok(&self, g: Option<&Graph>) {
        if let Some(which_error) = self.get_error_copy() {
            match which_error {
                FlowError::Err(e) => {
                    println!("Error {} {}\n", e.code, e.message_and_stack);
                    if (e.code == 72 || e.code == 73) && g.is_some() {
                        //                                let _ = ::flow::graph::print_to_stdout(
                        //                                    self.ptr.unwrap(),
                        //                                    g.unwrap() as &flow::graph::Graph);
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
                other => {
                    panic!("{:?}", other);
                }
            }
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
