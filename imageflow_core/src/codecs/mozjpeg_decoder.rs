use std;
use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::ffi;
use crate::{Context, CError,  Result, JsonResponse};
use crate::ffi::{CodecInstance, wrap_jpeg_get_custom_state, WrapJpegSourceManager, flow_node_execute_scale2d_render1d};
use crate::ffi::BitmapBgra;
use imageflow_types::collections::AddRemoveSet;
use crate::io::IoProxy;
use uuid::Uuid;
use imageflow_types::IoDirection;
use super::*;
use std::any::Any;
use std::rc::Rc;
use crate::io::IoProxyProxy;
use crate::io::IoProxyRef;
use rgb::alt::BGRA8;
extern crate mozjpeg_sys;
use ::mozjpeg_sys::*;
use imageflow_helpers::preludes::from_std::ptr::{null, slice_from_raw_parts, null_mut};

pub struct MozJpegDecoder{
    decoder: Box<MzDec>
}

impl MozJpegDecoder {
    pub fn create(c: &Context, io: IoProxy, io_id: i32) -> Result<MozJpegDecoder> {
        Ok(MozJpegDecoder{
            decoder: MzDec::new(c, io)?
        })
    }
}


impl Decoder for MozJpegDecoder {
    fn initialize(&mut self, c: &Context) -> Result<()> {
        Ok(())
    }

    fn get_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        let (w,h) = self.decoder.get_final_size()?;

        Ok(s::ImageInfo {
            frame_decodes_into: s::PixelFormat::Bgr32,
            image_width: w as i32,
            image_height: h as i32,
            preferred_mime_type: "image/jpeg".to_owned(),
            preferred_extension: "jpg".to_owned()
        })
    }

    fn get_exif_rotation_flag(&mut self, c: &Context) -> Result<Option<i32>> {
        Ok(self.decoder.get_exif_rotation_flag()?)
    }

    fn tell_decoder(&mut self, c: &Context, tell: s::DecoderCommand) -> Result<()> {
        match tell {
            s::DecoderCommand::JpegDownscaleHints(hints) => {
                let h = crate::ffi::DecoderDownscaleHints {
                    downscale_if_wider_than: hints.width,
                    downscaled_min_width: hints.width,
                    or_if_taller_than: hints.height,
                    downscaled_min_height: hints.height,
                    scale_luma_spatially: hints.scale_luma_spatially.unwrap_or(false),
                    gamma_correct_for_srgb_during_spatial_luma_scaling: hints.gamma_correct_for_srgb_during_spatial_luma_scaling.unwrap_or(false)
                };
                self.decoder.set_downscale_hints(h);
                Ok(())
            },
            s::DecoderCommand::WebPDecoderHints(hints) =>{
                Ok(()) // We can safely ignore webp hints
            }
            s::DecoderCommand::DiscardColorProfile => {
                self.decoder.ignore_color_profile = true;
                Ok(())
            }
        }
    }

    fn read_frame(&mut self, c: &Context) -> Result<*mut BitmapBgra> {

        let (w,h) = self.decoder.get_final_size()?;

        let canvas =
            BitmapBgra::create(c, w, h, ffi::PixelFormat::Bgr32, s::Color::Transparent)?;

        self.decoder.read_frame(canvas)?;

        Ok(canvas)
    }
    fn has_more_frames(&mut self) -> Result<bool> {
        Ok(false)
    }
    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}


#[repr(C)]
struct SourceManager{
    manager: ffi::WrapJpegSourceManager,
    bytes_have_been_read: bool,
    buffer: Vec<u8>
}

struct MzDec{
    error_state: Vec<u8>,
    codec_info: jpeg_decompress_struct,
    codec_info_disposed: bool,
    error: Option<FlowError>,
    io: IoProxy,
    source_manager: Option<Box<SourceManager>>,
    header_read: bool,
    original_width: u32,
    original_height: u32,
    hints: ffi::DecoderDownscaleHints,
    w: u32,
    h: u32,
    exif_rotation_flag: Option<i32>,
    pub ignore_color_profile: bool,
    color_profile: Option<Vec<u8>>,
    gamma: f64

}
impl Drop for MzDec{
    fn drop(&mut self) {
        self.dispose_codec();
    }
}


impl MzDec{

    #[no_mangle]
    extern "C" fn jpeg_error_handler(custom_state: *mut c_void,
                                     codec_info: *mut mozjpeg_sys::jpeg_common_struct,
                                     error_code: i32,
                                     message_buffer: *const u8,
                                     message_buffer_length: i32) -> bool{
        let decoder = unsafe{ &mut *(custom_state as *mut MzDec) };

        if decoder.error.is_none() {
            if !message_buffer.is_null(){
                let bytes = unsafe {
                    std::slice::from_raw_parts(message_buffer, message_buffer_length as usize)
                };
                let cstr = CStr::from_bytes_with_nul(bytes).expect("MozJpeg error message was not null terminated");
                let message = cstr.to_str().expect("MozJpeg error message was not UTF-8");
                decoder.error = Some(nerror!(ErrorKind::JpegDecodingError, "MozJPEG error {}: {}", error_code, message));
            }else {
                decoder.error = Some(nerror!(ErrorKind::JpegDecodingError, "MozJPEG error {}", error_code));
            }
        }

        return false; //false -> Fail, true -> ignore
    }

    fn new(context: &Context, io: IoProxy) -> Result<Box<MzDec>>{

        //Allocate space for the error state structure.
        let error_state_size = unsafe{ ffi::wrap_jpeg_error_state_bytes() };
        let mut error_state: Vec<u8> = Vec::with_capacity(error_state_size);
        for ix in 0..error_state_size{
            error_state.push(0u8);
        }

        let mut decoder = Box::new(MzDec {
            error_state,
            codec_info: unsafe { mem::zeroed() },
            codec_info_disposed: false,
            error: None,
            io,
            source_manager: None,
            header_read: false,
            original_width: 0,
            original_height: 0,
            hints: ffi::DecoderDownscaleHints {
                downscale_if_wider_than: 0,
                or_if_taller_than: 0,
                downscaled_min_width: 0,
                downscaled_min_height: 0,
                scale_luma_spatially: false,
                gamma_correct_for_srgb_during_spatial_luma_scaling: false
            },
            w: 0,
            h: 0,
            exif_rotation_flag: None,
            ignore_color_profile: false,
            color_profile: None,
            gamma: 0.45455
        });

        unsafe {
            ffi::wrap_jpeg_setup_error_handler(
                &mut decoder.codec_info,
                decoder.error_state.as_mut_ptr() as *mut c_void,
                decoder.as_mut() as *mut MzDec as *mut c_void,
                MzDec::jpeg_error_handler
            );

            if !ffi::wrap_jpeg_create_decompress(&mut decoder.codec_info){
                return Err(decoder.error.clone().expect("error missing").at(here!()));
            }
        }

        Ok(decoder)
    }

    fn dispose_codec(&mut self) {
        if !self.codec_info_disposed {
            unsafe {
                jpeg_destroy_decompress(&mut self.codec_info)
            }
            self.codec_info_disposed = true
        }
    }


    fn get_final_size(&mut self) -> Result<(u32,u32)>{
        self.read_header()?;
        self.apply_downscaling();
        Ok((self.w, self.h))
    }

    fn get_exif_rotation_flag(&mut self) -> Result<Option<i32>>{
        self.read_header()?;
        Ok(self.exif_rotation_flag)
    }

    fn read_frame(&mut self, canvas: *mut BitmapBgra) -> Result<()> {
        if self.codec_info_disposed{
            return Err(nerror!(ErrorKind::InvalidOperation, "MozJpeg decoder disposed before call to read_frame"))
        }

        self.read_header()?;
        self.apply_downscaling();

        unsafe {
            if self.w != (*canvas).w || self.h != (*canvas).h {
                return Err(nerror!(ErrorKind::InvalidArgument, "Canvas not sized for decoded jpeg"));
            }
        }

        let is_cmyk = self.codec_info.jpeg_color_space == mozjpeg_sys::JCS_CMYK ||
            self.codec_info.jpeg_color_space == mozjpeg_sys::JCS_YCCK;


        if !is_cmyk {
            self.codec_info.out_color_space = mozjpeg_sys::JCS_EXT_BGRA; //Why not BGRX? Maybe because it doesn't clear the alpha values
        } else {
            return Err(nerror!(ErrorKind::JpegDecodingError, "CMYK JPEG support not implemented"));
        }

        unsafe {
            if !ffi::wrap_jpeg_start_decompress(&mut self.codec_info) {
                return Err(self.error.clone().expect("error missing").at(here!()));
            }
        }

        self.gamma = self.codec_info.output_gamma;

        let mut row_pointers = unsafe{ (*canvas).get_row_pointers()}?;

        if row_pointers.len() != self.codec_info.output_height as usize{
            return Err(nerror!(ErrorKind::InvalidOperation, "get_row_pointers() length ({}) does not match image height ({})",
            row_pointers.len(), self.codec_info.output_height));
        }

        let mut scanlines_read = 0;

        while self.codec_info.output_scanline < self.codec_info.output_height {
            unsafe {
                let next_lines = &mut row_pointers[self.codec_info.output_scanline as usize];
                if !ffi::wrap_jpeg_read_scan_lines(&mut self.codec_info,
                                                   next_lines,
                                                    self.h,
                                                    &mut scanlines_read) {
                    return Err(self.error.clone().expect("error missing").at(here!()));
                }
            }

        }

        if scanlines_read < 1 {
            self.error = Some(nerror!(ErrorKind::JpegDecodingError, "Zero scanlines read from jpeg"));
            return Err(self.error.clone().expect("error missing").at(here!()));
        }


        // Read metadata again, ICC profile/exif flag (yes we look twice)
        self.interpret_metadata();

        unsafe {
            if !ffi::wrap_jpeg_finish_decompress(&mut self.codec_info) {
                return Err(self.error.clone().expect("error missing").at(here!()));
            }
        }

        let color_info = self.get_decoder_color_info();

        if !self.ignore_color_profile {
            ColorTransformCache::transform_to_srgb(unsafe { &mut *canvas }, &color_info)?;
        }

        self.dispose_codec();

        Ok(())
    }


    #[no_mangle]
    extern "C" fn source_fill_buffer(codec_info: &mut mozjpeg_sys::jpeg_decompress_struct, custom_state: *mut c_void, suspend_io: &mut bool) -> bool{
        let decoder = unsafe{ &mut *(custom_state as *mut MzDec) };

        let mut source_manager = decoder.source_manager.as_deref_mut().unwrap();


        let buffer = source_manager.buffer.as_mut();
        match decoder.io.read(buffer){
            Ok(size) => {
                if size == 0 {
                    if source_manager.bytes_have_been_read {
                        // Fake a correctly ended jpeg file so we can recover what's possible from this jpeg.
                        buffer[0] = 0xFF;
                        buffer[1] = 0xD9;
                        source_manager.manager.shared_mgr.next_input_byte = buffer.as_mut_ptr();
                        source_manager.manager.shared_mgr.bytes_in_buffer = 2;
                        return true;
                    }else{
                        decoder.error = Some(nerror!(ErrorKind::ImageDecodingError, "Empty source file"));
                        return false;
                    }
                }else{
                    source_manager.manager.shared_mgr.next_input_byte = buffer.as_mut_ptr();
                    source_manager.manager.shared_mgr.bytes_in_buffer = size;
                    source_manager.bytes_have_been_read = true;
                    return true;
                }
            },
            Err(err) => {
                decoder.error = Some(FlowError::from_decoder(err));
                return false;
            }
        }

    }

    #[no_mangle]
    extern "C" fn source_skip_bytes(codec_info: &mut mozjpeg_sys::jpeg_decompress_struct, custom_state: *mut c_void, mut byte_count: c_long) -> bool{
        if byte_count > 0 {
            let decoder = unsafe{ &mut *(custom_state as *mut MzDec) };
            let mut source_manager = decoder.source_manager.as_deref_mut().unwrap();

            while byte_count > source_manager.manager.shared_mgr.bytes_in_buffer as c_long {
                byte_count -= source_manager.manager.shared_mgr.bytes_in_buffer as c_long;
                let mut suspend = false;
                if !MzDec::source_fill_buffer(codec_info, custom_state, &mut suspend){
                    decoder.error = decoder.error.clone().map(|e| e.at(here!()));
                    return false;
                }
            }

            source_manager.manager.shared_mgr.next_input_byte =
               unsafe {
                   source_manager.manager.shared_mgr.next_input_byte.offset(byte_count as isize)
               };
            source_manager.manager.shared_mgr.bytes_in_buffer -= byte_count as usize;
        }
        true
    }

    fn setup_source_manager(&mut self){
        if self.source_manager.is_none() {
            let mut mgr = Box::new(
                SourceManager {
                    manager: WrapJpegSourceManager {
                        shared_mgr: unsafe { mem::zeroed()  },
                        init_source_fn: None,
                        term_source_fn: None,
                        fill_input_buffer_fn: Some(MzDec::source_fill_buffer),
                        skip_input_data_fn: Some(MzDec::source_skip_bytes),
                        custom_state: self as *mut MzDec as *mut c_void
                    },
                    bytes_have_been_read: false,
                    buffer: vec![0; 4096]
                }
            );
            unsafe {
                ffi::wrap_jpeg_setup_source_manager(&mut mgr.manager);
            }
            self.source_manager = Some(mgr);
            self.codec_info.src = &mut self.source_manager.as_deref_mut().unwrap().manager.shared_mgr;
        }
    }


    fn read_header(&mut self) -> Result<()>{
        if self.error.is_some(){
            return Err(self.error.clone().unwrap());
        }
        if self.header_read{
            return Ok(());
        }
        if self.codec_info_disposed{
            return Err(nerror!(ErrorKind::InvalidOperation, "MozJpeg decoder disposed before call to read_header"))
        }
        self.setup_source_manager();

        if unsafe{ !ffi::wrap_jpeg_save_markers(&mut self.codec_info, ffi::JpegMarker::ICC as i32, 0xffff) } {
            return Err(self.error.clone().expect("error missing").at(here!()));
        }
        if unsafe{ !ffi::wrap_jpeg_save_markers(&mut self.codec_info, ffi::JpegMarker::EXIF as i32, 0xffff) } {
            return Err(self.error.clone().expect("error missing").at(here!()));
        }

        if unsafe{ !ffi::wrap_jpeg_read_header(&mut self.codec_info) } {
            return Err(self.error.clone().expect("error missing").at(here!()));
        }

        self.interpret_metadata();

        self.original_width = self.codec_info.image_width;
        self.original_height = self.codec_info.image_height;
        self.w = self.original_width;
        self.h = self.original_height;

        self.header_read = true;
        Ok(())

    }

    fn set_downscale_hints(&mut self, hints: ffi::DecoderDownscaleHints){
        unsafe {
            ffi::wrap_jpeg_set_downscale_type(&mut self.codec_info,
                                              hints.scale_luma_spatially,
                                              hints.gamma_correct_for_srgb_during_spatial_luma_scaling)
        }
        self.hints = hints;
    }

    fn apply_downscaling(&mut self) {
        // It's a segfault to call
        if self.codec_info_disposed {
            return;
        }
        unsafe {
            ffi::wrap_jpeg_set_idct_method_selector(&mut self.codec_info)
        }


        if self.hints.downscaled_min_width > 0 && self.hints.downscaled_min_height > 0 {
            if self.original_width > self.hints.downscale_if_wider_than as u32
                || self.original_height > self.hints.or_if_taller_than as u32 {
                for i in 1..8 {
                    if i == 7 {
                        continue; // Because 7/8ths is slower than 8/8
                    }

                    let new_w = (self.original_width * i + 8 - 1) / 8;
                    let new_h = (self.original_height * i + 8 - 1) / 8;
                    if new_w >= self.hints.downscaled_min_width as u32 && new_h >= self.hints.downscaled_min_height as u32 {
                        self.codec_info.scale_denom = 8;
                        self.codec_info.scale_num = i;
                        self.w = new_w;
                        self.h = new_h;
                        return;
                    }
                }
            }
        }
    }

    fn interpret_metadata(&mut self){
        if self.color_profile.is_none() {
            self.color_profile =
                crate::codecs::mozjpeg_decoder_helpers::read_icc_profile(&self.codec_info);
        }
        if self.exif_rotation_flag.is_none(){
            self.exif_rotation_flag = crate::codecs::mozjpeg_decoder_helpers::get_exif_orientation(&self.codec_info);
        }
    }

    fn get_decoder_color_info(&mut self) -> ffi::DecoderColorInfo{
        let mut info = ffi::DecoderColorInfo{
            source: ColorProfileSource::Null,
            profile_buffer: null_mut(),
            buffer_length: 0,
            white_point: Default::default(),
            primaries: ::lcms2::CIExyYTRIPLE{
                Red: Default::default(),
                Green: Default::default(),
                Blue: Default::default()
            },
            gamma: self.gamma
        };
        
        if let Some(profile) = self.color_profile.as_deref_mut() {
            //let hash = imageflow_helpers::hashing::hash_64(&profile[80..]);

            // if hash == 250807001850340861 {
            //     info.source = ColorProfileSource::sRGB;
            // }else {
            info.profile_buffer = profile.as_mut_ptr();
            info.buffer_length = profile.len();
            info.source = ColorProfileSource::ICCP;
            //}
        }
        info
    }
}