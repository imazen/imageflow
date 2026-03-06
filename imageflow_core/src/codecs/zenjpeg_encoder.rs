use super::s::{EncodeResult, EncoderPreset};
use super::Encoder;
use crate::io::IoProxy;

use crate::graphics::bitmaps::BitmapKey;
use crate::{Context, ErrorKind, FlowError, Result};
use imageflow_types::{Color, PixelFormat};
use std::io::Write;

use zenjpeg::encoder::{ChromaSubsampling, EncoderConfig, PixelLayout, Quality};

pub struct ZenJpegEncoder {
    io: IoProxy,
    quality: Option<u8>,
    progressive: Option<bool>,
    matte: Option<Color>,
}

const DEFAULT_QUALITY: u8 = 75;

impl ZenJpegEncoder {
    pub(crate) fn create(
        c: &Context,
        quality: Option<u8>,
        progressive: Option<bool>,
        matte: Option<Color>,
        io: IoProxy,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::ZenJpegEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The ZenJpeg encoder has been disabled"
            ));
        }

        Ok(ZenJpegEncoder {
            io,
            quality: Some(quality.unwrap_or(DEFAULT_QUALITY).min(100)),
            progressive,
            matte,
        })
    }
}

impl Encoder for ZenJpegEncoder {
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
        let q = self.quality.unwrap_or(DEFAULT_QUALITY);

        // Map quality to chroma subsampling (match mozjpeg behavior)
        let subsampling = if q >= 90 {
            ChromaSubsampling::None // 4:4:4 for high quality
        } else {
            ChromaSubsampling::Quarter // 4:2:0 for normal quality
        };

        // Use ApproxMozjpeg quality mapping for backward compat with existing quality profiles
        let config = EncoderConfig::ycbcr(Quality::ApproxMozjpeg(q), subsampling)
            .auto_optimize(true)
            .progressive(self.progressive.unwrap_or(true));

        let pixel_layout = match window.pixel_format() {
            PixelFormat::Bgra32 => PixelLayout::Bgra8Srgb,
            PixelFormat::Bgr32 => PixelLayout::Bgrx8Srgb,
            PixelFormat::Bgr24 => PixelLayout::Bgr8Srgb,
            PixelFormat::Gray8 => PixelLayout::Gray8Srgb,
        };

        let w = window.w() as usize;
        let h = window.h() as usize;
        let src_stride = window.info().t_stride() as usize;

        let mut encoder = config
            .encode_from_bytes(w as u32, h as u32, pixel_layout)
            .map_err(|e| nerror!(ErrorKind::ImageEncodingError, "zenjpeg config error: {}", e))?;

        // Push scanlines (handles stride differences)
        let stop = c.stop();
        if w * window.pixel_format().bytes() == src_stride {
            encoder.push_packed(window.get_slice(), stop).map_err(|e| {
                nerror!(ErrorKind::ImageEncodingError, "zenjpeg encode error: {}", e)
            })?;
        } else {
            for line in window.scanlines() {
                encoder.push_packed(line.row(), stop).map_err(|e| {
                    nerror!(ErrorKind::ImageEncodingError, "zenjpeg encode error: {}", e)
                })?;
            }
        }

        let jpeg_bytes = encoder
            .finish()
            .map_err(|e| nerror!(ErrorKind::ImageEncodingError, "zenjpeg finish error: {}", e))?;

        self.io.write_all(&jpeg_bytes).map_err(|e| FlowError::from_encoder(e).at(here!()))?;

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
