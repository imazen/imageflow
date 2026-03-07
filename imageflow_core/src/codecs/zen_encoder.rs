use super::s::{EncodeResult, EncoderPreset};
use super::Encoder;
use crate::io::IoProxy;

use crate::graphics::bitmaps::BitmapKey;
use crate::{Context, ErrorKind, FlowError, Result};
use imageflow_types::PixelFormat;
use std::io::Write;

use zc::encode::{DynEncoderConfig, DynFrameEncoder};

use super::zen_decoder::ZenDecoder;

/// Encoding strategy — native JPEG path for backward compat, zencodec for everything else.
enum EncodeMode {
    /// Zencodec dyn dispatch (WebP, GIF, JXL, etc.)
    Zencodec(Box<dyn DynEncoderConfig>),
    /// Native zenjpeg streaming encoder (preserves exact output from old adapter)
    NativeJpeg { config: zenjpeg::encoder::EncoderConfig },
}

/// Unified encoder for all zen codec formats.
///
/// Uses zencodec-types dyn dispatch for WebP, GIF, JXL (and eventually AVIF, PNG).
/// JPEG uses the native zenjpeg streaming API for exact backward compatibility.
pub struct ZenEncoder {
    mode: EncodeMode,
    io: IoProxy,
    matte: Option<imageflow_types::Color>,
    // Persistent frame encoder for animation
    frame_enc: Option<Box<dyn DynFrameEncoder>>,
    // Whether this format supports animation (GIF, WebP, JXL)
    supports_animation: bool,
    // Format metadata
    preferred_extension: &'static str,
    preferred_mime_type: &'static str,
}

impl ZenEncoder {
    fn new_zencodec(
        config: Box<dyn DynEncoderConfig>,
        io: IoProxy,
        matte: Option<imageflow_types::Color>,
        supports_animation: bool,
        preferred_extension: &'static str,
        preferred_mime_type: &'static str,
    ) -> Self {
        ZenEncoder {
            mode: EncodeMode::Zencodec(config),
            io,
            matte,
            frame_enc: None,
            supports_animation,
            preferred_extension,
            preferred_mime_type,
        }
    }

    pub(crate) fn create_jpeg(
        c: &Context,
        io: IoProxy,
        quality: Option<u8>,
        progressive: Option<bool>,
        matte: Option<imageflow_types::Color>,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::ZenJpegEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The ZenJpeg encoder has been disabled"
            ));
        }
        use zenjpeg::encoder::{ChromaSubsampling, Quality};
        let q = quality.unwrap_or(75).min(100);
        let subsampling =
            if q >= 90 { ChromaSubsampling::None } else { ChromaSubsampling::Quarter };

        // Use the native EncoderConfig directly (not the zencodec JpegEncoderConfig wrapper)
        // to preserve exact backward compatibility with the old ZenJpegEncoder adapter.
        let mut config =
            zenjpeg::encoder::EncoderConfig::ycbcr(Quality::ApproxMozjpeg(q), subsampling)
                .auto_optimize(true)
                .progressive(progressive.unwrap_or(true));

        if let Some(threads) = c.security.max_encoder_threads {
            if threads > 1 {
                config = config.parallel(zenjpeg::encoder::ParallelEncoding::Auto);
            }
        }

        // JPEG doesn't support alpha — always apply matte (default white)
        let matte = Some(matte.unwrap_or(imageflow_types::Color::Srgb(
            imageflow_types::ColorSrgb::Hex("FFFFFFFF".to_owned()),
        )));

        Ok(ZenEncoder {
            mode: EncodeMode::NativeJpeg { config },
            io,
            matte,
            frame_enc: None,
            supports_animation: false,
            preferred_extension: "jpg",
            preferred_mime_type: "image/jpeg",
        })
    }

    pub(crate) fn create_webp(
        c: &Context,
        io: IoProxy,
        quality: Option<f32>,
        lossless: Option<bool>,
        matte: Option<imageflow_types::Color>,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::ZenWebPEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The ZenWebP encoder has been disabled"
            ));
        }
        let lossless = lossless.unwrap_or(false);
        let config = if lossless {
            zenwebp::WebpEncoderConfig::lossless().with_quality(quality.unwrap_or(85.0))
        } else {
            zenwebp::WebpEncoderConfig::lossy().with_quality(quality.unwrap_or(85.0))
        };

        Ok(Self::new_zencodec(
            Box::new(config),
            io,
            matte,
            false, // WebP animation not yet supported
            "webp",
            "image/webp",
        ))
    }

    pub(crate) fn create_gif(
        c: &Context,
        io: IoProxy,
        _first_frame_key: BitmapKey,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::ZenGifEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The zengif encoder has been disabled"
            ));
        }
        let config = zengif::GifEncoderConfig::new();
        Ok(Self::new_zencodec(
            Box::new(config),
            io,
            None,
            true, // GIF always supports animation
            "gif",
            "image/gif",
        ))
    }

    pub(crate) fn create_jxl(
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
        use zc::encode::EncoderConfig as _;
        let config = if lossless {
            zenjxl::JxlEncoderConfig::new().with_lossless(true)
        } else {
            // Convert JXL distance (0.0–25.0) to generic quality (0–100)
            // Mapping: distance = (100.0 - quality) * 0.25
            let d = distance.unwrap_or(1.0);
            let quality = (100.0 - d * 4.0).clamp(0.0, 100.0);
            zenjxl::JxlEncoderConfig::new().with_generic_quality(quality)
        };
        Ok(Self::new_zencodec(
            Box::new(config),
            io,
            None,
            false, // JXL animation not yet supported
            "jxl",
            "image/jxl",
        ))
    }
}

impl Encoder for ZenEncoder {
    fn write_frame(
        &mut self,
        c: &Context,
        _preset: &EncoderPreset,
        bitmap_key: BitmapKey,
        decoder_io_ids: &[i32],
    ) -> Result<EncodeResult> {
        return_if_cancelled!(c);

        // Determine encoding path before borrowing any mutable state.
        // Clone the JPEG config (cheap) or create the DynEncoder (releases borrow on self.mode).
        let jpeg_config = match &self.mode {
            EncodeMode::NativeJpeg { config } => Some(config.clone()),
            EncodeMode::Zencodec(_) => None,
        };

        if let Some(config) = jpeg_config {
            // ── Native JPEG path ──
            let bitmaps = c.borrow_bitmaps().map_err(|e| e.at(here!()))?;
            let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;

            // Always apply matte for JPEG (alpha not supported)
            bitmap
                .get_window_bgra32()
                .unwrap()
                .apply_matte(self.matte.clone().unwrap_or(imageflow_types::Color::Srgb(
                    imageflow_types::ColorSrgb::Hex("FFFFFFFF".to_owned()),
                )))
                .map_err(|e| e.at(here!()))?;
            bitmap.set_alpha_meaningful(false);

            let mut window = bitmap.get_window_u8().unwrap();

            let pixel_layout = match window.pixel_format() {
                PixelFormat::Bgra32 => zenjpeg::encoder::PixelLayout::Bgra8Srgb,
                PixelFormat::Bgr32 => zenjpeg::encoder::PixelLayout::Bgrx8Srgb,
                PixelFormat::Bgr24 => zenjpeg::encoder::PixelLayout::Bgr8Srgb,
                PixelFormat::Gray8 => zenjpeg::encoder::PixelLayout::Gray8Srgb,
            };

            let w = window.w() as usize;
            let h = window.h() as usize;
            let src_stride = window.info().t_stride() as usize;

            let mut encoder =
                config.encode_from_bytes(w as u32, h as u32, pixel_layout).map_err(|e| {
                    nerror!(ErrorKind::ImageEncodingError, "zenjpeg config error: {}", e)
                })?;

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

            let jpeg_bytes = encoder.finish().map_err(|e| {
                nerror!(ErrorKind::ImageEncodingError, "zenjpeg finish error: {}", e)
            })?;

            self.io.write_all(&jpeg_bytes).map_err(|e| FlowError::from_encoder(e).at(here!()))?;

            return Ok(EncodeResult {
                w: window.w_i32(),
                h: window.h_i32(),
                io_id: self.io.io_id(),
                bytes: ::imageflow_types::ResultBytes::Elsewhere,
                preferred_extension: "jpg".to_owned(),
                preferred_mime_type: "image/jpeg".to_owned(),
            });
        }

        // ── Zencodec path (WebP, GIF, JXL, etc.) ──
        let bitmaps = c.borrow_bitmaps().map_err(|e| e.at(here!()))?;
        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;

        // Apply matte if set
        if self.matte.is_some() {
            bitmap
                .get_window_bgra32()
                .unwrap()
                .apply_matte(self.matte.clone().unwrap_or(imageflow_types::Color::Srgb(
                    imageflow_types::ColorSrgb::Hex("FFFFFFFF".to_owned()),
                )))
                .map_err(|e| e.at(here!()))?;
            bitmap.set_alpha_meaningful(false);
        }

        // Get metadata before creating mutable window
        let make_opaque = !bitmap.info().alpha_meaningful();

        let mut window = bitmap.get_window_u8().unwrap();
        let (w, h) = (window.w(), window.h());
        let stride = window.info().t_stride() as usize;
        let fmt = window.pixel_format();
        let slice = window.slice_mut();

        // Create frame encoder on first call for animation-capable formats
        if self.supports_animation && self.frame_enc.is_none() {
            let config = match &self.mode {
                EncodeMode::Zencodec(config) => config,
                _ => unreachable!(),
            };
            let job = config.dyn_job();
            let mut frame_enc = job.into_frame_encoder().map_err(|e| {
                nerror!(
                    ErrorKind::ImageEncodingError,
                    "{} frame encoder create error: {}",
                    self.preferred_extension,
                    e
                )
            })?;

            // Try to get loop count from decoder
            for io_id in decoder_io_ids {
                if let Ok(mut codec) = c.get_codec(*io_id) {
                    if let Ok(decoder) = codec.get_decoder() {
                        if let Some(d) = decoder.as_any().downcast_ref::<ZenDecoder>() {
                            if let Some(lc) = d.get_loop_count() {
                                frame_enc.set_loop_count(Some(lc));
                            }
                            break;
                        }
                    }
                }
            }

            self.frame_enc = Some(frame_enc);
        }

        // Animation frame path — swizzle to RGBA for frame encoder
        if self.frame_enc.is_some() {
            // Swizzle BGRA→RGBA in-place for frame encoder
            match fmt {
                PixelFormat::Bgra32 | PixelFormat::Bgr32 => {
                    let _ = garb::bytes::bgra_to_rgba_inplace_strided(
                        slice, w as usize, h as usize, stride,
                    );
                }
                PixelFormat::Bgr24 | PixelFormat::Gray8 => {}
            }
            if make_opaque {
                let _ = garb::bytes::fill_alpha_rgba_strided(slice, w as usize, h as usize, stride);
            }

            let mut delay_ms = 100u32;
            for io_id in decoder_io_ids {
                if let Ok(mut codec) = c.get_codec(*io_id) {
                    if let Ok(decoder) = codec.get_decoder() {
                        if let Some(d) = decoder.as_any().downcast_ref::<ZenDecoder>() {
                            if let Some(d) = d.last_frame_delay() {
                                delay_ms = d as u32 * 10;
                            }
                            break;
                        }
                    }
                }
            }

            let frame_enc = self.frame_enc.as_mut().unwrap();
            let desc = zenpixels::PixelDescriptor::RGBA8_SRGB;
            let ps = zenpixels::PixelSlice::new(slice, w, h, stride, desc)
                .map_err(|e| nerror!(ErrorKind::ImageEncodingError, "pixel slice error: {}", e))?;

            frame_enc.push_frame(ps, delay_ms).map_err(|e| {
                nerror!(
                    ErrorKind::ImageEncodingError,
                    "{} frame encode error: {}",
                    self.preferred_extension,
                    e
                )
            })?;

            return Ok(EncodeResult {
                w: w as i32,
                h: h as i32,
                io_id: self.io.io_id(),
                bytes: ::imageflow_types::ResultBytes::Elsewhere,
                preferred_extension: self.preferred_extension.to_owned(),
                preferred_mime_type: self.preferred_mime_type.to_owned(),
            });
        }

        // Single-frame encode — use native BGRA pixel format (no swizzle needed)
        // Normalize alpha to 255 when not meaningful
        if make_opaque {
            let _ = garb::bytes::fill_alpha_bgra_strided(slice, w as usize, h as usize, stride);
        }

        let encoder = match &self.mode {
            EncodeMode::Zencodec(config) => {
                let job = config.dyn_job();
                job.into_encoder().map_err(|e| {
                    nerror!(
                        ErrorKind::ImageEncodingError,
                        "{} encoder create error: {}",
                        self.preferred_extension,
                        e
                    )
                })?
            }
            EncodeMode::NativeJpeg { .. } => unreachable!(),
        };

        let desc = zenpixels::PixelDescriptor::BGRA8_SRGB;
        let ps = zenpixels::PixelSlice::new(slice, w, h, stride, desc)
            .map_err(|e| nerror!(ErrorKind::ImageEncodingError, "pixel slice error: {}", e))?;
        let output = encoder.encode(ps).map_err(|e| {
            nerror!(
                ErrorKind::ImageEncodingError,
                "{} encode error: {}",
                self.preferred_extension,
                e
            )
        })?;

        self.io.write_all(output.data()).map_err(|e| FlowError::from_encoder(e).at(here!()))?;

        Ok(EncodeResult {
            w: w as i32,
            h: h as i32,
            io_id: self.io.io_id(),
            bytes: ::imageflow_types::ResultBytes::Elsewhere,
            preferred_extension: self.preferred_extension.to_owned(),
            preferred_mime_type: self.preferred_mime_type.to_owned(),
        })
    }

    fn into_io(self: Box<Self>) -> Result<IoProxy> {
        if let Some(frame_enc) = self.frame_enc {
            let output = frame_enc.finish().map_err(|e| {
                nerror!(
                    ErrorKind::ImageEncodingError,
                    "{} finish error: {}",
                    self.preferred_extension,
                    e
                )
            })?;
            let mut io = self.io;
            io.write_all(output.data()).map_err(|e| FlowError::from_encoder(e).at(here!()))?;
            Ok(io)
        } else {
            Ok(self.io)
        }
    }
}
