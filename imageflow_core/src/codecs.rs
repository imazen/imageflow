use ::std;
use ::for_other_imageflow_crates::preludes::external_without_std::*;
use ::ffi;
use ::job::Job;
use ::{Context, FlowErr,FlowError, Result, JsonResponse};
use ::ffi::CodecInstance;
use ::ffi::BitmapBgra;
use ::imageflow_types::collections::AddRemoveSet;
use io::IoProxy;
use uuid::Uuid;
use imageflow_types::IoDirection;


// We need a rust-friendly codec instance, codec definition, and a way to wrap C codecs
#[derive(Debug)]
pub struct CodecInstanceContainer{
    pub io_id: i32,
    pub proxy_uuid: Uuid,
    pub direction: IoDirection,
    pub classic: Option<CodecInstance>
}

impl CodecInstanceContainer {

    pub fn create(c: &Context, io: &IoProxy, io_id: i32, direction: IoDirection) -> Result<CodecInstanceContainer>{

        let default = CodecInstanceContainer
            {
                proxy_uuid: io.uuid,
                io_id: io_id,
                direction: direction,
                classic: None
            };

        if direction == IoDirection::Out{
            Ok(CodecInstanceContainer
            {
                classic: Some(CodecInstance{
                    codec_id: 0,
                    codec_state: ptr::null_mut(),
                    direction: direction,
                    io_id: io_id,
                    io: io.get_io_ptr()
                }),
                .. default
            })
        }else {
            let codec_id = unsafe {
                ::ffi::flow_codec_select_from_seekable_io(c.flow_c(), io.get_io_ptr())
            };
            if codec_id == 0 {
                Err(c.error().get_error_copy().unwrap())
            } else {
                let inst = CodecInstance {
                    codec_id: codec_id,
                    codec_state: ptr::null_mut(),
                    direction: direction,
                    io_id: io_id,
                    io: io.get_io_ptr()
                };



                Ok(CodecInstanceContainer
                    {
                        classic: Some(inst),
                        .. default
                    })
            }
        }
    }

    pub fn initialize(&mut self, c: &Context, job: &Job) -> Result<()>{
        unsafe {
            if self.direction == IoDirection::In {
                if !::ffi::flow_codec_initialize(c.flow_c(), self.classic.as_mut().unwrap() as *mut CodecInstance) {
                    return Err(c.error().get_error_copy().unwrap());
                }
            }
            Ok(())
        }

    }

    pub fn get_image_info(&mut self, c: &Context, job: &Job) -> Result<s::ImageInfo> {
        if self.direction != IoDirection::In{
            return Err(FlowError::NullArgument)
        }
        unsafe {
            let classic = self.classic.as_mut().unwrap();

            let mut info: ::ffi::DecoderInfo = ::ffi::DecoderInfo { ..Default::default() };

            if !::ffi::flow_codec_decoder_get_info(c.flow_c(), classic.codec_state, classic.codec_id, &mut info ){
                Err(c.c_error().unwrap())
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

    pub fn get_exif_rotation_flag(&mut self, c: &Context, job: &Job) -> Result<i32> {

        let exif_flag = unsafe {
            ffi::flow_codecs_jpg_decoder_get_exif(c.flow_c(),
                                                  self.classic.as_mut().unwrap() as*mut ffi::CodecInstance) };
        Ok(exif_flag)
    }
    pub fn read_frame(&mut self, c: &Context, job: &Job) -> Result<*mut BitmapBgra> {
        let result = unsafe {
            ffi::flow_codec_execute_read_frame(c.flow_c(),
                                                  self.classic.as_mut().unwrap() as *mut ffi::CodecInstance) };
        if result.is_null() {
            Err(c.error().get_error_copy().unwrap())
        }else {
            Ok(result)
        }
    }

    pub fn write_frame(&mut self, c: &Context, job: &Job, preset: &s::EncoderPreset, frame: &mut BitmapBgra) -> Result<s::EncodeResult>{

        let (wanted_id, hints) = match *preset {
            s::EncoderPreset::LibjpegTurbo { quality } => {
                (ffi::CodecType::EncodeJpeg as i64,
                 ffi::EncoderHints {
                     jpeg_encode_quality: quality.unwrap_or(90),
                     disable_png_alpha: false,
                 })
            }
            s::EncoderPreset::Libpng { ref matte,
                zlib_compression,
                ref depth } => {
                (ffi::CodecType::EncodePng as i64,
                 ffi::EncoderHints {
                     jpeg_encode_quality: -1,
                     disable_png_alpha: match *depth {
                         Some(s::PngBitDepth::Png24) => true,
                         _ => false,
                     },
                 })
            }
        };

        unsafe {
            let classic = self.classic.as_mut().unwrap();

            let (result_mime, result_ext) = match *preset {
                s::EncoderPreset::Libpng { .. } => ("image/png", "png"),
                s::EncoderPreset::LibjpegTurbo { .. } => ("image/jpeg", "jpg"),
            };

            classic.codec_id = wanted_id;
            if !ffi::flow_codec_initialize(c.flow_c(), classic as *mut ffi::CodecInstance) {
                c.error().assert_ok();
            }
            let codec_def = ffi::flow_codec_get_definition(c.flow_c(), wanted_id);
            if codec_def.is_null() {
                c.error().assert_ok();
            }
            let write_fn = (*codec_def).write_frame;
            if write_fn == None {
                panic!("Codec didn't implement write_frame");
            }

            if !write_fn.unwrap()(c.flow_c(),
                                  classic.codec_state,
                                  frame as *mut BitmapBgra,
                                  &hints as *const ffi::EncoderHints) {
                c.error().assert_ok();
            }

            Ok(s::EncodeResult {
                w: (*frame).w as i32,
                h: (*frame).h as i32,
                preferred_mime_type: result_mime.to_owned(),
                preferred_extension: result_ext.to_owned(),
                io_id: self.io_id,
                bytes: s::ResultBytes::Elsewhere,
            })
        }
    }
    pub fn tell_decoder(&mut self, c: &Context, job: &Job, tell: s::DecoderCommand) -> Result<()> {
        if self.direction != IoDirection::In{
            return Err(FlowError::NullArgument)
        }

        let classic = self.classic.as_mut().unwrap();

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

                    if !::ffi::flow_codec_decoder_set_downscale_hints(c.flow_c(), classic as *mut CodecInstance, &h, false) {
                        Err(c.c_error().unwrap())
                    } else {
                        Ok(())
                    }
                }
            }
        }
    }
}


//pub struct CodecInstance {
//    pub io_id: i32,
//    pub codec_id: i64,
//    pub codec_state: *mut c_void,
//    pub io: *mut ImageflowJobIo,
//    pub direction: IoDirection,
//}
//
