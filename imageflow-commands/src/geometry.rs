use serde::{Deserialize, Serialize};

use crate::{Color, Gravity};

/// Constrain image dimensions using a sizing mode.
///
/// This is the primary sizing operation. The engine expands it into
/// resize + crop/pad primitives during graph compilation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConstrainStep {
    /// Sizing mode.
    pub mode: ConstraintMode,
    /// Target width. `None` = unconstrained on this axis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<u32>,
    /// Target height. `None` = unconstrained on this axis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<u32>,
    /// Anchor point for crop/pad modes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gravity: Option<Gravity>,
    /// Background color for pad modes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
    /// Resize filter and sharpening hints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hints: Option<ResizeHints>,
}

/// How to fit an image into target dimensions.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintMode {
    /// Scale to fit within box, preserving aspect ratio. May be smaller than target.
    Fit,
    /// Scale so the smaller dimension matches. Result is at most target size.
    Within,
    /// Ensure the image is at least as large as the target. Upscale if needed.
    LargerThan,
    /// Fit then crop to exact target dimensions.
    FitCrop,
    /// Within then crop to exact target dimensions.
    WithinCrop,
    /// Fit then pad to exact target dimensions.
    FitPad,
    /// Within then pad to exact target dimensions.
    WithinPad,
    /// Pad to fill target, image centered (no scaling).
    PadWithin,
    /// Stretch/squash to exact target dimensions (ignores aspect ratio).
    Distort,
    /// Crop to target aspect ratio without scaling.
    AspectCrop,
}

/// Resize filter and quality hints.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResizeHints {
    /// Interpolation filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Filter>,
    /// Post-resize sharpening (0–100). `None` = auto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sharpen_percent: Option<f32>,
    /// Color space for resampling math.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scaling_colorspace: Option<ScalingColorspace>,
    /// When to apply resize. Default: when dimensions differ.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resample_when: Option<ResampleWhen>,
    /// When to apply sharpening.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sharpen_when: Option<SharpenWhen>,
}

/// Interpolation filter for resampling.
///
/// These map to the 31 filters in zenresize. The most common choices:
/// - `Robidoux` — balanced quality/speed (default for most pipelines)
/// - `Mitchell` — good for downscaling photographic content
/// - `Lanczos` — sharp, may ring on high-contrast edges
/// - `CatmullRom` — slight sharpening, popular for game textures
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Filter {
    Robidoux,
    RobidouxSharp,
    RobidouxFast,
    Lanczos,
    LanczosSharp,
    #[serde(alias = "lanczos_2")]
    Lanczos2,
    #[serde(alias = "lanczos_2_sharp")]
    Lanczos2Sharp,
    Ginseng,
    GinsengSharp,
    Mitchell,
    CatmullRom,
    CubicBSpline,
    Hermite,
    Triangle,
    Box,
    Fastest,
    Cubic,
    CubicSharp,
    CubicFast,
    #[serde(alias = "n_cubic")]
    NCubic,
    #[serde(alias = "n_cubic_sharp")]
    NCubicSharp,
    Jinc,
    Linear,
}

/// Color space for resampling math.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalingColorspace {
    /// Linear light (physically correct, avoids darkening).
    Linear,
    /// sRGB gamma (faster, legacy behavior).
    Srgb,
}

/// When to apply the resize operation.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResampleWhen {
    /// Only when input and output sizes differ.
    SizeDiffers,
    /// When sizes differ or sharpening is requested.
    SizeDiffersOrSharpeningRequested,
    /// Always, even if sizes match.
    Always,
}

/// When to apply sharpening.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharpenWhen {
    Downscaling,
    Upscaling,
    SizeDiffers,
    Always,
}

/// Crop to a pixel rectangle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CropStep {
    pub x1: u32,
    pub y1: u32,
    pub x2: u32,
    pub y2: u32,
}

/// Crop whitespace from image edges.
///
/// This is an eager (non-streaming) operation — it needs to scan the
/// full image to detect whitespace boundaries.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CropWhitespaceStep {
    /// Color distance threshold for whitespace detection (0–255).
    #[serde(default = "default_threshold")]
    pub threshold: u32,
    /// Padding to add after cropping, as percentage of cropped dimensions.
    #[serde(default)]
    pub percent_padding: f32,
}

/// Extract or extend a region using floating-point coordinates.
///
/// Coordinates can extend beyond image bounds — the background color fills
/// the excess area.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegionStep {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
}

/// Extract a percentage-based region.
///
/// Coordinates are 0.0–100.0 percentages of image dimensions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegionPercentStep {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
}

/// EXIF orientation handling.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrientStep {
    /// Apply orientation from EXIF metadata.
    Auto,
    /// Apply a specific EXIF orientation flag (1–8).
    Exif(u8),
}

/// Resize to exact pixel dimensions.
///
/// Unlike `Constrain`, this doesn't preserve aspect ratio or apply
/// any sizing logic. For most use cases, prefer `Constrain`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResizeStep {
    pub w: u32,
    pub h: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hints: Option<ResizeHints>,
}

/// Expand canvas with padding on each side.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExpandCanvasStep {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub color: Color,
}

/// Fill a rectangle with a solid color.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FillRectStep {
    pub x1: u32,
    pub y1: u32,
    pub x2: u32,
    pub y2: u32,
    pub color: Color,
}

/// Create a new blank canvas.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCanvasStep {
    pub w: u32,
    pub h: u32,
    pub color: Color,
}

/// Round image corners.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoundCornersStep {
    #[serde(flatten)]
    pub mode: RoundCornersMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
}

/// Corner rounding mode.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundCornersMode {
    /// Radius in pixels.
    Pixels(f32),
    /// Radius as percentage of the smaller dimension.
    Percent(f32),
    /// Perfect circle (min dimension / 2).
    Circle,
    /// Per-corner pixel radii [top-left, top-right, bottom-right, bottom-left].
    Custom([f32; 4]),
}

fn default_threshold() -> u32 {
    80
}
