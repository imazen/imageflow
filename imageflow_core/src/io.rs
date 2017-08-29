use ::std;
use ::for_other_imageflow_crates::preludes::external_without_std::*;
use ::ffi;
use ::job::Job;
use ::{Context, CError, Result, JsonResponse, ErrorKind};
use ::ffi::ImageflowJobIo;
use ::imageflow_types::collections::AddRemoveSet;
use std::ascii::AsciiExt;
use uuid::Uuid;


pub struct IoProxy{
    c: &'static Context,
    classic: *mut ImageflowJobIo,
    pub uuid: Uuid,
    path: Option<PathBuf>,
    c_path: Option<CString>,
    drop_with_job: bool
}


impl io::Read for IoProxy{

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>{
        self.read_to_buffer(self.c, buf).map(|v|
            if v < 0 || v as u64 > <usize>::max_value() as u64 {
                panic!("");
            }else{
                v as usize
            }).map_err(|e| std::io::Error::new(io::ErrorKind::Other, e))
    }
}
impl io::Write for IoProxy{

    fn write(&mut self, buf: &[u8]) -> io::Result<usize>{
        self.write_from_buffer(self.c, buf).map(|v|
            if v < 0 || v as u64 > <usize>::max_value() as u64 {
                panic!("");
            }else{
                v as usize
            }).map_err(|e| std::io::Error::new(io::ErrorKind::Other, e))
    }
    fn flush(&mut self) -> io::Result<()>{
        Ok(())
    }

}


//
//context: *mut ImageflowContext,
//mode: IoMode,// Call nothing, dereference nothing, if this is 0
//read_fn: Option<IoReadFn>,// Optional for write modes
//write_fn: Option<IoWriteFn>,// Optional for read modes
//position_fn: Option<IoPositionFn>, // Optional for sequential modes
//seek_fn: Option<IoSeekFn>, // Optional for sequential modes
//dispose_fn: Option<DestructorFn>,// Optional
//user_data: *mut c_void,
///// Whoever sets up this structure can populate this value - or set it to -1 - as they
///// wish. useful for resource estimation.
//optional_file_length: i64

impl IoProxy {
    pub fn internal_use_only_create(context: &Context) -> IoProxy {
        IoProxy {
            //This ugly breaking of lifetimes means that
            //NOTHING is preventing use-after-free
            //if someone finds a way to access an owned Job that isn't borrowed from the Context
            //TODO: Consider replacing with Weak<T>
            c: unsafe { &*(context as *const Context) },
            //io_id: io_id,
            classic: ptr::null_mut(),
            path: None,
            c_path: None,
            drop_with_job: false,
            uuid: Uuid::new_v4()
        }
    }
    pub fn wrap_classic(context: &Context, classic_io: *mut ::ffi::ImageflowJobIo) -> Result<RefMut<IoProxy>> {
        if classic_io.is_null() {
            Err(cerror!(context))
        } else {
            let mut proxy = context.create_io_proxy();
            proxy.classic = classic_io;
            Ok(proxy)
        }
    }

    pub fn get_io_ptr(&self) -> *mut ::ffi::ImageflowJobIo {
        self.classic
    }

    fn classic_io(&self) -> Option<&ImageflowJobIo>{
        if self.classic.is_null(){
            None
        }else{
            Some(unsafe{ &*self.classic})
        }

    }

    pub fn read_to_buffer(&self, context: &Context, buffer: &mut [u8]) -> Result<i64> {
        // Return result for missing function instead of panicking.
        let read = self.classic_io().unwrap().read_fn.unwrap()(context.flow_c(), self.classic, buffer.as_mut_ptr(), buffer.len());
        if read < buffer.len() as i64{
            if context.c_error().has_error() {
                Err(cerror!(context))
            }else{
                Ok(read)
            }

        } else {
            Ok(read)
        }

    }
    pub fn write_from_buffer(&self, context: &Context, buffer: &[u8]) -> Result<i64> {
        // Return result for missing function instead of panicking.
        let read = self.classic_io().unwrap().write_fn.unwrap()(context.flow_c(), self.classic, buffer.as_ptr(), buffer.len());
        if read < buffer.len() as i64{
            if context.c_error().has_error() {
                Err(cerror!(context))
            }else{
                Ok(read)
            }

        } else {
            Ok(read)
        }

    }

    pub fn seek(&self, context: &Context, position: i64) -> Result<bool> {
        // Return result for missing function instead of panicking.
        let success = self.classic_io().unwrap().seek_fn.unwrap()(context.flow_c(), self.classic, position);
        Ok(success)
    }


    pub fn read_slice<'a>(context: &'a Context, bytes: &'a [u8]) -> Result<RefMut<'a, IoProxy>> {
        unsafe {
            let p = ::ffi::flow_io_create_from_memory(context.flow_c(),
                                                      ::ffi::IoMode::ReadSeekable,
                                                      bytes.as_ptr(),
                                                      bytes.len(),
                                                      context.flow_c() as *const libc::c_void,
                                                      ptr::null());
            IoProxy::wrap_classic(context, p)
        }
    }

    // This could actually live as long as the context, but this isn't on the context....
    // but if a constraint, we could add context as an input parameter
    pub fn get_output_buffer_bytes(&self) -> Result<&[u8]> {
        unsafe {
            let mut buf_start: *const u8 = ptr::null();
            let mut buf_len: usize = 0;
            let worked = ::ffi::flow_io_get_output_buffer(self.c.flow_c(),
                                                          self.classic,
                                                          &mut buf_start as *mut *const u8,
                                                          &mut buf_len as *mut usize);
            if !worked {
                Err(cerror!(self.c))
            } else if buf_start.is_null() {
                // Not sure how output buffer is null... no writes yet?
                Err(unimpl!())
            } else {
                Ok((std::slice::from_raw_parts(buf_start, buf_len)))
            }
        }
    }

    pub fn create_output_buffer(context: &Context) -> Result<RefMut<IoProxy>> {
        unsafe {
            let p =
                ::ffi::flow_io_create_for_output_buffer(context.flow_c(),
                                                        context.flow_c() as *const libc::c_void);
            IoProxy::wrap_classic(context, p)
        }
    }


    pub fn copy_slice<'a, 'b>(context: &'a Context, bytes: &'b [u8]) -> Result<RefMut<'a,IoProxy>> {
        unsafe {
            let buf: *mut u8 =
                ::ffi::flow_context_calloc(context.flow_c(),
                                           1,
                                           bytes.len(),
                                           ptr::null(),
                                           context.flow_c() as *const libc::c_void,
                                           ptr::null(),
                                           0) as *mut u8;

            if buf.is_null() {
                return Err(err_oom!());
            }
            ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len());

            let io_ptr = ::ffi::flow_io_create_from_memory(context.flow_c(),
                                                           ::ffi::IoMode::ReadSeekable,
                                                           buf,
                                                           bytes.len(),
                                                           context.flow_c() as *const libc::c_void,
                                                           ptr::null());

            IoProxy::wrap_classic(context, io_ptr)
        }
    }

    pub fn file_with_mode<T: AsRef<Path>>(context: &Context, path: T, mode: ::IoMode) -> Result<RefMut<IoProxy>> {
        unsafe {
            // TODO: character sets matter!
            // Winows fopen needs ansii
            let path_buf = path.as_ref().to_path_buf();
            let c_path = CString::new(path_buf.to_str().expect("Paths should be valid UTF-8 wihtout null characters"))
                .map_err(|e| nerror!( ErrorKind::InvalidArgument))?;
            let p = ::ffi::flow_io_create_for_file(context.flow_c(),
                                                   mode,
                                                   c_path.as_ptr(),
                                                   context.flow_c() as *const libc::c_void);
            let mut result = IoProxy::wrap_classic(context, p);
            if let Ok(ref mut proxy) = result{
                proxy.c_path = Some(c_path);
                proxy.path = Some(path_buf);
            }
            result
        }
    }
    pub fn file<T: AsRef<Path>>(context: &Context, path: T, dir: ::IoDirection) -> Result<RefMut<IoProxy>> {
        let mode = match dir {
            s::IoDirection::In => ::ffi::IoMode::ReadSeekable,
            s::IoDirection::Out => ::ffi::IoMode::WriteSequential,
        };
        IoProxy::file_with_mode(context, path, mode)
    }


}

//
//
//pub unsafe extern "C" fn imageflow_io_create_for_file(context: *mut Context,
//                                                      mode: IoMode,
//                                                      filename: *const libc::c_char,
//                                                      cleanup: CleanupWith)
//                                                      -> *mut JobIo {
//    // TODO: validate that 'owner' is capable of being an owner
//
//    ffi::flow_io_create_for_file(uw(context), std::mem::transmute(mode), filename, uw(context) as *const libc::c_void)
//}
//
/////
///// Creates an imageflow_io structure for reading from the provided buffer.
///// You are ALWAYS responsible for freeing the memory provided in accordance with the Lifetime value.
///// If you specify OutlivesFunctionCall, then the buffer will be copied.
/////
/////
//#[no_mangle]
//#[allow(unused_variables)]
//pub unsafe extern "C" fn imageflow_io_create_from_buffer(context: *mut Context,
//                                                         buffer: *const u8,
//                                                         buffer_byte_count: libc::size_t,
//                                                         lifetime: Lifetime,
//                                                         cleanup: CleanupWith)
//                                                         -> *mut JobIo {
//
//    let mut final_buffer = buffer;
//    if lifetime == Lifetime::OutlivesFunctionCall {
//        let buf : *mut u8 = c::ffi::flow_context_calloc(uw(context), 1, buffer_byte_count, ptr::null(), uw(context) as *const libc::c_void, ptr::null(), 0) as *mut u8 ;
//        if buf.is_null() {
//            //TODO: raise OOM
//            return ptr::null_mut();
//        }
//        ptr::copy_nonoverlapping(buffer, buf, buffer_byte_count);
//
//        final_buffer = buf;
//    }
//    ffi::flow_io_create_from_memory(uw(context), std::mem::transmute(IoMode::ReadSeekable), final_buffer, buffer_byte_count, uw(context) as *mut libc::c_void, ptr::null())
//}
//
//
/////
///// Creates an imageflow_io structure for writing to an expanding memory buffer.
/////
///// Reads/seeks, are, in theory, supported, but unless you've written, there will be nothing to read.
/////
///// The I/O structure and buffer will be freed with the context.
/////
/////
///// Returns null if allocation failed; check the context for error details.
//#[no_mangle]
//#[allow(unused_variables)]
//pub unsafe extern "C" fn imageflow_io_create_for_output_buffer(context: *mut Context)
//                                                               -> *mut JobIo {
//    // The current implementation of output buffer only sheds its actual buffer with the context.
//    // No need for the shell to have an earlier lifetime for mem reasons.
//    ffi::flow_io_create_for_output_buffer(uw(context), uw(context) as *mut libc::c_void)
//}
//
//pub unsafe extern "C" fn imageflow_io_get_output_buffer(context: *mut Context,
//                                                        io: *mut JobIo,
//                                                        result_buffer: *mut *const u8,
//                                                        result_buffer_length: *mut libc::size_t)
//                                                        -> bool {
//
//    let mut result_len: usize = 0;
//    let b = ffi::flow_io_get_output_buffer(uw(context), io, result_buffer, &mut result_len);
//    (* result_buffer_length) = result_len;
//    b
//}
//#[no_mangle]
//pub unsafe extern "C" fn imageflow_job_get_io(context: *mut Context,
//                                              job: *mut Job,
//                                              io_id: i32)
//                                              -> *mut JobIo {
//    (&*job).get_io(io_id).unwrap_or(ptr::null_mut())
//}
///// The io_id will correspond with io_id in the graph
/////
///// direction is in or out.
//#[no_mangle]
//pub unsafe extern "C" fn imageflow_job_add_io(context: *mut Context,
//                                              job: *mut Job,
//                                              io: *mut JobIo,
//                                              io_id: i32,
//                                              direction: Direction)
//                                              -> bool {
//    (&mut *job).add_io(io, io_id, std::mem::transmute(direction))
//        .map(|_| true)
//        .unwrap_or_else(|e| { e.write_to_context_ptr(context); false })
//
//}
//s::IoEnum::ByteArray(vec) => {
//let bytes = vec;
//self.c.create_io_from_copy_of_slice(&bytes)
//}
//s::IoEnum::Base64(b64_string) => {
//let bytes = b64_string.as_str().from_base64().unwrap();
//self.c.create_io_from_copy_of_slice(&bytes)
//}
//s::IoEnum::BytesHex(hex_string) => {
//let bytes = hex_string.as_str().from_hex().unwrap();
//self.c.create_io_from_copy_of_slice(&bytes)
//}
//s::IoEnum::Filename(path) => {
//self.c.create_io_from_filename(&path, dir)
//}
//s::IoEnum::Url(url) => {
//let bytes = ::imageflow_helpers::fetching::fetch_bytes(&url).unwrap();
//self.c.create_io_from_copy_of_slice(&bytes)
//}
//

