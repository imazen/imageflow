use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::Color;

/// Encode an image to an I/O destination.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncodeStep {
    /// I/O identifier for the destination.
    pub io_id: i32,
    /// Output format. `None` = auto-detect from source or file extension.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<OutputFormat>,
    /// Quality targeting strategy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<QualityTarget>,
    /// Output color space handling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<OutputColor>,
    /// UltraHDR encoding config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultrahdr: Option<UltraHdrEncode>,
    /// Prefer lossless JPEG transform when pipeline allows (orient-only).
    #[serde(default)]
    pub prefer_lossless_jpeg: bool,
    /// Per-format encoder hints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hints: Option<EncoderHints>,
    /// Matte color for alpha compositing onto opaque formats (JPEG).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matte: Option<Color>,
}

/// Output image format.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Jpeg,
    Png,
    #[serde(alias = "webp")]
    WebP,
    Gif,
    Avif,
    Jxl,
    Heic,
    Bmp,
    /// Keep the same format as the source image.
    Keep,
    /// Auto-select format based on content analysis.
    Auto {
        /// Allowed format names (empty = all allowed).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        allowed: Option<Vec<String>>,
    },
}

/// Quality targeting strategy.
///
/// Goes beyond fixed quality numbers — supports perceptual quality metrics,
/// source quality matching, and lossless mode.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityTarget {
    /// Fixed quality 0–100.
    Quality(f32),
    /// Estimate source quality, re-encode to match with calibrated settings.
    MatchSource {
        /// Butteraugli tolerance for quality estimation. Default: 0.3.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tolerance: Option<f32>,
        /// Guarantee output ≤ source file size.
        #[serde(default)]
        shrink_guarantee: bool,
    },
    /// Target Butteraugli distance (lower = higher quality, typical: 0.5–3.0).
    Butteraugli(f32),
    /// Target SSIMULACRA2 score (higher = higher quality, typical: 60–90).
    Ssimulacra2(f32),
    /// Lossless encoding.
    Lossless,
}

/// Output color space handling.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputColor {
    /// Output profile strategy.
    #[serde(default)]
    pub profile: OutputProfile,
}

/// Output color profile strategy.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputProfile {
    /// Re-embed original ICC/CICP from source.
    #[default]
    SameAsOrigin,
    /// Convert to sRGB.
    Srgb,
    /// Convert to Display P3.
    DisplayP3,
    /// Convert to Rec. 2020 (for HDR content).
    Rec2020,
    /// Explicit CICP signaling.
    Cicp {
        color_primaries: u8,
        transfer_characteristics: u8,
        matrix_coefficients: u8,
        full_range: bool,
    },
}

/// UltraHDR encoding configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UltraHdrEncode {
    /// Enable UltraHDR gain map embedding.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Gain map JPEG quality (0–100).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gainmap_quality: Option<f32>,
    /// Gain map spatial downscale factor (1 = full res, 4 = quarter).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gainmap_scale: Option<u32>,
    /// Target peak display luminance in nits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_peak_nits: Option<f32>,
}

/// Per-format encoder hints.
///
/// Only the hints matching the selected output format are used.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EncoderHints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jpeg: Option<JpegHints>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub png: Option<PngHints>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webp: Option<WebPHints>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avif: Option<AvifHints>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jxl: Option<JxlHints>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gif: Option<GifHints>,
}

/// JPEG encoder hints.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct JpegHints {
    /// Enable progressive JPEG.
    #[serde(default)]
    pub progressive: bool,
    /// Chroma subsampling (e.g., "4:2:0", "4:4:4").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subsampling: Option<String>,
    /// Enable trellis quantization (mozjpeg/jpegli).
    #[serde(default)]
    pub trellis: bool,
    /// Optimize Huffman coding.
    #[serde(default)]
    pub optimize_huffman: bool,
}

/// PNG encoder hints.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PngHints {
    /// Bit depth (1, 2, 4, 8, 16). `None` = auto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u8>,
    /// Maximum deflate compression.
    #[serde(default)]
    pub max_deflate: bool,
    /// Enable palette quantization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantize: Option<PngQuantize>,
}

/// PNG palette quantization settings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PngQuantize {
    /// Target quality (0–100).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<u8>,
    /// Minimum acceptable quality (0–100).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_quality: Option<u8>,
    /// Quantization speed (1 = best, 10 = fastest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<u8>,
}

/// WebP encoder hints.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WebPHints {
    /// Force lossless WebP.
    #[serde(default)]
    pub lossless: bool,
}

/// AVIF encoder hints.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AvifHints {
    /// Encoding speed (0 = slowest/best, 10 = fastest). `None` = codec default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<u8>,
    /// Alpha channel quality (0–100). `None` = same as main quality.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha_quality: Option<f32>,
}

/// JPEG XL encoder hints.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct JxlHints {
    /// Butteraugli distance (0.0 = lossless, 1.0 = visually lossless, 3.0+ = lossy).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance: Option<f32>,
    /// Encoding effort (1–9). Higher = slower + smaller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<u8>,
}

/// GIF encoder hints.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GifHints {
    /// Dithering method.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dither: Option<String>,
}

fn default_true() -> bool {
    true
}
