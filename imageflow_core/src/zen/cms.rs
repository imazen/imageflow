//! Color management: ICC profile transforms, PNG gAMA/cHRM/cICP handling.
//!
//! Extracted from `execute.rs`. All color management logic for the zen pipeline
//! lives here: sRGB detection, ICC→sRGB transforms, PNG gamma/chromaticity
//! profile synthesis, cICP→ICC synthesis, and pixel format conversion.

use super::execute::ZenError;
use zencodecs::SourceColorExt as _;

// ─── sRGB ICC profile cache ───

/// Get the sRGB ICC profile bytes (without CICP to avoid interference).
fn srgb_icc_profile() -> Vec<u8> {
    use std::sync::OnceLock;
    static SRGB: OnceLock<Vec<u8>> = OnceLock::new();
    SRGB.get_or_init(|| {
        // Build sRGB profile without CICP metadata to avoid
        // CICP-based TRC override when re-parsed by moxcms.
        let mut profile = moxcms::ColorProfile::new_srgb();
        profile.cicp = None;
        profile.encode().unwrap_or_default()
    })
    .clone()
}

// ─── sRGB detection ───

/// Check if an ICC profile represents sRGB (or close enough to skip transform).
///
/// Parses the profile with moxcms and checks if its primaries and TRC match sRGB.
/// Camera JPEGs embed vendor-specific sRGB profiles with different bytes but
/// same color space — byte comparison doesn't work, need semantic comparison.
/// Loose sRGB check matching v2 behavior: skip if profile description says "sRGB".
///
/// This is intentionally loose — vendor-calibrated profiles (Canon, Sony) have
/// "sRGB" in their description but slightly different primaries/TRC. V2 skips
/// transforms for these, so we do too in compat mode.
fn is_srgb_icc_profile_loose(icc_bytes: &[u8]) -> bool {
    // Check if the ICC profile description contains "sRGB".
    zencodecs::icc_profile_is_srgb(icc_bytes)
}

/// Check if an ICC profile is sRGB-equivalent by comparing primaries AND TRC curves.
///
/// Uses moxcms to parse the profile and compares colorants (with 0.0001 tolerance
/// via Xyzd::PartialEq) and TRC parametric parameters (with tolerance for vendor
/// rounding). Catches vendor sRGB variants (Canon, Sony, etc.) that have different
/// bytes but identical color behavior.
fn is_srgb_icc_profile(icc_bytes: &[u8]) -> bool {
    let Ok(src) = moxcms::ColorProfile::new_from_slice(icc_bytes) else {
        return false;
    };
    let srgb = moxcms::ColorProfile::new_srgb();

    // 1. Primaries must match (Xyzd::PartialEq has 0.0001 tolerance).
    if src.red_colorant != srgb.red_colorant
        || src.green_colorant != srgb.green_colorant
        || src.blue_colorant != srgb.blue_colorant
    {
        return false;
    }

    // 2. TRC: must be sRGB-equivalent (parametric or LUT).
    trc_matches_srgb(&src.red_trc)
        && trc_matches_srgb(&src.green_trc)
        && trc_matches_srgb(&src.blue_trc)
}

/// Check if a TRC curve matches the sRGB parametric curve within tolerance.
///
/// sRGB TRC is parametric type 4: [2.4, 1/1.055, 0.055/1.055, 1/12.92, 0.04045]
/// Vendor profiles may round these differently (e.g., 0.947867... vs 0.9479).
fn trc_matches_srgb(trc: &Option<moxcms::ToneReprCurve>) -> bool {
    let Some(trc) = trc else { return false };

    match trc {
        moxcms::ToneReprCurve::Parametric(params) => {
            // sRGB parametric: [gamma, a, b, c, d]
            // Expected: [2.4, 1/1.055 ≈ 0.94787, 0.055/1.055 ≈ 0.05213, 1/12.92 ≈ 0.07739, 0.04045]
            const SRGB_PARAMS: [f32; 5] = [
                2.4,
                1.0 / 1.055,   // 0.947867...
                0.055 / 1.055, // 0.052132...
                1.0 / 12.92,   // 0.077399...
                0.04045,
            ];
            const TOL: f32 = 0.001;

            if params.len() < 5 {
                return false;
            }
            params[..5].iter().zip(SRGB_PARAMS.iter()).all(|(a, b)| (a - b).abs() < TOL)
        }
        moxcms::ToneReprCurve::Lut(lut) => {
            // Some profiles encode sRGB as a 1024 or 4096 entry LUT.
            // Check a few diagnostic points against expected sRGB values.
            if lut.is_empty() {
                return false;
            }
            let n = lut.len();

            // sRGB curve: output = ((input/1.055 + 0.055/1.055)^2.4) for input > 0.04045
            // Check at 25%, 50%, 75% input.
            let check_points = [n / 4, n / 2, 3 * n / 4];
            for &idx in &check_points {
                let input = idx as f64 / (n - 1) as f64;
                let expected = if input <= 0.04045 {
                    input / 12.92
                } else {
                    ((input + 0.055) / 1.055).powf(2.4)
                };
                let actual = lut[idx] as f64 / 65535.0;
                if (actual - expected).abs() > 0.002 {
                    return false;
                }
            }
            true
        }
    }
}

// ─── ICC profile synthesis ───

/// Synthesize an ICC profile from PNG gAMA (and optional cHRM) metadata.
///
/// If gAMA is close to sRGB (0.45455), returns None (no transform needed).
/// Otherwise, creates a gamma+primaries profile using moxcms.
fn synthesize_icc_from_gama(
    gamma_scaled: u32,
    chromaticities: &Option<[u32; 8]>,
) -> Option<Vec<u8>> {
    let gamma_f = gamma_scaled as f64 / 100000.0;
    let neutral_low = 0.4318;
    let neutral_high = 0.4773;

    let chrm_is_srgb = chromaticities.map_or(true, |c| {
        // sRGB primaries scaled by 100000. Tolerance: 1% (1000) to handle rounding.
        let srgb = [31270u32, 32900, 64000, 33000, 30000, 60000, 15000, 6000];
        c.iter().zip(srgb.iter()).all(|(a, b)| (*a as i64 - *b as i64).unsigned_abs() < 1000)
    });

    if gamma_f >= neutral_low && gamma_f <= neutral_high && chrm_is_srgb {
        return None;
    }

    // Build profile using moxcms: start from sRGB, update colorimetry + TRC, clear CICP.
    // Pattern from moxcms issue #154.
    let display_gamma = 1.0 / gamma_f;

    let mut profile = moxcms::ColorProfile::new_srgb();

    // Update primaries if cHRM is present and non-sRGB.
    if let Some(c) = chromaticities {
        if !chrm_is_srgb {
            let white = moxcms::XyY::new(c[0] as f64 / 100000.0, c[1] as f64 / 100000.0, 1.0);
            let primaries = moxcms::ColorPrimaries {
                red: moxcms::Chromaticity { x: c[2] as f32 / 100000.0, y: c[3] as f32 / 100000.0 },
                green: moxcms::Chromaticity {
                    x: c[4] as f32 / 100000.0,
                    y: c[5] as f32 / 100000.0,
                },
                blue: moxcms::Chromaticity { x: c[6] as f32 / 100000.0, y: c[7] as f32 / 100000.0 },
            };
            profile.update_rgb_colorimetry(white, primaries);
        }
    }

    // Override TRC with pure gamma curve (parametric type 0: Y = X^gamma).
    let trc = moxcms::ToneReprCurve::Parametric(vec![display_gamma as f32]);
    profile.red_trc = Some(trc.clone());
    profile.green_trc = Some(trc.clone());
    profile.blue_trc = Some(trc);

    // Clear CICP to prevent it from overriding our TRC (issue #154).
    profile.cicp = None;

    profile.encode().ok()
}

/// Synthesize an ICC profile from cICP values.
///
/// Returns None for unrecognized primaries/transfer combinations.
fn synthesize_icc_from_cicp(cicp: &CicpValues) -> Option<Vec<u8>> {
    let mut profile = moxcms::ColorProfile::new_srgb();

    // Set primaries based on colour_primaries code.
    match cicp.colour_primaries {
        1 => {
            // BT.709 / sRGB primaries — already default from new_srgb()
        }
        9 => {
            // BT.2020
            let white = moxcms::XyY::new(0.3127, 0.3290, 1.0);
            let primaries = moxcms::ColorPrimaries {
                red: moxcms::Chromaticity { x: 0.708, y: 0.292 },
                green: moxcms::Chromaticity { x: 0.170, y: 0.797 },
                blue: moxcms::Chromaticity { x: 0.131, y: 0.046 },
            };
            profile.update_rgb_colorimetry(white, primaries);
        }
        12 => {
            // Display P3
            let white = moxcms::XyY::new(0.3127, 0.3290, 1.0);
            let primaries = moxcms::ColorPrimaries {
                red: moxcms::Chromaticity { x: 0.680, y: 0.320 },
                green: moxcms::Chromaticity { x: 0.265, y: 0.690 },
                blue: moxcms::Chromaticity { x: 0.150, y: 0.060 },
            };
            profile.update_rgb_colorimetry(white, primaries);
        }
        _ => {
            // Unrecognized primaries — cannot synthesize accurately.
            return None;
        }
    }

    // Set transfer characteristics.
    match cicp.transfer_characteristics {
        1 | 6 => {
            // BT.709 / BT.601 transfer: parametric type 4 (IEC 61966-2-1)
            // V = a * L^gamma + b  for L >= d
            // V = c * L             for L < d
            // BT.709 OETF: a=1.099, b=-0.099, gamma=0.45, c=4.5, d=0.018
            let trc = moxcms::ToneReprCurve::Parametric(vec![
                0.45_f32, // gamma
                1.099,    // a
                -0.099,   // b (offset)
                4.5,      // c (linear slope)
                0.018,    // d (linear cutoff)
            ]);
            profile.red_trc = Some(trc.clone());
            profile.green_trc = Some(trc.clone());
            profile.blue_trc = Some(trc);
        }
        13 => {
            // sRGB transfer — leave the TRC from new_srgb() as-is.
        }
        _ => {
            // Unrecognized transfer — cannot synthesize accurately.
            // PQ (16), HLG (18) etc. are HDR transfers that need scene-referred
            // handling, not simple ICC profile synthesis.
            return None;
        }
    }

    // Clear CICP to prevent it from overriding our TRC (issue #154).
    profile.cicp = None;

    profile.encode().ok()
}

// ─── PNG chunk parsing ───

/// Parsed cICP chunk values.
#[derive(Clone, Copy, Debug)]
struct CicpValues {
    /// Colour primaries (cp): 1=BT.709/sRGB, 9=BT.2020, 12=Display P3, etc.
    colour_primaries: u8,
    /// Transfer characteristics (tc): 1=BT.709, 13=sRGB, 16=PQ, 18=HLG, etc.
    transfer_characteristics: u8,
    /// Matrix coefficients (mc): 0=identity for RGB.
    #[allow(dead_code)]
    matrix_coefficients: u8,
    /// Full range flag: 1=full range, 0=video range.
    #[allow(dead_code)]
    full_range: u8,
}

/// Parse PNG color-related chunks: gAMA, cHRM, sRGB, cICP.
fn parse_png_color_chunks(
    data: &[u8],
) -> (Option<u32>, Option<[u32; 8]>, bool, Option<CicpValues>) {
    let mut gamma = None;
    let mut chrm = None;
    let mut has_srgb = false;
    let mut cicp = None;

    if data.len() < 8 || &data[0..8] != b"\x89PNG\r\n\x1a\n" {
        return (None, None, false, None);
    }
    let mut pos = 8;
    while pos + 8 <= data.len() {
        let len =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let chunk_type = &data[pos + 4..pos + 8];
        let chunk_data_start = pos + 8;
        let chunk_end = chunk_data_start + len + 4;
        if chunk_end > data.len() {
            break;
        }

        match chunk_type {
            b"gAMA" if len == 4 => {
                gamma = Some(u32::from_be_bytes([
                    data[chunk_data_start],
                    data[chunk_data_start + 1],
                    data[chunk_data_start + 2],
                    data[chunk_data_start + 3],
                ]));
            }
            b"cHRM" if len == 32 => {
                let d = &data[chunk_data_start..];
                let r =
                    |off: usize| u32::from_be_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]]);
                chrm = Some([r(0), r(4), r(8), r(12), r(16), r(20), r(24), r(28)]);
            }
            b"sRGB" => {
                has_srgb = true;
            }
            b"cICP" if len == 4 => {
                cicp = Some(CicpValues {
                    colour_primaries: data[chunk_data_start],
                    transfer_characteristics: data[chunk_data_start + 1],
                    matrix_coefficients: data[chunk_data_start + 2],
                    full_range: data[chunk_data_start + 3],
                });
            }
            b"IDAT" | b"IEND" => break,
            _ => {}
        }
        pos = chunk_end;
    }
    (gamma, chrm, has_srgb, cicp)
}

// ─── Transform application ───

/// Apply ICC→sRGB transform if the source image has a non-sRGB ICC profile.
///
/// On failure (unsupported pixel format, bad ICC data), returns the source
/// unchanged — falling back to format-only conversion.
pub(super) fn apply_icc_transform(
    source: Box<dyn zenpipe::Source>,
    info: &zencodecs::ImageInfo,
    cms_mode: imageflow_types::CmsMode,
) -> Result<Box<dyn zenpipe::Source>, ZenError> {
    // 1. Try embedded ICC profile first.
    let src_icc = if let Some(icc) = &info.source_color.icc_profile {
        if !icc.is_empty() {
            Some(icc.clone())
        } else {
            None
        }
    } else {
        None
    };

    // 2. If no ICC, skip CMS.
    let src_icc = match src_icc {
        Some(icc) => icc,
        None => return Ok(source),
    };

    // In compat mode, skip transforms for sRGB-like profiles (loose match).
    // In scene-referred mode, only skip for exact sRGB (strict match).
    match cms_mode {
        imageflow_types::CmsMode::Imageflow2Compat => {
            if info.source_color.is_srgb() || is_srgb_icc_profile_loose(&src_icc) {
                return Ok(source);
            }
        }
        imageflow_types::CmsMode::SceneReferred => {
            // Strict: only skip for exact sRGB (primaries + TRC match).
            if is_srgb_icc_profile(&src_icc) {
                return Ok(source);
            }
        }
    }

    let srgb_icc = srgb_icc_profile();
    let src_format = source.format();
    let pixel_format = src_format.pixel_format();

    // Pre-check: try to build the CMS transform without consuming the source.
    use zenpipe::ColorManagement as _;
    let transform =
        zenpipe::MoxCms.build_transform_for_format(&src_icc, &srgb_icc, pixel_format, pixel_format);

    match transform {
        Ok(row_transform) => {
            let dst_icc: std::sync::Arc<[u8]> = std::sync::Arc::from(srgb_icc.as_slice());
            let transformed = zenpipe::sources::IccTransformSource::from_transform(
                source,
                row_transform,
                dst_icc,
            );
            Ok(Box::new(transformed))
        }
        Err(_e) => {
            // ICC transform not possible for this pixel format.
            // Fall back to format-only conversion (preserves source).
            Ok(source)
        }
    }
}

/// Parse gAMA/cHRM/cICP from raw PNG bytes, synthesize ICC, and apply transform.
///
/// PNG 3rd Ed precedence: cICP > iCCP > sRGB > gAMA+cHRM.
/// iCCP is handled by the ICC path in `apply_icc_transform`. This function
/// handles cICP and gAMA+cHRM.
pub(super) fn apply_png_gamma_transform(
    source: Box<dyn zenpipe::Source>,
    png_data: &[u8],
    honor_gama_only: bool,
) -> Result<Box<dyn zenpipe::Source>, ZenError> {
    let (gamma, chrm, has_srgb, cicp) = parse_png_color_chunks(png_data);

    // cICP chunk takes highest precedence (PNG 3rd Ed spec).
    if let Some(cicp) = cicp {
        return apply_cicp_transform(source, &cicp);
    }

    // sRGB chunk → already sRGB, no transform.
    if has_srgb {
        return Ok(source);
    }

    let gamma = match gamma {
        Some(g) if g > 0 => g,
        _ => return Ok(source),
    };

    // Validate cHRM: reject degenerate chromaticities (y=0 causes division by zero).
    if let Some(ref c) = chrm {
        if c.iter().enumerate().any(|(i, v)| i % 2 == 1 && *v == 0) {
            return Ok(source); // Degenerate cHRM — skip
        }
    }

    // gAMA-only (no cHRM) is ignored unless HonorGamaOnly is set.
    if chrm.is_none() && !honor_gama_only {
        return Ok(source);
    }

    let icc = match synthesize_icc_from_gama(gamma, &chrm) {
        Some(icc) => icc,
        None => return Ok(source), // Gamma is neutral sRGB — no transform
    };

    apply_icc_to_source(source, &icc)
}

/// Apply a cICP color profile to a source, transforming to sRGB.
///
/// Handles common CICP transfer characteristics:
/// - tc=13 (sRGB): no-op
/// - tc=1 (BT.709): parametric TRC with BT.709 OETF
/// - Other tc values: synthesize ICC from CICP primaries + gamma approximation
fn apply_cicp_transform(
    source: Box<dyn zenpipe::Source>,
    cicp: &CicpValues,
) -> Result<Box<dyn zenpipe::Source>, ZenError> {
    // sRGB transfer (tc=13) with sRGB primaries (cp=1) → already sRGB, no-op.
    if cicp.transfer_characteristics == 13 && cicp.colour_primaries == 1 {
        return Ok(source);
    }

    // Build an ICC profile from CICP values.
    let icc = match synthesize_icc_from_cicp(cicp) {
        Some(icc) => icc,
        None => return Ok(source), // Unrecognized CICP — skip rather than corrupt
    };

    apply_icc_to_source(source, &icc)
}

/// Apply a source ICC profile to a pixel source, transforming to sRGB.
/// Shared by both gAMA and cICP paths.
fn apply_icc_to_source(
    source: Box<dyn zenpipe::Source>,
    src_icc: &[u8],
) -> Result<Box<dyn zenpipe::Source>, ZenError> {
    let srgb_icc = srgb_icc_profile();
    let src_format = source.format();
    let pixel_format = src_format.pixel_format();

    use zenpipe::ColorManagement as _;
    let transform =
        zenpipe::MoxCms.build_transform_for_format(src_icc, &srgb_icc, pixel_format, pixel_format);

    match transform {
        Ok(row_transform) => {
            let dst_icc: std::sync::Arc<[u8]> = std::sync::Arc::from(srgb_icc.as_slice());
            let transformed = zenpipe::sources::IccTransformSource::from_transform(
                source,
                row_transform,
                dst_icc,
            );
            Ok(Box::new(transformed))
        }
        Err(_) => Ok(source),
    }
}

// ─── Pixel format conversion ───

/// Wrap a source with a format conversion to RGBA8 sRGB if needed.
///
/// v2 compatibility: the v2 engine always decodes to BGRA32 sRGB via CMS.
/// The zen pipeline preserves the source format. This function inserts a
/// conversion to RGBA8 sRGB when the source isn't already in that format.
pub(super) fn ensure_srgb_rgba8(
    source: Box<dyn zenpipe::Source>,
) -> Result<Box<dyn zenpipe::Source>, ZenError> {
    let src_format = source.format();
    let target = zenpipe::format::RGBA8_SRGB;

    if src_format == target {
        return Ok(source);
    }
    // Try to create a format conversion.
    if let Some(converter) = zenpipe::ops::RowConverterOp::new(src_format, target) {
        let transform =
            zenpipe::sources::TransformSource::new(source).push_boxed(Box::new(converter));
        Ok(Box::new(transform))
    } else {
        // No conversion path — log and proceed with original format.
        // The pipeline will attempt format negotiation at later stages.
        Ok(source)
    }
}
