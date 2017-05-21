use ffi::EdgeKind;
use flow::definitions::{Graph, Node, NodeParams};
use flow::nodes;
use internal_prelude::works_everywhere::*;
use ::ffi;
use ::rustc_serialize::hex::FromHex;
use ::rustc_serialize::base64::FromBase64;
use ::{Context,Job,IoProxy};

pub struct IoTranslator<'a> {
    ctx: *mut ::ffi::ImageflowContext,
    c: &'a Context,
}
impl<'a> IoTranslator<'a> {
    pub fn new(c: &'a Context) -> IoTranslator<'a> {
        IoTranslator { ctx: c.flow_c(), c: c }
    }

    fn create_io_proxy_from_enum(&self,
                                         io_enum: s::IoEnum,
                                         dir: s::IoDirection)
                                         -> Result<RefMut<IoProxy>> {
        match io_enum {
            s::IoEnum::ByteArray(vec) => {
                let bytes = vec;
                self.c.create_io_from_copy_of_slice(&bytes)
            }
            s::IoEnum::Base64(b64_string) => {
                let bytes = b64_string.as_str().from_base64().unwrap();
                self.c.create_io_from_copy_of_slice(&bytes)
            }
            s::IoEnum::BytesHex(hex_string) => {
                let bytes = hex_string.as_str().from_hex().unwrap();
                self.c.create_io_from_copy_of_slice(&bytes)
            }
            s::IoEnum::Filename(path) => {
                self.c.create_io_from_filename(&path, dir)
            }
            s::IoEnum::Url(url) => {
                let bytes = ::imageflow_helpers::fetching::fetch_bytes(&url).unwrap();
                self.c.create_io_from_copy_of_slice(&bytes)
            }
            s::IoEnum::OutputBuffer |
            s::IoEnum::OutputBase64 => {
                self.c.create_io_output_buffer()
            },
            s::IoEnum::Placeholder => {
                panic!("Placeholder was never substituted!")
            }
        }
    }

    pub fn add_to_job(&self, job: &mut Job, io_vec: Vec<s::IoObject>) {
        let mut io_list = Vec::new();
        for io_obj in io_vec {
            //TODO: add format!("Failed to create IO for {:?}", &io_obj)
            let proxy = self.create_io_proxy_from_enum(io_obj.io, io_obj.direction).unwrap();

            io_list.push((proxy, io_obj.io_id, io_obj.direction));
        }

        for io_list in io_list {
            job.add_io(&io_list.0, io_list.1, io_list.2).unwrap();
        }

    }
}
