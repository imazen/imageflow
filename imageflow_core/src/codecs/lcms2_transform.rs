use crate::codecs::source_profile::SourceProfile;
use crate::codecs::tiny_lru::TinyLru;
use crate::errors::{ErrorCategory, ErrorKind, FlowError, Result};
use crate::graphics::bitmaps::BitmapWindowMut;
use imageflow_types::PixelLayout;
use lcms2::*;
use std::cell::RefCell;

static PROFILE_TRANSFORMS: TinyLru<Transform<u32, u32, ThreadContext, DisallowCache>> =
    TinyLru::new(9);
static GRAY_TRANSFORMS: TinyLru<Transform<u8, u32, ThreadContext, DisallowCache>> = TinyLru::new(4);
static GAMA_TRANSFORMS: TinyLru<Transform<u32, u32, ThreadContext, DisallowCache>> =
    TinyLru::new(4);

const HASH_SEED: u64 = 0x8ed1_2ad9_483d_28a0;

thread_local!(static LAST_PROFILE_ERROR_MESSAGE: RefCell<Option<String>> = const { RefCell::new(None) });

pub struct Lcms2TransformCache;

impl Lcms2TransformCache {
    unsafe extern "C" fn error_logger(
        _context_id: lcms2_sys::Context,
        error_code: u32,
        text: *const core::ffi::c_char,
    ) {
        if text.is_null() {
            return;
        }
        let text_str =
            std::ffi::CStr::from_ptr(text).to_str().unwrap_or("LCMS error message not valid UTF8");
        let message = format!("Error {}: {}", error_code, text_str);

        LAST_PROFILE_ERROR_MESSAGE.with(|m| {
            *m.borrow_mut() = Some(message);
        })
    }

    fn create_thread_context() -> ThreadContext {
        let mut context = ThreadContext::new();
        context.set_error_logging_function(Some(Lcms2TransformCache::error_logger));
        context
    }

    fn get_lcms_error(error: lcms2::Error) -> FlowError {
        LAST_PROFILE_ERROR_MESSAGE.with(|m| {
            let error = if let Some(message) = m.borrow().as_ref() {
                FlowError::without_location(
                    ErrorKind::ColorProfileError,
                    format!("{} ({:?})", message, error),
                )
            } else {
                FlowError::without_location(ErrorKind::ColorProfileError, format!("{:?}", error))
            };
            *m.borrow_mut() = None;
            error
        })
    }

    /// Apply a color transform from `profile` to sRGB on the given BGRA frame.
    pub fn transform_to_srgb(
        frame: &mut BitmapWindowMut<u8>,
        profile: &SourceProfile,
    ) -> Result<()> {
        if profile.is_srgb() {
            return Ok(());
        }
        if frame.info().pixel_layout() != PixelLayout::BGRA {
            return Err(nerror!(
                ErrorKind::Category(ErrorCategory::InternalError),
                "Color profile application is only supported for Bgr32 and Bgra32 canvases"
            ));
        }

        match profile {
            SourceProfile::Srgb => Ok(()),
            SourceProfile::IccProfile(bytes) => {
                Self::transform_icc(frame, bytes, PixelFormat::BGRA_8, PixelFormat::BGRA_8)
            }
            SourceProfile::IccProfileGray(bytes) => Self::transform_icc_gray(frame, bytes),
            SourceProfile::CmykIcc(bytes) => {
                Self::transform_icc(frame, bytes, PixelFormat::CMYK_8_REV, PixelFormat::BGRA_8)
            }
            SourceProfile::GammaPrimaries {
                gamma,
                white_x,
                white_y,
                red_x,
                red_y,
                green_x,
                green_y,
                blue_x,
                blue_y,
            } => Self::transform_gama(
                frame, *gamma, *white_x, *white_y, *red_x, *red_y, *green_x, *green_y, *blue_x,
                *blue_y,
            ),
            SourceProfile::Cicp { .. } => Err(nerror!(
                ErrorKind::ColorProfileError,
                "CICP color profiles are not supported by the lcms2 backend"
            )),
        }
    }

    fn transform_icc(
        frame: &mut BitmapWindowMut<u8>,
        icc_bytes: &[u8],
        input_pixel_format: PixelFormat,
        output_pixel_format: PixelFormat,
    ) -> Result<()> {
        let hash = Self::hash_icc(icc_bytes, input_pixel_format, output_pixel_format);
        // try_get_or_create_apply: the closure receives &Transform while the lock
        // is held, avoiding the need for Clone on lcms2 Transform.
        PROFILE_TRANSFORMS.try_get_or_create_apply(
            hash,
            || Self::create_icc_transform(icc_bytes, input_pixel_format, output_pixel_format),
            |t| Self::apply_transform(frame, t),
        )
    }

    fn transform_icc_gray(frame: &mut BitmapWindowMut<u8>, icc_bytes: &[u8]) -> Result<()> {
        let hash = Self::hash_icc(icc_bytes, PixelFormat::GRAY_8, PixelFormat::BGRA_8);
        GRAY_TRANSFORMS.try_get_or_create_apply(
            hash,
            || Self::create_icc_transform_gray(icc_bytes),
            |t| Self::apply_gray_transform(frame, t),
        )
    }

    fn create_icc_transform_gray(
        icc_bytes: &[u8],
    ) -> Result<Transform<u8, u32, ThreadContext, DisallowCache>> {
        let srgb = Profile::new_srgb_context(Self::create_thread_context());
        let p = Profile::new_icc_context(Self::create_thread_context(), icc_bytes)
            .map_err(|e| Self::get_lcms_error(e).at(here!()))?;

        Transform::new_flags_context(
            Self::create_thread_context(),
            &p,
            PixelFormat::GRAY_8,
            &srgb,
            PixelFormat::BGRA_8,
            Intent::Perceptual,
            Flags::NO_CACHE,
        )
        .map_err(|e| Self::get_lcms_error(e).at(here!()))
    }

    fn apply_gray_transform(
        frame: &mut BitmapWindowMut<u8>,
        transform: &Transform<u8, u32, ThreadContext, DisallowCache>,
    ) {
        let w = frame.w() as usize;
        let mut gray_buf = vec![0u8; w];
        let mut bgra_buf = vec![0u32; w];
        for mut line in frame.scanlines_u32().unwrap() {
            let row = line.row();
            // Save alpha values (high byte of each BGRA u32 on LE)
            let alphas: Vec<u32> = row.iter().map(|&px| px & 0xFF00_0000).collect();
            // Extract gray value (B channel) from each BGRA pixel.
            // Mozjpeg decodes grayscale as [gray, gray, gray, 255] in BGRA order.
            for (i, &px) in row.iter().enumerate() {
                gray_buf[i] = (px & 0xFF) as u8;
            }
            // GRAY_8 → BGRA_8 transform
            transform.transform_pixels(&gray_buf, &mut bgra_buf);
            // Restore original alpha (lcms2 GRAY→BGRA may zero it)
            for (px, a) in bgra_buf.iter_mut().zip(alphas.iter()) {
                *px = (*px & 0x00FF_FFFF) | a;
            }
            line.row_mut().copy_from_slice(&bgra_buf);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn transform_gama(
        frame: &mut BitmapWindowMut<u8>,
        gamma: f64,
        white_x: f64,
        white_y: f64,
        red_x: f64,
        red_y: f64,
        green_x: f64,
        green_y: f64,
        blue_x: f64,
        blue_y: f64,
    ) -> Result<()> {
        let pixel_format = PixelFormat::BGRA_8;
        let hash = Self::hash_gama(
            gamma, white_x, white_y, red_x, red_y, green_x, green_y, blue_x, blue_y,
        );
        GAMA_TRANSFORMS.try_get_or_create_apply(
            hash,
            || {
                Self::create_gama_transform(
                    gamma,
                    white_x,
                    white_y,
                    red_x,
                    red_y,
                    green_x,
                    green_y,
                    blue_x,
                    blue_y,
                    pixel_format,
                )
            },
            |t| Self::apply_transform(frame, t),
        )
    }

    fn create_icc_transform(
        icc_bytes: &[u8],
        input_pixel_format: PixelFormat,
        output_pixel_format: PixelFormat,
    ) -> Result<Transform<u32, u32, ThreadContext, DisallowCache>> {
        let srgb = Profile::new_srgb_context(Self::create_thread_context());
        let p = Profile::new_icc_context(Self::create_thread_context(), icc_bytes)
            .map_err(|e| Self::get_lcms_error(e).at(here!()))?;

        Transform::new_flags_context(
            Self::create_thread_context(),
            &p,
            input_pixel_format,
            &srgb,
            output_pixel_format,
            Intent::Perceptual,
            Flags::NO_CACHE,
        )
        .map_err(|e| Self::get_lcms_error(e).at(here!()))
    }

    #[allow(clippy::too_many_arguments)]
    fn create_gama_transform(
        gamma: f64,
        white_x: f64,
        white_y: f64,
        red_x: f64,
        red_y: f64,
        green_x: f64,
        green_y: f64,
        blue_x: f64,
        blue_y: f64,
        pixel_format: PixelFormat,
    ) -> Result<Transform<u32, u32, ThreadContext, DisallowCache>> {
        let srgb = Profile::new_srgb_context(Self::create_thread_context());
        let gama = ToneCurve::new(1f64 / gamma);
        let white_point = lcms2::CIExyY { x: white_x, y: white_y, Y: 1.0 };
        let primaries = lcms2::CIExyYTRIPLE {
            Red: lcms2::CIExyY { x: red_x, y: red_y, Y: 1.0 },
            Green: lcms2::CIExyY { x: green_x, y: green_y, Y: 1.0 },
            Blue: lcms2::CIExyY { x: blue_x, y: blue_y, Y: 1.0 },
        };
        let p = Profile::new_rgb_context(
            Self::create_thread_context(),
            &white_point,
            &primaries,
            &[&gama, &gama, &gama],
        )
        .map_err(|e| Self::get_lcms_error(e).at(here!()))?;

        Transform::new_flags_context(
            Self::create_thread_context(),
            &p,
            pixel_format,
            &srgb,
            pixel_format,
            Intent::Perceptual,
            Flags::NO_CACHE,
        )
        .map_err(|e| Self::get_lcms_error(e).at(here!()))
    }

    fn apply_transform(
        frame: &mut BitmapWindowMut<u8>,
        transform: &Transform<u32, u32, ThreadContext, DisallowCache>,
    ) {
        for mut line in frame.scanlines_u32().unwrap() {
            let row = line.row_mut();
            // lcms2 may zero the alpha channel when transforming RGB ICC profiles
            // via BGRA_8 pixel format. Save alpha before transform and restore after.
            // BGRA as u32 little-endian: alpha is the high byte (bits 24-31).
            let alphas: Vec<u32> = row.iter().map(|&px| px & 0xFF00_0000).collect();
            transform.transform_in_place(row);
            for (px, a) in row.iter_mut().zip(alphas.iter()) {
                *px = (*px & 0x00FF_FFFF) | a;
            }
        }
    }

    // ---- Hashing helpers ----

    fn hash_icc(
        icc_bytes: &[u8],
        input_pixel_format: PixelFormat,
        output_pixel_format: PixelFormat,
    ) -> u64 {
        use std::hash::Hasher;
        let mut h = twox_hash::XxHash64::with_seed(HASH_SEED);
        // ICC header (128 bytes): selectively hash mathematical fields, skip metadata.
        // Bytes 8-23: version, deviceClass, colorSpace, PCS (affect transform interpretation)
        // Bytes 64-79: renderingIntent, illuminant (affect color math)
        // Skip: size, cmmId, date, magic, platform, flags, manufacturer, model,
        //        attributes, creator, profileID, reserved (metadata only)
        if icc_bytes.len() >= 128 {
            h.write(&icc_bytes[8..24]);
            h.write(&icc_bytes[64..80]);
            h.write(&icc_bytes[128..]);
        } else {
            h.write(icc_bytes);
        }
        // Mix in pixel formats to avoid collisions between different format transforms
        h.write_u32((input_pixel_format.0 << 16) ^ output_pixel_format.0);
        h.finish()
    }

    #[allow(clippy::too_many_arguments)]
    fn hash_gama(
        gamma: f64,
        white_x: f64,
        white_y: f64,
        red_x: f64,
        red_y: f64,
        green_x: f64,
        green_y: f64,
        blue_x: f64,
        blue_y: f64,
    ) -> u64 {
        use std::hash::Hasher;
        let mut h = twox_hash::XxHash64::with_seed(HASH_SEED);
        for v in [gamma, white_x, white_y, red_x, red_y, green_x, green_y, blue_x, blue_y] {
            h.write(&v.to_ne_bytes());
        }
        h.finish()
    }
}
