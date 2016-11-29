use internal_prelude::works_everywhere::*;
use flow::definitions::{Graph,Node, NodeParams};
use flow::nodes;
use ffi::EdgeKind;
extern crate curl;
use self::curl::easy::Easy;
use ::rustc_serialize::hex::FromHex;
use ::ffi;

pub struct IoTranslator {
    ctx: *mut ::ffi::Context,
}
impl IoTranslator {

    pub fn new(context: *mut ::ffi::Context) -> IoTranslator{
        IoTranslator{ ctx: context}
    }

    unsafe fn create_jobio_ptr_from_enum(&self, io_enum: s::IoEnum, dir: s::IoDirection) -> *mut ::ffi::JobIO {
        let p = self.ctx;
        let result_ptr = match io_enum {
            s::IoEnum::ByteArray(vec) => {
                let bytes = vec;


                let buf : *mut u8 = ::ffi::flow_context_calloc(p, 1, bytes.len(), ptr::null(), p as *const libc::c_void, ptr::null(), 0) as *mut u8 ;
                if buf.is_null() {
                    panic!("OOM");
                }
                ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len());

                let io_ptr =
                ::ffi::flow_io_create_from_memory(p,
                                                  ::ffi::IoMode::read_seekable,
                                                  buf,
                                                  bytes.len(),
                                                  p as *const libc::c_void,
                                                  ptr::null());

                if io_ptr.is_null() {
                    panic!("Failed to create I/O");
                }
                io_ptr
            }
            s::IoEnum::BytesHex(hex_string) => {
                let bytes = hex_string.as_str().from_hex().unwrap();


                let buf : *mut u8 = ::ffi::flow_context_calloc(p, 1, bytes.len(), ptr::null(), p as *const libc::c_void, ptr::null(), 0) as *mut u8 ;
                if buf.is_null() {
                    panic!("OOM");
                }
                ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len());

                let io_ptr =
                ::ffi::flow_io_create_from_memory(p,
                                                  ::ffi::IoMode::read_seekable,
                                                  buf,
                                                  bytes.len(),
                                                  p as *const libc::c_void,
                                                  ptr::null());

                if io_ptr.is_null() {
                    panic!("Failed to create I/O");
                }
                io_ptr
            }
            s::IoEnum::Filename(path) => {

                let path_str: String = path;
                //TODO: character sets matter!
                let mode = match dir{
                    s::IoDirection::Input => ::ffi::IoMode::read_seekable,
                    s::IoDirection::Output => ::ffi::IoMode::write_sequential,
                };

                let mut vec = Vec::new();
                vec.extend_from_slice(path_str.as_bytes());
                vec.push(0);

                let c_path = std::ffi::CStr::from_bytes_with_nul(vec.as_slice()).unwrap();


                let io_ptr = ::ffi::flow_io_create_for_file(p,
                                               mode,
                                               c_path.as_ptr(), p as *const libc::c_void);
                if io_ptr.is_null() {
                    println!("Failed to open file {} with mode {:?}", &path_str, mode);
                    ::ContextPtr::from_ptr(p).assert_ok(None);
                }
                io_ptr
            }
            s::IoEnum::Url(url) => {
                let mut dst = Vec::new();
                {
                    let mut easy = Easy::new();
                    easy.url(&url).unwrap();

                    let mut transfer = easy.transfer();
                    transfer.write_function(|data| {
                        dst.extend_from_slice(data);
                        Ok(data.len())
                    })
                        .unwrap();
                    transfer.perform().unwrap();
                }

                let bytes = dst;


                let buf : *mut u8 = ::ffi::flow_context_calloc(p, 1, bytes.len(), ptr::null(), p as *const libc::c_void, ptr::null(), 0) as *mut u8 ;
                if buf.is_null() {
                    panic!("OOM");
                }
                ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len());

                let io_ptr =
                ::ffi::flow_io_create_from_memory(p,
                                                  ::ffi::IoMode::read_seekable,
                                                  buf,
                                                  bytes.len(),
                                                  p as *const libc::c_void,
                                                  ptr::null());

                if io_ptr.is_null() {
                    panic!("Failed to create I/O");
                }
                io_ptr
            }
            s::IoEnum::OutputBuffer | s::IoEnum::OutputBase64 => {
                let io_ptr =
                ::ffi::flow_io_create_for_output_buffer(p,
                                                        self.ctx as *const libc::c_void);
                if io_ptr.is_null() {
                    panic!("Failed to create I/O");
                }
                io_ptr
            }
        } as *mut ffi::JobIO;

        result_ptr
    }

    pub unsafe fn add_to_job(&self, job: *mut ::ffi::Job, io_vec: Vec<s::IoObject>){
        let mut io_list = Vec::new();
        for io_obj in io_vec {
            let io_ptr = self.create_jobio_ptr_from_enum(io_obj.io, io_obj.direction);


            let new_direction = match io_obj.direction {
                s::IoDirection::Input => ffi::IoDirection::In,
                s::IoDirection::Output => ffi::IoDirection::Out,
            };

            io_list.push((io_ptr, io_obj.io_id, new_direction));
        }

        for io_list in io_list {
            if !::ffi::flow_job_add_io(self.ctx, job, io_list.0, io_list.1, io_list.2) {
                panic!("flow_job_add_io failed");
            }
        }

    }
}