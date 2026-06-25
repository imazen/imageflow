use super::s::{EncodeResult, EncoderPreset};
use super::Encoder;
use crate::io::IoProxy;

use crate::graphics::bitmaps::BitmapKey;
use crate::{Context, ErrorKind, FlowError, Result};
use imageflow_types::PixelFormat;
use std::io::Write;

use zc::encode::{DynAnimationFrameEncoder, DynEncoderConfig, EncodeOutput};
use zc::StopToken;
// Bare `Stop` / `StopToken` to thread per-call cancellation without qualification noise.
use enough::Stop;

use super::zen_decoder::{
    build_pool, check_stage_memory, encode_limits_capped, install_pooled, run_pooled, ZenDecoder,
};

/// Encoding strategy — native JPEG path for backward compat, zencodec for everything else.
enum EncodeMode {
    /// Zencodec dyn dispatch (WebP, GIF, JXL, etc.)
    Zencodec(Box<dyn DynEncoderConfig>),
    /// Native zenjpeg streaming encoder (preserves exact output from old adapter)
    NativeJpeg { config: zenjpeg::encoder::EncoderConfig },
}

/// Unified encoder for all zen codec formats.
///
/// Uses zencodec-types dyn dispatch for WebP, GIF, JXL, AVIF (and eventually PNG).
/// JPEG uses the native zenjpeg streaming API for exact backward compatibility.
pub struct ZenEncoder {
    mode: EncodeMode,
    io: IoProxy,
    matte: Option<imageflow_types::Color>,
    // Persistent frame encoder for animation
    frame_enc: Option<Box<dyn DynAnimationFrameEncoder>>,
    // Whether this format supports animation (GIF, WebP, JXL)
    supports_animation: bool,
    // Format metadata
    preferred_extension: &'static str,
    preferred_mime_type: &'static str,
    // Cached max_threads-sized rayon pool, reused across animation frames + finish
    // so the codec's par_iter is bounded without re-spawning a pool per frame.
    thread_pool: Option<rayon::ThreadPool>,
    // Cancellation token persisted at animation-encoder setup so the final
    // finish() (in into_io, which has no Context) still honors cancellation.
    stop_token: Option<StopToken>,
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
            thread_pool: None,
            stop_token: None,
        }
    }

    pub(crate) fn create_jpeg(
        c: &Context,
        io: IoProxy,
        quality: Option<u8>,
        progressive: Option<bool>,
        matte: Option<imageflow_types::Color>,
        generic_quality: Option<f32>,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::ZenJpegEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The ZenJpeg encoder has been disabled"
            ));
        }
        use zenjpeg::encoder::{ChromaSubsampling, Quality};
        // A quality_profile drives the native zenjpeg encoder in SSIMULACRA2 units
        // (Quality::ApproxSsim2, resolved through zenjpeg's ssim2->jpegli mapping);
        // explicit callers pass a libjpeg-turbo quality via `quality`.
        // mozjpeg's evalchroma::adjust_sampling stays at 4:2:0 even at q=90 for
        // typical content; only above ~90 (≈ ssim2 85) does it adopt 4:4:4. Match
        // that boundary so default-quality JPEG output agrees byte-class with the
        // c-codecs path.
        let (q_setting, full_chroma) = if let Some(gq) = generic_quality {
            (Quality::ApproxSsim2(gq), gq > 85.0)
        } else {
            let q = quality.unwrap_or(75).min(100);
            (Quality::ApproxMozjpeg(q), q > 90)
        };
        let subsampling =
            if full_chroma { ChromaSubsampling::None } else { ChromaSubsampling::Quarter };

        // Use the native EncoderConfig directly (not the zencodec JpegEncoderConfig wrapper)
        // to preserve exact backward compatibility with the old ZenJpegEncoder adapter.
        let mut config = zenjpeg::encoder::EncoderConfig::ycbcr(q_setting, subsampling)
            .auto_optimize(true)
            .progressive(progressive.unwrap_or(true));

        // Enable parallel encoding by default
        config = config.parallel(zenjpeg::encoder::ParallelEncoding::Auto);

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
            thread_pool: None,
            stop_token: None,
        })
    }

    /// Zen JPEG encoder configured for libjpeg-turbo-compatible semantics.
    ///
    /// Differs from `create_jpeg` (the Mozjpeg-style default) in three ways:
    /// - No adaptive quantization (`auto_optimize(false)`), matching classic libjpeg.
    /// - Optional Huffman optimization — default off, matching libjpeg-turbo's
    ///   single-pass Annex K behavior. mozjpeg-rs always optimizes, so we route
    ///   `LibjpegTurbo { optimize_huffman_coding: Some(false) }` through zenjpeg
    ///   specifically to honor the disable toggle.
    /// - Baseline (non-progressive) default, matching libjpeg-turbo.
    ///
    /// Quality scale is `Quality::ApproxMozjpeg` since libjpeg-turbo and mozjpeg
    /// share the same ~0–100 quality scale at the quantization-table level.
    pub(crate) fn create_jpeg_libjpeg_turbo_style(
        c: &Context,
        io: IoProxy,
        quality: Option<i32>,
        progressive: Option<bool>,
        optimize_huffman_coding: Option<bool>,
        matte: Option<imageflow_types::Color>,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::ZenJpegEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The ZenJpeg encoder has been disabled"
            ));
        }
        use zenjpeg::encoder::{ChromaSubsampling, Quality};
        let q = quality.unwrap_or(100).clamp(0, 100) as u8;
        let subsampling =
            if q >= 90 { ChromaSubsampling::None } else { ChromaSubsampling::Quarter };

        let mut config =
            zenjpeg::encoder::EncoderConfig::ycbcr(Quality::ApproxMozjpeg(q), subsampling)
                .auto_optimize(false)
                .optimize_huffman(optimize_huffman_coding.unwrap_or(false))
                .progressive(progressive.unwrap_or(false));

        config = config.parallel(zenjpeg::encoder::ParallelEncoding::Auto);

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
            thread_pool: None,
            stop_token: None,
        })
    }

    pub(crate) fn create_webp(
        c: &Context,
        io: IoProxy,
        quality: Option<f32>,
        lossless: Option<bool>,
        matte: Option<imageflow_types::Color>,
        generic_quality: Option<f32>,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::ZenWebPEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The ZenWebP encoder has been disabled"
            ));
        }
        use zc::encode::EncoderConfig as _;
        let lossless = lossless.unwrap_or(false);
        let config = if lossless {
            // Lossless "quality" is an effort/compression knob, not perceptual, so a
            // generic (ssim2) quality is meaningless here — keep the native value.
            zenwebp::zencodec::WebpEncoderConfig::lossless().with_quality(quality.unwrap_or(85.0))
        } else if let Some(gq) = generic_quality {
            zenwebp::zencodec::WebpEncoderConfig::lossy().with_generic_quality(gq)
        } else {
            zenwebp::zencodec::WebpEncoderConfig::lossy().with_quality(quality.unwrap_or(85.0))
        };

        Ok(Self::new_zencodec(
            Box::new(config),
            io,
            matte,
            // zenwebp ≥0.4.3 downgrades a 1-frame animation container to a
            // static WebP in AnimationEncoder::finalize(), so routing every
            // WebP through the animation path is safe for single-frame
            // inputs too. This lets multi-frame inputs (e.g. animated GIF →
            // WebP) preserve animation without per-call frame-count signaling.
            true,
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

    pub(crate) fn create_avif(
        c: &Context,
        io: IoProxy,
        quality: Option<f32>,
        speed: Option<u8>,
        lossless: bool,
        matte: Option<imageflow_types::Color>,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::ZenAvifEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The ZenAvif encoder has been disabled"
            ));
        }
        use zc::encode::EncoderConfig as _;
        let mut config = zenavif::AvifEncoderConfig::new();
        if lossless {
            config = config.with_lossless(true);
        } else {
            let q = quality.unwrap_or(75.0).clamp(0.0, 100.0);
            config = config.with_generic_quality(q);
        }
        if let Some(s) = speed {
            config = config.with_effort_u32(s as u32);
        }

        // AVIF doesn't support alpha in lossy mode without extra work — apply matte if set
        Ok(Self::new_zencodec(
            Box::new(config),
            io,
            matte,
            false, // AVIF animation not yet supported
            "avif",
            "image/avif",
        ))
    }

    pub(crate) fn create_mozjpeg_rs(
        c: &Context,
        io: IoProxy,
        quality: Option<u8>,
        progressive: Option<bool>,
        matte: Option<imageflow_types::Color>,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::MozjpegRsEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The mozjpeg-rs encoder has been disabled"
            ));
        }
        use zc::encode::EncoderConfig as _;
        let q = quality.unwrap_or(85).min(100);
        let effort = if progressive.unwrap_or(true) { 2 } else { 1 };
        let config = mozjpeg_rs::MozjpegEncoderConfig::new()
            .with_generic_quality(q as f32)
            .with_generic_effort(effort);
        // JPEG doesn't support alpha — always apply matte (default white)
        let matte = Some(matte.unwrap_or(imageflow_types::Color::Srgb(
            imageflow_types::ColorSrgb::Hex("FFFFFFFF".to_owned()),
        )));
        Ok(Self::new_zencodec(Box::new(config), io, matte, false, "jpg", "image/jpeg"))
    }

    pub(crate) fn create_jxl(
        c: &Context,
        io: IoProxy,
        distance: Option<f32>,
        lossless: bool,
        generic_quality: Option<f32>,
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
        } else if let Some(gq) = generic_quality {
            // quality_profile → SSIMULACRA2-unit generic quality (zenjxl calibrates
            // it to a butteraugli distance internally).
            zenjxl::JxlEncoderConfig::new().with_generic_quality(gq)
        } else {
            // Explicit distance: 0.0-25.0 → quality 0-100 (distance = (100-q) * 0.25)
            let d = distance.unwrap_or(1.0);
            let quality = (100.0 - d * 4.0).clamp(0.0, 100.0);
            zenjxl::JxlEncoderConfig::new().with_generic_quality(quality)
        };
        Ok(Self::new_zencodec(Box::new(config), io, None, false, "jxl", "image/jxl"))
    }

    pub(crate) fn create_bmp(
        c: &Context,
        io: IoProxy,
        matte: Option<imageflow_types::Color>,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::ZenBmpEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The ZenBmp encoder has been disabled"
            ));
        }
        let config = zenbitmaps::BmpEncoderConfig::new();
        Ok(Self::new_zencodec(Box::new(config), io, matte, false, "bmp", "image/bmp"))
    }

    pub(crate) fn create_png(
        c: &Context,
        io: IoProxy,
        matte: Option<imageflow_types::Color>,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::ZenPngEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The ZenPng encoder has been disabled"
            ));
        }
        let config = zenpng::PngEncoderConfig::new();
        Ok(Self::new_zencodec(Box::new(config), io, matte, false, "png", "image/png"))
    }
}

impl ZenEncoder {
    /// Write encoded output to the IoProxy. Uses zero-copy swap for empty
    /// memory-backed output buffers; falls back to write_all otherwise.
    fn write_output(io: &mut IoProxy, output: EncodeOutput) -> Result<()> {
        if io.can_swap_output() {
            io.swap_output_vec(output.into_vec());
            Ok(())
        } else {
            io.write_all(output.data()).map_err(|e| FlowError::from_encoder(e).at(here!()))
        }
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

            if self.io.can_swap_output() {
                self.io.swap_output_vec(jpeg_bytes);
            } else {
                self.io
                    .write_all(&jpeg_bytes)
                    .map_err(|e| FlowError::from_encoder(e).at(here!()))?;
            }

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
        let slice = window.slice_mut();

        // Pre-flight encode memory gate: codec working-set estimate + source buffer.
        if let (EncodeMode::Zencodec(config), Some(policy)) =
            (&self.mode, c.security.mem_budget_policy.as_ref())
        {
            let chars =
                zc::estimate::ImageCharacteristics::new(w, h, zenpixels::PixelDescriptor::BGRA8_SRGB);
            let mut env = zc::estimate::ComputeEnvironment::new();
            if let Some(n) = c.security.max_threads {
                env = env.with_cores(n as usize);
            }
            let est = config.estimate_encode_resources(&chars, &env);
            if let Err((name, limit, actual)) = check_stage_memory(policy, &est, w, h) {
                return Err(nerror!(
                    ErrorKind::ImageEncodingError,
                    "encode memory budget {} exceeded: estimated {} bytes >= limit {} bytes",
                    name,
                    actual,
                    limit
                ));
            }
        }

        // Create frame encoder on first call for animation-capable formats
        if self.supports_animation && self.frame_enc.is_none() {
            let config = match &self.mode {
                EncodeMode::Zencodec(config) => config,
                _ => unreachable!(),
            };
            let mut job = config.dyn_job();
            job.set_stop(StopToken::new(c.cancellation_token()));
            if let Some(limits) = encode_limits_capped(&c.security, w, h) {
                job.set_limits(limits);
            }

            // Try to get loop count from decoder and set on the job
            for io_id in decoder_io_ids {
                if let Ok(mut codec) = c.get_codec(*io_id)
                    && let Ok(decoder) = codec.get_decoder()
                    && let Some(d) = decoder.as_any().downcast_ref::<ZenDecoder>()
                {
                    if let Some(lc) = d.get_loop_count() {
                        job.set_loop_count(Some(lc));
                    }
                    break;
                }
            }

            let frame_enc = job.into_animation_frame_encoder().map_err(|e| {
                nerror!(
                    ErrorKind::ImageEncodingError,
                    "{} frame encoder create error: {}",
                    self.preferred_extension,
                    e
                )
            })?;

            self.frame_enc = Some(frame_enc);
            // Bound this format's per-frame par_iter to max_threads — spawned once
            // and reused for every push_frame + the final finish().
            self.thread_pool = build_pool(c.security.max_threads);
            // Persist a cancellation token so finish() (into_io has no Context)
            // can still honor cancellation, mirroring the per-frame push below.
            self.stop_token = Some(StopToken::new(c.cancellation_token()));
        }

        // Animation frame path.
        //
        // Pick the encoder's preferred 4bpp layout: BGRA if the encoder lists
        // it (zenpng/zenwebp/zengif/zenjxl/zenavif all do), else RGBA with a
        // one-pass in-place swizzle (mozjpeg-rs only lists RGB8/RGBA8/Gray8).
        //
        // Alpha handling when make_opaque is set:
        //   - native_alpha codecs (PNG/WebP/GIF/JXL/AVIF) compress whatever
        //     alpha bytes we pass into the output alpha channel. Garbage
        //     bytes = garbage output. Fill to 0xFF so the encoded alpha is
        //     valid and highly compressible.
        //   - !native_alpha codecs (JPEG) discard alpha entirely; filling is
        //     wasted work.
        // We don't set AlphaMode::Undefined/Opaque because zen codecs use
        // exact-descriptor equality (desc == BGRA8_SRGB) and reject variants.
        if let Some(frame_enc) = self.frame_enc.as_mut() {
            let config = match &self.mode {
                EncodeMode::Zencodec(config) => config,
                _ => unreachable!(),
            };
            let use_bgra = config
                .supported_descriptors()
                .contains(&zenpixels::PixelDescriptor::BGRA8_SRGB);
            let needs_alpha_fill = make_opaque && config.capabilities().native_alpha();
            let desc = if use_bgra {
                if needs_alpha_fill {
                    let _ = garb::bytes::fill_alpha_bgra_strided(
                        slice, w as usize, h as usize, stride,
                    );
                }
                zenpixels::PixelDescriptor::BGRA8_SRGB
            } else {
                let _ = garb::bytes::bgra_to_rgba_inplace_strided(
                    slice, w as usize, h as usize, stride,
                );
                if needs_alpha_fill {
                    let _ = garb::bytes::fill_alpha_rgba_strided(
                        slice, w as usize, h as usize, stride,
                    );
                }
                zenpixels::PixelDescriptor::RGBA8_SRGB
            };

            let mut delay_ms = 100u32;
            for io_id in decoder_io_ids {
                if let Ok(mut codec) = c.get_codec(*io_id)
                    && let Ok(decoder) = codec.get_decoder()
                    && let Some(d) = decoder.as_any().downcast_ref::<ZenDecoder>()
                {
                    if let Some(d) = d.last_frame_delay() {
                        delay_ms = d as u32 * 10;
                    }
                    break;
                }
            }

            let ps = zenpixels::PixelSlice::new(slice, w, h, stride, desc)
                .map_err(|e| nerror!(ErrorKind::ImageEncodingError, "pixel slice error: {}", e))?;

            // Thread the per-call stop: animation frame encoders honor only this
            // argument, not the job stop, so None would leave animated encode
            // uncancellable.
            let stop = c.cancellation_token();
            install_pooled(&self.thread_pool, move || {
                frame_enc.push_frame(ps, delay_ms, Some(&stop as &dyn Stop))
            })
            .map_err(|e| {
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

        // Single-frame encode. Same descriptor negotiation and alpha rules as
        // the animation path above.
        let config = match &self.mode {
            EncodeMode::Zencodec(config) => config,
            EncodeMode::NativeJpeg { .. } => unreachable!(),
        };
        let use_bgra = config
            .supported_descriptors()
            .contains(&zenpixels::PixelDescriptor::BGRA8_SRGB);
        let needs_alpha_fill = make_opaque && config.capabilities().native_alpha();
        let desc = if use_bgra {
            if needs_alpha_fill {
                let _ = garb::bytes::fill_alpha_bgra_strided(
                    slice, w as usize, h as usize, stride,
                );
            }
            zenpixels::PixelDescriptor::BGRA8_SRGB
        } else {
            let _ = garb::bytes::bgra_to_rgba_inplace_strided(
                slice, w as usize, h as usize, stride,
            );
            if needs_alpha_fill {
                let _ = garb::bytes::fill_alpha_rgba_strided(
                    slice, w as usize, h as usize, stride,
                );
            }
            zenpixels::PixelDescriptor::RGBA8_SRGB
        };

        let mut job = config.dyn_job();
        job.set_stop(StopToken::new(c.cancellation_token()));
        if let Some(limits) = encode_limits_capped(&c.security, w, h) {
            job.set_limits(limits);
        }
        let encoder = job.into_encoder().map_err(|e| {
            nerror!(
                ErrorKind::ImageEncodingError,
                "{} encoder create error: {}",
                self.preferred_extension,
                e
            )
        })?;

        let ps = zenpixels::PixelSlice::new(slice, w, h, stride, desc)
            .map_err(|e| nerror!(ErrorKind::ImageEncodingError, "pixel slice error: {}", e))?;
        // Bound the encoder's rayon parallelism to the job's max_threads.
        let output = run_pooled(c.security.max_threads, move || encoder.encode(ps)).map_err(|e| {
            nerror!(
                ErrorKind::ImageEncodingError,
                "{} encode error: {}",
                self.preferred_extension,
                e
            )
        })?;

        Self::write_output(&mut self.io, output)?;

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
            // Persisted at setup (into_io has no Context); honored by codecs that
            // check the per-call stop during the final flush/encode.
            let stop = self.stop_token.clone();
            let output = install_pooled(&self.thread_pool, move || {
                frame_enc.finish(stop.as_ref().map(|s| s as &dyn Stop))
            })
            .map_err(|e| {
                nerror!(
                    ErrorKind::ImageEncodingError,
                    "{} finish error: {}",
                    self.preferred_extension,
                    e
                )
            })?;
            let mut io = self.io;
            Self::write_output(&mut io, output)?;
            Ok(io)
        } else {
            Ok(self.io)
        }
    }
}
