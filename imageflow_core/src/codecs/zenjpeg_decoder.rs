use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::{Context, Result};

use super::*;
use crate::codecs::cms;
use crate::codecs::source_profile::SourceProfile;
use crate::graphics::bitmaps::{BitmapCompositing, ColorSpace};
use crate::io::IoProxy;
use imageflow_types::PixelLayout;
use std::any::Any;

pub struct ZenJpegDecoder {
    io: IoProxy,
    data: Option<Vec<u8>>,
    info: Option<zenjpeg::decoder::JpegInfo>,
    ignore_color_profile: bool,
    ignore_color_profile_errors: bool,
}

impl ZenJpegDecoder {
    pub fn create(_c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        Ok(ZenJpegDecoder {
            io,
            data: None,
            info: None,
            ignore_color_profile: false,
            ignore_color_profile_errors: false,
        })
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
            let decoder = zenjpeg::decoder::Decoder::new().apply_icc(false).preserve_all();
            let info = decoder
                .read_info(data)
                .map_err(|e| nerror!(ErrorKind::ImageDecodingError, "zenjpeg info error: {}", e))?;
            self.info = Some(info);
        }
        Ok(())
    }
}

impl Decoder for ZenJpegDecoder {
    fn initialize(&mut self, _c: &Context) -> Result<()> {
        Ok(())
    }

    fn get_unscaled_image_info(&mut self, _c: &Context) -> Result<s::ImageInfo> {
        self.ensure_info_read()?;
        let info = self.info.as_ref().unwrap();
        Ok(s::ImageInfo {
            frame_decodes_into: s::PixelFormat::Bgra32,
            image_width: info.dimensions.width as i32,
            image_height: info.dimensions.height as i32,
            preferred_mime_type: "image/jpeg".to_owned(),
            preferred_extension: "jpg".to_owned(),
            lossless: false,
            multiple_frames: false,
        })
    }

    fn get_scaled_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        self.get_unscaled_image_info(c)
    }

    fn get_exif_rotation_flag(&mut self, _c: &Context) -> Result<Option<i32>> {
        self.ensure_info_read()?;
        let info = self.info.as_ref().unwrap();
        if let Some(ref exif_data) = info.exif {
            if let Some(orientation) = zenjpeg::lossless::parse_exif_orientation(exif_data) {
                return Ok(Some(orientation as i32));
            }
        }
        Ok(None)
    }

    fn tell_decoder(&mut self, _c: &Context, tell: s::DecoderCommand) -> Result<()> {
        match tell {
            s::DecoderCommand::JpegDownscaleHints(_hints) => {
                // zenjpeg doesn't support downscaled decoding yet
                Ok(())
            }
            s::DecoderCommand::DiscardColorProfile => {
                self.ignore_color_profile = true;
                Ok(())
            }
            s::DecoderCommand::IgnoreColorProfileErrors => {
                self.ignore_color_profile_errors = true;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn read_frame(&mut self, c: &Context) -> Result<BitmapKey> {
        self.ensure_data_buffered()?;
        let data = self.data.as_ref().unwrap();

        // Decode to BGRA with ICC disabled (imageflow handles CMS)
        let decoder = zenjpeg::decoder::Decoder::new()
            .output_format(zenjpeg::decoder::PixelFormat::Bgra)
            .apply_icc(false)
            .preserve_all();

        let result = decoder
            .decode(data, c.stop())
            .map_err(|e| nerror!(ErrorKind::ImageDecodingError, "zenjpeg decode error: {}", e))?;

        let w = result.width();
        let h = result.height();
        let pixels = result.pixels_u8().ok_or_else(|| {
            nerror!(ErrorKind::ImageDecodingError, "zenjpeg returned no u8 pixels")
        })?;

        // Determine color profile from ICC data.
        // Note: zenjpeg already converts CMYK→RGB during decode (since we request
        // BGRA output), so we cannot apply a CMYK ICC profile to the result.
        // For CMYK JPEGs, we skip ICC-based color management.
        let is_cmyk = self
            .info
            .as_ref()
            .map(|i| {
                matches!(
                    i.color_space,
                    zenjpeg::decoder::ColorSpace::Cmyk | zenjpeg::decoder::ColorSpace::Ycck
                )
            })
            .unwrap_or(false);

        let source_profile = if self.ignore_color_profile || is_cmyk {
            // CMYK: zenjpeg already converted to RGB, skip CMS
            // ignore_color_profile: user requested no ICC
            SourceProfile::Srgb
        } else if let Some(extras) = result.extras() {
            if let Some(icc_data) = extras.icc_profile() {
                let info = self.info.as_ref().unwrap();
                match info.color_space {
                    zenjpeg::decoder::ColorSpace::Grayscale => {
                        SourceProfile::IccProfileGray(icc_data.to_vec())
                    }
                    _ => SourceProfile::IccProfile(icc_data.to_vec()),
                }
            } else {
                SourceProfile::Srgb
            }
        } else {
            SourceProfile::Srgb
        };

        let has_alpha = false; // JPEG never has meaningful alpha

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

            // Copy decoded pixels into bitmap (stride may differ)
            let dst = window.slice_mut();
            if dst_stride == src_stride {
                dst[..pixels.len()].copy_from_slice(pixels);
            } else {
                for y in 0..h as usize {
                    let src_row = &pixels[y * src_stride..(y + 1) * src_stride];
                    let dst_row = &mut dst[y * dst_stride..y * dst_stride + src_stride];
                    dst_row.copy_from_slice(src_row);
                }
            }
        }

        // Apply CMS transform if needed
        if !matches!(source_profile, SourceProfile::Srgb) {
            let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;
            let mut window = bitmap.get_window_u8().unwrap();
            let result = cms::transform_to_srgb(&mut window, &source_profile);
            if let Err(e) = result {
                if !self.ignore_color_profile_errors {
                    return Err(e);
                }
                // Silently ignore CMS errors when ignore_icc_errors is set
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
