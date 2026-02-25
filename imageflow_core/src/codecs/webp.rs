use crate::ffi;
use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::{Context, JsonResponse, Result};

use super::*;
use crate::graphics::bitmaps::{BitmapCompositing, ColorSpace};
use crate::io::IoProxy;
use crate::io::IoProxyProxy;
use imageflow_helpers::preludes::from_std::ptr::null;
use imageflow_types::collections::AddRemoveSet;
use imageflow_types::{IoDirection, PixelLayout};
use libwebp_sys::WEBP_CSP_MODE::MODE_BGRA;
use libwebp_sys::*;
use rgb::alt::BGRA8;
use std::any::Any;
use std::rc::Rc;
use uuid::Uuid;

pub struct WebPDecoder {
    io: IoProxy,
    bytes: Option<Vec<u8>>,
    config: WebPDecoderConfig,
    features_read: bool,
}

impl WebPDecoder {
    pub fn create(c: &Context, io: IoProxy, io_id: i32) -> Result<WebPDecoder> {
        Ok(WebPDecoder {
            io,
            bytes: None,
            config: WebPDecoderConfig::new().expect("Failed to initialize WebPDecoderConfig"),
            features_read: false,
        })
    }

    fn ensure_data_buffered(&mut self) -> Result<()> {
        if self.bytes.is_none() {
            let mut bytes = Vec::with_capacity(2048);
            let _ = self.io.read_to_end(&mut bytes).map_err(FlowError::from_decoder);
            self.bytes = Some(bytes);
        }
        Ok(())
    }

    pub fn input_width(&self) -> Option<i32> {
        if self.features_read {
            Some(self.config.input.width)
        } else {
            None
        }
    }

    pub fn has_animation(&self) -> Option<bool> {
        if self.features_read {
            Some(self.config.input.has_animation == 1)
        } else {
            None
        }
    }

    pub fn has_alpha(&self) -> Option<bool> {
        if self.features_read {
            Some(self.config.input.has_alpha == 1)
        } else {
            None
        }
    }
    pub fn is_lossless(&self) -> Option<bool> {
        if self.features_read {
            Some(self.config.input.format == 2) // 1= lossy, 0 = mixed/undefined
        } else {
            None
        }
    }
    pub fn input_height(&self) -> Option<i32> {
        if self.features_read {
            Some(self.config.input.height)
        } else {
            None
        }
    }
    pub fn output_width(&self) -> Option<i32> {
        if self.features_read && self.config.options.use_scaling == 1 {
            Some(self.config.options.scaled_width)
        } else {
            self.input_width()
        }
    }
    pub fn output_height(&self) -> Option<i32> {
        if self.features_read && self.config.options.use_scaling == 1 {
            Some(self.config.options.scaled_height)
        } else {
            self.input_height()
        }
    }

    fn ensure_features_read(&mut self) -> Result<()> {
        self.ensure_data_buffered()?;
        if !self.features_read {
            let buf = self.bytes.as_ref().unwrap(); //Cannot fail after ensure_data_buffered
            let len = buf.len();
            unsafe {
                let error_code = WebPGetFeatures(buf.as_ptr(), len, &mut self.config.input);
                if error_code != VP8StatusCode::VP8_STATUS_OK {
                    return Err(nerror!(
                        ErrorKind::ImageDecodingError,
                        "libwebp features decoding error {:?}",
                        error_code
                    ));
                }
            }
            self.features_read = true;
        }
        Ok(())
    }
}

impl Decoder for WebPDecoder {
    fn initialize(&mut self, c: &Context) -> Result<()> {
        Ok(())
    }

    fn get_scaled_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        self.ensure_features_read()?;

        Ok(s::ImageInfo {
            frame_decodes_into: if self.has_alpha().unwrap() {
                s::PixelFormat::Bgra32
            } else {
                s::PixelFormat::Bgr32
            },
            image_width: self.output_width().unwrap(),
            image_height: self.output_height().unwrap(),
            preferred_mime_type: "image/webp".to_owned(),
            preferred_extension: "webp".to_owned(),
            lossless: self.is_lossless().unwrap_or(false),
            multiple_frames: self.has_animation().unwrap_or(false),
        })
    }

    fn get_unscaled_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        self.ensure_features_read()?;

        Ok(s::ImageInfo {
            frame_decodes_into: if self.has_alpha().unwrap() {
                s::PixelFormat::Bgra32
            } else {
                s::PixelFormat::Bgr32
            },
            image_width: self.input_width().unwrap(),
            image_height: self.input_height().unwrap(),
            preferred_mime_type: "image/webp".to_owned(),
            preferred_extension: "webp".to_owned(),
            lossless: self.is_lossless().unwrap_or(false),
            multiple_frames: self.has_animation().unwrap_or(false),
        })
    }

    //Webp ignores exif rotation in Chrome, so we ignore it
    fn get_exif_rotation_flag(&mut self, c: &Context) -> Result<Option<i32>> {
        Ok(None)
    }

    fn tell_decoder(&mut self, c: &Context, tell: s::DecoderCommand) -> Result<()> {
        if let s::DecoderCommand::WebPDecoderHints(hints) = tell {
            self.config.options.use_scaling = 1;
            self.config.options.scaled_width = hints.width;
            self.config.options.scaled_height = hints.height;
        }
        Ok(())
    }

    fn read_frame(&mut self, c: &Context) -> Result<BitmapKey> {
        let _ = self.get_scaled_image_info(c)?;

        let w = self.output_width().unwrap();
        let h = self.output_height().unwrap();

        let mut bitmaps = c.borrow_bitmaps_mut().map_err(|e| e.at(here!()))?;

        let bitmap_key = bitmaps
            .create_bitmap_u8(
                w as u32,
                h as u32,
                PixelLayout::BGRA,
                false,
                self.has_alpha().unwrap(),
                ColorSpace::StandardRGB,
                BitmapCompositing::ReplaceSelf,
            )
            .map_err(|e| e.at(here!()))?;

        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;

        let mut window = bitmap.get_window_u8().unwrap();

        let stride = window.info().t_stride();
        let slice = window.slice_mut();
        let slice_len = slice.len();

        unsafe {
            // Specify the desired output colorspace:
            self.config.output.colorspace = MODE_BGRA;
            // Have config.output point to an external buffer:
            self.config.output.u.RGBA.rgba = slice.as_mut_ptr();
            self.config.output.u.RGBA.stride = stride as i32;
            self.config.output.u.RGBA.size = slice_len;
            self.config.output.is_external_memory = 1;

            let input_ptr = self.bytes.as_ref().unwrap().as_ptr();
            let input_len = self.bytes.as_ref().unwrap().len();

            let error_code = WebPDecode(input_ptr, input_len, &mut self.config);
            if error_code != VP8StatusCode::VP8_STATUS_OK {
                return Err(nerror!(
                    ErrorKind::ImageDecodingError,
                    "libwebp decoding error {:?}",
                    error_code
                ));
            }

            Ok(bitmap_key)
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
    io: IoProxy,
    quality: Option<f32>,
    lossless: Option<bool>,
    matte: Option<s::Color>,
}

impl WebPEncoder {
    pub(crate) fn create(
        c: &Context,
        io: IoProxy,
        quality: Option<f32>,
        lossless: Option<bool>,
        matte: Option<s::Color>,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::WebPEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The LodePNG encoder has been disabled"
            ));
        }
        if lossless == Some(true) && quality.is_some() {
            return Err(nerror!(
                ErrorKind::InvalidState,
                "Cannot specify both lossless=true and quality"
            ));
        }
        Ok(WebPEncoder { io, quality, lossless, matte })
    }
}

impl Encoder for WebPEncoder {
    fn write_frame(
        &mut self,
        c: &Context,
        _preset: &s::EncoderPreset,
        bitmap_key: BitmapKey,
        decoder_io_ids: &[i32],
    ) -> Result<s::EncodeResult> {
        let bitmaps = c.borrow_bitmaps().map_err(|e| e.at(here!()))?;
        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;

        if self.matte.is_some() {
            bitmap.apply_matte(self.matte.clone().unwrap())?;
        }

        let mut window = bitmap.get_window_u8().unwrap();

        let (w, h) = window.size_i32();
        let layout = window.info().pixel_layout();
        let stride = window.info().t_stride() as i32;
        window.normalize_unused_alpha()?;

        let mut_slice = window.slice_mut();
        let length = mut_slice.len();

        let lossless = self.lossless.unwrap_or(false);
        let quality = self.quality.unwrap_or(85.0).clamp(0.0, 100.0);

        unsafe {
            let mut output: *mut u8 = ptr::null_mut();
            let mut output_len: usize = 0;
            if !lossless {
                if layout == PixelLayout::BGRA {
                    output_len =
                        WebPEncodeBGRA(mut_slice.as_ptr(), w, h, stride, quality, &mut output);
                } else if layout == PixelLayout::BGR {
                    output_len =
                        WebPEncodeBGR(mut_slice.as_ptr(), w, h, stride, quality, &mut output);
                }
            } else if layout == PixelLayout::BGRA {
                output_len = WebPEncodeLosslessBGRA(mut_slice.as_ptr(), w, h, stride, &mut output);
            } else if layout == PixelLayout::BGR {
                output_len = WebPEncodeLosslessBGR(mut_slice.as_ptr(), w, h, stride, &mut output);
            }

            if output_len == 0 || output.is_null() {
                return Err(nerror!(ErrorKind::ImageEncodingError, "libwebp encoding error"));
            } else {
                let bytes = slice::from_raw_parts(output, output_len);
                let result =
                    self.io.write_all(bytes).map_err(|e| FlowError::from_encoder(e).at(here!()));
                WebPFree(output as *mut core::ffi::c_void);
                result?
            }
        }

        Ok(s::EncodeResult {
            w,
            h,
            io_id: self.io.io_id(),
            bytes: ::imageflow_types::ResultBytes::Elsewhere,
            preferred_extension: "webp".to_owned(),
            preferred_mime_type: "image/webp".to_owned(),
        })
    }

    fn into_io(self: Box<Self>) -> Result<IoProxy> {
        Ok(self.io)
    }
}
