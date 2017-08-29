use ::std;
use ::for_other_imageflow_crates::preludes::external_without_std::*;
use ::ffi;
use ::job::Job;
use ::{Context, CError, Result, JsonResponse, ErrorKind, FlowError};
use ::ffi::CodecInstance;
use ::ffi::BitmapBgra;
use ::imageflow_types::collections::AddRemoveSet;
use io::IoProxy;
use uuid::Uuid;
use imageflow_types::IoDirection;
use std::borrow::BorrowMut;
use std::ops::DerefMut;
mod gif;


pub trait DecoderFactory{
    fn create(c: &Context, io: &mut IoProxy, io_id: i32) -> Option<Result<Box<Decoder>>>;
}
pub trait Decoder{
    fn initialize(&mut self, c: &Context, job: &Job) -> Result<()>;
    fn get_image_info(&mut self, c: &Context, job: &Job, io: &mut IoProxy) -> Result<s::ImageInfo>;
    fn get_exif_rotation_flag(&mut self, c: &Context, job: &Job) -> Result<i32>;
    fn tell_decoder(&mut self, c: &Context, job: &Job, tell: s::DecoderCommand) -> Result<()>;
    fn read_frame(&mut self, c: &Context, job: &Job, io: &mut IoProxy) -> Result<*mut BitmapBgra>;
}
pub trait Encoder{
    // GIF encoder will need to know if transparency is required (we could guess based on first input frame)
    // If not required, we can do frame shrinking and delta encoding. Otherwise we have to
    // encode entire frames and enable transparency (default)
    fn write_frame(&mut self, c: &Context, job: &Job, io: &mut IoProxy, preset: &s::EncoderPreset, frame: &mut BitmapBgra) -> Result<s::EncodeResult>;
}

enum CodecKind{
    EncoderPlaceholder,
    Encoder(Box<Encoder>),
    Decoder(Box<Decoder>)
}
// We need a rust-friendly codec instance, codec definition, and a way to wrap C codecs
pub struct CodecInstanceContainer{
    pub io_id: i32,
    pub proxy_uuid: Uuid,
    codec: CodecKind
}

impl CodecInstanceContainer {

    pub fn get_decoder(&mut self) -> Result<&mut Box<Decoder>>{
        if let CodecKind::Decoder(ref mut d) = self.codec{
            Ok(d)
        }else{
            Err(nerror!(ErrorKind::InvalidArgument))
        }

    }

    pub fn create(c: &Context, io: &mut IoProxy, io_id: i32, direction: IoDirection) -> Result<CodecInstanceContainer>{
        if direction == IoDirection::Out {
            Ok(CodecInstanceContainer
                {
                    proxy_uuid: io.uuid,
                    io_id: io_id,
                    codec: CodecKind::EncoderPlaceholder
                })
        }else {
            let mut buffer =  [0u8; 8];
            let result = io.read_to_buffer(c, &mut buffer);
            if let Ok(count) = result {
                io.seek(c, 0).unwrap();
                if buffer.starts_with(b"GIF89a") || buffer.starts_with(b"GIF87a") {
                    return Ok(CodecInstanceContainer
                        {
                            proxy_uuid: io.uuid,
                            io_id: io_id,
                            codec: CodecKind::Decoder(Box::new(gif::GifDecoder::create(c, io, io_id)?))
                        });
                } else {
                    Ok(CodecInstanceContainer
                        {
                            proxy_uuid: io.uuid,
                            io_id: io_id,
                            codec: CodecKind::Decoder(ClassicDecoder::create(c, io, io_id)?)
                        })
                }
            } else {
                Err(result.unwrap_err()) //TODO: add detail
            }
        }
    }

}

struct ClassicDecoder{
    classic: CodecInstance
}

impl ClassicDecoder {
    fn create(c: &Context, io: &mut IoProxy, io_id: i32) -> Result<Box<impl Decoder>> {
        let codec_id = unsafe {
            ::ffi::flow_codec_select_from_seekable_io(c.flow_c(), io.get_io_ptr())
        };
        if codec_id == 0 {
            Err(cerror!(c))
        } else {
            Ok(Box::new(ClassicDecoder {
                classic: CodecInstance {
                    codec_id: codec_id,
                    codec_state: ptr::null_mut(),
                    direction: IoDirection::In,
                    io_id: io_id,
                    io: io.get_io_ptr()
                }
            }))
        }
    }
}

impl Decoder for ClassicDecoder{
    fn initialize(&mut self, c: &Context, job: &Job) -> Result<()>{
        unsafe {
            if !::ffi::flow_codec_initialize(c.flow_c(), &mut self.classic as *mut CodecInstance) {
                return Err(cerror!(c));
            }

            Ok(())
        }

    }

    fn get_image_info(&mut self, c: &Context, job: &Job, io: &mut IoProxy) -> Result<s::ImageInfo> {
        unsafe {
            let classic = &self.classic;

            let mut info: ::ffi::DecoderInfo = ::ffi::DecoderInfo { ..Default::default() };

            if !::ffi::flow_codec_decoder_get_info(c.flow_c(), classic.codec_state, classic.codec_id, &mut info ){
                Err(cerror!(c))
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

    fn get_exif_rotation_flag(&mut self, c: &Context, job: &Job) -> Result<i32> {

        let exif_flag = unsafe {
            ffi::flow_codecs_jpg_decoder_get_exif(c.flow_c(),
                                                  &mut self.classic as*mut ffi::CodecInstance) };
        Ok(exif_flag)
    }
    fn read_frame(&mut self, c: &Context, job: &Job, io: &mut IoProxy) -> Result<*mut BitmapBgra> {
        let result = unsafe {
            ffi::flow_codec_execute_read_frame(c.flow_c(),
                                               &mut  self.classic as *mut ffi::CodecInstance) };
        if result.is_null() {
            Err(cerror!(c))
        }else {
            Ok(result)
        }
    }


    fn tell_decoder(&mut self, c: &Context, job: &Job, tell: s::DecoderCommand) -> Result<()> {
        let classic = &mut self.classic;

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
                        Err(cerror!(c))
                    } else {
                        Ok(())
                    }
                }
            }
        }
    }
}

struct ClassicEncoder{
    classic: CodecInstance,
    io_id: i32
}

impl ClassicEncoder{
    fn get_codec_id_and_hints(preset: &s::EncoderPreset) -> (i64, ffi::EncoderHints){
        match *preset {
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
            s::EncoderPreset::Gif => {
                panic!("Classic encoder only supports libjpeg and libpng");
            }
        }
    }
    fn get_empty(io_id: i32, io_ptr:  *mut ffi::ImageflowJobIo) -> ClassicEncoder {
        ClassicEncoder {
            io_id: io_id,
            classic: CodecInstance {
                io_id: io_id,
                codec_id: 0,
                codec_state: ptr::null_mut(),
                direction: IoDirection::Out,
                io: io_ptr
            }
        }
    }


}

impl Encoder for ClassicEncoder{

    fn write_frame(&mut self, c: &Context, job: &Job,  io: &mut IoProxy, preset: &s::EncoderPreset, frame: &mut BitmapBgra) -> Result<s::EncodeResult> {
        let (wanted_id, hints) = ClassicEncoder::get_codec_id_and_hints(preset);
        unsafe {
            let classic = &mut self.classic;

            let (result_mime, result_ext) = match *preset {
                s::EncoderPreset::Libpng { .. } => ("image/png", "png"),
                s::EncoderPreset::LibjpegTurbo { .. } => ("image/jpeg", "jpg"),

                s::EncoderPreset::Gif { .. } => ("image/gif", "gif"),
            };

            classic.codec_id = wanted_id;
            if !ffi::flow_codec_initialize(c.flow_c(), classic as *mut ffi::CodecInstance) {
                cerror!(c).panic();
            }
            let codec_def = ffi::flow_codec_get_definition(c.flow_c(), wanted_id);
            if codec_def.is_null() {
                cerror!(c).panic();
            }
            let write_fn = (*codec_def).write_frame;
            if write_fn == None {
                panic!("Codec didn't implement write_frame");
            }

            if !write_fn.unwrap()(c.flow_c(),
                                  classic.codec_state,
                                  frame as *mut BitmapBgra,
                                  &hints as *const ffi::EncoderHints) {
                cerror!(c).panic();
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
}

impl CodecInstanceContainer{

     pub fn write_frame(&mut self, c: &Context, job: &Job, preset: &s::EncoderPreset, frame: &mut BitmapBgra) -> Result<s::EncodeResult>{
         // Pick encoder
         if let CodecKind::EncoderPlaceholder = self.codec{
             match *preset {
                 s::EncoderPreset::Gif => {
                     println!("Using gif encoder");
                     self.codec = CodecKind::Encoder(Box::new(gif::GifEncoder::create(c, job, c.get_proxy_mut(self.proxy_uuid)?.deref_mut(), preset, self.io_id)));
                 },
                 _ => {
                     //println!("Using classic encoder");
                     self.codec = CodecKind::Encoder(Box::new(
                         ClassicEncoder::get_empty(self.io_id, c.get_proxy_mut(self.proxy_uuid)?.get_io_ptr())));
                 }
             }
         }
         if let CodecKind::Encoder(ref mut e) = self.codec {
             e.write_frame(c, job,  &mut c.get_proxy_mut(self.proxy_uuid)?.deref_mut(), preset, frame)
         }else{
             panic!("");
             //Err(FlowError::ErrNotImpl)
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
