use crate::codecs::source_profile::SourceProfile;
use crate::graphics::bitmaps::{BitmapWindowMut, PixelLayout};
use crate::{ErrorKind, FlowError, Result};
use dashmap::DashMap;
use moxcms::{
    curve_from_gamma, Chromaticity, CicpColorPrimaries, CicpProfile, CmsError, ColorPrimaries,
    ColorProfile, DataColorSpace, InPlaceTransformExecutor, Layout, MatrixCoefficients,
    TransferCharacteristics, Transform8BitExecutor, TransformOptions, XyY,
};
use std::sync::{Arc, LazyLock};

/// Cached transforms keyed by hash of profile parameters.
static ICC_TRANSFORMS: LazyLock<DashMap<u64, CachedTransform>> =
    LazyLock::new(|| DashMap::with_capacity(4));
static GAMA_TRANSFORMS: LazyLock<DashMap<u64, CachedTransform>> =
    LazyLock::new(|| DashMap::with_capacity(4));
static CICP_TRANSFORMS: LazyLock<DashMap<u64, CachedTransform>> =
    LazyLock::new(|| DashMap::with_capacity(4));
static CMYK_TRANSFORMS: LazyLock<DashMap<u64, Arc<Transform8BitExecutor>>> =
    LazyLock::new(|| DashMap::with_capacity(2));

const HASH_SEED: u64 = 0x8ed1_2ad9_483d_28a0;

/// A cached transform — either in-place or with a scratch buffer.
#[derive(Clone)]
enum CachedTransform {
    InPlace(Arc<dyn InPlaceTransformExecutor<u8> + Send + Sync>),
    Regular(Arc<Transform8BitExecutor>),
    /// Gray ICC: GrayAlpha input (2 bpp) → RGBA output (4 bpp).
    /// Needs dedicated apply logic since the frame is BGRA (4 bpp).
    Gray(Arc<Transform8BitExecutor>),
}

pub struct MoxcmsTransformCache;

#[allow(clippy::too_many_arguments)]
impl MoxcmsTransformCache {
    /// Apply a color transform from `profile` to sRGB on the given BGRA frame.
    pub fn transform_to_srgb(
        frame: &mut BitmapWindowMut<u8>,
        profile: &SourceProfile,
    ) -> Result<()> {
        if frame.info().pixel_layout() != PixelLayout::BGRA {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "moxcms transform requires BGRA pixel layout, got {:?}",
                frame.info().pixel_layout()
            ));
        }

        if profile.is_srgb() {
            return Ok(());
        }

        if let SourceProfile::CmykIcc(ref icc_bytes) = profile {
            return Self::transform_cmyk_to_srgb(frame, icc_bytes);
        }

        let transform = Self::get_or_create_transform(profile)?;
        Self::apply_transform(frame, &transform)
    }

    fn get_or_create_transform(profile: &SourceProfile) -> Result<CachedTransform> {
        match profile {
            SourceProfile::Srgb => unreachable!("Srgb should be handled before calling this"),
            SourceProfile::Cicp {
                color_primaries,
                transfer_characteristics,
                matrix_coefficients,
                full_range,
            } => {
                let hash = Self::hash_cicp(
                    *color_primaries,
                    *transfer_characteristics,
                    *matrix_coefficients,
                    *full_range,
                );
                Self::cached_or_create(&CICP_TRANSFORMS, hash, 4, || {
                    Self::create_cicp_transform(
                        *color_primaries,
                        *transfer_characteristics,
                        *matrix_coefficients,
                        *full_range,
                    )
                })
            }
            SourceProfile::IccProfile(bytes) => {
                let hash = Self::hash_icc_bytes(bytes, false);
                Self::cached_or_create(&ICC_TRANSFORMS, hash, 9, || {
                    Self::create_icc_transform(bytes, false)
                })
            }
            SourceProfile::IccProfileGray(bytes) => {
                let hash = Self::hash_icc_bytes(bytes, true);
                Self::cached_or_create(&ICC_TRANSFORMS, hash, 9, || {
                    Self::create_icc_transform(bytes, true)
                })
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
            } => {
                let hash = Self::hash_gamma_primaries(
                    *gamma, *white_x, *white_y, *red_x, *red_y, *green_x, *green_y, *blue_x,
                    *blue_y,
                );
                Self::cached_or_create(&GAMA_TRANSFORMS, hash, 4, || {
                    Self::create_gamma_primaries_transform(
                        *gamma, *white_x, *white_y, *red_x, *red_y, *green_x, *green_y, *blue_x,
                        *blue_y,
                    )
                })
            }
            SourceProfile::CmykIcc(_) => {
                unreachable!("CmykIcc is handled separately in transform_to_srgb")
            }
        }
    }

    fn cached_or_create(
        cache: &DashMap<u64, CachedTransform>,
        hash: u64,
        max_entries: usize,
        create: impl FnOnce() -> Result<CachedTransform>,
    ) -> Result<CachedTransform> {
        if let Some(entry) = cache.get(&hash) {
            return Ok(entry.value().clone());
        }
        let transform = create()?;
        if cache.len() < max_entries {
            cache.insert(hash, transform.clone());
        }
        Ok(transform)
    }

    fn create_cicp_transform(
        color_primaries: u8,
        transfer_characteristics: u8,
        matrix_coefficients: u8,
        full_range: bool,
    ) -> Result<CachedTransform> {
        let cp = CicpColorPrimaries::try_from(color_primaries)
            .map_err(|e| FlowError::from_cms_error(e).at(here!()))?;
        let tc = TransferCharacteristics::try_from(transfer_characteristics)
            .map_err(|e| FlowError::from_cms_error(e).at(here!()))?;
        let mc = MatrixCoefficients::try_from(matrix_coefficients)
            .map_err(|e| FlowError::from_cms_error(e).at(here!()))?;

        let cicp = CicpProfile {
            color_primaries: cp,
            transfer_characteristics: tc,
            matrix_coefficients: mc,
            full_range,
        };

        let src = ColorProfile::new_from_cicp(cicp);
        let dst = ColorProfile::new_srgb();
        Self::create_transform_prefer_in_place(&src, &dst)
    }

    fn create_icc_transform(bytes: &[u8], is_gray: bool) -> Result<CachedTransform> {
        let src = ColorProfile::new_from_slice(bytes)
            .map_err(|e| FlowError::from_cms_error(e).at(here!()))?;

        // Reject CMYK ICC profiles in the RGB path — they should use CmykIcc variant.
        // This catches mismatched files (e.g., RGB JPEG with CMYK ICC profile embedded).
        if !is_gray && src.color_space == DataColorSpace::Cmyk {
            return Err(nerror!(
                ErrorKind::ColorProfileError,
                "ICC profile has CMYK color space but image data is RGB"
            ));
        }

        let dst = ColorProfile::new_srgb();

        if is_gray {
            // Gray ICC → RGBA: needs dedicated apply path because the frame
            // is BGRA (4 bpp) but the transform expects GrayAlpha input (2 bpp).
            let transform = src
                .create_transform_8bit(
                    Layout::GrayAlpha,
                    &dst,
                    Layout::Rgba,
                    TransformOptions::default(),
                )
                .map_err(|e| FlowError::from_cms_error(e).at(here!()))?;
            Ok(CachedTransform::Gray(transform))
        } else {
            Self::create_transform_prefer_in_place(&src, &dst)
        }
    }

    fn create_gamma_primaries_transform(
        gamma: f64,
        white_x: f64,
        white_y: f64,
        red_x: f64,
        red_y: f64,
        green_x: f64,
        green_y: f64,
        blue_x: f64,
        blue_y: f64,
    ) -> Result<CachedTransform> {
        // PNG gAMA stores 1/gamma (the encoding gamma), so we pass it as the
        // decoding gamma directly to curve_from_gamma.
        // e.g., gAMA=0.45455 means encoding gamma 0.45455, decoding gamma 1/0.45455 ≈ 2.2
        let decoding_gamma = 1.0 / gamma;
        let trc = curve_from_gamma(decoding_gamma as f32);

        let mut src = ColorProfile::new_srgb();
        src.update_rgb_colorimetry(
            XyY::new(white_x, white_y, 1.0),
            ColorPrimaries {
                red: Chromaticity::new(red_x as f32, red_y as f32),
                green: Chromaticity::new(green_x as f32, green_y as f32),
                blue: Chromaticity::new(blue_x as f32, blue_y as f32),
            },
        );
        src.red_trc = Some(trc.clone());
        src.green_trc = Some(trc.clone());
        src.blue_trc = Some(trc);

        let dst = ColorProfile::new_srgb();
        Self::create_transform_prefer_in_place(&src, &dst)
    }

    /// Transform inverted-CMYK frame data to sRGB BGRA using a CMYK ICC profile.
    ///
    /// mozjpeg outputs CMYK as 4 bytes/pixel with inverted values (255-C, 255-M, 255-Y, 255-K).
    /// We un-invert, apply the ICC CMYK→RGB transform, and write BGRA output.
    fn transform_cmyk_to_srgb(frame: &mut BitmapWindowMut<u8>, icc_bytes: &[u8]) -> Result<()> {
        let hash = Self::hash_icc_bytes(icc_bytes, false);
        let transform = if let Some(entry) = CMYK_TRANSFORMS.get(&hash) {
            entry.value().clone()
        } else {
            let src = ColorProfile::new_from_slice(icc_bytes)
                .map_err(|e| FlowError::from_cms_error(e).at(here!()))?;
            let dst = ColorProfile::new_srgb();
            // CMYK ICC profiles require Layout::Rgba (4 channels) — moxcms maps
            // channel semantics from the ICC profile, not from the Layout enum.
            let t = src
                .create_transform_8bit(
                    Layout::Rgba,
                    &dst,
                    Layout::Rgba,
                    TransformOptions::default(),
                )
                .map_err(|e| FlowError::from_cms_error(e).at(here!()))?;
            if CMYK_TRANSFORMS.len() < 4 {
                CMYK_TRANSFORMS.insert(hash, t.clone());
            }
            t
        };

        let row_bytes = frame.w() as usize * 4;
        let mut scratch = vec![0u8; row_bytes];

        for mut scanline in frame.scanlines() {
            let row = scanline.row_mut();
            // Un-invert CMYK bytes in-place: mozjpeg stores 255-C, 255-M, 255-Y, 255-K
            for byte in row.iter_mut() {
                *byte = 255 - *byte;
            }
            // Transform CMYK → RGBA into scratch
            transform
                .transform(row, &mut scratch)
                .map_err(|e| FlowError::from_cms_error(e).at(here!()))?;
            // Copy RGBA → BGRA (swap R↔B)
            for (src_pixel, dst_pixel) in scratch.chunks_exact(4).zip(row.chunks_exact_mut(4)) {
                dst_pixel[0] = src_pixel[2]; // B ← R
                dst_pixel[1] = src_pixel[1]; // G ← G
                dst_pixel[2] = src_pixel[0]; // R ← B
                dst_pixel[3] = src_pixel[3]; // A ← A
            }
        }

        Ok(())
    }

    /// Try in-place first (works for matrix-shaper profiles), fall back to regular.
    fn create_transform_prefer_in_place(
        src: &ColorProfile,
        dst: &ColorProfile,
    ) -> Result<CachedTransform> {
        match src.create_in_place_transform_8bit(Layout::Rgba, dst, TransformOptions::default()) {
            Ok(t) => Ok(CachedTransform::InPlace(t)),
            Err(CmsError::UnsupportedProfileConnection) => {
                // Fall back to regular transform for LUT-based profiles
                let t = src
                    .create_transform_8bit(
                        Layout::Rgba,
                        dst,
                        Layout::Rgba,
                        TransformOptions::default(),
                    )
                    .map_err(|e| FlowError::from_cms_error(e).at(here!()))?;
                Ok(CachedTransform::Regular(t))
            }
            Err(e) => Err(FlowError::from_cms_error(e).at(here!())),
        }
    }

    /// Apply a cached transform to a BGRA frame.
    /// moxcms only supports RGBA layout, so we swizzle B↔R around the transform.
    fn apply_transform(frame: &mut BitmapWindowMut<u8>, transform: &CachedTransform) -> Result<()> {
        match transform {
            CachedTransform::InPlace(t) => {
                for mut scanline in frame.scanlines() {
                    let row = scanline.row_mut();
                    // BGRA → RGBA swap (B↔R)
                    swap_br_inplace(row);
                    // In-place color transform
                    t.transform(row).map_err(|e| FlowError::from_cms_error(e).at(here!()))?;
                    // RGBA → BGRA swap (B↔R)
                    swap_br_inplace(row);
                }
                Ok(())
            }
            CachedTransform::Regular(t) => {
                // Need a scratch buffer for the regular transform path
                let row_bytes = frame.w() as usize * frame.t_per_pixel();
                let mut scratch = vec![0u8; row_bytes];
                for mut scanline in frame.scanlines() {
                    let row = scanline.row_mut();
                    // Copy BGRA → RGBA into scratch
                    copy_bgra_to_rgba(row, &mut scratch);
                    // Transform scratch(RGBA) → row(RGBA)
                    t.transform(&scratch, row)
                        .map_err(|e| FlowError::from_cms_error(e).at(here!()))?;
                    // Row is now RGBA, swap to BGRA
                    swap_br_inplace(row);
                }
                Ok(())
            }
            CachedTransform::Gray(t) => {
                // Gray ICC: frame is BGRA (4 bpp) but transform expects GrayAlpha (2 bpp)
                // input and produces RGBA (4 bpp) output.
                // For grayscale images, R=G=B so we take B channel as gray value.
                let w = frame.w() as usize;
                let mut gray_alpha = vec![0u8; w * 2];
                let mut rgba_out = vec![0u8; w * 4];
                for mut scanline in frame.scanlines() {
                    let row = scanline.row_mut();
                    // Extract gray + alpha from BGRA pixels
                    for (pixel, ga) in row.chunks_exact(4).zip(gray_alpha.chunks_exact_mut(2)) {
                        ga[0] = pixel[0]; // Gray ← B (R=G=B for grayscale)
                        ga[1] = pixel[3]; // Alpha
                    }
                    // Transform GrayAlpha → RGBA
                    t.transform(&gray_alpha, &mut rgba_out)
                        .map_err(|e| FlowError::from_cms_error(e).at(here!()))?;
                    // Write RGBA output back as BGRA
                    for (rgba, pixel) in rgba_out.chunks_exact(4).zip(row.chunks_exact_mut(4)) {
                        pixel[0] = rgba[2]; // B ← B(from RGBA)
                        pixel[1] = rgba[1]; // G
                        pixel[2] = rgba[0]; // R ← R(from RGBA)
                        pixel[3] = rgba[3]; // A
                    }
                }
                Ok(())
            }
        }
    }

    // ---- Hashing helpers ----

    fn hash_cicp(cp: u8, tc: u8, mc: u8, fr: bool) -> u64 {
        use std::hash::Hasher;
        let mut h = twox_hash::XxHash64::with_seed(HASH_SEED);
        h.write_u8(cp);
        h.write_u8(tc);
        h.write_u8(mc);
        h.write_u8(fr as u8);
        h.finish()
    }

    fn hash_icc_bytes(bytes: &[u8], is_gray: bool) -> u64 {
        use std::hash::Hasher;
        let base = if bytes.len() > 80 {
            // Skip the 80-byte ICC header (variable timestamp/tool info, stable data after)
            imageflow_helpers::hashing::hash_64(&bytes[80..])
        } else {
            imageflow_helpers::hashing::hash_64(bytes)
        };
        // Mix in is_gray flag to avoid collisions between RGB and Gray transforms
        // for the same ICC profile bytes
        let mut h = twox_hash::XxHash64::with_seed(base);
        h.write_u8(is_gray as u8);
        h.finish()
    }

    fn hash_gamma_primaries(
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

/// Swap B and R channels in-place for a row of BGRA/RGBA pixels.
#[inline]
fn swap_br_inplace(row: &mut [u8]) {
    for pixel in row.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
}

/// Copy BGRA row into RGBA scratch buffer (swapping B↔R during copy).
#[inline]
fn copy_bgra_to_rgba(bgra: &[u8], rgba: &mut [u8]) {
    for (src, dst) in bgra.chunks_exact(4).zip(rgba.chunks_exact_mut(4)) {
        dst[0] = src[2]; // R ← B
        dst[1] = src[1]; // G ← G
        dst[2] = src[0]; // B ← R
        dst[3] = src[3]; // A ← A
    }
}
