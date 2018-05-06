use std;
use std::sync::*;
use for_other_imageflow_crates::preludes::external_without_std::*;
use ffi;
use ::{Context, CError, Result, JsonResponse, ErrorKind, FlowError, ErrorCategory};
use ffi::CodecInstance;
use ffi::BitmapBgra;
use ffi::DecoderColorInfo;
use ffi::ColorProfileSource;
use imageflow_types::collections::AddRemoveSet;
use io::IoProxy;
use uuid::Uuid;
use imageflow_types::IoDirection;
use std::borrow::BorrowMut;
use std::ops::DerefMut;
use std::any::Any;
use lcms2::*;
use lcms2;
mod gif;
mod pngquant;
mod lode;
mod mozjpeg;
mod libjpeg_turbo;
use io::IoProxyRef;

pub trait DecoderFactory{
    fn create(c: &Context, io: &mut IoProxy, io_id: i32) -> Option<Result<Box<Decoder>>>;
}
pub trait Decoder : Any{
    fn initialize(&mut self, c: &Context) -> Result<()>;
    fn get_image_info(&mut self, c: &Context) -> Result<s::ImageInfo>;
    fn get_exif_rotation_flag(&mut self, c: &Context) -> Result<Option<i32>>;
    fn tell_decoder(&mut self, c: &Context, tell: s::DecoderCommand) -> Result<()>;
    fn read_frame(&mut self, c: &Context) -> Result<*mut BitmapBgra>;
    fn has_more_frames(&mut self) -> Result<bool>;
    fn as_any(&self) -> &Any;
}
pub trait Encoder{
    // GIF encoder will need to know if transparency is required (we could guess based on first input frame)
    // If not required, we can do frame shrinking and delta encoding. Otherwise we have to
    // encode entire frames and enable transparency (default)
    fn write_frame(&mut self, c: &Context, preset: &s::EncoderPreset, frame: &mut BitmapBgra, decoder_io_ids: &[i32]) -> Result<s::EncodeResult>;

    fn get_io(&self) -> Result<IoProxyRef>;
}




enum CodecKind{
    EncoderPlaceholder,
    Encoder(Box<Encoder>),
    Decoder(Box<Decoder>)
}
// We need a rust-friendly codec instance, codec definition, and a way to wrap C codecs
pub struct CodecInstanceContainer{
    pub io_id: i32,
    codec: CodecKind,
    encode_io: Option<IoProxy>
}

impl CodecInstanceContainer {

    pub fn get_decoder(&mut self) -> Result<&mut Decoder>{
        if let CodecKind::Decoder(ref mut d) = self.codec{
            Ok(&mut **d)
        }else{
            Err(nerror!(ErrorKind::InvalidArgument, "Not a decoder"))
        }

    }

    pub fn create(c: &Context, io: IoProxy, io_id: i32, direction: IoDirection) -> Result<CodecInstanceContainer>{
        if direction == IoDirection::Out {
            Ok(CodecInstanceContainer
                {
                    io_id,
                    codec: CodecKind::EncoderPlaceholder,
                    encode_io: Some(io),
                })
        }else {
            let mut buffer = [0u8; 8];
            let result = io.read_to_buffer(c, &mut buffer).map_err(|e| e.at(here!()))?;

            io.seek(c, 0).map_err(|e| e.at(here!()))?;
            if buffer.starts_with(b"GIF89a") || buffer.starts_with(b"GIF87a") {
                Ok(CodecInstanceContainer
                    {
                        io_id,
                        codec: CodecKind::Decoder(Box::new(gif::GifDecoder::create(c, io, io_id)?)),
                        encode_io: None
                    })
            } else {
                Ok(CodecInstanceContainer
                    {
                        io_id,
                        codec: CodecKind::Decoder(ClassicDecoder::create(c, io, io_id)?),
                        encode_io: None
                    })
            }
        }
    }

}

struct ClassicDecoder{
    classic: CodecInstance,
    ignore_color_profile: bool,
    io: IoProxy
}

impl ClassicDecoder {
    fn create(c: &Context, io:  IoProxy, io_id: i32) -> Result<Box<impl Decoder>> {
        let codec_id = unsafe {
            ::ffi::flow_codec_select_from_seekable_io(c.flow_c(), io.get_io_ptr())
        };
        if codec_id == 0 {
            Err(cerror!(c))
        } else {
            Ok(Box::new(ClassicDecoder {
                classic: CodecInstance {
                    codec_id,
                    codec_state: ptr::null_mut(),
                    direction: IoDirection::In,
                    io_id,
                    io: io.get_io_ptr()
                },
                io,
                ignore_color_profile: false
            }))
        }
    }
}

impl Decoder for ClassicDecoder{
    fn initialize(&mut self, c: &Context) -> Result<()>{
        unsafe {
            if !::ffi::flow_codec_initialize(c.flow_c(), &mut self.classic as *mut CodecInstance) {
                return Err(cerror!(c));
            }

            Ok(())
        }

    }

    fn get_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
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
                    // frame_count: info.frame_count,
                    // current_frame_index: info.current_frame_index,
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

    fn get_exif_rotation_flag(&mut self, c: &Context) -> Result<Option<i32>> {
        let exif_flag = unsafe {
            ffi::flow_codecs_jpg_decoder_get_exif(c.flow_c(),
                                                  &mut self.classic as *mut ffi::CodecInstance)
        };
        if exif_flag >= 0 {
            Ok(Some(exif_flag))
        } else {
            Ok(None)
        }
    }
    fn read_frame(&mut self, c: &Context) -> Result<*mut BitmapBgra> {

        #[allow(non_snake_case)]
        let blank_xyY = CIExyY{
            x: 0f64,
            y: 0f64,
            Y: 0f64,
        };
        let mut color_info = ffi::DecoderColorInfo{
            source: ffi::ColorProfileSource::Null,
            profile_buffer: ptr::null_mut(),
            buffer_length: 0,
            primaries: CIExyYTRIPLE{
                Red: blank_xyY,
                Green: blank_xyY,
                Blue: blank_xyY,
            },
            gamma: 0.0f64,
            white_point: blank_xyY
        };
        let result = unsafe {
            ffi::flow_codec_execute_read_frame(c.flow_c(),
                                               &mut  self.classic as *mut ffi::CodecInstance,
                                        &mut color_info as *mut ffi::DecoderColorInfo) };
        if result.is_null() {
            Err(cerror!(c))
        }else {
            if !self.ignore_color_profile {
                ColorTransformCache::transform_to_srgb(unsafe { &mut *result }, &color_info)?;
            }
            ColorTransformCache::dispose_color_info(&mut color_info);


            Ok(result)
        }
    }


    fn tell_decoder(&mut self, c: &Context, tell: s::DecoderCommand) -> Result<()> {
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
            },
            s::DecoderCommand::DiscardColorProfile => {
                self.ignore_color_profile = true;
                Ok(())
            }
        }
    }
    fn has_more_frames(&mut self) -> Result<bool> {
        Ok(false)
    }
    fn as_any(& self) -> &Any {
        self as &Any
    }
}

struct ClassicEncoder{
    classic: CodecInstance,
    io_id: i32,
    io: IoProxy
}

impl ClassicEncoder{
    fn get_codec_id_and_hints(preset: &s::EncoderPreset) -> Result<(i64, ffi::EncoderHints)>{
        match *preset {
            s::EncoderPreset::LibjpegTurboClassic { quality, progressive, optimize_huffman_coding } => {
                Ok((ffi::CodecType::EncodeJpeg as i64,
                 ffi::EncoderHints {
                     jpeg_encode_quality: quality.unwrap_or(90),
                     disable_png_alpha: false,
                     jpeg_allow_low_quality_non_baseline: false,
                     jpeg_optimize_huffman_coding: optimize_huffman_coding.unwrap_or(false), //2x slowdown
                     jpeg_progressive: progressive.unwrap_or(false), //5x slowdown
                     jpeg_use_arithmetic_coding: false, // arithmetic coding is not widely supported
                 }))
            }
            s::EncoderPreset::Libpng { ref matte,
                zlib_compression,
                ref depth } => {
                Ok((ffi::CodecType::EncodePng as i64,
                 ffi::EncoderHints {
                     jpeg_encode_quality: -1,
                     jpeg_allow_low_quality_non_baseline: false,
                     jpeg_optimize_huffman_coding: true,
                     jpeg_progressive: true,
                     jpeg_use_arithmetic_coding: false, // arithmetic coding is not widely supported
                     disable_png_alpha: match *depth {
                         Some(s::PngBitDepth::Png24) => true,
                         _ => false,
                     },
                 }))
            }
            _ => {
                Err(unimpl!("Classic encoder only supports libjpeg and libpng"))
            }
        }
    }
    fn get_empty(io_id: i32, io: IoProxy) -> Result<ClassicEncoder> {
        let ptr = io.get_io_ptr();
        Ok(ClassicEncoder {
            io_id,
            io,
            classic: CodecInstance {
                io_id,
                codec_id: 0,
                codec_state: ptr::null_mut(),
                direction: IoDirection::Out,
                io: ptr
            }
        })
    }


}

impl Encoder for ClassicEncoder{

    fn write_frame(&mut self, c: &Context, preset: &s::EncoderPreset, frame: &mut BitmapBgra, decoder_io_ids: &[i32]) -> Result<s::EncodeResult> {

        let (wanted_id, hints) = ClassicEncoder::get_codec_id_and_hints(preset)?;
        unsafe {
            let classic = &mut self.classic;

            let (result_mime, result_ext) = match *preset {
                s::EncoderPreset::Libpng { .. } |
                s::EncoderPreset::Lodepng { .. } |
                s::EncoderPreset::Pngquant { .. } => ("image/png", "png"),
                s::EncoderPreset::Mozjpeg { .. } |
                s::EncoderPreset::LibjpegTurbo { .. } |
                s::EncoderPreset::LibjpegTurboClassic { .. } => ("image/jpeg", "jpg"),
                s::EncoderPreset::Gif { .. } => ("image/gif", "gif"),
            };

            classic.codec_id = wanted_id;
            if !ffi::flow_codec_initialize(c.flow_c(), classic as *mut ffi::CodecInstance) {
                return Err(cerror!(c))?
            }
            let codec_def = ffi::flow_codec_get_definition(c.flow_c(), wanted_id);
            if codec_def.is_null() {
                return Err(cerror!(c))?
            }
            let write_fn = (*codec_def).write_frame;
            if write_fn == None {
                unimpl!("Codec didn't implement write_frame");
            }

            if !write_fn.unwrap()(c.flow_c(),
                                  classic.codec_state,
                                  frame as *mut BitmapBgra,
                                  &hints as *const ffi::EncoderHints) {
                return Err(cerror!(c))?
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
    fn get_io(&self) -> Result<IoProxyRef> {
        Ok(IoProxyRef::Borrow(&self.io))
    }
}

impl CodecInstanceContainer{

     pub fn write_frame(&mut self, c: &Context, preset: &s::EncoderPreset, frame: &mut BitmapBgra, decoder_io_ids: &[i32]) -> Result<s::EncodeResult>{

         // Pick encoder
         if let CodecKind::EncoderPlaceholder = self.codec {

             let io = self.encode_io.take().unwrap();

             let codec = match *preset {
                 s::EncoderPreset::Gif => {
                     CodecKind::Encoder(Box::new(gif::GifEncoder::create(c, preset, io, frame)?))
                 },
                 s::EncoderPreset::Pngquant {speed, quality} => {
                     CodecKind::Encoder(Box::new(pngquant::PngquantEncoder::create(c, speed, quality, io)?))
                 },
                 s::EncoderPreset::Mozjpeg {quality, progressive} => {
                     CodecKind::Encoder(Box::new(mozjpeg::MozjpegEncoder::create(c, quality, progressive, io)?))
                 },
                 s::EncoderPreset::LibjpegTurbo {quality, progressive, optimize_huffman_coding} => {
                     CodecKind::Encoder(Box::new(mozjpeg::MozjpegEncoder::create_classic(c, quality.map(|q| q as u8), progressive, optimize_huffman_coding, io)?))
                 },
                 s::EncoderPreset::Lodepng => {
                     CodecKind::Encoder(Box::new(lode::LodepngEncoder::create(c, io)?))
                 },
                 s::EncoderPreset::Libpng {..} | s::EncoderPreset::LibjpegTurboClassic {..} => {
                     CodecKind::Encoder(Box::new(
                         ClassicEncoder::get_empty(self.io_id, io)?))
                 }
             };
             self.codec = codec;
         };


         if let CodecKind::Encoder(ref mut e) = self.codec {
             match e.write_frame(c, preset, frame, decoder_io_ids).map_err(|e| e.at(here!())){
                 Err(e) => Err(e),
                 Ok(result) => {
                     match result.bytes{
                         s::ResultBytes::Elsewhere => Ok(result),
                         other => Err(nerror!(ErrorKind::InternalError, "Encoders must return s::ResultBytes::Elsewhere and write to their owned IO. Found {:?}", other))

                     }
                 }
             }
         }else{
             Err(unimpl!())
             //Err(FlowError::ErrNotImpl)
         }
    }

    pub fn get_encode_io(&self) -> Result<Option<IoProxyRef>>{
        if let CodecKind::Encoder(ref e) = self.codec {
            Ok(Some(e.get_io().map_err(|e| e.at(here!()))?))
        }else if let Some(ref e) = self.encode_io{
            Ok(Some(IoProxyRef::Borrow(e)))
        } else {
            Ok(None)
        }
    }
}


struct ColorTransformCache{

}

lazy_static!{
    static ref PROFILE_TRANSFORMS: ::chashmap::CHashMap<u64, Transform<u32,u32,ThreadContext, DisallowCache>> = ::chashmap::CHashMap::with_capacity(4);
    static ref GAMA_TRANSFORMS: ::chashmap::CHashMap<u64, Transform<u32,u32, ThreadContext,DisallowCache>> = ::chashmap::CHashMap::with_capacity(4);

}



impl ColorTransformCache{

    fn get_pixel_format(fmt: ffi::PixelFormat) -> PixelFormat{
        match fmt {
            ffi::PixelFormat::Bgr32 | ffi::PixelFormat::Bgra32 => PixelFormat::BGRA_8,
            ffi::PixelFormat::Bgr24 => PixelFormat::BGR_8,
            ffi::PixelFormat::Gray8 => PixelFormat::GRAY_8
        }
    }

    fn create_gama_transform(color: &ffi::DecoderColorInfo, pixel_format: PixelFormat) -> Result<Transform<u32,u32, ThreadContext,DisallowCache>>{
        let srgb = Profile::new_srgb_context(ThreadContext::new()); // Save 1ms by caching - but not sync

        let gama = ToneCurve::new(1f64 / color.gamma);
        let p = Profile::new_rgb_context(ThreadContext::new(),&color.white_point, &color.primaries, &[&gama, &gama, &gama] ).map_err(|e| FlowError::from(e).at(here!()))?;

        let transform = Transform::new_flags_context(ThreadContext::new(),&p, pixel_format, &srgb, pixel_format, Intent::Perceptual, Flags::NO_CACHE).map_err(|e| FlowError::from(e).at(here!()))?;
        Ok(transform)
    }
    fn create_profile_transform(color: &ffi::DecoderColorInfo, pixel_format: PixelFormat) -> Result<Transform<u32,u32, ThreadContext,DisallowCache>> {

        if color.profile_buffer.is_null() || color.buffer_length < 1{
            unreachable!();
        }
        let srgb = Profile::new_srgb_context(ThreadContext::new()); // Save 1ms by caching - but not sync

        let bytes = unsafe { slice::from_raw_parts(color.profile_buffer, color.buffer_length) };

        let p = Profile::new_icc_context(ThreadContext::new(), bytes).map_err(|e| FlowError::from(e).at(here!()))?;
        //TODO: handle gray transform on rgb expanded images.
        //TODO: Add test coverage for grayscale png

        let transform = Transform::new_flags_context(ThreadContext::new(),
                                                     &p, pixel_format, &srgb, pixel_format, Intent::Perceptual, Flags::NO_CACHE).map_err(|e| FlowError::from(e).at(here!()))?;

        Ok(transform)
    }
    fn hash(color: &ffi::DecoderColorInfo, pixel_format: PixelFormat) -> Option<u64>{
        match color.source {
            ffi::ColorProfileSource::Null | ffi::ColorProfileSource::sRGB => None,
            ffi::ColorProfileSource::GAMA_CHRM => {
                let struct_bytes = unsafe {
                    slice::from_raw_parts(color as *const DecoderColorInfo as *const u8, mem::size_of::<DecoderColorInfo>())
                };
                Some(imageflow_helpers::hashing::hash_64(struct_bytes) ^ pixel_format as u64)
            },
            ffi::ColorProfileSource::ICCP | ffi::ColorProfileSource::ICCP_GRAY => {
                if !color.profile_buffer.is_null() && color.buffer_length > 0 {
                    let bytes = unsafe { slice::from_raw_parts(color.profile_buffer, color.buffer_length) };

                    // Skip first 80 bytes when hashing.
                    Some(imageflow_helpers::hashing::hash_64(&bytes[80..]) ^ pixel_format as u64)
                } else {
                    unreachable!("Profile source should never be set to ICCP without a profile buffer. Buffer length {}", color.buffer_length);
                }
            }
        }
    }

    fn apply_transform(frame: &mut BitmapBgra, transform: &Transform<u32,u32, ThreadContext,DisallowCache>) {
        for row in 0..frame.h {
            let pixels = unsafe{ slice::from_raw_parts_mut(frame.pixels.offset((row * frame.stride) as isize) as *mut u32, frame.w as usize) };
            transform.transform_in_place(pixels)
        }
    }

    pub fn transform_to_srgb(frame: &mut BitmapBgra, color: &ffi::DecoderColorInfo) -> Result<()>{

        if frame.fmt.bytes() != 4{
            return Err(nerror!(ErrorKind::Category(ErrorCategory::InternalError), "Color profile application is only supported for Bgr32 and Bgra32 canvases"));
        }
        let pixel_format = ColorTransformCache::get_pixel_format(frame.fmt);

        match color.source {
            ffi::ColorProfileSource::Null | ffi::ColorProfileSource::sRGB => Ok(()),
            ffi::ColorProfileSource::GAMA_CHRM => {

                // Cache up to 4 GAMA x PixelFormat transforms
                if GAMA_TRANSFORMS.len() > 3{
                    let transform = ColorTransformCache::create_gama_transform(color, pixel_format).map_err(|e| e.at(here!()))?;
                    ColorTransformCache::apply_transform(frame, &transform);
                    Ok(())
                }else{
                    let hash = ColorTransformCache::hash(color, pixel_format).unwrap();
                    if !GAMA_TRANSFORMS.contains_key(&hash) {
                        let transform = ColorTransformCache::create_gama_transform(color, pixel_format).map_err(|e| e.at(here!()))?;
                        GAMA_TRANSFORMS.insert(hash, transform);
                    }
                    ColorTransformCache::apply_transform(frame, &*GAMA_TRANSFORMS.get(&hash).unwrap());
                    Ok(())
                }
            },
            ffi::ColorProfileSource::ICCP | ffi::ColorProfileSource::ICCP_GRAY => {
                // Cache up to 9 ICC profile x PixelFormat transforms
                if PROFILE_TRANSFORMS.len() > 8{
                    let transform = ColorTransformCache::create_profile_transform(color, pixel_format).map_err(|e| e.at(here!()))?;
                    ColorTransformCache::apply_transform(frame, &transform);
                    Ok(())
                }else{
                    let hash = ColorTransformCache::hash(color, pixel_format).unwrap();
                    if !PROFILE_TRANSFORMS.contains_key(&hash) {
                        let transform = ColorTransformCache::create_profile_transform(color, pixel_format).map_err(|e| e.at(here!()))?;
                        PROFILE_TRANSFORMS.insert(hash, transform);
                    }
                    ColorTransformCache::apply_transform(frame, &*PROFILE_TRANSFORMS.get(&hash).unwrap());
                    Ok(())
                }
            }
        }
    }

    pub fn dispose_color_info(color: &mut ffi::DecoderColorInfo){
        // DecoderColor info is cleaned up by the context. For now this is the best option, so that dangling pointers don't happen
    }
}
