use super::s::{EncodeResult, EncoderPreset};
use super::Encoder;
use crate::io::IoProxy;

use crate::codecs::lode;
use crate::graphics::bitmaps::BitmapKey;
use crate::io::IoProxyRef;
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

        let mut data =
            crate::codecs::diagnostic_collector::DiagnosticCollector::new("mozjpeg.encoder.");

        let had_alpha = bitmap.info().alpha_meaningful();
        data.add("input.had_alpha", &had_alpha);
        if self.matte.is_some() {
            data.add("params.matte", &self.matte.clone().unwrap());
            data.add("params.matte_is_opaque", &self.matte.clone().unwrap().is_opaque());
            bitmap.apply_matte(self.matte.clone().unwrap())?;
            bitmap.set_alpha_meaningful(false);
            data.add("result.applied_custom_matte", &self.matte.clone().unwrap());
        }
        if bitmap.info().alpha_meaningful() {
            let white = imageflow_types::Color::Srgb(imageflow_types::ColorSrgb::Hex(
                "FFFFFFFF".to_owned(),
            ));
            bitmap.apply_matte(white.clone())?;
            data.add("result.applied_white_matte", true);
        }
        data.add("input.has_alpha", bitmap.info().alpha_meaningful());

        let mut window = bitmap.get_window_u8().unwrap();

        // mozjpeg Default quality is 75
        let quality = self.quality.map(|q| u8::min(100, q as u8)).unwrap_or(DEFAULT_QUALITY);

        data.add("params.quality", &quality);

        let in_color_space = match window.pixel_format() {
            PixelFormat::Bgra32 => mozjpeg::ColorSpace::JCS_EXT_BGRA,
            PixelFormat::Bgr32 => mozjpeg::ColorSpace::JCS_EXT_BGRX,
            PixelFormat::Bgr24 => mozjpeg::ColorSpace::JCS_EXT_BGR,
            PixelFormat::Gray8 => mozjpeg::ColorSpace::JCS_GRAYSCALE,
        };
        let mut cinfo = mozjpeg::Compress::new(in_color_space);
        data.add_debug("params.color_space", &in_color_space);

        cinfo.set_size(window.w() as usize, window.h() as usize);

        match self.defaults {
            Defaults::MozJPEG => {}
            Defaults::LibJPEGv6 => {
                cinfo.set_fastest_defaults();
                data.add("params.mimic_libjpeg_turbo", true);
            }
        }
        cinfo.set_quality(quality.into());

        if let Some(p) = self.progressive {
            if p {
                cinfo.set_progressive_mode();
            }
        }
        data.add_debug("params.progressive", &self.progressive);
        data.add_debug("params.optimize_coding", &self.optimize_coding);
        if let Some(o) = self.optimize_coding {
            cinfo.set_optimize_coding(o);
        }

        let chroma_quality = quality as f32; // Lower values allow blurrier color
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
        data.add_debug("result.evalchroma.pixel_size", &res.subsampling);
        data.add_debug("result.evalchroma.chroma_quality", &res.chroma_quality);
        data.add_debug("result.evalchroma.sharpness", &res.sharpness);

        // Translate chroma pixel size into JPEG's channel-relative samples per pixel
        let max_sampling_h = res.subsampling.cb.0.max(res.subsampling.cr.0);
        let max_sampling_v = res.subsampling.cb.1.max(res.subsampling.cr.1);
        let px_sizes = &[(1, 1), res.subsampling.cb, res.subsampling.cr];
        for (c, &(h, v)) in cinfo.components_mut().iter_mut().zip(px_sizes) {
            c.h_samp_factor = (max_sampling_h / h).into();
            c.v_samp_factor = (max_sampling_v / v).into();
            data.add(
                &format!("result.component[{}]", c.component_index),
                format!("h={}, v={}", c.h_samp_factor, c.v_samp_factor),
            );
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
            diagnostic_data: data.into_diagnostic_data(),
        })
    }

    fn get_io(&self) -> Result<IoProxyRef<'_>> {
        Ok(IoProxyRef::Borrow(&self.io))
    }

    fn into_io(self: Box<Self>) -> Result<IoProxy> {
        // MozJPEG encoder calls finish() during write_frame, so no additional cleanup needed
        Ok(self.io)
    }
}
