use std;
use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::ffi;
use crate::{Context, CError,  Result, JsonResponse};
use crate::ffi::BitmapBgra;
use imageflow_types::collections::AddRemoveSet;
use crate::io::{IoProxy, ZuneIoProxyReader};
use uuid::Uuid;
use imageflow_types::{DecoderCommand, ImageInfo, IoDirection, PixelLayout};
use super::*;
use std::any::Any;
use crate::gif::Frame;
use std::rc::Rc;
use crate::io::IoProxyProxy;
use crate::io::IoProxyRef;
use crate::graphics::bitmaps::{BitmapKey, ColorSpace, BitmapCompositing};

use zune_bmp;

pub struct BmpDecoder{
    decoder: zune_bmp::BmpDecoder<zune_bmp::zune_core::bytestream::ZCursor<ZuneIoProxyReader>>,
}

impl BmpDecoder {
    pub fn create(c: &Context, io: IoProxy, io_id: i32) -> Result<BmpDecoder> {
        let reader = zune_bmp::zune_core::bytestream::ZCursor::new(ZuneIoProxyReader::wrap_or_buffer(io).map_err(|e| FlowError::from_decoder(e).at(here!()))?);

        let mut decoder =  zune_bmp::BmpDecoder::new(reader);
        decoder.decode_headers().map_err(|e| FlowError::from(e).at(here!()))?;

        let (w,h) = decoder.dimensions().expect("Dimensions not found");

        Ok(BmpDecoder{
            decoder
        })
    }

    fn has_alpha(&self) -> bool {
        let colorspace = self.decoder.colorspace().expect("Colorspace not found");
        colorspace.has_alpha()
    }

    fn decodes_into(&self) -> imageflow_types::PixelFormat {
        let colorspace = self.decoder.colorspace().expect("Colorspace not found");
        if colorspace.has_alpha() {
            imageflow_types::PixelFormat::Bgra32
        } else {
            imageflow_types::PixelFormat::Bgr32
        }
    }
}
impl Decoder for BmpDecoder{
    fn initialize(&mut self, c: &Context) -> Result<()> {
        Ok(())
    }

    fn get_unscaled_image_info(&mut self, c: &Context) -> Result<ImageInfo> {
        let (w,h) = self.decoder.dimensions().expect("Dimensions not found");
        let colorspace = self.decoder.colorspace().expect("Colorspace not found");
        let decodes_into = self.decodes_into();
        Ok(ImageInfo{
            preferred_mime_type: "image/bmp".to_string(),
            preferred_extension: "bmp".to_string(),
            image_width: w as i32,
            image_height: h as i32,
            frame_decodes_into: decodes_into
        })
    }

    fn get_scaled_image_info(&mut self, c: &Context) -> Result<ImageInfo> {
        self.get_unscaled_image_info(c)
    }

    fn get_exif_rotation_flag(&mut self, c: &Context) -> Result<Option<i32>> {
        Ok(None)
    }

    fn tell_decoder(&mut self, c: &Context, tell: DecoderCommand) -> Result<()> {
        Ok(())
    }

    fn read_frame(&mut self, c: &Context) -> Result<BitmapKey> {
        let (w,h) = self.decoder.dimensions().expect("Dimensions not found");
        let colorspace = self.decoder.colorspace().expect("Colorspace not found");
        let fmt = self.decodes_into();

        let mut bitmaps = c.borrow_bitmaps_mut()
            .map_err(|e| e.at(here!()))?;

        let canvas_key = bitmaps.create_bitmap_u8(
            w as u32,h as u32,fmt.pixel_layout(),
            false, fmt.alpha_meaningful(),
            ColorSpace::StandardRGB,
            BitmapCompositing::ReplaceSelf)
            .map_err(|e| e.at(here!()))?;


        let mut canvas = unsafe { bitmaps.try_borrow_mut(canvas_key)
            .map_err(|e| e.at(here!()))?
            .get_window_u8().unwrap()
            .to_bitmap_bgra().map_err(|e| e.at(here!()))?
        };

        let bytes = self.decoder.decode().map_err(|e| FlowError::from(e).at(here!()))?;
        let input_stride = colorspace.num_components() * w;
        let output_stride = canvas.stride() as usize;
        let input_alpha = colorspace.has_alpha();

        unsafe {
            let mut canvas = canvas.pixels_slice_mut().unwrap();
            for y in 0..h {
                let input_row = &bytes[y * input_stride..(y + 1) * input_stride];
                let output_row = &mut canvas[y * output_stride..(y + 1) * output_stride];

                if input_alpha {
                    for (input_chunk, output_chunk) in input_row.chunks_exact(4).zip(output_row.chunks_exact_mut(4)) {

                        // SAFETY: The access is guaranteed to be within bounds because the chunks are of size 4.
                        *output_chunk.get_unchecked_mut(0) = *input_chunk.get_unchecked(2);
                        *output_chunk.get_unchecked_mut(1) = *input_chunk.get_unchecked(1);
                        *output_chunk.get_unchecked_mut(2) = *input_chunk.get_unchecked(0);
                        *output_chunk.get_unchecked_mut(3) = *input_chunk.get_unchecked(3);
                    }
                } else {
                    for (input_chunk, output_chunk) in input_row.chunks_exact(3).zip(output_row.chunks_exact_mut(4)) {
                        // SAFETY: The access is guaranteed to be within bounds because the chunks are of size 3.
                        *output_chunk.get_unchecked_mut(0) = *input_chunk.get_unchecked(0);
                        *output_chunk.get_unchecked_mut(1) = *input_chunk.get_unchecked(1);
                        *output_chunk.get_unchecked_mut(2) = *input_chunk.get_unchecked(2);
                        *output_chunk.get_unchecked_mut(3) = 255;
                    }
                }
            }
        }

        Ok(canvas_key)
    }

    fn has_more_frames(&mut self) -> Result<bool> {
        Ok(false)
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}