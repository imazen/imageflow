use std::io::Read;

use crate::codecs::Decoder;
use crate::graphics::bitmaps::{BitmapCompositing, BitmapKey, ColorSpace};
use crate::io::IoProxy;
use crate::{Context, ErrorKind, FlowError, Result};
use imageflow_helpers::preludes::from_std::*;
use imageflow_types as s;
//use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::graphics::bitmaps::BitmapRowAccess;

pub struct ImagePngDecoder {
    reader: png::Reader<IoProxy>,
    info: png::Info<'static>,
}

impl ImagePngDecoder {
    pub fn create(c: &Context, io: IoProxy, io_id: i32) -> Result<ImagePngDecoder> {
        let mut decoder = png::Decoder::new(io);
        decoder.set_transformations(png::Transformations::normalize_to_color8());

        let reader = decoder.read_info().map_err(|e| FlowError::from_png_decoder(e).at(here!()))?;

        let info = reader.info().clone();

        // Validate dimensions against security limits BEFORE any decode allocation
        let w = info.width;
        let h = info.height;
        let limit = c.security.max_decode_size.as_ref().or(c.security.max_frame_size.as_ref());
        if let Some(limit) = limit {
            if w > limit.w {
                return Err(nerror!(
                    ErrorKind::SizeLimitExceeded,
                    "PNG width {} exceeds max_decode_size.w {}",
                    w,
                    limit.w
                ));
            }
            if h > limit.h {
                return Err(nerror!(
                    ErrorKind::SizeLimitExceeded,
                    "PNG height {} exceeds max_decode_size.h {}",
                    h,
                    limit.h
                ));
            }
            let megapixels = w as f32 * h as f32 / 1_000_000f32;
            if megapixels > limit.megapixels {
                return Err(nerror!(
                    ErrorKind::SizeLimitExceeded,
                    "PNG megapixels {:.2} exceeds max_decode_size.megapixels {}",
                    megapixels,
                    limit.megapixels
                ));
            }
        }

        Ok(ImagePngDecoder { reader, info })
    }
}
impl Decoder for ImagePngDecoder {
    fn initialize(&mut self, c: &Context) -> Result<()> {
        Ok(())
    }

    fn get_unscaled_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        Ok(s::ImageInfo {
            frame_decodes_into: s::PixelFormat::Bgra32,
            image_width: self.info.width as i32,
            image_height: self.info.height as i32,
            preferred_mime_type: "image/png".to_owned(),
            preferred_extension: "png".to_owned(),
            lossless: true,
            multiple_frames: false,
        })
    }

    fn get_scaled_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        self.get_unscaled_image_info(c)
    }

    fn get_exif_rotation_flag(&mut self, c: &Context) -> Result<Option<i32>> {
        Ok(None)
    }

    fn tell_decoder(&mut self, c: &Context, tell: s::DecoderCommand) -> Result<()> {
        Ok(())
    }

    fn read_frame(&mut self, c: &Context) -> Result<BitmapKey> {
        return_if_cancelled!(c);

        let mut bitmaps = c.borrow_bitmaps_mut().map_err(|e| e.at(here!()))?;
        let info = self.reader.info();

        let canvas_key = bitmaps
            .create_bitmap_u8(
                info.width,
                info.height,
                imageflow_types::PixelLayout::BGRA,
                false,
                true,
                ColorSpace::StandardRGB,
                BitmapCompositing::ReplaceSelf,
            )
            .map_err(|e| e.at(here!()))?;

        let mut bitmap = bitmaps.try_borrow_mut(canvas_key).map_err(|e| e.at(here!()))?;

        let mut canvas = bitmap.get_window_u8().unwrap();

        return_if_cancelled!(c);

        let buffer_size = self.reader.output_buffer_size().ok_or_else(|| {
            nerror!(ErrorKind::ImageDecodingError, "PNG output buffer size unknown")
        })?;
        let mut buffer = vec![0; buffer_size];
        let output_info = self
            .reader
            .next_frame(&mut buffer)
            .map_err(|e| FlowError::from_png_decoder(e).at(here!()))?;

        return_if_cancelled!(c);

        let h = output_info.height as usize;
        let stride = output_info.line_size;
        let w = output_info.width as usize;
        if output_info.bit_depth != png::BitDepth::Eight {
            return Err(nerror!(
                ErrorKind::ImageDecodingError,
                "image/png decoder did not expand to 8-bit channels"
            )
            .at(here!()));
        }
        match output_info.color_type {
            png::ColorType::Rgb => {
                for row_ix in 0..h {
                    let from = buffer.row_mut_rgb8(row_ix, stride).unwrap();
                    let to = canvas.row_mut_bgra(row_ix as u32).unwrap();
                    from.iter_mut().zip(to.iter_mut()).for_each(|(from, to)| {
                        to.r = from.r;
                        to.g = from.g;
                        to.b = from.b;
                        to.a = 255;
                    });
                }
            }
            png::ColorType::Rgba => {
                for row_ix in 0..h {
                    let from = buffer.row_mut_rgba8(row_ix, stride).unwrap();
                    let to = canvas.row_mut_bgra(row_ix as u32).unwrap();
                    from.iter_mut().zip(to.iter_mut()).for_each(|(from, to)| {
                        to.r = from.r;
                        to.g = from.g;
                        to.b = from.b;
                        to.a = from.a;
                    });
                }
            }

            png::ColorType::Grayscale => {
                for row_ix in 0..h {
                    let from = buffer.row_mut_gray8(row_ix, stride).unwrap();
                    let to = canvas.row_mut_bgra(row_ix as u32).unwrap();
                    from.iter_mut().zip(to.iter_mut()).for_each(|(f, to)| {
                        // TODO(rgb-0.8.91): Can simplify to f.v when Gray has named field
                        let v = f.value();
                        to.r = v;
                        to.g = v;
                        to.b = v;
                        to.a = 255;
                    });
                }
            }
            png::ColorType::GrayscaleAlpha => {
                for row_ix in 0..h {
                    let from = buffer.row_mut_grayalpha8(row_ix, stride).unwrap();
                    let to = canvas.row_mut_bgra(row_ix as u32).unwrap();
                    from.iter_mut().zip(to.iter_mut()).for_each(|(from, to)| {
                        // GrayA/GrayAlpha uses .v and .a fields (via Deref)
                        to.r = from.v;
                        to.g = from.v;
                        to.b = from.v;
                        to.a = from.a;
                    });
                }
            }

            _ => panic!("png decoder bug: indexed image was not expanded despite flags."),
        }

        Ok(canvas_key)
    }

    fn has_more_frames(&mut self) -> Result<bool> {
        Ok(false)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self as &dyn std::any::Any
    }
}
