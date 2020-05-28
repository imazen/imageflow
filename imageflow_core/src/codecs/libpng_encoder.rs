use super::Encoder;
use super::s::{EncoderPreset, EncodeResult};
use crate::io::IoProxy;
use crate::ffi::BitmapBgra;
use imageflow_types::PixelFormat;
use crate::{Context, Result, ErrorKind, FlowError};
use crate::io::IoProxyRef;
use std::slice;
use std::io::Write;
use std::rc::Rc;
use std::cell::RefCell;
use std::os::raw::{c_int, c_uint, c_ulong, c_char};
use libc;
use rgb;
use crate::ffi;
use imageflow_helpers::preludes::from_std::CStr;
use std::ffi::c_void;

pub struct LibPngEncoder {
    io: IoProxy,
    error: Option<FlowError>
}

impl LibPngEncoder {
    pub(crate) fn create(c: &Context, io: IoProxy) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::LibPngRsEncoder){
            return Err(nerror!(ErrorKind::CodecDisabledError, "The LibPNG encoder has been disabled"));
        }
        Ok(LibPngEncoder {
            io,
            error: None
        })
    }
}

impl Encoder for LibPngEncoder {
    fn write_frame(&mut self, c: &Context, preset: &EncoderPreset, frame: &mut BitmapBgra, decoder_io_ids: &[i32]) -> Result<EncodeResult> {
        self.write_png(frame, preset).map_err(|e| e.at(here!()))?;

        Ok(EncodeResult {
            w: frame.w as i32,
            h: frame.h as i32,
            io_id: self.io.io_id(),
            bytes: ::imageflow_types::ResultBytes::Elsewhere,
            preferred_extension: "png".to_owned(),
            preferred_mime_type: "image/png".to_owned(),
        })
    }

    fn get_io(&self) -> Result<IoProxyRef> {
        Ok(IoProxyRef::Borrow(&self.io))
    }
}

impl LibPngEncoder {
    #[no_mangle]
    extern "C" fn png_encoder_error_handler(png_ptr: *mut c_void, custom_state: *mut c_void,
                                    message: *const c_char) {
        let encoder = unsafe { &mut *(custom_state as *mut LibPngEncoder) };

        if encoder.error.is_none() {
            if !message.is_null() {
                let cstr = unsafe { CStr::from_ptr(message) };
                let message = cstr.to_str().expect("LibPNG error message was not UTF-8");

                // TODO: detect OOM and categorize them
                // if message.contains("OOM"){
                //     encoder.error = Some(nerror!(ErrorKind::OutOfMemory))
                // }

                encoder.error = Some(nerror!(ErrorKind::ImageDecodingError, "LibPNG encoding error: {}", message));
            }
        }
    }


    #[no_mangle]
    extern "C" fn png_encoder_custom_write_function(png_ptr: *mut c_void, custom_state: *mut c_void, buffer: *mut u8, buffer_length: usize) -> bool {
        let encoder: &mut LibPngEncoder = unsafe { &mut *(custom_state as *mut LibPngEncoder) };

        let buffer_slice = unsafe { std::slice::from_raw_parts(buffer, buffer_length) };

        return match encoder.io.write_all(buffer_slice) {
            Ok(()) => true,
            Err(err) => {
                encoder.error = Some(FlowError::from_encoder(err).at(here!()));
                false
            }
        }
    }

    pub fn write_png(&mut self, frame: &BitmapBgra, preset: &EncoderPreset) -> Result<()> {
        if let EncoderPreset::Libpng { depth, matte, zlib_compression } = preset {
            let rows = frame.get_row_pointers().map_err(|e| e.at(here!()))?;
            let disable_alpha = depth.unwrap_or(imageflow_types::PngBitDepth::Png32) == imageflow_types::PngBitDepth::Png24;
            unsafe {
                if !ffi::wrap_png_encoder_write_png(self as *mut LibPngEncoder as *mut c_void,
                                                    Self::png_encoder_error_handler,
                                                    Self::png_encoder_custom_write_function,
                                                    rows.as_ptr(),
                                                    frame.w as usize,
                                                    frame.h as usize,
                                                    disable_alpha,
                                                    zlib_compression.unwrap_or(6),
                                                    frame.fmt) {
                    Err(self.error.clone().expect("error missing").at(here!()))
                } else {
                    Ok(())
                }
            }
        } else {
            Err(nerror!(ErrorKind::InvalidArgument, "LibPngEncoder requires Libpng Encoder Preset"))
        }
    }
}
