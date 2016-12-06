use ::{JsonResponse, IoDirection, MethodRouter,  Context};
use ::ffi::ImageflowJsonResponse;
use ::ffi::CodecInstance;
use flow::definitions::Graph;
use ::internal_prelude::works_everywhere::*;
use ::rustc_serialize::base64;
use ::rustc_serialize::base64::ToBase64;
use ::imageflow_types::collections::*;

pub struct Job{
    c: &'static Context,
    pub debug_job_id: i32,
    pub next_stable_node_id: i32,
    pub next_graph_version: i32,
    pub max_calc_flatten_execute_passes: i32,
    pub graph_recording: s::Build001GraphRecording,
    pub codecs: AppendOnlySet<CodecInstance>,

}
impl Job{
    pub fn internal_use_only_create(context: &Context) -> Job {
        Job {
            //This ugly breaking of lifetimes means that
            //NOTHING is preventing use-after-free
            //if someone finds a way to access an owned Job that isn't borrowed from the Context
            c: unsafe{ &*(context as *const Context) },
            debug_job_id: 0,
            next_graph_version: 0,
            next_stable_node_id: 0,
            max_calc_flatten_execute_passes: 40,
            graph_recording: s::Build001GraphRecording::off(),
            codecs: AppendOnlySet::with_capacity(4)
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

    pub fn message(&mut self, method: &str, json: &[u8]) -> Result<JsonResponse> {
        ::job_methods::JOB_ROUTER.invoke(self, method, json)
    }

    pub fn codec_instance_by_io_id(&self,io_id: i32) -> Option<&CodecInstance>{
        self.codecs.iter().find(|r| r.io_id == io_id)
    }

    pub unsafe fn add_io(&self, io: *mut ::ffi::ImageflowJobIo, io_id: i32, direction: IoDirection) -> Result<()>{


        if direction == IoDirection::Out{
            let inst = self.codecs.add(CodecInstance{
                codec_id: 0,
                codec_state: ptr::null_mut(),
                direction: direction,
                io_id: io_id,
                io: io
            });
            Ok(())
        }else {
            let codec_id = ::ffi::flow_codec_select_from_seekable_io(self.flow_c(), io);
            if codec_id == 0 {
                Err(self.c.error().get_error_copy().unwrap())
            } else {
                let inst = self.codecs.add(CodecInstance {
                    codec_id: codec_id,
                    codec_state: ptr::null_mut(),
                    direction: direction,
                    io_id: io_id,
                    io: io
                });


                let force_mutable_pointer: *mut CodecInstance = mem::transmute(&*inst as *const CodecInstance);
                if !::ffi::flow_codec_initialize(self.flow_c(), force_mutable_pointer) {
                    return Err(self.c.error().get_error_copy().unwrap());
                }

                Ok(())
            }
        }
    }

    pub fn get_io(&self, io_id: i32) -> Result<*mut ::ffi::ImageflowJobIo>{
        //TODO
        //We're treating failed borrows the same as everything else right now... :(
        self.codecs.iter().find(|c| c.io_id == io_id).map(|res| res.io).ok_or(FlowError::NullArgument)
    }

    fn c_error(&self) -> Option<FlowError>{
        self.c.error().get_error_copy()
    }
    fn flow_c(&self) -> *mut ::ffi::ImageflowContext{
        self.c.flow_c()
    }
    // This could actually live as long as the context, but this isn't on the context....
    // but if a constraint, we could add context as an input parameter
    pub fn io_get_output_buffer_slice(&self, io_id: i32) -> Result<&[u8]> {
        unsafe {
            let io_p = self.get_io(io_id)?;

            let mut buf_start: *const u8 = ptr::null();
            let mut buf_len: usize = 0;
            let worked = ::ffi::flow_io_get_output_buffer(self.flow_c(),
                                                          io_p,
                                                          &mut buf_start as *mut *const u8,
                                                          &mut buf_len as *mut usize);
            if !worked {
                Err(self.c_error().unwrap())
            } else if buf_start.is_null() {
                // Not sure how output buffer is null... no writes yet?
                Err(FlowError::ErrNotImpl)
            } else {
                Ok((std::slice::from_raw_parts(buf_start, buf_len)))
            }
        }
    }

    pub fn io_get_output_buffer_copy(&self, io_id: i32) -> Result<Vec<u8>> {
        self.io_get_output_buffer_slice(io_id).map(|s| s.to_vec())
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


    pub fn add_input_bytes<'b>(&'b self, io_id: i32, bytes: &'b [u8]) -> Result<()> {
        unsafe {
            self.add_io(self.c.create_io_from_slice(bytes)?, io_id, IoDirection::In)
        }
    }

    pub fn add_output_buffer(&self, io_id: i32) -> Result<()> {
        unsafe {
            self.add_io(self.c.create_io_output_buffer()?, io_id, IoDirection::Out)
        }
    }




    pub fn get_image_info(&mut self, io_id: i32) -> Result<s::ImageInfo> {

        let instance = self.codec_instance_by_io_id(io_id).ok_or(FlowError::NullArgument)?;

        if instance.direction != IoDirection::In{
            return Err(FlowError::NullArgument)
        }
        unsafe {
            let mut info: ::ffi::DecoderInfo = ::ffi::DecoderInfo { ..Default::default() };

            if !::ffi::flow_codec_decoder_get_info(self.flow_c(), instance.codec_state, instance.codec_id, &mut info ){
                Err(self.c_error().unwrap())
            }else {
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

    }

    pub fn tell_decoder(&mut self, io_id: i32, tell: s::DecoderCommand) -> Result<()> {
        let instance = self.codec_instance_by_io_id(io_id).ok_or(FlowError::NullArgument)?;

        if instance.direction != IoDirection::In {
            return Err(FlowError::NullArgument)
        }

        match tell {
            s::DecoderCommand::JpegDownscaleHints(hints) => {
                let h = ::ffi::DecoderDownscaleHints {
                    downscale_if_wider_than: hints.width,
                    downscaled_min_width: hints.width,
                    or_if_taller_than: hints.height,
                    downscaled_min_height: hints.height,
                    scale_luma_spatially: hints.scale_luma_spatially.unwrap_or(false),
                    gamma_correct_for_srgb_during_spatial_luma_scaling: hints.gamma_correct_for_srgb_during_spatial_luma_scaling.unwrap_or(false)
                };
                unsafe {
                    let force_mutable_pointer: *mut CodecInstance = mem::transmute(&*instance as *const CodecInstance);

                    if !::ffi::flow_codec_decoder_set_downscale_hints(self.flow_c(), force_mutable_pointer, &h, false) {
                        Err(self.c_error().unwrap())
                    } else {
                        Ok(())
                    }
                }
            }
        }
    }

}