use crate::ffi::{ColorProfileSource, DecoderColorInfo};

/// CMS-agnostic description of an image's source color space.
/// Constructed by decoders, consumed by transform caches.
#[derive(Clone, Debug)]
pub enum SourceProfile {
    /// sRGB or no color info. No transform needed.
    Srgb,

    /// CICP code points (ITU-T H.273). Highest priority in PNG 3rd Ed.
    Cicp {
        color_primaries: u8,
        transfer_characteristics: u8,
        matrix_coefficients: u8,
        full_range: bool,
    },

    /// Raw ICC profile bytes (RGB color space).
    IccProfile(Vec<u8>),

    /// Raw ICC profile bytes (grayscale).
    IccProfileGray(Vec<u8>),

    /// Raw ICC profile bytes for a CMYK image. The frame contains inverted CMYK data
    /// (4 bytes/pixel: 255-C, 255-M, 255-Y, 255-K as produced by mozjpeg).
    CmykIcc(Vec<u8>),

    /// Gamma + chromaticities (PNG gAMA+cHRM).
    GammaPrimaries {
        gamma: f64,
        white_x: f64,
        white_y: f64,
        red_x: f64,
        red_y: f64,
        green_x: f64,
        green_y: f64,
        blue_x: f64,
        blue_y: f64,
    },
}

impl SourceProfile {
    /// Construct from a `png::Info`, applying PNG 3rd Edition precedence:
    /// cICP > iCCP > sRGB > gAMA+cHRM > assume sRGB
    pub fn from_png_info(info: &png::Info<'_>) -> Self {
        // 1. cICP (highest priority)
        if let Some(ref cicp) = info.coding_independent_code_points {
            // CICP sRGB detection: primaries=1 (BT.709), transfer=13 (sRGB)
            if cicp.color_primaries == 1 && cicp.transfer_function == 13 {
                return SourceProfile::Srgb;
            }
            return SourceProfile::Cicp {
                color_primaries: cicp.color_primaries,
                transfer_characteristics: cicp.transfer_function,
                matrix_coefficients: cicp.matrix_coefficients,
                full_range: cicp.is_video_full_range_image,
            };
        }

        // 2. iCCP
        if let Some(ref icc_data) = info.icc_profile {
            let bytes = icc_data.to_vec();
            let is_gray = matches!(
                info.color_type,
                png::ColorType::Grayscale | png::ColorType::GrayscaleAlpha
            );
            return if is_gray {
                SourceProfile::IccProfileGray(bytes)
            } else {
                SourceProfile::IccProfile(bytes)
            };
        }

        // 3. sRGB chunk
        if info.srgb.is_some() {
            return SourceProfile::Srgb;
        }

        // 4. gAMA + cHRM
        if let Some(gamma) = info.source_gamma {
            if let Some(ref chrm) = info.source_chromaticities {
                return SourceProfile::GammaPrimaries {
                    gamma: gamma.into_value() as f64,
                    white_x: chrm.white.0.into_value() as f64,
                    white_y: chrm.white.1.into_value() as f64,
                    red_x: chrm.red.0.into_value() as f64,
                    red_y: chrm.red.1.into_value() as f64,
                    green_x: chrm.green.0.into_value() as f64,
                    green_y: chrm.green.1.into_value() as f64,
                    blue_x: chrm.blue.0.into_value() as f64,
                    blue_y: chrm.blue.1.into_value() as f64,
                };
            }
            // gAMA without cHRM: not enough to build a full transform
        }

        // 5. No color info — assume sRGB
        SourceProfile::Srgb
    }

    /// Construct from raw ICC profile bytes (for JPEG/other decoders).
    pub fn from_icc_bytes(bytes: Vec<u8>) -> Self {
        SourceProfile::IccProfile(bytes)
    }

    /// Construct from a legacy `DecoderColorInfo` (used by C-based libpng decoder).
    ///
    /// # Safety
    /// The `profile_buffer` pointer in `color` must be valid for `buffer_length` bytes
    /// if `source` is `ICCP` or `ICCP_GRAY`.
    pub unsafe fn from_decoder_color_info(color: &DecoderColorInfo) -> Self {
        match color.source {
            ColorProfileSource::Null | ColorProfileSource::sRGB => SourceProfile::Srgb,
            ColorProfileSource::ICCP => {
                if color.profile_buffer.is_null() || color.buffer_length == 0 {
                    return SourceProfile::Srgb;
                }
                let bytes =
                    std::slice::from_raw_parts(color.profile_buffer, color.buffer_length).to_vec();
                SourceProfile::IccProfile(bytes)
            }
            ColorProfileSource::ICCP_GRAY => {
                if color.profile_buffer.is_null() || color.buffer_length == 0 {
                    return SourceProfile::Srgb;
                }
                let bytes =
                    std::slice::from_raw_parts(color.profile_buffer, color.buffer_length).to_vec();
                SourceProfile::IccProfileGray(bytes)
            }
            ColorProfileSource::GAMA_CHRM => SourceProfile::GammaPrimaries {
                gamma: color.gamma,
                white_x: color.white_point.x,
                white_y: color.white_point.y,
                red_x: color.primaries.Red.x,
                red_y: color.primaries.Red.y,
                green_x: color.primaries.Green.x,
                green_y: color.primaries.Green.y,
                blue_x: color.primaries.Blue.x,
                blue_y: color.primaries.Blue.y,
            },
        }
    }

    /// Returns true if this profile is sRGB (no transform needed).
    pub fn is_srgb(&self) -> bool {
        matches!(self, SourceProfile::Srgb)
    }
}
