use std;
use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::ffi;
use crate::{Context, CError,  Result, JsonResponse};
use crate::ffi::{wrap_jpeg_get_custom_state, WrapJpegSourceManager, flow_node_execute_scale2d_render1d};
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

use imageflow_helpers::preludes::from_std::ptr::{null, slice_from_raw_parts, null_mut};
use mozjpeg_sys::c_void;
use std::os::raw::c_char;


pub struct LibPngDecoder{
    decoder: Box<PngDec>
}

impl LibPngDecoder {
    pub fn create(c: &Context, io: IoProxy, io_id: i32) -> Result<LibPngDecoder> {
        Ok(LibPngDecoder{
            decoder: PngDec::new(c, io)?
        })
    }
}


impl Decoder for LibPngDecoder {
    fn initialize(&mut self, c: &Context) -> Result<()> {
        Ok(())
    }

    fn get_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        let (w,h,fmt) = self.decoder.get_info()?;

        Ok(s::ImageInfo {
            frame_decodes_into: fmt,
            image_width: w as i32,
            image_height: h as i32,
            preferred_mime_type: "image/png".to_owned(),
            preferred_extension: "png".to_owned()
        })
    }

    fn get_exif_rotation_flag(&mut self, c: &Context) -> Result<Option<i32>> {
        Ok(None)
    }

    fn tell_decoder(&mut self, c: &Context, tell: s::DecoderCommand) -> Result<()> {
        match tell {
            s::DecoderCommand::JpegDownscaleHints(hints) => Ok(()),
            s::DecoderCommand::WebPDecoderHints(hints) => Ok(()),
            s::DecoderCommand::DiscardColorProfile => {
                self.decoder.ignore_color_profile = true;
                Ok(())
            }
            s::DecoderCommand::IgnoreColorProfileErrors => {
                self.decoder.ignore_color_profile_errors = true;
                Ok(())
            }
        }
    }

    fn read_frame(&mut self, c: &Context) -> Result<*mut BitmapBgra> {

        let (w,h, fmt) = self.decoder.get_info()?;

        let canvas =
            BitmapBgra::create(c, w, h, fmt, s::Color::Transparent)?;

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


struct PngDec{
    c_state: Vec<u8>,
    c_state_disposed: bool,
    error: Option<FlowError>,
    io: IoProxy,
    bytes_have_been_read: bool,
    header_read: bool,
    w: u32,
    h: u32,
    pixel_format: ffi::PixelFormat,
    pub ignore_color_profile: bool,
    pub ignore_color_profile_errors: bool,
    color_profile: Option<Vec<u8>>,
    color: ffi::DecoderColorInfo
}
impl Drop for PngDec{
    fn drop(&mut self) {
        let _ = self.dispose_codec();
    }
}

impl PngDec{
    #[no_mangle]
    extern "C" fn png_decoder_error_handler(png_ptr: *mut c_void, custom_state: *mut c_void,
                                            message: *const c_char){
        let decoder = unsafe{ &mut *(custom_state as *mut PngDec) };

        if decoder.error.is_none() {
            if !message.is_null(){

                let cstr = unsafe{ CStr::from_ptr(message) };
                let message = cstr.to_str().expect("LibPNG error message was not UTF-8");
                decoder.error = Some(nerror!(ErrorKind::ImageDecodingError, "LibPNG error: {}", message));
            }
        }
    }


    #[no_mangle]
    extern "C" fn png_decoder_custom_read_function(png_ptr: *mut c_void, custom_state: *mut c_void, buffer: *mut u8, bytes_requested: usize, out_bytes_read: &mut usize) -> bool {
        let decoder = unsafe{ &mut *(custom_state as *mut PngDec) };

        let buffer_slice = unsafe{ std::slice::from_raw_parts_mut(buffer, bytes_requested) };

        return match decoder.io.read_exact(buffer_slice) {
            Ok(()) => {
                *out_bytes_read = buffer_slice.len();
                true
            },
            Err(err) => {
                decoder.error = Some(FlowError::from_decoder(err).at(here!()));
                false
            }
        }

    }


    fn new(context: &Context, io: IoProxy) -> Result<Box<PngDec>>{

        //Allocate space for the error state structure.
        let c_state_size = unsafe{ ffi::wrap_png_decoder_state_bytes() };
        let mut c_state: Vec<u8> = Vec::with_capacity(c_state_size);
        for ix in 0..c_state_size{
            c_state.push(0u8);
        }

        let mut decoder = Box::new(PngDec {
            c_state,
            c_state_disposed: false,
            error: None,
            io,
            header_read: false,
            bytes_have_been_read: false,
            w: 0,
            h: 0,
            pixel_format: ffi::PixelFormat::Bgra32,
            ignore_color_profile: false,
            ignore_color_profile_errors: false,
            color_profile: None,
            color: ffi::DecoderColorInfo{
                source: ColorProfileSource::Null,
                profile_buffer: null_mut(),
                buffer_length: 0,
                white_point: Default::default(),
                primaries: ::lcms2::CIExyYTRIPLE{
                    Red: Default::default(),
                    Green: Default::default(),
                    Blue: Default::default()
                },
                gamma: 0.45455
            }
        });

        unsafe {
            if !ffi::wrap_png_decoder_state_init(decoder.c_state.as_mut_ptr() as *mut c_void,
                                                 decoder.as_mut() as *mut PngDec as *mut c_void,
                                                 PngDec::png_decoder_error_handler,
                                                 PngDec::png_decoder_custom_read_function){
                return Err(decoder.error.clone().expect("error missing").at(here!()));
            }
        }

        Ok(decoder)
    }


    fn read_header(&mut self) -> Result<()> {
        if self.error.is_some() {
            return Err(self.error.clone().unwrap());
        }
        if self.header_read {
            return Ok(());
        }
        if self.c_state_disposed {
            return Err(nerror!(ErrorKind::InvalidOperation, "LibPNG decoder disposed before call to read_header"))
        }

        let c_state = self.c_state.as_mut_ptr() as *mut c_void;

        if unsafe { !ffi::wrap_png_decode_image_info(c_state) } {
            return Err(self.error.clone().expect("error missing").at(here!()));
        }


        let mut w: u32 = 0;
        let mut h: u32 = 0;
        let mut uses_alpha = true;
        if unsafe {!ffi::wrap_png_decoder_get_info(c_state, &mut w, &mut h, &mut uses_alpha)}
        {
            return Err(self.error.clone().expect("error missing").at(here!()));
        }
        self.w = w;
        self.h = h;
        self.pixel_format = if uses_alpha { ffi::PixelFormat::Bgra32 } else { ffi::PixelFormat::Bgr32 };


        self.header_read = true;
        Ok(())
    }


    fn get_info(&mut self) -> Result<(u32,u32, ffi::PixelFormat)>{
        self.read_header()?;
        Ok((self.w, self.h, self.pixel_format))
    }

    fn read_frame(&mut self, canvas: *mut BitmapBgra) -> Result<()> {
        if self.c_state_disposed{
            return Err(nerror!(ErrorKind::InvalidOperation, "LibPNG decoder disposed before call to read_frame"))
        }

        self.read_header().map_err(|e| e.at(here!()))?;

        unsafe {
            if self.w != (*canvas).w || self.h != (*canvas).h {
                return Err(nerror!(ErrorKind::InvalidArgument, "Canvas not sized for decoded image"));
            }
        }

        let mut row_pointers = unsafe{ (*canvas).get_row_pointers()
            .map_err(|e| e.at(here!()))}?;


        unsafe {
            let c_state = self.c_state.as_mut_ptr() as *mut c_void;

            if !ffi::wrap_png_decode_finish(c_state, row_pointers.as_mut_ptr(), row_pointers.len(), (*canvas).fmt.bytes() * (*canvas).width()) {
                return Err(self.error.clone().expect("error missing").at(here!()));
            }


            let color_info = &*ffi::wrap_png_decoder_get_color_info(c_state);

            if !self.ignore_color_profile {

                let result = ColorTransformCache::transform_to_srgb(&mut *canvas, color_info, PixelFormat::BGRA_8, PixelFormat::BGRA_8)
                    .map_err(|e| e.at(here!()));
                if result.is_err() && !self.ignore_color_profile_errors{
                    return result;
                }
            }
        }

        self.dispose_codec()?;

        Ok(())
    }

    fn dispose_codec(&mut self) -> Result<()>{
        let c_state = self.c_state.as_mut_ptr() as *mut c_void;

        unsafe {
            if !ffi::wrap_png_decoder_destroy(c_state) {
                Err(self.error.clone().expect("error missing").at(here!()))
            }else{
                Ok(())
            }
        }
    }
}