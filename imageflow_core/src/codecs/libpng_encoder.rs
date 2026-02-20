use super::s::{EncodeResult, EncoderPreset};
use super::Encoder;
use crate::io::IoProxy;

use crate::ffi;
use crate::graphics::bitmaps::{BitmapKey, BitmapWindowMut};
use crate::io::IoProxyRef;
use crate::{Context, ErrorKind, FlowError, Result};
use imageflow_helpers::preludes::from_std::CStr;
use imageflow_types::{Color, PixelFormat};
use std::cell::RefCell;
use std::ffi::c_void;
use std::io::Write;
use std::os::raw::{c_char, c_int, c_uint, c_ulong};
use std::rc::Rc;
use std::slice;

pub struct LibPngEncoder {
    io: IoProxy,
    error: Option<FlowError>,
    depth: Option<imageflow_types::PngBitDepth>,
    matte: Option<Color>,
    zlib_compression: Option<u8>,
}

impl LibPngEncoder {
    pub(crate) fn create(
        c: &Context,
        io: IoProxy,
        depth: Option<imageflow_types::PngBitDepth>,
        matte: Option<Color>,
        zlib_compression: Option<u8>,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::LibPngRsEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The LibPNG encoder has been disabled"
            ));
        }
        Ok(LibPngEncoder { io, error: None, depth, matte, zlib_compression })
    }
}

impl Encoder for LibPngEncoder {
    fn write_frame(
        &mut self,
        c: &Context,
        preset: &EncoderPreset,
        bitmap_key: BitmapKey,
        decoder_io_ids: &[i32],
    ) -> Result<EncodeResult> {
        let bitmaps = c.borrow_bitmaps().map_err(|e| e.at(here!()))?;

        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;

        if let Some(ref matte) = self.matte {
            bitmap.apply_matte(matte.clone()).map_err(|e| e.at(here!()))?;
        }

        let mut window = bitmap.get_window_u8().unwrap();

        self.write_png(&mut window, self.depth, self.zlib_compression)
            .map_err(|e| e.at(here!()))?;

        Ok(EncodeResult {
            w: window.w_i32(),
            h: window.h_i32(),
            io_id: self.io.io_id(),
            bytes: ::imageflow_types::ResultBytes::Elsewhere,
            preferred_extension: "png".to_owned(),
            preferred_mime_type: "image/png".to_owned(),
        })
    }

    fn get_io(&self) -> Result<IoProxyRef<'_>> {
        Ok(IoProxyRef::Borrow(&self.io))
    }

    fn into_io(self: Box<Self>) -> Result<IoProxy> {
        Ok(self.io)
    }
}

impl LibPngEncoder {
    #[no_mangle]
    extern "C" fn png_encoder_error_handler(
        png_ptr: *mut c_void,
        custom_state: *mut c_void,
        message: *const c_char,
    ) {
        if custom_state.is_null() {
            eprintln!("LibPNG encoder error handler called with null custom_state");
            return;
        }
        let encoder = unsafe { &mut *(custom_state as *mut LibPngEncoder) };

        if encoder.error.is_none() && !message.is_null() {
            let cstr = unsafe { CStr::from_ptr(message) };
            let message = cstr.to_str().expect("LibPNG error message was not UTF-8");

            // TODO: detect OOM and categorize them
            // if message.contains("OOM"){
            //     encoder.error = Some(nerror!(ErrorKind::OutOfMemory))
            // }

            encoder.error =
                Some(nerror!(ErrorKind::ImageDecodingError, "LibPNG encoding error: {}", message));
        }
    }

    #[no_mangle]
    extern "C" fn png_encoder_custom_write_function(
        png_ptr: *mut c_void,
        custom_state: *mut c_void,
        buffer: *mut u8,
        buffer_length: usize,
    ) -> bool {
        if custom_state.is_null() {
            eprintln!("LibPNG encoder custom write function called with null custom_state");
            return false;
        }
        let encoder: &mut LibPngEncoder = unsafe { &mut *(custom_state as *mut LibPngEncoder) };

        if buffer.is_null() {
            eprintln!("LibPNG encoder custom write function called with null buffer");
            return false;
        }
        let buffer_slice = unsafe { std::slice::from_raw_parts(buffer, buffer_length) };

        match encoder.io.write_all(buffer_slice) {
            Ok(()) => true,
            Err(err) => {
                encoder.error = Some(FlowError::from_encoder(err).at(here!()));
                false
            }
        }
    }

    pub fn write_png(
        &mut self,
        frame: &mut BitmapWindowMut<u8>,
        depth: Option<imageflow_types::PngBitDepth>,
        zlib_compression: Option<u8>,
    ) -> Result<()> {
        let rows = frame.create_row_pointers().map_err(|e| e.at(here!()))?;
        let disable_alpha = depth.unwrap_or(imageflow_types::PngBitDepth::Png32)
            == imageflow_types::PngBitDepth::Png24;
        unsafe {
            if !ffi::wrap_png_encoder_write_png(
                self as *mut LibPngEncoder as *mut c_void,
                Self::png_encoder_error_handler,
                Self::png_encoder_custom_write_function,
                rows.rows.as_ptr(),
                rows.w,
                rows.h,
                disable_alpha,
                zlib_compression.unwrap_or(6) as i32,
                frame.pixel_format(),
            ) {
                Err(self.error.clone().expect("error missing").at(here!()))
            } else {
                Ok(())
            }
        }
    }
}
