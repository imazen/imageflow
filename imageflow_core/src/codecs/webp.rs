use std;
use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::ffi;
use crate::{Context, CError,  Result, JsonResponse};
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
use libwebp_sys::*;
use libwebp_sys::WEBP_CSP_MODE::MODE_BGRA;
use imageflow_helpers::preludes::from_std::ptr::null;


pub struct WebPDecoder{
    io:  IoProxy,
    bytes: Option<Vec<u8>>,
    config: WebPDecoderConfig,
    features_read: bool
}

impl WebPDecoder {
    pub fn create(c: &Context, io: IoProxy, io_id: i32) -> Result<WebPDecoder> {
        Ok(WebPDecoder{
            io,
            bytes: None,
            config: WebPDecoderConfig::new()
                .expect("Failed to initialize WebPDecoderConfig"),
            features_read: false
        })
    }

    pub fn ensure_data_buffered(&mut self, c: &Context) -> Result<()>{
        if self.bytes.is_none() {
            let mut bytes = Vec::with_capacity(2048);
            let _ = self.io.read_to_end(&mut bytes).map_err(|e| FlowError::from_decoder(e));
            self.bytes = Some(bytes);
        }
        Ok(())
    }

    pub fn input_width(&self) -> Option<i32>{
        if self.features_read {
            Some(self.config.input.width)
        }else{
            None
        }
    }
    pub fn input_height(&self) -> Option<i32>{
        if self.features_read {
            Some(self.config.input.height)
        }else{
            None
        }
    }
    pub fn output_width(&self) -> Option<i32>{
        if self.features_read && self.config.options.use_scaling == 1{
            Some(self.config.options.scaled_width)
        }else{
            self.input_width()
        }
    }
    pub fn output_height(&self) -> Option<i32>{
        if self.features_read && self.config.options.use_scaling == 1{
            Some(self.config.options.scaled_height)
        }else{
            self.input_height()
        }
    }
}


impl Decoder for WebPDecoder {
    fn initialize(&mut self, c: &Context) -> Result<()> {
        Ok(())
    }


    fn get_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        self.ensure_data_buffered(c)?;
        if !self.features_read {
            let buf = &self.bytes.as_ref().unwrap(); //Cannot fail after ensure_data_buffered
            let len = buf.len();
            unsafe {
                let error_code = WebPGetFeatures(buf.as_ptr(), len, &mut self.config.input);
                if error_code != VP8StatusCode::VP8_STATUS_OK {
                    return Err(nerror!(ErrorKind::ImageDecodingError, "libwebp features decoding error {:?}", error_code));
                }
            }
            self.features_read = true;
        }
        Ok(s::ImageInfo {
            frame_decodes_into: s::PixelFormat::Bgra32,
            image_width: self.input_width().unwrap(),
            image_height: self.input_height().unwrap(),
            preferred_mime_type: "image/webp".to_owned(),
            preferred_extension: "webp".to_owned()
        })
    }

    //Webp ignores exif rotation in Chrome, so we ignore it
    fn get_exif_rotation_flag(&mut self, c: &Context) -> Result<Option<i32>> {
        Ok(None)
    }

    fn tell_decoder(&mut self, c: &Context, tell: s::DecoderCommand) -> Result<()> {
        if let s::DecoderCommand::WebPDecoderHints(hints) = tell{
            self.config.options.use_scaling = 1;
            self.config.options.scaled_width = hints.width;
            self.config.options.scaled_height = hints.height;
        }
        Ok(())
    }

    fn read_frame(&mut self, c: &Context) -> Result<*mut BitmapBgra> {
        let _ = self.get_image_info(c)?;

        unsafe {
            let w = self.output_width().unwrap();
            let h = self.output_height().unwrap();
            let copy = ffi::flow_bitmap_bgra_create(c.flow_c(), w as i32, h as i32, false, ffi::PixelFormat::Bgra32);
            if copy.is_null() {
                cerror!(c).panic();
            }


            // Specify the desired output colorspace:
            self.config.output.colorspace = MODE_BGRA;
            // Have config.output point to an external buffer:
            self.config.output.u.RGBA.rgba = (*copy).pixels;
            self.config.output.u.RGBA.stride = (*copy).stride as i32;
            self.config.output.u.RGBA.size = (*copy).stride as usize * (*copy).h as usize;
            self.config.output.is_external_memory = 1;


            let len = self.bytes.as_ref().unwrap().len();

            let error_code = WebPDecode(self.bytes.as_ref().unwrap().as_ptr(), len, &mut self.config);
            if error_code != VP8StatusCode::VP8_STATUS_OK {
                return Err(nerror!(ErrorKind::ImageDecodingError, "libwebp decoding error {:?}", error_code));
            }

            Ok(copy)
        }
    }
    fn has_more_frames(&mut self) -> Result<bool> {
        Ok(false) // TODO: support webp animation
    }
    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}


pub struct WebPEncoder {
    io: IoProxy
}

impl WebPEncoder {
    pub(crate) fn create(c: &Context, io: IoProxy) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::WebPEncoder){
            return Err(nerror!(ErrorKind::CodecDisabledError, "The LodePNG encoder has been disabled"));
        }
        Ok(WebPEncoder {
            io
        })
    }
}

impl Encoder for WebPEncoder {
    fn write_frame(&mut self, c: &Context, preset: &s::EncoderPreset, frame: &mut BitmapBgra, decoder_io_ids: &[i32]) -> Result<s::EncodeResult> {

        unsafe {
            let mut output: *mut u8 = ptr::null_mut();
            let mut output_len: usize = 0;

            match preset {
                s::EncoderPreset::WebPLossy { quality } => {
                    let quality = f32::min(100f32,f32::max(0f32,*quality));
                    if frame.fmt == ffi::PixelFormat::Bgra32 || frame.fmt == ffi::PixelFormat::Bgr32{
                        if frame.fmt == ffi::PixelFormat::Bgr32{
                            frame.normalize_alpha()?;
                        }

                        output_len = WebPEncodeBGRA(frame.pixels, frame.width() as i32, frame.height() as i32, frame.stride() as i32, quality, &mut output);
                    }else if frame.fmt == ffi::PixelFormat::Bgr24{
                        output_len = WebPEncodeBGR(frame.pixels, frame.width() as i32, frame.height() as i32, frame.stride() as i32, quality, &mut output);
                    }

                },
                s::EncoderPreset::WebPLossless => {
                    if frame.fmt == ffi::PixelFormat::Bgra32 || frame.fmt == ffi::PixelFormat::Bgr32{
                        if frame.fmt == ffi::PixelFormat::Bgr32{
                            frame.normalize_alpha()?;
                        }
                        output_len = WebPEncodeLosslessBGRA(frame.pixels, frame.width() as i32, frame.height() as i32, frame.stride() as i32,  &mut output);
                    }else if frame.fmt == ffi::PixelFormat::Bgr24{
                        output_len = WebPEncodeLosslessBGR(frame.pixels, frame.width() as i32, frame.height() as i32, frame.stride() as i32,  &mut output);
                    }

                },
                _ => {
                    panic!("Incorrect encoder for encoder preset")
                }
            }
            if output_len == 0 {
                return Err(nerror!(ErrorKind::ImageEncodingError, "libwebp encoding error"));
            } else {
                let bytes = slice::from_raw_parts(output, output_len);
                self.io.write_all(bytes).map_err(|e| FlowError::from_encoder(e).at(here!()))?;
                WebPFree(output as *mut libc::c_void);
            }
        }

        Ok(s::EncodeResult {
            w: frame.w as i32,
            h: frame.h as i32,
            io_id: self.io.io_id(),
            bytes: ::imageflow_types::ResultBytes::Elsewhere,
            preferred_extension: "webp".to_owned(),
            preferred_mime_type: "image/webp".to_owned(),
        })
    }

    fn get_io(&self) -> Result<IoProxyRef> {
        Ok(IoProxyRef::Borrow(&self.io))
    }
}
