//! # Bad AVIF Decoder - FOR INTEGRATION TESTING ONLY
//!
//! **DO NOT USE IN PRODUCTION.**
//!
//! This decoder exists solely for verifying AVIF encoding roundtrips in integration tests.
//! It has significant limitations that make it unsuitable for production use:
//!
//! ## Missing Color Management
//! - **No ICC color profile handling**: Images with embedded ICC profiles will have incorrect colors
//! - **No CICP/NCLX color space conversion**: The decoder ignores color space metadata entirely,
//!   meaning colors may be significantly wrong for images using non-sRGB color spaces
//! - All pixels are assumed to be sRGB, which is often incorrect
//!
//! ## Security Concerns
//! - **Increased attack surface**: The avif-decode and aom-decode dependencies add significant
//!   native code that has not been audited for security vulnerabilities
//! - **Not fuzzing-tested**: Unlike the main imageflow decoders, this has not been subjected
//!   to extensive fuzzing or security review
//! - AVIF/AV1 decoders have historically been a source of security vulnerabilities
//!
//! ## Correct Usage
//! Only enable via the `bad-avif-decoder` feature flag, and only in test code:
//! ```ignore
//! context.enabled_codecs.enable_bad_avif_decoder();
//! ```

use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::{Context, ErrorKind, FlowError, Result};

use super::*;
use crate::graphics::bitmaps::{BitmapCompositing, ColorSpace};
use crate::io::IoProxy;
use imageflow_types::PixelLayout;
use std::any::Any;

pub struct AvifDecoder {
    io: IoProxy,
    bytes: Option<Vec<u8>>,
    width: Option<u32>,
    height: Option<u32>,
    has_alpha: Option<bool>,
}

impl AvifDecoder {
    pub fn create(_c: &Context, io: IoProxy, _io_id: i32) -> Result<AvifDecoder> {
        Ok(AvifDecoder {
            io,
            bytes: None,
            width: None,
            height: None,
            has_alpha: None,
        })
    }

    fn ensure_data_buffered(&mut self) -> Result<()> {
        if self.bytes.is_none() {
            let mut bytes = Vec::with_capacity(2048);
            let _ = self.io.read_to_end(&mut bytes).map_err(FlowError::from_decoder)?;
            self.bytes = Some(bytes);
        }
        Ok(())
    }

    fn ensure_header_read(&mut self) -> Result<()> {
        self.ensure_data_buffered()?;
        if self.width.is_none() {
            let bytes = self.bytes.as_ref().unwrap();
            let decoder = avif_decode::Decoder::from_avif(bytes)
                .map_err(|e| nerror!(ErrorKind::ImageDecodingError, "AVIF parse error: {:?}", e))?;

            let image = decoder
                .to_image()
                .map_err(|e| nerror!(ErrorKind::ImageDecodingError, "AVIF decode error: {:?}", e))?;

            let (w, h, alpha) = match &image {
                avif_decode::Image::Rgb8(img) => (img.width() as u32, img.height() as u32, false),
                avif_decode::Image::Rgb16(img) => (img.width() as u32, img.height() as u32, false),
                avif_decode::Image::Rgba8(img) => (img.width() as u32, img.height() as u32, true),
                avif_decode::Image::Rgba16(img) => (img.width() as u32, img.height() as u32, true),
                avif_decode::Image::Gray8(img) => (img.width() as u32, img.height() as u32, false),
                avif_decode::Image::Gray16(img) => (img.width() as u32, img.height() as u32, false),
            };

            self.width = Some(w);
            self.height = Some(h);
            self.has_alpha = Some(alpha);
        }
        Ok(())
    }
}

impl Decoder for AvifDecoder {
    fn initialize(&mut self, _c: &Context) -> Result<()> {
        Ok(())
    }

    fn get_scaled_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        self.get_unscaled_image_info(c)
    }

    fn get_unscaled_image_info(&mut self, _c: &Context) -> Result<s::ImageInfo> {
        self.ensure_header_read()?;

        Ok(s::ImageInfo {
            frame_decodes_into: if self.has_alpha.unwrap() {
                s::PixelFormat::Bgra32
            } else {
                s::PixelFormat::Bgr32
            },
            image_width: self.width.unwrap() as i32,
            image_height: self.height.unwrap() as i32,
            preferred_mime_type: "image/avif".to_owned(),
            preferred_extension: "avif".to_owned(),
            lossless: false, // AVIF is typically lossy
            multiple_frames: false,
        })
    }

    fn get_exif_rotation_flag(&mut self, _c: &Context) -> Result<Option<i32>> {
        Ok(None)
    }

    fn tell_decoder(&mut self, _c: &Context, _tell: s::DecoderCommand) -> Result<()> {
        Ok(())
    }

    fn read_frame(&mut self, c: &Context) -> Result<BitmapKey> {
        self.ensure_data_buffered()?;

        let bytes = self.bytes.as_ref().unwrap();
        let decoder = avif_decode::Decoder::from_avif(bytes)
            .map_err(|e| nerror!(ErrorKind::ImageDecodingError, "AVIF parse error: {:?}", e))?;

        let image = decoder
            .to_image()
            .map_err(|e| nerror!(ErrorKind::ImageDecodingError, "AVIF decode error: {:?}", e))?;

        let (w, h, has_alpha) = match &image {
            avif_decode::Image::Rgb8(img) => (img.width() as u32, img.height() as u32, false),
            avif_decode::Image::Rgb16(img) => (img.width() as u32, img.height() as u32, false),
            avif_decode::Image::Rgba8(img) => (img.width() as u32, img.height() as u32, true),
            avif_decode::Image::Rgba16(img) => (img.width() as u32, img.height() as u32, true),
            avif_decode::Image::Gray8(img) => (img.width() as u32, img.height() as u32, false),
            avif_decode::Image::Gray16(img) => (img.width() as u32, img.height() as u32, false),
        };

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

        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;
        let mut window = bitmap.get_window_u8().unwrap();
        let stride = window.info().t_stride() as usize;
        let dest_slice = window.slice_mut();

        match image {
            avif_decode::Image::Rgb8(img) => {
                for y in 0..h as usize {
                    for x in 0..w as usize {
                        let pixel = img.buf()[y * img.stride() + x];
                        let offset = y * stride + x * 4;
                        dest_slice[offset] = pixel.b;
                        dest_slice[offset + 1] = pixel.g;
                        dest_slice[offset + 2] = pixel.r;
                        dest_slice[offset + 3] = 255;
                    }
                }
            }
            avif_decode::Image::Rgb16(img) => {
                for y in 0..h as usize {
                    for x in 0..w as usize {
                        let pixel = img.buf()[y * img.stride() + x];
                        let offset = y * stride + x * 4;
                        dest_slice[offset] = (pixel.b >> 8) as u8;
                        dest_slice[offset + 1] = (pixel.g >> 8) as u8;
                        dest_slice[offset + 2] = (pixel.r >> 8) as u8;
                        dest_slice[offset + 3] = 255;
                    }
                }
            }
            avif_decode::Image::Rgba8(img) => {
                for y in 0..h as usize {
                    for x in 0..w as usize {
                        let pixel = img.buf()[y * img.stride() + x];
                        let offset = y * stride + x * 4;
                        dest_slice[offset] = pixel.b;
                        dest_slice[offset + 1] = pixel.g;
                        dest_slice[offset + 2] = pixel.r;
                        dest_slice[offset + 3] = pixel.a;
                    }
                }
            }
            avif_decode::Image::Rgba16(img) => {
                for y in 0..h as usize {
                    for x in 0..w as usize {
                        let pixel = img.buf()[y * img.stride() + x];
                        let offset = y * stride + x * 4;
                        dest_slice[offset] = (pixel.b >> 8) as u8;
                        dest_slice[offset + 1] = (pixel.g >> 8) as u8;
                        dest_slice[offset + 2] = (pixel.r >> 8) as u8;
                        dest_slice[offset + 3] = (pixel.a >> 8) as u8;
                    }
                }
            }
            avif_decode::Image::Gray8(img) => {
                for y in 0..h as usize {
                    for x in 0..w as usize {
                        let pixel = img.buf()[y * img.stride() + x];
                        let gray = pixel.0;
                        let offset = y * stride + x * 4;
                        dest_slice[offset] = gray;
                        dest_slice[offset + 1] = gray;
                        dest_slice[offset + 2] = gray;
                        dest_slice[offset + 3] = 255;
                    }
                }
            }
            avif_decode::Image::Gray16(img) => {
                for y in 0..h as usize {
                    for x in 0..w as usize {
                        let pixel = img.buf()[y * img.stride() + x];
                        let gray = (pixel.0 >> 8) as u8;
                        let offset = y * stride + x * 4;
                        dest_slice[offset] = gray;
                        dest_slice[offset + 1] = gray;
                        dest_slice[offset + 2] = gray;
                        dest_slice[offset + 3] = 255;
                    }
                }
            }
        }

        Ok(bitmap_key)
    }

    fn has_more_frames(&mut self) -> Result<bool> {
        Ok(false)
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use imageflow_types::{
        Color, ColorSrgb, EncoderPreset, Execute001, Framewise, Node, OutputImageFormat,
        QualityProfile,
    };
    use imageflow_types::PixelFormat as PixFmt;

    fn generate_tiny_avif() -> Result<Vec<u8>> {
        let mut context = Context::create()?;

        context.add_output_buffer(1)?;

        let execute = Execute001 {
            graph_recording: None,
            security: None,
            framewise: Framewise::Steps(vec![
                Node::CreateCanvas {
                    w: 8,
                    h: 8,
                    format: PixFmt::Bgra32,
                    color: Color::Srgb(ColorSrgb::Hex("FF0000FF".to_owned())),
                },
                Node::Encode {
                    io_id: 1,
                    preset: EncoderPreset::Format {
                        format: OutputImageFormat::Avif,
                        quality_profile: Some(QualityProfile::High),
                        quality_profile_dpr: None,
                        lossless: None,
                        matte: None,
                        allow: None,
                        encoder_hints: None,
                    },
                },
            ]),
        };

        context.execute_1(execute).map_err(|e| e.at(here!()))?;
        context.get_output_buffer_slice(1).map_err(|e| e.at(here!())).map(|slice| slice.to_vec())
    }

    #[test]
    fn test_avif_decode_roundtrip() {
        let avif_bytes = generate_tiny_avif().expect("Failed to generate AVIF");

        assert!(avif_bytes.len() >= 12, "AVIF too small");
        assert_eq!(&avif_bytes[4..8], b"ftyp", "Missing ftyp box");

        let mut context = Context::create().unwrap();
        context.enabled_codecs.enable_bad_avif_decoder();

        context.add_input_bytes(0, &avif_bytes).unwrap();
        context.add_output_buffer(1).unwrap();

        let execute = Execute001 {
            graph_recording: None,
            security: None,
            framewise: Framewise::Steps(vec![
                Node::Decode { io_id: 0, commands: None },
                Node::Encode {
                    io_id: 1,
                    preset: EncoderPreset::Lodepng { maximum_deflate: None },
                },
            ]),
        };

        let result = context.execute_1(execute);
        assert!(result.is_ok(), "AVIF decode failed: {:?}", result.err());

        let png_bytes = context.get_output_buffer_slice(1).unwrap();
        assert!(png_bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]), "Output is not PNG");
    }

    #[test]
    fn test_avif_decode_image_info() {
        let avif_bytes = generate_tiny_avif().expect("Failed to generate AVIF");

        let mut context = Context::create().unwrap();
        context.enabled_codecs.enable_bad_avif_decoder();

        context.add_input_bytes(0, &avif_bytes).unwrap();

        let image_info = context.get_unscaled_unrotated_image_info(0).unwrap();

        assert_eq!(image_info.image_width, 8);
        assert_eq!(image_info.image_height, 8);
        assert_eq!(image_info.preferred_mime_type, "image/avif");
        assert_eq!(image_info.preferred_extension, "avif");
    }
}
