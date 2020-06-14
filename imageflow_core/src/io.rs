use std;
use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::{ffi, FlowError};
use crate::{Context, Result, JsonResponse, ErrorKind};
use imageflow_types::collections::AddRemoveSet;
use uuid::Uuid;
use std::rc::Rc;
use crate::internal_prelude::external_without_std::io::{Cursor, BufReader};
use crate::internal_prelude::external::std::io::SeekFrom;
use imageflow_types::IoDirection;

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

enum IoBackend{
    ReadSlice(Cursor<&'static [u8]>),
    ReadVec(Cursor<Vec<u8>>),
    WriteVec(Cursor<Vec<u8>>),
    ReadFile(BufReader<File>),
    WriteFile(BufWriter<File>)
}
impl IoBackend{
    pub fn get_write(&mut self) -> Option<&mut dyn Write>{
        match self{
            IoBackend::WriteVec(w) => Some(w),
            IoBackend::WriteFile(w) => Some(w),
            _ => None
        }
    }
    pub fn get_read(&mut self) -> Option<&mut dyn Read>{
        match self{
            IoBackend::ReadSlice(w) => Some(w),
            IoBackend::ReadVec(w) => Some(w),
            IoBackend::ReadFile(w) => Some(w),
            _ => None
        }
    }
    pub fn get_seek(&mut self) -> Option<&mut dyn Seek>{
        match self{
            IoBackend::ReadSlice(w) => Some(w),
            IoBackend::ReadVec(w) => Some(w),
            IoBackend::ReadFile(w) => Some(w),
            _ => None
        }
    }
}

/// A safer proxy over the C IO object.
/// Implements Read/Write
pub struct IoProxy{
    io_id: i32,
    path: Option<PathBuf>,
    backend: IoBackend,
}



impl io::Read for IoProxy{

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>{
        self.backend.get_read().expect("cannot read from writer").read(buf)
    }
}
impl io::Seek for IoProxy{
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.backend.get_seek().expect("cannot read from writer").seek(pos)
    }
}
impl io::Write for IoProxy{

    fn write(&mut self, buf: &[u8]) -> io::Result<usize>{
        self.backend.get_write().expect("cannot write from reader").write(buf)
    }
    fn flush(&mut self) -> io::Result<()>{
        self.backend.get_write().expect("cannot write from reader").flush()
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


impl IoProxy {
    pub fn io_id(&self) -> i32{
        self.io_id
    }

    pub fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()>{
        self.backend.get_read().expect("cannot read from writer").read_exact(buf)
    }

    pub fn read_file(context: &Context, filename: PathBuf, io_id: i32) -> Result<IoProxy> {
        IoProxy::file_with_mode(context,  io_id, filename,IoDirection::In)
    }
    pub fn write_file(context: &Context, filename: PathBuf, io_id: i32) -> Result<IoProxy> {
        IoProxy::file_with_mode(context,  io_id, filename,IoDirection::Out)
    }

    fn check_io_id(context: &Context, io_id: i32) -> Result<()>{
        if context.io_id_present(io_id){
            Err(nerror!(ErrorKind::DuplicateIoId, "io_id {} is already in use on this context", io_id))
        }else{
            Ok(())
        }
    }

    /// Only valid as long as the life of the IoProxy does not exceed the life of the Context, 'a
    pub unsafe fn read_slice<'a>(context: &'a Context, io_id: i32,  bytes: &'a [u8]) -> Result<IoProxy> {
        IoProxy::check_io_id(context,io_id)?;

        Ok(IoProxy {
            path: None,
            io_id,
            backend: IoBackend::ReadSlice(Cursor::new(std::mem::transmute(bytes)))
        })
    }

    // This could actually live as long as the context, but this isn't on the context....
    // but if a constraint, we could add context as an input parameter
    /// Only valid as long as the life of the IoProxy is the same as the life of the Context, 'b
    pub fn get_output_buffer_bytes<'b>(&self, c: &'b Context) -> Result<&'b[u8]> {
        match &self.backend{
            &IoBackend::WriteVec(ref v) => Ok(unsafe{ std::mem::transmute(v.get_ref().as_slice())}),
            _ => Err(nerror!(ErrorKind::InvalidOperation, "get_output_buffer_bytes only works on output buffers"))
        }
    }

    pub fn create_output_buffer(context: &Context, io_id: i32) -> Result<IoProxy> {
        IoProxy::check_io_id(context,io_id)?;
        Ok(IoProxy {
            path: None,
            io_id,
            backend: IoBackend::WriteVec(Cursor::new(Vec::new()))
        })
    }


    pub fn read_vec(context: &Context, io_id: i32, bytes: Vec<u8>) -> Result<IoProxy> {
        IoProxy::check_io_id(context,io_id)?;

        Ok(IoProxy {
            path: None,
            io_id,
            backend: IoBackend::ReadVec(Cursor::new(bytes))
        })
    }

    pub fn copy_slice(context: &Context, io_id: i32, bytes: & [u8]) -> Result<IoProxy> {
        IoProxy::check_io_id(context,io_id)?;

        Ok(IoProxy {
            path: None,
            io_id,
            backend: IoBackend::ReadVec(Cursor::new(Vec::from(bytes)))
        })
    }

    pub fn file_with_mode<T: AsRef<Path>>(context: &Context, io_id: i32, path: T, direction: IoDirection) -> Result<IoProxy> {
        IoProxy::check_io_id(context,io_id)?;

        let backend = match direction {
            IoDirection::In => {
                let file = File::open(path.as_ref())
                    .map_err(|e| FlowError::from_decoder(e))?;
                IoBackend::ReadFile(BufReader::new(file))
            },
            IoDirection::Out => {
                let file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(path.as_ref())
                    .map_err(|e| FlowError::from_encoder(e))?;
                IoBackend::WriteFile(BufWriter::new(file))
            }
        };
        Ok(IoProxy {
            path: Some(path.as_ref().to_owned()),
            io_id,
            backend
        })
    }




}
