use serde::{Deserialize, Serialize};

/// Perceptual color/exposure adjustments in Oklab space.
///
/// All values default to 0.0 (no adjustment). Ranges noted per field.
/// These are the "Lightroom-style" adjustments from zenimage.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AdjustStep {
    /// Exposure in stops (-5.0 to +5.0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub exposure: f32,
    /// Contrast (-1.0 to +1.0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub contrast: f32,
    /// Highlights recovery (-1.0 to +1.0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub highlights: f32,
    /// Shadows recovery (-1.0 to +1.0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub shadows: f32,
    /// Vibrance — smart saturation that protects already-saturated colors (-1.0 to +1.0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub vibrance: f32,
    /// Saturation — linear chroma scale (-1.0 to +1.0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub saturation: f32,
    /// Clarity — local contrast enhancement (0.0 to 1.0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub clarity: f32,
    /// Temperature shift (-1.0 cool to +1.0 warm).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub temperature: f32,
    /// Tint shift (-1.0 green to +1.0 magenta).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub tint: f32,
    /// Noise reduction strength (0.0 to 1.0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub noise_reduction: f32,
    /// Deblock/dejpeg strength (0.0 to 1.0).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub deblock: f32,
    /// Brightness (-1.0 to +1.0). Simpler than exposure — linear offset.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub brightness: f32,
}

/// 5×5 color matrix transform.
///
/// Row-major: `[R', G', B', A', 1] = M × [R, G, B, A, 1]`.
/// Applied in sRGB space.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColorMatrixStep {
    pub matrix: [f32; 25],
}

/// Predefined color filters.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorFilterStep {
    /// ITU-R BT.709 grayscale (HDTV luminance weights).
    GrayscaleBt709,
    /// NTSC/BT.601 grayscale.
    GrayscaleNtsc,
    /// Equal-weight grayscale.
    GrayscaleFlat,
    /// R-Y channel grayscale (legacy imageflow filter).
    GrayscaleRy,
    /// Sepia tone.
    Sepia,
    /// Color inversion.
    Invert,
    /// Multiply alpha channel (0.0–1.0).
    Alpha(f32),
    /// Contrast adjustment factor (1.0 = no change).
    Contrast(f32),
    /// Brightness adjustment factor (1.0 = no change).
    Brightness(f32),
    /// Saturation adjustment factor (1.0 = no change).
    Saturation(f32),
}

/// Oklab L-channel sharpening (no color fringing).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharpenStep {
    /// Sharpening amount (0–100).
    #[serde(default = "default_sharpen")]
    pub amount: f32,
}

/// Gaussian blur.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlurStep {
    /// Blur radius in sigma.
    pub sigma: f32,
}

/// White balance using histogram area threshold.
///
/// Finds the white point by histogram analysis in sRGB space.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WhiteBalanceStep {
    /// Area threshold (0.0–1.0). Lower = whiter reference point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

fn default_sharpen() -> f32 {
    15.0
}
