use super::s::{EncodeResult, EncoderPreset};
use super::Encoder;
use crate::io::IoProxy;

use crate::codecs::lode;
use crate::graphics::bitmaps::BitmapKey;
use crate::{Context, ErrorKind, FlowError, Result};
use evalchroma::PixelSize;
use imageflow_types::PixelBuffer;
use imageflow_types::{Color, PixelFormat};
use std::cell::RefCell;
use std::io::Write;
use std::os::raw::c_int;
use std::rc::Rc;
use std::result::Result as StdResult;
use std::slice;

#[derive(Copy, Clone)]
enum Defaults {
    MozJPEG,
    LibJPEGv6,
}

pub struct MozjpegEncoder {
    io: IoProxy,
    quality: Option<u8>,
    progressive: Option<bool>,
    optimize_coding: Option<bool>,
    defaults: Defaults,
    matte: Option<Color>,
}

const DEFAULT_QUALITY: u8 = 75;

impl MozjpegEncoder {
    // Quality is in range 0-100
    pub(crate) fn create(
        c: &Context,
        quality: Option<u8>,
        progressive: Option<bool>,
        matte: Option<Color>,
        io: IoProxy,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::MozJpegEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The MozJpeg encoder has been disabled"
            ));
        }

        Ok(MozjpegEncoder {
            io,
            quality: Some(u8::min(100, quality.unwrap_or(DEFAULT_QUALITY))),
            progressive,
            matte,
            optimize_coding: Some(true),
            defaults: Defaults::MozJPEG,
        })
    }

    pub(crate) fn create_classic(
        c: &Context,
        quality: Option<u8>,
        progressive: Option<bool>,
        optimize_coding: Option<bool>,
        matte: Option<Color>,
        io: IoProxy,
    ) -> Result<Self> {
        Ok(MozjpegEncoder {
            io,
            quality: Some(u8::min(100, quality.unwrap_or(DEFAULT_QUALITY))),
            progressive,
            matte,
            optimize_coding,
            defaults: Defaults::LibJPEGv6,
        })
    }
}

impl Encoder for MozjpegEncoder {
    fn write_frame(
        &mut self,
        c: &Context,
        _preset: &EncoderPreset,
        bitmap_key: BitmapKey,
        _decoder_io_ids: &[i32],
    ) -> Result<EncodeResult> {
        return_if_cancelled!(c);
        let bitmaps = c.borrow_bitmaps().map_err(|e| e.at(here!()))?;

        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;

        bitmap
            .get_window_bgra32()
            .unwrap()
            .apply_matte(self.matte.clone().unwrap_or(imageflow_types::Color::Srgb(
                imageflow_types::ColorSrgb::Hex("FFFFFFFF".to_owned()),
            )))
            .map_err(|e| e.at(here!()))?;
        bitmap.set_alpha_meaningful(false);

        let mut window = bitmap.get_window_u8().unwrap();

        let in_color_space = match window.pixel_format() {
            PixelFormat::Bgra32 => mozjpeg::ColorSpace::JCS_EXT_BGRA,
            PixelFormat::Bgr32 => mozjpeg::ColorSpace::JCS_EXT_BGRX,
            PixelFormat::Bgr24 => mozjpeg::ColorSpace::JCS_EXT_BGR,
            PixelFormat::Gray8 => mozjpeg::ColorSpace::JCS_GRAYSCALE,
        };
        let mut cinfo = mozjpeg::Compress::new(in_color_space);
        cinfo.set_size(window.w() as usize, window.h() as usize);
        match self.defaults {
            Defaults::MozJPEG => {}
            Defaults::LibJPEGv6 => {
                cinfo.set_fastest_defaults();
            }
        }
        if let Some(q) = self.quality {
            cinfo.set_quality(u8::min(100, q).into());
        }
        if let Some(p) = self.progressive {
            if p {
                cinfo.set_progressive_mode();
            }
        }
        if let Some(o) = self.optimize_coding {
            cinfo.set_optimize_coding(o);
        }

        let chroma_quality = self.quality.unwrap_or(DEFAULT_QUALITY) as f32; // Lower values allow blurrier color

        let pixel_buffer = window.get_pixel_buffer().unwrap();

        let max_sampling = PixelSize { cb: (2, 2), cr: (2, 2) }; // Set to 1 to force higher res
        let res = match pixel_buffer {
            PixelBuffer::Bgra32(buf) => {
                evalchroma::adjust_sampling(buf, max_sampling, chroma_quality)
            }
            PixelBuffer::Bgr32(buf) => {
                evalchroma::adjust_sampling(buf, max_sampling, chroma_quality)
            }
            PixelBuffer::Bgr24(buf) => {
                evalchroma::adjust_sampling(buf, max_sampling, chroma_quality)
            }
            PixelBuffer::Gray8(buf) => {
                evalchroma::adjust_sampling(buf, max_sampling, chroma_quality)
            }
        };

        // Translate chroma pixel size into JPEG's channel-relative samples per pixel
        let max_sampling_h = res.subsampling.cb.0.max(res.subsampling.cr.0);
        let max_sampling_v = res.subsampling.cb.1.max(res.subsampling.cr.1);
        let px_sizes = &[(1, 1), res.subsampling.cb, res.subsampling.cr];
        for (c, &(h, v)) in cinfo.components_mut().iter_mut().zip(px_sizes) {
            c.h_samp_factor = (max_sampling_h / h).into();
            c.v_samp_factor = (max_sampling_v / v).into();
        }

        let mut compressor = cinfo
            .start_compress(&mut self.io)
            .map_err(|io_error| nerror!(ErrorKind::EncodingIoError, "{:?}", io_error))?;

        if window.w() as usize == window.t_stride() {
            compressor
                .write_scanlines(window.get_slice())
                .map_err(|io_error| nerror!(ErrorKind::EncodingIoError, "{:?}", io_error))?;
        } else {
            let mut pixels_since_check = 0;
            for line in window.scanlines() {
                compressor
                    .write_scanlines(line.row())
                    .map_err(|io_error| nerror!(ErrorKind::EncodingIoError, "{:?}", io_error))?;
                pixels_since_check += line.row().len();
                if pixels_since_check >= 100_000 {
                    pixels_since_check = 0;
                    return_if_cancelled!(c);
                }
            }
        }
        compressor
            .finish()
            .map_err(|io_error| nerror!(ErrorKind::EncodingIoError, "{:?}", io_error))?;

        Ok(EncodeResult {
            w: window.w_i32(),
            h: window.h_i32(),
            io_id: self.io.io_id(),
            bytes: ::imageflow_types::ResultBytes::Elsewhere,
            preferred_extension: "jpg".to_owned(),
            preferred_mime_type: "image/jpeg".to_owned(),
        })
    }

    fn into_io(self: Box<Self>) -> Result<IoProxy> {
        Ok(self.io)
    }
}
