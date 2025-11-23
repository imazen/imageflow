use super::s::{EncodeResult, EncoderPreset};
use super::Encoder;
use crate::io::IoProxy;

use crate::graphics::bitmaps::BitmapKey;
use crate::io::IoProxyRef;
use crate::{Context, ErrorKind, FlowError, Result};
use imageflow_types::{Color, PixelFormat};
use imgref::ImgExt;
use rgb::RGBA8;
use std::io::Write;

pub struct AvifEncoder {
    io: IoProxy,
    quality: Option<f32>,
    speed: Option<u8>,
    alpha_quality: Option<f32>,
    matte: Option<Color>,
}

impl AvifEncoder {
    pub(crate) fn create(
        c: &Context,
        io: IoProxy,
        quality: Option<f32>,
        speed: Option<u8>,
        alpha_quality: Option<f32>,
        matte: Option<Color>,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::AvifEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The AVIF encoder has been disabled"
            ));
        }

        Ok(AvifEncoder {
            io,
            quality: Some(quality.unwrap_or(80.0).clamp(0.0, 100.0)),
            speed: Some(speed.unwrap_or(6).clamp(0, 10)),
            alpha_quality,
            matte,
        })
    }
}

impl Encoder for AvifEncoder {
    fn write_frame(
        &mut self,
        c: &Context,
        _preset: &EncoderPreset,
        bitmap_key: BitmapKey,
        _decoder_io_ids: &[i32],
    ) -> Result<EncodeResult> {
        return_if_cancelled!(c);

        let mut data = crate::codecs::diagnostic_collector::DiagnosticCollector::new("avif.encoder.");

        // 1. Borrow bitmap from Context
        let bitmaps = c.borrow_bitmaps().map_err(|e| e.at(here!()))?;
        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;

        // 2. Apply matte if needed (blend alpha with background color)
        if let Some(matte) = &self.matte {
            let was_alpha_meaningful = bitmap.info().alpha_meaningful();
            data.add("params.matte", &matte);
            data.add("input.had_alpha", &was_alpha_meaningful);
            bitmap
                .get_window_bgra32()
                .unwrap()
                .apply_matte(matte.clone())
                .map_err(|e| e.at(here!()))?;
            bitmap.set_alpha_meaningful(!matte.is_opaque() && was_alpha_meaningful);
        } 
        // Ensure alpha is normalized if not meaningful
        if !bitmap.info().alpha_meaningful() {
            bitmap
                .get_window_u8()
                .unwrap()
                .normalize_unused_alpha()
                .map_err(|e| e.at(here!()))?;
        }
        data.add("input.has_alpha", &bitmap.info().alpha_meaningful());

        // 3. Get window for pixel access
        let mut window = bitmap.get_window_u8().unwrap();
        let (w, h) = window.size_i32();
        let stride_in_bytes = window.info().t_stride();
        let stride_in_pixels = stride_in_bytes / 4; // Convert from u8 stride to pixel stride

        // 4. Verify pixel format (AVIF encoder requires BGRA or BGR)
        match window.pixel_format() {
            PixelFormat::Bgra32 | PixelFormat::Bgr32 => {
                // Supported - we'll convert BGRA to RGBA
            }
            other => {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "AVIF encoder requires BGRA32 or BGR32 format, got {:?}",
                    other
                ));
            }
        }

        // 5. Convert BGRA to RGBA in-place using scanlines
        // This swaps the red and blue channels since imageflow uses BGRA but ravif expects RGBA
        // Cast window to BGRA8 for easier manipulation
        let mut bgra_window = window.to_window_bgra32()?;
        for mut line in bgra_window.scanlines() {
            for pixel in line.row_mut() {
                // Swap red and blue channels: BGRA to RGBA
                std::mem::swap(&mut pixel.r, &mut pixel.b);
            }
        }

        // 6. Cast to RGBA8 slice (safe because we just swapped to RGBA order)
        // Both BGRA8 and RGBA8 are #[repr(C)] with identical memory layout
        let rgba_slice: &[RGBA8] = bytemuck::cast_slice(bgra_window.slice_mut());

        // 7. Create imgref with stride to handle padding
        // ravif's imgref supports stride, so we can pass the padded buffer directly
        let img = imgref::Img::new_stride(rgba_slice, w as usize, h as usize, stride_in_pixels as usize);

        
        // 8. Configure encoder with quality and speed settings
        let quality_value = self.quality.unwrap_or(80.0);
        let speed_value = self.speed.unwrap_or(6);
        let alpha_color_mode = ravif::AlphaColorMode::UnassociatedClean;

        data.add("params.quality", &quality_value);
        data.add("params.speed", &speed_value);
        data.add_debug("params.alpha_color_mode", &alpha_color_mode);

        let mut encoder = ravif::Encoder::new();
        encoder = encoder
            .with_quality(quality_value)
            .with_speed(speed_value)
            .with_alpha_color_mode(alpha_color_mode);

        // Apply separate alpha quality if specified
        if let Some(alpha_q) = self.alpha_quality {
            encoder = encoder.with_alpha_quality(alpha_q.clamp(0.0, 100.0));
            data.add("params.alpha_quality", &alpha_q);
        }

        // 9. Encode to AVIF format
        let encoded = encoder
            .encode_rgba(img.as_ref())
            .map_err(|e| nerror!(ErrorKind::ImageEncodingError, "AVIF encoding failed: {:?}", e))?;

        // 10. Write encoded AVIF data to IoProxy
        self.io
            .write_all(&encoded.avif_file)
            .map_err(|e| nerror!(ErrorKind::EncodingIoError, "Failed to write AVIF data: {:?}", e))?;

        data.add("result.alpha_byte_size", &encoded.alpha_byte_size);
        data.add("result.color_byte_size", &encoded.color_byte_size);
        data.add("result.total_byte_size", &encoded.avif_file.len());

        // 11. Return encoding result metadata
        Ok(EncodeResult {
            w,
            h,
            io_id: self.io.io_id(),
            bytes: ::imageflow_types::ResultBytes::Elsewhere, // Data written to io
            preferred_extension: "avif".to_owned(),
            preferred_mime_type: "image/avif".to_owned(),
            diagnostic_data: data.into()
        })
    }

    fn get_io(&self) -> Result<IoProxyRef<'_>> {
        Ok(IoProxyRef::Borrow(&self.io))
    }

    fn into_io(self: Box<Self>) -> Result<IoProxy> {
        // AVIF encoder writes all data during write_frame, no additional cleanup needed
        Ok(self.io)
    }
}
