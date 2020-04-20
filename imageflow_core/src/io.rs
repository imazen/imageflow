use std;
use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::ffi;
use crate::{Context, CError, Result, JsonResponse, ErrorKind};
use crate::ffi::ImageflowJobIo;
use imageflow_types::collections::AddRemoveSet;
use uuid::Uuid;
use std::rc::Rc;

/// Codecs own their IoProxy, but sometimes Imageflow needs access (like when it needs to read a buffer).
/// This enum can be extended as needed.
pub enum IoProxyRef<'a>{
   Borrow(&'a IoProxy),
    BoxedAsRef(Box<dyn AsRef<IoProxy>>),
    Ref(Ref<'a, IoProxy>)
}
impl<'a> IoProxyRef<'a> {
    pub fn map<B, F>(self, mut f: F) -> B
        where
            F: FnMut(&IoProxy) -> B{

        match self {
            IoProxyRef::Borrow(r) => f(r),
            IoProxyRef::BoxedAsRef(r) => f((*r).as_ref()),
            IoProxyRef::Ref(r) => f(&*r)
        }

    }
}

/// A safer proxy over the C IO object.
/// Implements Read/Write
pub struct IoProxy{
    c: &'static Context,
    classic: *mut ImageflowJobIo,
    io_id: i32,
    path: Option<PathBuf>,
    c_path: Option<CString>,
    drop_with_job: bool
}



impl io::Read for IoProxy{

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>{
        self.read_to_buffer(self.c, buf).and_then(|read_bytes|
            if read_bytes.leading_zeros() < 1 {
                Err(nerror!(ErrorKind::InvalidArgument, "read_bytes likely came from a negative integer. Imageflow prohibits having the leading bit set on unsigned integers (this reduces the maximum value to 2^31 or 2^63)."))

            }else{
                Ok(read_bytes as usize)
            }).map_err(|e| std::io::Error::new(io::ErrorKind::Other, e))
    }
}
impl io::Write for IoProxy{

    fn write(&mut self, buf: &[u8]) -> io::Result<usize>{
        self.write_from_buffer(self.c, buf).and_then(|written_bytes|
            if written_bytes.leading_zeros() < 1 {
                Err(nerror!(ErrorKind::InvalidArgument, "written_bytes likely came from a negative integer. Imageflow prohibits having the leading bit set on unsigned integers (this reduces the maximum value to 2^31 or 2^63)."))
            }else{
                Ok(written_bytes as usize)
            }).map_err(|e| std::io::Error::new(io::ErrorKind::Other, e))
    }
    fn flush(&mut self) -> io::Result<()>{
        Ok(())
    }

}


/// Allows access to Write trait through an Rc<RefCell<>>
pub struct IoProxyProxy(pub Rc<RefCell<IoProxy>>);
impl Write for IoProxyProxy{
    fn write(&mut self, buf: &[u8]) -> ::std::io::Result<usize> {
        self.0.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> ::std::io::Result<()> {
        self.0.borrow_mut().flush()
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
    pub fn io_id(&self) -> i32{
        self.io_id
    }

    pub fn create(context: &Context, io_id: i32) -> IoProxy {
        IoProxy {
            //This ugly breaking of lifetimes means that
            //NOTHING is preventing use-after-free
            //if someone finds a way to access an owned Codec that isn't borrowed from the Context
            //TODO: Consider replacing with Weak<T>
            c: unsafe { &*(context as *const Context) },
            //io_id: io_id,
            classic: ptr::null_mut(),
            path: None,
            c_path: None,
            drop_with_job: false,
            io_id
        }
    }
    pub fn wrap_classic(context: &Context, classic_io: *mut crate::ffi::ImageflowJobIo, io_id: i32) -> Result<IoProxy> {
        if classic_io.is_null() {
            Err(cerror!(context, "Failed to create ImageflowJobIo *"))
        } else {
            let mut proxy = IoProxy::create(context, io_id);
            proxy.classic = classic_io;
            Ok(proxy)
        }
    }

    pub fn get_io_ptr(&self) -> *mut crate::ffi::ImageflowJobIo {
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
        if let Some(classic) = self.classic_io() {
            if let Some(read_fn) = classic.read_fn {
                let read = read_fn(context.flow_c(), self.classic, buffer.as_mut_ptr(), buffer.len());
                if read < buffer.len() as i64 {
                    if context.c_error().has_error() {
                        Err(cerror!(context, "Read failed"))
                    } else {
                        Ok(read)
                    }
                } else {
                    Ok(read)
                }
            }else {
                Err(unimpl!())
            }
        }else{
            Err(unimpl!())
        }

    }
    pub fn write_from_buffer(&self, context: &Context, buffer: &[u8]) -> Result<i64> {
        if let Some(classic) = self.classic_io() {
            if let Some(write_fn) = classic.write_fn {
                let write =write_fn(context.flow_c(), self.classic, buffer.as_ptr(), buffer.len());
                if write < buffer.len() as i64 {
                    if context.c_error().has_error() {
                        Err(cerror!(context, "Write failed"))
                    } else {
                        Ok(write)
                    }
                } else {
                    Ok(write)
                }
            } else {
                Err(unimpl!())
            }
        } else {
            Err(unimpl!())
        }
    }

    pub fn seek(&self, context: &Context, position: i64) -> Result<bool> {
        if let Some(classic) = self.classic_io() {
            if let Some(seek_fn) = classic.seek_fn {
                Ok(seek_fn(context.flow_c(), self.classic, position))
            } else {
                Err(unimpl!())
            }
        } else {
            Err(unimpl!())
        }
    }

    fn check_io_id(context: &Context, io_id: i32) -> Result<()>{
        if context.io_id_present(io_id){
            Err(nerror!(ErrorKind::DuplicateIoId, "io_id {} is already in use on this context", io_id))
        }else{
            Ok(())
        }
    }

    pub fn read_slice<'a>(context: &'a Context, io_id: i32,  bytes: &'a [u8]) -> Result<IoProxy> {
        IoProxy::check_io_id(context,io_id)?;
        unsafe {
            // Owner parameter is only for io_struct, not buffer.
            let p = crate::ffi::flow_io_create_from_memory(context.flow_c(),
                                                      crate::ffi::IoMode::ReadSeekable,
                                                      bytes.as_ptr(),
                                                      bytes.len(),
                                                      context.flow_c() as *const libc::c_void,
                                                      ptr::null());
            IoProxy::wrap_classic(context, p, io_id).map_err(|e| e.at(here!()))
        }
    }

    // This could actually live as long as the context, but this isn't on the context....
    // but if a constraint, we could add context as an input parameter
    pub fn get_output_buffer_bytes<'b>(&self, c: &'b Context) -> Result<&'b[u8]> {
        unsafe {
            let mut buf_start: *const u8 = ptr::null();
            let mut buf_len: usize = 0;
            let flow_c = self.c.flow_c();
            let classic = self.classic;

            let worked = crate::ffi::flow_io_get_output_buffer(flow_c    ,
                                                          classic,
                                                          &mut buf_start as *mut *const u8,
                                                          &mut buf_len as *mut usize);
            if !worked {
                Err(cerror!(self.c))
            } else if buf_start.is_null() {
                // Not sure how output buffer is null... no writes yet?
                Err(unimpl!())
            } else {
                Ok(std::slice::from_raw_parts(buf_start, buf_len))
            }
        }
    }

    pub fn create_output_buffer(context: &Context, io_id: i32) -> Result<IoProxy> {
        IoProxy::check_io_id(context,io_id)?;
        unsafe {
            let p =
                crate::ffi::flow_io_create_for_output_buffer(context.flow_c(),
                                                        context.flow_c() as *const libc::c_void);
            IoProxy::wrap_classic(context, p, io_id).map_err(|e| e.at(here!()))
        }
    }


    pub fn copy_slice(context: &Context, io_id: i32, bytes: & [u8]) -> Result<IoProxy> {
        IoProxy::check_io_id(context,io_id)?;
        unsafe {
            let buf: *mut u8 =
                crate::ffi::flow_context_calloc(context.flow_c(),
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

            let io_ptr = crate::ffi::flow_io_create_from_memory(context.flow_c(),
                                                           crate::ffi::IoMode::ReadSeekable,
                                                           buf,
                                                           bytes.len(),
                                                           context.flow_c() as *const libc::c_void,
                                                           ptr::null());
            if io_ptr.is_null(){
                let _ = crate::ffi::flow_destroy(context.flow_c(), buf as *mut libc::c_void, ptr::null(), 0);
            }
            IoProxy::wrap_classic(context, io_ptr, io_id).map_err(|e| e.at(here!()))
        }
    }

    pub fn file_with_mode<T: AsRef<Path>>(context: &Context, io_id: i32, path: T, mode: crate::IoMode) -> Result<IoProxy> {
        IoProxy::check_io_id(context,io_id)?;
        unsafe {
            // TODO: add support for a wider variety of character sets
            // Windows fopen needs ansii
            let path_buf = path.as_ref().to_path_buf();

            let c_path = {
                let path_str = path_buf.to_str().ok_or_else(||nerror!(ErrorKind::InvalidArgument, "The argument 'path' is invalid UTF-8."))?;
                CString::new(path_str).map_err(|e| nerror!(ErrorKind::InvalidArgument, "The argument 'path' contains a null byte.") )?
            };

            let p = crate::ffi::flow_io_create_for_file(context.flow_c(),
                                                   mode,
                                                   c_path.as_ptr(),
                                                   context.flow_c() as *const libc::c_void);

            let mut proxy = IoProxy::wrap_classic(context, p, io_id).map_err(|e| e.at(here!()))?;
            proxy.c_path = Some(c_path);
            proxy.path = Some(path_buf);
            Ok(proxy)
        }
    }



}
