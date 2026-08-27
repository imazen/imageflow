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
use std::io::Read;
use std::rc::Rc;
use uuid::Uuid;

pub struct WebPDecoder {
    io: IoProxy,
    bytes: Option<Vec<u8>>,
    config: WebPDecoderConfig,
    features_read: bool,
    max_input_file_bytes: Option<usize>,
}

impl WebPDecoder {
    pub fn create(c: &Context, io: IoProxy, io_id: i32) -> Result<WebPDecoder> {
        Ok(WebPDecoder {
            io,
            bytes: None,
            config: WebPDecoderConfig::new().expect("Failed to initialize WebPDecoderConfig"),
            features_read: false,
            max_input_file_bytes: c.security.max_input_file_bytes,
        })
    }

    fn ensure_data_buffered(&mut self) -> Result<()> {
        if self.bytes.is_none() {
            let mut bytes = Vec::with_capacity(2048);
            if let Some(max_bytes) = self.max_input_file_bytes {
                let bytes_read = Read::by_ref(&mut self.io)
                    .take(max_bytes as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(FlowError::from_decoder)?;
                if bytes_read > max_bytes {
                    return Err(nerror!(
                        ErrorKind::ImageDecodingError,
                        "WebP input exceeds maximum of {} bytes",
                        max_bytes
                    ));
                }
            } else {
                self.io.read_to_end(&mut bytes).map_err(FlowError::from_decoder)?;
            }
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
    /// Frames written so far. The first frame decides the mode: if an input
    /// decoder reports more frames to come, an animated WebP is assembled via
    /// libwebp's WebPAnimEncoder (issue #606); otherwise the still path below
    /// is used unchanged (byte-identical output for single-frame inputs).
    frames_written: u32,
    anim: Option<AnimEncoderState>,
}

/// State for the animated (multi-frame) libwebp path.
struct AnimEncoderState {
    enc: *mut WebPAnimEncoder,
    w: i32,
    h: i32,
    /// Timestamp (ms) at which the next frame starts.
    next_timestamp_ms: i32,
}

impl Drop for AnimEncoderState {
    fn drop(&mut self) {
        if !self.enc.is_null() {
            unsafe { WebPAnimEncoderDelete(self.enc) };
            self.enc = ptr::null_mut();
        }
    }
}

/// Delay (ms) of the frame most recently produced by one of the input decoders,
/// and whether that decoder still has frames to produce. Defaults to 100 ms /
/// false when no input decoder exposes frame timing.
fn input_frame_info(c: &Context, decoder_io_ids: &[i32]) -> (i32, bool) {
    for io_id in decoder_io_ids {
        if let Ok(mut codec) = c.get_codec(*io_id)
            && let Ok(decoder) = codec.get_decoder()
        {
            let more = decoder.has_more_frames().unwrap_or(false);
            let mut delay_ms = 100;
            if let Some(gif) = decoder.as_any().downcast_ref::<super::gif::GifDecoder>()
                && let Some(frame) = gif.current_frame()
            {
                // GIF delays are in centiseconds
                delay_ms = i32::from(frame.delay) * 10;
            }
            #[cfg(feature = "zen-codecs")]
            if let Some(zd) = decoder.as_any().downcast_ref::<super::zen_decoder::ZenDecoder>()
                && let Some(d) = zd.last_frame_delay()
            {
                delay_ms = d as i32 * 10;
            }
            return (delay_ms.max(0), more);
        }
    }
    (100, false)
}

/// Loop count for the animation, from the first GIF input decoder (0 = infinite).
fn input_loop_count(c: &Context, decoder_io_ids: &[i32]) -> i32 {
    for io_id in decoder_io_ids {
        if let Ok(mut codec) = c.get_codec(*io_id)
            && let Ok(decoder) = codec.get_decoder()
            && let Some(gif) = decoder.as_any().downcast_ref::<super::gif::GifDecoder>()
        {
            return match gif.get_repeat() {
                Some(::gif::Repeat::Finite(n)) => i32::from(n),
                _ => 0,
            };
        }
    }
    0
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
        Ok(WebPEncoder { io, quality, lossless, matte, frames_written: 0, anim: None })
    }

    fn webp_config(&self) -> Result<WebPConfig> {
        let mut config = std::mem::MaybeUninit::<WebPConfig>::uninit();
        // SAFETY: WebPConfigInitInternal fully initializes the struct on success.
        let ok = unsafe {
            WebPConfigInitInternal(
                config.as_mut_ptr(),
                WebPPreset::WEBP_PRESET_DEFAULT,
                self.quality.unwrap_or(85.0).clamp(0.0, 100.0),
                WEBP_ENCODER_ABI_VERSION as i32,
            )
        };
        if ok == 0 {
            return Err(nerror!(ErrorKind::ImageEncodingError, "WebPConfigInit failed"));
        }
        let mut config = unsafe { config.assume_init() };
        if self.lossless.unwrap_or(false) {
            // Mirror the simple lossless API: lossless with exact RGB under alpha.
            config.lossless = 1;
            config.exact = 1;
        }
        Ok(config)
    }

    /// Start the animated path with the canvas size of the first frame.
    fn start_animation(
        &mut self,
        c: &Context,
        w: i32,
        h: i32,
        decoder_io_ids: &[i32],
    ) -> Result<()> {
        let mut options = std::mem::MaybeUninit::<WebPAnimEncoderOptions>::uninit();
        // SAFETY: WebPAnimEncoderOptionsInitInternal initializes every field on success.
        let ok = unsafe {
            WebPAnimEncoderOptionsInitInternal(options.as_mut_ptr(), WEBP_MUX_ABI_VERSION as i32)
        };
        if ok == 0 {
            return Err(nerror!(
                ErrorKind::ImageEncodingError,
                "WebPAnimEncoderOptionsInit failed"
            ));
        }
        let mut options = unsafe { options.assume_init() };
        options.anim_params.loop_count = input_loop_count(c, decoder_io_ids);
        let enc =
            unsafe { WebPAnimEncoderNewInternal(w, h, &options, WEBP_MUX_ABI_VERSION as i32) };
        if enc.is_null() {
            return Err(nerror!(ErrorKind::ImageEncodingError, "WebPAnimEncoderNew failed"));
        }
        self.anim = Some(AnimEncoderState { enc, w, h, next_timestamp_ms: 0 });
        Ok(())
    }

    /// Add one frame to the animation. `pixels` is a strided BGRA/BGR window.
    fn add_animation_frame(
        &mut self,
        pixels: &[u8],
        w: i32,
        h: i32,
        stride: i32,
        layout: PixelLayout,
        delay_ms: i32,
    ) -> Result<()> {
        let config = self.webp_config()?;
        let anim = self
            .anim
            .as_mut()
            .ok_or_else(|| nerror!(ErrorKind::InternalError, "animation encoder not started"))?;
        if (w, h) != (anim.w, anim.h) {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "Animated WebP frames must all be {}x{}, got {}x{}",
                anim.w,
                anim.h,
                w,
                h
            ));
        }
        let mut pic = std::mem::MaybeUninit::<WebPPicture>::uninit();
        // SAFETY: WebPPictureInitInternal initializes every field on success.
        let ok =
            unsafe { WebPPictureInitInternal(pic.as_mut_ptr(), WEBP_ENCODER_ABI_VERSION as i32) };
        if ok == 0 {
            return Err(nerror!(ErrorKind::ImageEncodingError, "WebPPictureInit failed"));
        }
        let mut pic = unsafe { pic.assume_init() };
        pic.use_argb = 1;
        pic.width = w;
        pic.height = h;
        // SAFETY: `pixels` covers h rows of `stride` bytes in the given layout; the
        // import copies into picture-owned memory, freed by WebPPictureFree below.
        let imported = unsafe {
            match layout {
                PixelLayout::BGRA => WebPPictureImportBGRA(&mut pic, pixels.as_ptr(), stride),
                PixelLayout::BGR => WebPPictureImportBGR(&mut pic, pixels.as_ptr(), stride),
                other => {
                    return Err(nerror!(
                        ErrorKind::InvalidArgument,
                        "PixelLayout {:?} not supported for WebP encoding",
                        other
                    ))
                }
            }
        };
        if imported == 0 {
            return Err(nerror!(ErrorKind::ImageEncodingError, "WebPPictureImport failed"));
        }
        let added =
            unsafe { WebPAnimEncoderAdd(anim.enc, &mut pic, anim.next_timestamp_ms, &config) };
        unsafe { WebPPictureFree(&mut pic) };
        if added == 0 {
            let msg = unsafe { std::ffi::CStr::from_ptr(WebPAnimEncoderGetError(anim.enc)) }
                .to_string_lossy()
                .into_owned();
            return Err(nerror!(
                ErrorKind::ImageEncodingError,
                "WebPAnimEncoderAdd failed: {}",
                msg
            ));
        }
        anim.next_timestamp_ms = anim.next_timestamp_ms.saturating_add(delay_ms);
        Ok(())
    }

    /// Flush the animation and write the assembled WebP to the output.
    fn finish_animation(&mut self) -> Result<()> {
        let Some(anim) = self.anim.take() else { return Ok(()) };
        // A NULL frame marks the end of the last frame's duration.
        let flushed = unsafe {
            WebPAnimEncoderAdd(anim.enc, ptr::null_mut(), anim.next_timestamp_ms, ptr::null())
        };
        if flushed == 0 {
            let msg = unsafe { std::ffi::CStr::from_ptr(WebPAnimEncoderGetError(anim.enc)) }
                .to_string_lossy()
                .into_owned();
            return Err(nerror!(
                ErrorKind::ImageEncodingError,
                "WebPAnimEncoderAdd(flush) failed: {}",
                msg
            ));
        }
        let mut data = WebPData { bytes: ptr::null(), size: 0 };
        let assembled = unsafe { WebPAnimEncoderAssemble(anim.enc, &mut data) };
        if assembled == 0 || data.bytes.is_null() || data.size == 0 {
            let msg = unsafe { std::ffi::CStr::from_ptr(WebPAnimEncoderGetError(anim.enc)) }
                .to_string_lossy()
                .into_owned();
            return Err(nerror!(
                ErrorKind::ImageEncodingError,
                "WebPAnimEncoderAssemble failed: {}",
                msg
            ));
        }
        // SAFETY: data.bytes/size come from libwebp and stay valid until WebPFree.
        let result = unsafe {
            let bytes = slice::from_raw_parts(data.bytes, data.size);
            let r = self.io.write_all(bytes).map_err(|e| FlowError::from_encoder(e).at(here!()));
            WebPFree(data.bytes as *mut core::ffi::c_void);
            r
        };
        drop(anim); // WebPAnimEncoderDelete
        result
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

        // Animated path (issue #606): decoders pre-read the next frame, so on the
        // first frame we already know whether more are coming.
        let (delay_ms, more_frames) = input_frame_info(c, decoder_io_ids);
        if self.frames_written == 0 && more_frames {
            self.start_animation(c, w, h, decoder_io_ids)?;
        }
        if self.anim.is_some() {
            self.add_animation_frame(mut_slice, w, h, stride, layout, delay_ms)?;
            self.frames_written += 1;
            return Ok(s::EncodeResult {
                w,
                h,
                io_id: self.io.io_id(),
                bytes: ::imageflow_types::ResultBytes::Elsewhere,
                preferred_extension: "webp".to_owned(),
                preferred_mime_type: "image/webp".to_owned(),
            });
        }
        if self.frames_written > 0 {
            return Err(nerror!(
                ErrorKind::InvalidState,
                "libwebp encoder already wrote a still WebP; cannot append frame {}",
                self.frames_written + 1
            ));
        }
        self.frames_written += 1;

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

    fn into_io(mut self: Box<Self>) -> Result<IoProxy> {
        self.finish_animation()?;
        Ok(self.io)
    }
}
