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

pub struct ZenJxlDecoder {
    io: IoProxy,
    data: Option<Vec<u8>>,
    info: Option<zenjxl::JxlInfo>,
}

impl ZenJxlDecoder {
    pub fn create(_c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        Ok(ZenJxlDecoder { io, data: None, info: None })
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
            let info = zenjxl::probe(data)
                .map_err(|e| nerror!(ErrorKind::ImageDecodingError, "zenjxl probe error: {}", e))?;
            self.info = Some(info);
        }
        Ok(())
    }
}

impl Decoder for ZenJxlDecoder {
    fn initialize(&mut self, _c: &Context) -> Result<()> {
        Ok(())
    }

    fn get_unscaled_image_info(&mut self, _c: &Context) -> Result<s::ImageInfo> {
        self.ensure_info_read()?;
        let info = self.info.as_ref().unwrap();
        Ok(s::ImageInfo {
            frame_decodes_into: s::PixelFormat::Bgra32,
            image_width: info.width as i32,
            image_height: info.height as i32,
            preferred_mime_type: "image/jxl".to_owned(),
            preferred_extension: "jxl".to_owned(),
            lossless: false, // JXL can be either; we don't know until decode
            multiple_frames: info.has_animation,
        })
    }

    fn get_scaled_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        self.get_unscaled_image_info(c)
    }

    fn get_exif_rotation_flag(&mut self, _c: &Context) -> Result<Option<i32>> {
        self.ensure_info_read()?;
        let info = self.info.as_ref().unwrap();
        if info.orientation == 1 {
            Ok(None)
        } else {
            Ok(Some(info.orientation as i32))
        }
    }

    fn tell_decoder(&mut self, _c: &Context, _tell: s::DecoderCommand) -> Result<()> {
        Ok(())
    }

    fn read_frame(&mut self, c: &Context) -> Result<BitmapKey> {
        self.ensure_data_buffered()?;
        let data = self.data.as_ref().unwrap();

        // Compute limits from security settings
        let limit = c.security.max_decode_size.as_ref().or(c.security.max_frame_size.as_ref());
        let jxl_limits = if let Some(limit) = limit {
            let max_pixels = (limit.megapixels * 1_000_000.0) as u64;
            let max_bytes = max_pixels * 4;
            Some(zenjxl::JxlLimits {
                max_pixels: Some(max_pixels),
                max_memory_bytes: Some(max_bytes),
            })
        } else {
            None
        };

        // Request BGRA8 output for direct copy into imageflow bitmaps
        let preferred = [zenpixels::PixelDescriptor::BGRA8];
        let output = zenjxl::decode(data, jxl_limits.as_ref(), &preferred)
            .map_err(|e| nerror!(ErrorKind::ImageDecodingError, "zenjxl decode error: {}", e))?;

        let w = output.info.width;
        let h = output.info.height;
        let has_alpha = output.info.has_alpha;

        // Get raw pixel bytes
        let pixel_bytes = output.pixels.copy_to_contiguous_bytes();
        let desc = output.pixels.descriptor();

        // Check if we got BGRA8 as requested
        let is_bgra = desc.layout() == zenpixels::ChannelLayout::Bgra
            && desc.channel_type() == zenpixels::ChannelType::U8;

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
            let dst = window.slice_mut();

            if is_bgra {
                // Direct copy — decoder already produced BGRA8
                let src_stride = w as usize * 4;
                if dst_stride == src_stride {
                    dst[..pixel_bytes.len()].copy_from_slice(&pixel_bytes);
                } else {
                    for y in 0..h as usize {
                        let src_row = &pixel_bytes[y * src_stride..(y + 1) * src_stride];
                        let dst_row = &mut dst[y * dst_stride..y * dst_stride + src_stride];
                        dst_row.copy_from_slice(src_row);
                    }
                }
            } else {
                // Fallback: convert RGBA→BGRA (or RGB→BGRA)
                let channels = desc.channels() as usize;
                let src_stride = w as usize * channels;
                for y in 0..h as usize {
                    for x in 0..w as usize {
                        let si = y * src_stride + x * channels;
                        let di = y * dst_stride + x * 4;
                        if channels >= 4 {
                            // RGBA → BGRA
                            dst[di] = pixel_bytes[si + 2]; // B
                            dst[di + 1] = pixel_bytes[si + 1]; // G
                            dst[di + 2] = pixel_bytes[si]; // R
                            dst[di + 3] = pixel_bytes[si + 3]; // A
                        } else if channels == 3 {
                            // RGB → BGRA
                            dst[di] = pixel_bytes[si + 2]; // B
                            dst[di + 1] = pixel_bytes[si + 1]; // G
                            dst[di + 2] = pixel_bytes[si]; // R
                            dst[di + 3] = 255; // A
                        } else {
                            // Gray → BGRA
                            let v = pixel_bytes[si];
                            dst[di] = v;
                            dst[di + 1] = v;
                            dst[di + 2] = v;
                            dst[di + 3] = 255;
                        }
                    }
                }
            }
        }

        Ok(bitmap_key)
    }

    fn has_more_frames(&mut self) -> Result<bool> {
        Ok(false) // TODO: support JXL animation
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}

// ============================================================================
// Encoder
// ============================================================================

pub struct ZenJxlEncoder {
    io: IoProxy,
    distance: Option<f32>,
    lossless: bool,
}

impl ZenJxlEncoder {
    pub(crate) fn create(
        c: &Context,
        io: IoProxy,
        distance: Option<f32>,
        lossless: bool,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::ZenJxlEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The ZenJxl encoder has been disabled"
            ));
        }
        Ok(ZenJxlEncoder { io, distance, lossless })
    }
}

impl Encoder for ZenJxlEncoder {
    fn write_frame(
        &mut self,
        c: &Context,
        _preset: &s::EncoderPreset,
        bitmap_key: BitmapKey,
        _decoder_io_ids: &[i32],
    ) -> Result<s::EncodeResult> {
        let bitmaps = c.borrow_bitmaps().map_err(|e| e.at(here!()))?;
        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;
        let window = bitmap.get_window_u8().unwrap();
        let (w, h) = (window.w(), window.h());
        let stride = window.info().t_stride() as usize;
        let fmt = window.pixel_format();

        let slice = window.get_slice();

        // Build contiguous BGRA pixel buffer for encoding
        let row_bytes = w as usize * 4;
        let bgra_pixels: Vec<rgb::alt::BGRA<u8>> = match fmt {
            crate::ffi::PixelFormat::Bgra32 | crate::ffi::PixelFormat::Bgr32 => {
                let mut pixels = Vec::with_capacity(w as usize * h as usize);
                for y in 0..h as usize {
                    let row = &slice[y * stride..y * stride + row_bytes];
                    for pix in row.chunks_exact(4) {
                        pixels.push(rgb::alt::BGRA {
                            b: pix[0],
                            g: pix[1],
                            r: pix[2],
                            a: if fmt == crate::ffi::PixelFormat::Bgra32 { pix[3] } else { 255 },
                        });
                    }
                }
                pixels
            }
            crate::ffi::PixelFormat::Bgr24 => {
                let row_bytes_24 = w as usize * 3;
                let mut pixels = Vec::with_capacity(w as usize * h as usize);
                for y in 0..h as usize {
                    let row = &slice[y * stride..y * stride + row_bytes_24];
                    for pix in row.chunks_exact(3) {
                        pixels.push(rgb::alt::BGRA { b: pix[0], g: pix[1], r: pix[2], a: 255 });
                    }
                }
                pixels
            }
            other => {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "PixelFormat {:?} not supported for JXL encoding",
                    other
                ));
            }
        };

        let img = imgref::ImgRef::new(&bgra_pixels, w as usize, h as usize);

        let jxl_bytes = if self.lossless {
            let config = zenjxl::LosslessConfig::new();
            zenjxl::encode_bgra8_lossless(img, &config).map_err(|e| {
                nerror!(ErrorKind::ImageEncodingError, "zenjxl lossless error: {}", e)
            })?
        } else {
            let distance = self.distance.unwrap_or(1.0);
            let config = zenjxl::LossyConfig::new(distance);
            zenjxl::encode_bgra8(img, &config)
                .map_err(|e| nerror!(ErrorKind::ImageEncodingError, "zenjxl lossy error: {}", e))?
        };

        self.io.write_all(&jxl_bytes).map_err(|e| FlowError::from_encoder(e).at(here!()))?;

        Ok(s::EncodeResult {
            w: w as i32,
            h: h as i32,
            io_id: self.io.io_id(),
            bytes: ::imageflow_types::ResultBytes::Elsewhere,
            preferred_extension: "jxl".to_owned(),
            preferred_mime_type: "image/jxl".to_owned(),
        })
    }

    fn into_io(self: Box<Self>) -> Result<IoProxy> {
        Ok(self.io)
    }
}
