use crate::codecs::source_profile::SourceProfile;
use crate::errors::{ErrorCategory, ErrorKind, FlowError, Result};
use crate::graphics::bitmaps::BitmapWindowMut;
use dashmap::DashMap;
use imageflow_types::PixelLayout;
use lcms2::*;
use std::cell::RefCell;
use std::sync::*;

static PROFILE_TRANSFORMS: LazyLock<
    DashMap<u64, Transform<u32, u32, ThreadContext, DisallowCache>>,
> = LazyLock::new(|| DashMap::with_capacity(4));
static GAMA_TRANSFORMS: LazyLock<DashMap<u64, Transform<u32, u32, ThreadContext, DisallowCache>>> =
    LazyLock::new(|| DashMap::with_capacity(4));

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
            SourceProfile::IccProfileGray(bytes) => {
                Self::transform_icc(frame, bytes, PixelFormat::GRAY_8, PixelFormat::BGRA_8)
            }
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

        // Cache up to 9 ICC profile transforms
        if PROFILE_TRANSFORMS.len() > 8 {
            let transform =
                Self::create_icc_transform(icc_bytes, input_pixel_format, output_pixel_format)?;
            Self::apply_transform(frame, &transform);
            return Ok(());
        }

        if !PROFILE_TRANSFORMS.contains_key(&hash) {
            let transform =
                Self::create_icc_transform(icc_bytes, input_pixel_format, output_pixel_format)?;
            PROFILE_TRANSFORMS.insert(hash, transform);
        }
        Self::apply_transform(frame, &PROFILE_TRANSFORMS.get(&hash).unwrap());
        Ok(())
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

        // Cache up to 4 gama transforms
        if GAMA_TRANSFORMS.len() > 3 {
            let transform = Self::create_gama_transform(
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
            )?;
            Self::apply_transform(frame, &transform);
            return Ok(());
        }

        if !GAMA_TRANSFORMS.contains_key(&hash) {
            let transform = Self::create_gama_transform(
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
            )?;
            GAMA_TRANSFORMS.insert(hash, transform);
        }
        Self::apply_transform(frame, &GAMA_TRANSFORMS.get(&hash).unwrap());
        Ok(())
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
            transform.transform_in_place(line.row_mut());
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
