use ::{JsonResponse, IoDirection, MethodRouter,  Context};
use ::ffi::CodecInstance;
use flow::definitions::Graph;
use ::internal_prelude::works_everywhere::*;
use ::rustc_serialize::base64;
use ::rustc_serialize::base64::ToBase64;
use ::imageflow_types::collections::*;
use io::IoProxy;
use codecs::CodecInstanceContainer;
use ::uuid::Uuid;

pub struct Job{
    c: &'static Context,
    pub debug_job_id: i32,
    pub next_stable_node_id: i32,
    pub next_graph_version: i32,
    pub max_calc_flatten_execute_passes: i32,
    pub graph_recording: s::Build001GraphRecording,
    pub codecs: AddRemoveSet<CodecInstanceContainer>,
    pub io_to_proxy_uuids: Vec<(i32,Uuid)>

}
static mut JOB_ID: i32 = 0;
impl Job{
    pub fn internal_use_only_create(context: &Context) -> Job {
        unsafe { JOB_ID+=1;}
        Job {
            //This ugly breaking of lifetimes means that
            //NOTHING is preventing use-after-free
            //if someone finds a way to access an owned Job that isn't borrowed from the Context
            c: unsafe{ &*(context as *const Context) },
            debug_job_id: unsafe{ JOB_ID },
            next_graph_version: 0,
            next_stable_node_id: 0,
            max_calc_flatten_execute_passes: 40,
            graph_recording: s::Build001GraphRecording::off(),
            codecs: AddRemoveSet::with_capacity(4),
            io_to_proxy_uuids: Vec::with_capacity(4)
        }
    }
    pub fn context(&self) -> &Context{
        self.c
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
            //Cheat on lifetimes so Job can remain mutable
            let split_context = unsafe{ &*(self.context() as *const Context)};
            ::flow::execution_engine::Engine::create(split_context, self, &mut g).execute().map_err(|e| e.at(here!()))?;

            Ok(s::ResponsePayload::JobResult(s::JobResult { encodes: Job::collect_encode_results(&g) }))
    }

    pub fn message(&mut self, method: &str, json: &[u8]) -> (JsonResponse, Result<()>) {
        ::job_methods::JOB_ROUTER.invoke(self, method, json)
    }

    pub fn get_codec(&self, io_id: i32) -> Result<RefMut<CodecInstanceContainer>>{
        //TODO
        //We're treating failed borrows the same as everything else right now... :(
        Ok(self.codecs.iter_mut().filter(|r| r.is_ok()).map(|r| r.unwrap()).find(|c| c.io_id == io_id).ok_or(unimpl!()).unwrap())
    }

    pub fn get_io(&self, io_id: i32) -> Result<RefMut<IoProxy>>{
        let uuid = self.io_to_proxy_uuids.iter().find(|&&(id, uuid)| id == io_id).map(|&(_,uuid)| uuid).unwrap();
        self.c.get_proxy_mut(uuid)
    }

    pub fn add_io(&mut self, io: &mut IoProxy, io_id: i32, direction: IoDirection) -> Result<()>{
        self.io_to_proxy_uuids.push((io_id, io.uuid));
        let mut codec = self.codecs.add_mut(CodecInstanceContainer::create(self.c, io, io_id, direction)?);
        if let Ok(d) = codec.get_decoder(){
            d.initialize(self.c, self)?;
        }
        Ok(())
    }


    fn flow_c(&self) -> *mut ::ffi::ImageflowContext{
        self.c.flow_c()
    }
    // This could actually live as long as the context, but this isn't on the context....
    // but if a constraint, we could add context as an input parameter
//    pub fn io_get_output_buffer_slice(&self, io_id: i32) -> Result<&[u8]> {
//        self.get_io(io_id)?.get_output_buffer_bytes()
//    }

    pub fn io_get_output_buffer_copy(&self, io_id: i32) -> Result<Vec<u8>> {
        self.get_io(io_id)?.get_output_buffer_bytes().map(|s| s.to_vec())
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
        Job::collect_encode_results(g).into_iter().map(|r: s::EncodeResult|{
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


    pub fn add_input_bytes<'b>(&'b mut self, io_id: i32, bytes: &'b [u8]) -> Result<()> {
        self.add_io(&mut *self.c.create_io_from_slice(bytes)?, io_id, IoDirection::In)
    }

    pub fn add_output_buffer(&mut self, io_id: i32) -> Result<()> {
       self.add_io(&mut *self.c.create_io_output_buffer()?, io_id, IoDirection::Out)
    }


    pub fn get_image_info(&mut self, io_id: i32) -> Result<s::ImageInfo> {
        self.get_codec(io_id)?.get_decoder()?.get_image_info(self.c, self, &mut *self.get_io(io_id)?)
    }

    pub fn tell_decoder(&mut self, io_id: i32, tell: s::DecoderCommand) -> Result<()> {
        self.get_codec(io_id)?.get_decoder()?.tell_decoder(self.c, self, tell)
    }

    pub fn get_exif_rotation_flag(&mut self, io_id: i32) -> Result<i32>{
        self.get_codec(io_id)?.get_decoder()?.get_exif_rotation_flag(self.c, self)

    }

}
