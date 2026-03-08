#![forbid(unsafe_code)]
//! # imageflow_types v4
//!
//! JSON API schema for imageflow 4. All types that flow across the C ABI
//! as JSON messages.
//!
//! ## Naming
//!
//! All serde types use `snake_case` for JSON keys. Enum variants use
//! `snake_case` via `#[serde(rename_all = "snake_case")]`.

use serde::{Deserialize, Serialize};

// ─── Top-Level Request Types ───────────────────────────────────────────

/// Complete build request: I/O objects + pipeline + optional security.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildRequest {
    pub io: Vec<IoObject>,
    pub pipeline: Vec<Step>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<SecurityLimits>,
}

/// Execute request: pipeline only (I/O already attached to context).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecuteRequest {
    pub pipeline: Vec<Step>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<SecurityLimits>,
}

// ─── I/O ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IoObject {
    pub io_id: i32,
    pub direction: IoDirection,
    #[serde(flatten)]
    pub io: IoEnum,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IoDirection {
    In,
    Out,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IoEnum {
    BytesHex(String),
    Base64(String),
    ByteArray(Vec<u8>),
    Filename(String),
    OutputBuffer,
    OutputBase64,
    Placeholder,
}

// ─── Pipeline Steps ────────────────────────────────────────────────────

/// A single pipeline step. Steps execute in order.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    // I/O
    Decode(DecodeStep),
    Encode(EncodeStep),

    // Geometry
    Constrain(ConstrainStep),
    Crop(CropStep),
    Region(RegionStep),
    Orient(OrientStep),
    FlipH,
    FlipV,
    Rotate90,
    Rotate180,
    Rotate270,
    Transpose,

    // Canvas
    ExpandCanvas(ExpandCanvasStep),
    FillRect(FillRectStep),
    RoundCorners(RoundCornersStep),

    // Color & Filters
    ColorAdjust(ColorAdjustStep),
    ColorMatrix(ColorMatrixStep),
    ColorFilter(ColorFilterStep),
    Sharpen(SharpenStep),
    Blur(BlurStep),

    // Composition
    DrawImage(DrawImageStep),
    Watermark(WatermarkStep),

    // Legacy RIAPI
    CommandString(CommandStringStep),
}

// ─── Decode ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecodeStep {
    pub io_id: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorHandling>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hints: Option<DecodeHints>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultrahdr: Option<UltraHdrDecodeMode>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ColorHandling {
    #[serde(default)]
    pub icc: IccHandling,
    #[serde(default)]
    pub profile_errors: ProfileErrorHandling,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IccHandling {
    /// Preserve ICC/CICP through pipeline and re-embed on encode.
    #[default]
    Preserve,
    /// Convert pixels to sRGB at decode.
    ConvertToSrgb,
    /// Strip color profile, don't touch pixels.
    Strip,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileErrorHandling {
    #[default]
    Error,
    Ignore,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecodeHints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jpeg_downscale: Option<DownscaleTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownscaleTarget {
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UltraHdrDecodeMode {
    /// Decode SDR base only.
    SdrOnly,
    /// Reconstruct HDR from gain map.
    HdrReconstruct {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        boost: Option<f32>,
    },
    /// Keep SDR + gain map as separate layers.
    PreserveLayers,
}

// ─── Encode ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncodeStep {
    pub io_id: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<OutputFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<QualityTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<OutputColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultrahdr: Option<UltraHdrEncode>,
    /// Prefer lossless JPEG transform when pipeline allows (orient-only).
    #[serde(default)]
    pub prefer_lossless_jpeg: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hints: Option<EncoderHints>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matte: Option<Color>,
}

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
    Keep,
    Auto {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        allowed: Option<Vec<String>>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityTarget {
    /// Fixed quality 0-100.
    Quality(f32),
    /// Estimate source quality, re-encode to match with calibrated settings.
    MatchSource {
        /// Butteraugli tolerance. Default: 0.3
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tolerance: Option<f32>,
        /// Guarantee output ≤ source size.
        #[serde(default)]
        shrink_guarantee: bool,
    },
    /// Target Butteraugli distance.
    Butteraugli(f32),
    /// Target SSIMULACRA2 score.
    Ssimulacra2(f32),
    Lossless,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputColor {
    #[serde(default)]
    pub profile: OutputProfile,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputProfile {
    /// Re-embed original ICC/CICP.
    #[default]
    SameAsOrigin,
    Srgb,
    DisplayP3,
    Cicp {
        color_primaries: u8,
        transfer_characteristics: u8,
        matrix_coefficients: u8,
        full_range: bool,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UltraHdrEncode {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gainmap_quality: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gainmap_scale: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_peak_nits: Option<f32>,
}

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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JpegHints {
    #[serde(default)]
    pub progressive: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subsampling: Option<String>,
    #[serde(default)]
    pub trellis: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PngHints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u8>,
    #[serde(default)]
    pub max_deflate: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebPHints {
    #[serde(default)]
    pub lossless: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AvifHints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JxlHints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<u8>,
}

// ─── Geometry ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConstrainStep {
    pub mode: ConstraintMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gravity: Option<Gravity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hints: Option<ResizeHints>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintMode {
    Fit,
    Within,
    FitCrop,
    WithinCrop,
    FitPad,
    WithinPad,
    PadWithin,
    Distort,
    AspectCrop,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResizeHints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Filter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sharpen_percent: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scaling_colorspace: Option<ScalingColorspace>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Filter {
    Robidoux,
    RobidouxSharp,
    RobidouxFast,
    Lanczos,
    LanczosSharp,
    Lanczos2,
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
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalingColorspace {
    Linear,
    Srgb,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CropStep {
    pub x1: u32,
    pub y1: u32,
    pub x2: u32,
    pub y2: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegionStep {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrientStep {
    Auto,
    Exif(u8),
}

// ─── Canvas ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExpandCanvasStep {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub color: Color,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FillRectStep {
    pub x1: u32,
    pub y1: u32,
    pub x2: u32,
    pub y2: u32,
    pub color: Color,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoundCornersStep {
    #[serde(flatten)]
    pub mode: RoundCornersMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundCornersMode {
    Pixels(f32),
    Percent(f32),
    Circle,
    Custom([f32; 4]),
}

// ─── Color & Filters ───────────────────────────────────────────────────

/// Perceptual adjustments in Oklab space.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ColorAdjustStep {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brightness: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contrast: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saturation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vibrance: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exposure: Option<f32>,
}

/// 5×5 color matrix. Row-major: [R',G',B',A',1] = M × [R,G,B,A,1]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColorMatrixStep {
    pub matrix: [f32; 25],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorFilterStep {
    GrayscaleBt709,
    GrayscaleNtsc,
    GrayscaleFlat,
    Sepia,
    Invert,
    Alpha(f32),
}

/// Oklab L-channel sharpening (no color fringing).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharpenStep {
    /// Sharpening amount (0-100).
    #[serde(default = "default_sharpen")]
    pub amount: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlurStep {
    pub sigma: f32,
}

// ─── Composition ───────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrawImageStep {
    pub io_id: i32,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    #[serde(default)]
    pub blend: BlendMode,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    #[default]
    Normal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WatermarkStep {
    pub io_id: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit_box: Option<FitBox>,
    #[serde(default)]
    pub gravity: Gravity,
    #[serde(default = "default_one")]
    pub opacity: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_canvas_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_canvas_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hints: Option<ResizeHints>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FitBox {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

// ─── Legacy ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandStringStep {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decode: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encode: Option<i32>,
}

// ─── Shared ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Color {
    Hex(String),
    Srgb {
        r: u8,
        g: u8,
        b: u8,
        #[serde(default = "default_255")]
        a: u8,
    },
}

impl Color {
    pub fn transparent() -> Self {
        Color::Srgb { r: 0, g: 0, b: 0, a: 0 }
    }
    pub fn white() -> Self {
        Color::Srgb { r: 255, g: 255, b: 255, a: 255 }
    }
    pub fn black() -> Self {
        Color::Srgb { r: 0, g: 0, b: 0, a: 255 }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gravity {
    TopLeft,
    Top,
    TopRight,
    Left,
    #[default]
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

// ─── Security ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecurityLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_decode_size: Option<SizeLimit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_encode_size: Option<SizeLimit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_memory_bytes: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SizeLimit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub megapixels: Option<f32>,
}

// ─── Responses ─────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Response {
    pub code: u32,
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<ResponseData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseData {
    BuildResult(BuildResult),
    ImageInfo(ImageInfo),
    VersionInfo(VersionInfo),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildResult {
    pub outputs: Vec<EncodeResult>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncodeResult {
    pub io_id: i32,
    pub format: String,
    pub mime_type: String,
    pub w: u32,
    pub h: u32,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageInfo {
    pub format: String,
    pub w: u32,
    pub h: u32,
    pub has_alpha: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_profile: Option<ColorProfileInfo>,
    #[serde(default)]
    pub has_ultrahdr: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColorProfileInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cicp: Option<CicpInfo>,
    pub has_icc: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primaries: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CicpInfo {
    pub color_primaries: u8,
    pub transfer_characteristics: u8,
    pub matrix_coefficients: u8,
    pub full_range: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_commit: Option<String>,
    pub codecs: Vec<String>,
}

// ─── Endpoints ─────────────────────────────────────────────────────────

pub mod endpoints {
    pub const BUILD: &str = "v2/build";
    pub const EXECUTE: &str = "v2/execute";
    pub const GET_IMAGE_INFO: &str = "v2/get_image_info";
    pub const GET_VERSION_INFO: &str = "v2/get_version_info";

    // v1 compatibility
    pub const V1_BUILD: &str = "v1/build";
    pub const V1_EXECUTE: &str = "v1/execute";
}

// ─── Defaults ──────────────────────────────────────────────────────────

fn default_true() -> bool {
    true
}
fn default_one() -> f32 {
    1.0
}
fn default_sharpen() -> f32 {
    15.0
}
fn default_255() -> u8 {
    255
}

// ─── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_basic_pipeline() {
        let req = BuildRequest {
            io: vec![
                IoObject { io_id: 0, direction: IoDirection::In, io: IoEnum::Placeholder },
                IoObject { io_id: 1, direction: IoDirection::Out, io: IoEnum::OutputBuffer },
            ],
            pipeline: vec![
                Step::Decode(DecodeStep { io_id: 0, color: None, hints: None, ultrahdr: None }),
                Step::Constrain(ConstrainStep {
                    mode: ConstraintMode::Fit,
                    w: Some(800),
                    h: Some(600),
                    gravity: None,
                    background: None,
                    hints: None,
                }),
                Step::Encode(EncodeStep {
                    io_id: 1,
                    format: Some(OutputFormat::Jpeg),
                    quality: Some(QualityTarget::Quality(85.0)),
                    color: None,
                    ultrahdr: None,
                    prefer_lossless_jpeg: false,
                    hints: None,
                    matte: None,
                }),
            ],
            security: None,
        };
        let json = serde_json::to_string_pretty(&req).unwrap();
        let back: BuildRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.pipeline.len(), 3);
    }

    #[test]
    fn quality_target_variants() {
        let cases: Vec<(&str, fn(&QualityTarget) -> bool)> = vec![
            (
                r#"{"quality": 85.0}"#,
                |q| matches!(q, QualityTarget::Quality(v) if (*v - 85.0).abs() < 0.01),
            ),
            (r#"{"match_source": {"shrink_guarantee": true}}"#, |q| {
                matches!(q, QualityTarget::MatchSource { shrink_guarantee: true, .. })
            }),
            (r#"{"butteraugli": 1.5}"#, |q| matches!(q, QualityTarget::Butteraugli(_))),
            (r#""lossless""#, |q| matches!(q, QualityTarget::Lossless)),
        ];
        for (json, check) in cases {
            let q: QualityTarget = serde_json::from_str(json).unwrap();
            assert!(check(&q), "failed for {json}");
        }
    }

    #[test]
    fn constraint_modes() {
        let json = r#"{"mode": "fit_crop", "w": 800, "h": 600}"#;
        let c: ConstrainStep = serde_json::from_str(json).unwrap();
        assert!(matches!(c.mode, ConstraintMode::FitCrop));
        assert_eq!(c.w, Some(800));
    }
}
