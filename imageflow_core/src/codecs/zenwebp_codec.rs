use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::{Context, Result};

use super::*;
use crate::graphics::bitmaps::{BitmapCompositing, ColorSpace};
use crate::io::IoProxy;
use imageflow_types::PixelLayout;
use std::any::Any;
use std::io::Write;

// ============================================================================
// Decoder
// ============================================================================

pub struct ZenWebPDecoder {
    io: IoProxy,
    data: Option<Vec<u8>>,
    info: Option<zenwebp::ImageInfo>,
}

impl ZenWebPDecoder {
    pub fn create(_c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        Ok(ZenWebPDecoder { io, data: None, info: None })
    }

    fn ensure_data_buffered(&mut self) -> Result<()> {
        if self.data.is_none() {
            let mut bytes = Vec::with_capacity(8192);
            self.io.read_to_end(&mut bytes).map_err(FlowError::from_decoder)?;
            self.data = Some(bytes);
        }
        Ok(())
    }

    fn ensure_info_read(&mut self) -> Result<()> {
        self.ensure_data_buffered()?;
        if self.info.is_none() {
            let data = self.data.as_ref().unwrap();
            let info = zenwebp::ImageInfo::from_bytes(data).map_err(|e| {
                nerror!(ErrorKind::ImageDecodingError, "zenwebp header error: {}", e)
            })?;
            self.info = Some(info);
        }
        Ok(())
    }
}

impl Decoder for ZenWebPDecoder {
    fn initialize(&mut self, _c: &Context) -> Result<()> {
        Ok(())
    }

    fn get_unscaled_image_info(&mut self, _c: &Context) -> Result<s::ImageInfo> {
        self.ensure_info_read()?;
        let info = self.info.as_ref().unwrap();
        Ok(s::ImageInfo {
            frame_decodes_into: if info.has_alpha {
                s::PixelFormat::Bgra32
            } else {
                s::PixelFormat::Bgr32
            },
            image_width: info.width as i32,
            image_height: info.height as i32,
            preferred_mime_type: "image/webp".to_owned(),
            preferred_extension: "webp".to_owned(),
            lossless: !info.is_lossy,
            multiple_frames: info.has_animation,
        })
    }

    fn get_scaled_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        self.get_unscaled_image_info(c)
    }

    // WebP ignores EXIF rotation in Chrome, so we ignore it too
    fn get_exif_rotation_flag(&mut self, _c: &Context) -> Result<Option<i32>> {
        Ok(None)
    }

    fn tell_decoder(&mut self, _c: &Context, _tell: s::DecoderCommand) -> Result<()> {
        Ok(())
    }

    fn read_frame(&mut self, c: &Context) -> Result<BitmapKey> {
        self.ensure_data_buffered()?;
        let data = self.data.as_ref().unwrap();

        // Decode to BGRA using convenience function
        let (pixels, w, h) = zenwebp::decode_bgra(data)
            .map_err(|e| nerror!(ErrorKind::ImageDecodingError, "zenwebp decode error: {}", e))?;

        let has_alpha = self.info.as_ref().map(|i| i.has_alpha).unwrap_or(true);

        let mut bitmaps = c.borrow_bitmaps_mut().map_err(|e| e.at(here!()))?;
        let bitmap_key = bitmaps
            .create_bitmap_u8(
                w,
                h,
                PixelLayout::BGRA,
                false,
                has_alpha,
                ColorSpace::StandardRGB,
                BitmapCompositing::ReplaceSelf,
            )
            .map_err(|e| e.at(here!()))?;

        {
            let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;
            let mut window = bitmap.get_window_u8().unwrap();
            let dst_stride = window.info().t_stride() as usize;
            let src_stride = w as usize * 4;

            let dst = window.slice_mut();
            if dst_stride == src_stride {
                dst[..pixels.len()].copy_from_slice(&pixels);
            } else {
                for y in 0..h as usize {
                    let src_row = &pixels[y * src_stride..(y + 1) * src_stride];
                    let dst_row = &mut dst[y * dst_stride..y * dst_stride + src_stride];
                    dst_row.copy_from_slice(src_row);
                }
            }
        }

        Ok(bitmap_key)
    }

    fn has_more_frames(&mut self) -> Result<bool> {
        Ok(false) // TODO: support WebP animation
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}

// ============================================================================
// Encoder
// ============================================================================

pub struct ZenWebPEncoder {
    io: IoProxy,
    quality: Option<f32>,
    lossless: Option<bool>,
    matte: Option<s::Color>,
}

impl ZenWebPEncoder {
    pub(crate) fn create(
        c: &Context,
        io: IoProxy,
        quality: Option<f32>,
        lossless: Option<bool>,
        matte: Option<s::Color>,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::ZenWebPEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The ZenWebP encoder has been disabled"
            ));
        }
        if lossless == Some(true) && quality.is_some() {
            return Err(nerror!(
                ErrorKind::InvalidState,
                "Cannot specify both lossless=true and quality"
            ));
        }
        Ok(ZenWebPEncoder { io, quality, lossless, matte })
    }
}

impl Encoder for ZenWebPEncoder {
    fn write_frame(
        &mut self,
        c: &Context,
        _preset: &s::EncoderPreset,
        bitmap_key: BitmapKey,
        _decoder_io_ids: &[i32],
    ) -> Result<s::EncodeResult> {
        let bitmaps = c.borrow_bitmaps().map_err(|e| e.at(here!()))?;
        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;

        if self.matte.is_some() {
            bitmap.apply_matte(self.matte.clone().unwrap())?;
        }

        let mut window = bitmap.get_window_u8().unwrap();
        let (w, h) = (window.w(), window.h());
        let layout = window.info().pixel_layout();
        window.normalize_unused_alpha()?;

        let pixel_layout = match layout {
            PixelLayout::BGRA => zenwebp::PixelLayout::Bgra8,
            PixelLayout::BGR => zenwebp::PixelLayout::Bgr8,
            _ => {
                return Err(nerror!(
                    ErrorKind::ImageEncodingError,
                    "zenwebp: unsupported pixel layout {:?}",
                    layout
                ))
            }
        };

        let src_stride = window.info().t_stride() as usize;
        let pixel_bytes = match layout {
            PixelLayout::BGRA => 4,
            PixelLayout::BGR => 3,
            _ => 4,
        };
        let row_bytes = w as usize * pixel_bytes;
        let total_bytes = row_bytes * h as usize;

        // Borrow directly when strides match; copy only when padding exists
        let owned_buf: Vec<u8>;
        let pixels: &[u8] = if src_stride == row_bytes {
            &window.get_slice()[..total_bytes]
        } else {
            let slice = window.get_slice();
            let mut buf = Vec::with_capacity(total_bytes);
            for y in 0..h as usize {
                buf.extend_from_slice(&slice[y * src_stride..y * src_stride + row_bytes]);
            }
            owned_buf = buf;
            &owned_buf
        };

        let lossless = self.lossless.unwrap_or(false);
        let quality = self.quality.unwrap_or(85.0).clamp(0.0, 100.0);

        let webp_bytes = if lossless {
            let config = zenwebp::LosslessConfig::new().with_quality(quality).with_method(6);
            zenwebp::EncodeRequest::lossless(&config, pixels, pixel_layout, w, h).encode().map_err(
                |e| nerror!(ErrorKind::ImageEncodingError, "zenwebp lossless error: {}", e),
            )?
        } else {
            let config = zenwebp::LossyConfig::new().with_quality(quality).with_method(4);
            zenwebp::EncodeRequest::lossy(&config, pixels, pixel_layout, w, h)
                .encode()
                .map_err(|e| nerror!(ErrorKind::ImageEncodingError, "zenwebp lossy error: {}", e))?
        };

        self.io.write_all(&webp_bytes).map_err(|e| FlowError::from_encoder(e).at(here!()))?;

        Ok(s::EncodeResult {
            w: w as i32,
            h: h as i32,
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
