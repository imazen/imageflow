//! Shared declarative JSON/TOML schemas for bulk media processing (`imageflow-bulk-v1`),
//! responsive asset manifests (`imageflow-manifest-v1`), and interactive/static galleries
//! (`imageflow-gallery-v1`).
//!
//! Field names and enum tokens are intentionally explicit and stable across Rust CLI,
//! interactive gallery studio, and NativeAOT `.NET` (`Imageflow.Media`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub const BULK_SCHEMA_VERSION: &str = "imageflow-bulk-v1";
pub const MANIFEST_SCHEMA_VERSION: &str = "imageflow-manifest-v1";
pub const GALLERY_SCHEMA_VERSION: &str = "imageflow-gallery-v1";

/// Perceptual or calibrated quality target for lossy encoding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum QualityTarget {
    /// Closed-loop verified `zensim_a` perceptual score (e.g. `85.0` = crisp web, `90.0` = gallery HQ).
    VerifyZensimA { target: f32 },
    /// Fast open-loop `zensim_a` target (estimated without decode-readback verification).
    AimForZensimA { target: f32 },
    /// Backend-independent calibrated quality dial (`1.0..=100.0`).
    CalibratedQuality { quality: f32 },
    /// Semantic quality profile (`low`, `good`, `high`, `visually_lossless`) with DPR hint.
    QualityProfile {
        level: QualityProfileLevel,
        #[serde(default = "default_dpr")]
        dpr: f32,
    },
    /// Bit-exact lossless encoding.
    Lossless,
}

fn default_dpr() -> f32 {
    1.0
}

impl Default for QualityTarget {
    fn default() -> Self {
        Self::VerifyZensimA { target: 85.0 }
    }
}

impl QualityTarget {
    pub fn validate(&self) -> Result<(), String> {
        match *self {
            Self::VerifyZensimA { target } | Self::AimForZensimA { target } => {
                if !target.is_finite() || !(20.0..=99.5).contains(&target) {
                    return Err(format!(
                        "zensim_a target must be finite and within 20.0..=99.5 (got {target})"
                    ));
                }
            }
            Self::CalibratedQuality { quality } => {
                if !quality.is_finite() || !(1.0..=100.0).contains(&quality) {
                    return Err(format!(
                        "calibrated_quality must be finite and within 1.0..=100.0 (got {quality})"
                    ));
                }
            }
            Self::QualityProfile { dpr, .. } => {
                if !dpr.is_finite() || !(0.5..=4.0).contains(&dpr) {
                    return Err(format!(
                        "quality_profile dpr must be finite and within 0.5..=4.0 (got {dpr})"
                    ));
                }
            }
            Self::Lossless => {}
        }
        Ok(())
    }

    /// Short human-readable label recorded in manifests and reports.
    pub fn display_label(&self) -> String {
        match *self {
            Self::VerifyZensimA { target } => format!("verify(zensim_a>={target:.1})"),
            Self::AimForZensimA { target } => format!("aim_for(zensim_a~{target:.1})"),
            Self::CalibratedQuality { quality } => format!("calibrated_quality({quality:.1})"),
            Self::QualityProfile { level, dpr } => format!("profile({level:?}@{dpr:.1}x)"),
            Self::Lossless => "lossless".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityProfileLevel {
    Low,
    Good,
    High,
    VisuallyLossless,
}

impl QualityProfileLevel {
    pub fn as_wire_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Good => "good",
            Self::High => "high",
            Self::VisuallyLossless => "visually_lossless",
        }
    }
}

/// Controls how a `VerifyZensimA` / `AimForZensimA` request behaves when a target format
/// (such as lossless PNG or an encoder without verified-quality calibration under the active
/// workspace policy) does not advertise closed-loop verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityFallbackPolicy {
    /// Fall back transparently to calibrated quality (or lossless for PNG) ONLY for known
    /// unsupported format × workspace routes, recording both requested and effective coding
    /// plus the exact fallback reason in the manifest. Never swallows runtime failures.
    #[default]
    Calibrated,
    /// Refuse up front if any requested lossy format cannot honor closed-loop verification
    /// under the active workspace policy.
    StrictVerify,
}

/// Supported still and video output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Avif,
    Jxl,
    Webp,
    Jpeg,
    Png,
    Mp4Av1,
    WebmVp9,
    WebmVp8,
}

impl OutputFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Avif => "avif",
            Self::Jxl => "jxl",
            Self::Webp => "webp",
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Mp4Av1 => "mp4",
            Self::WebmVp9 | Self::WebmVp8 => "webm",
        }
    }

    pub fn wire_format(self) -> &'static str {
        match self {
            Self::Avif => "avif",
            Self::Jxl => "jxl",
            Self::Webp => "webp",
            Self::Jpeg => "jpeg",
            Self::Png => "png",
            Self::Mp4Av1 => "mp4",
            Self::WebmVp9 | Self::WebmVp8 => "webm",
        }
    }

    pub fn video_codec(self) -> Option<&'static str> {
        match self {
            Self::Mp4Av1 => Some("av1"),
            Self::WebmVp9 => Some("vp9"),
            Self::WebmVp8 => Some("vp8"),
            _ => None,
        }
    }

    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Avif => "image/avif",
            Self::Jxl => "image/jxl",
            Self::Webp => "image/webp",
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Mp4Av1 => "video/mp4",
            Self::WebmVp9 | Self::WebmVp8 => "video/webm",
        }
    }

    pub fn is_lossless_only(self) -> bool {
        matches!(self, Self::Png)
    }

    pub fn is_video(self) -> bool {
        self.video_codec().is_some()
    }

    /// Priority for `<picture><source>` ordering (modern high-efficiency codecs first).
    pub fn picture_source_priority(self) -> u8 {
        match self {
            Self::Avif => 0,
            Self::Jxl => 1,
            Self::Webp => 2,
            Self::Jpeg => 3,
            Self::Png => 4,
            Self::Mp4Av1 => 5,
            Self::WebmVp9 => 6,
            Self::WebmVp8 => 7,
        }
    }
}

impl FromStr for OutputFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "avif" => Ok(Self::Avif),
            "jxl" => Ok(Self::Jxl),
            "webp" => Ok(Self::Webp),
            "jpeg" | "jpg" => Ok(Self::Jpeg),
            "png" => Ok(Self::Png),
            "mp4" | "mp4_av1" | "av1" => Ok(Self::Mp4Av1),
            "webm" | "webm_vp9" | "vp9" => Ok(Self::WebmVp9),
            "webm_vp8" | "vp8" => Ok(Self::WebmVp8),
            other => Err(format!(
                "unknown output format '{other}' (expected avif, webp, jpeg, png, jxl, mp4_av1, webm_vp9, webm_vp8)"
            )),
        }
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Avif => "avif",
            Self::Jxl => "jxl",
            Self::Webp => "webp",
            Self::Jpeg => "jpeg",
            Self::Png => "png",
            Self::Mp4Av1 => "mp4_av1",
            Self::WebmVp9 => "webm_vp9",
            Self::WebmVp8 => "webm_vp8",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitMode {
    #[default]
    Within,
    Fit,
    FitCrop,
    WithinCrop,
    WithinPad,
    FitPad,
}

impl FitMode {
    pub fn as_wire_str(self) -> &'static str {
        match self {
            Self::Within => "within",
            Self::Fit => "fit",
            Self::FitCrop => "fit_crop",
            Self::WithinCrop => "within_crop",
            Self::WithinPad => "within_pad",
            Self::FitPad => "fit_pad",
        }
    }
}

impl FromStr for FitMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "within" | "contain" => Ok(Self::Within),
            "fit" => Ok(Self::Fit),
            "fit_crop" | "cover" | "crop" => Ok(Self::FitCrop),
            "within_crop" => Ok(Self::WithinCrop),
            "within_pad" | "pad" => Ok(Self::WithinPad),
            "fit_pad" => Ok(Self::FitPad),
            other => Err(format!(
                "unknown fit mode '{other}' (expected within, fit, fit_crop, within_crop, within_pad, fit_pad)"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncodeEffort {
    Fast,
    #[default]
    Balanced,
    Thorough,
}

impl EncodeEffort {
    pub fn as_wire_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Balanced => "balanced",
            Self::Thorough => "thorough",
        }
    }
}

impl FromStr for EncodeEffort {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "fast" => Ok(Self::Fast),
            "balanced" => Ok(Self::Balanced),
            "thorough" => Ok(Self::Thorough),
            other => Err(format!("unknown effort '{other}' (expected fast, balanced, thorough)")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataRetention {
    #[default]
    Web,
    Preserve,
    ColorAndRotation,
    StripAll,
}

impl MetadataRetention {
    pub fn as_wire_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Preserve => "preserve",
            Self::ColorAndRotation => "color_and_rotation",
            Self::StripAll => "strip_all",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceAdmission {
    /// Allows qualified encoders and WebP/video routes whose native workspaces are best-effort
    /// bounded (recommended for mixed-format web galleries and video posters).
    #[default]
    BestEffort,
    /// Requires proven strict workspace bounds on every decode/encode step.
    Strict,
}

impl WorkspaceAdmission {
    pub fn as_wire_str(self) -> &'static str {
        match self {
            Self::BestEffort => "best_effort",
            Self::Strict => "strict",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AspectRatio {
    pub width: u32,
    pub height: u32,
}

impl AspectRatio {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        if width == 0 || height == 0 || width > 100 || height > 100 {
            return Err(format!("aspect ratio components must be 1..=100 (got {width}:{height})"));
        }
        Ok(Self { width, height })
    }

    pub fn css_string(self) -> String {
        format!("{} / {}", self.width, self.height)
    }
}

impl FromStr for AspectRatio {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        let (a, b) = trimmed
            .split_once(':')
            .or_else(|| trimmed.split_once('/'))
            .ok_or_else(|| format!("invalid aspect ratio '{trimmed}' (expected W:H like 16:9)"))?;
        let w: u32 = a.trim().parse().map_err(|_| format!("invalid aspect width '{a}'"))?;
        let h: u32 = b.trim().parse().map_err(|_| format!("invalid aspect height '{b}'"))?;
        Self::new(w, h)
    }
}

/// Reusable responsive encoding preset shared across CLI bulk jobs, gallery builds, and C#.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponsivePreset {
    #[serde(default = "default_widths")]
    pub widths_px: Vec<u32>,
    #[serde(default = "default_formats")]
    pub formats: Vec<OutputFormat>,
    #[serde(default)]
    pub quality: QualityTarget,
    #[serde(default)]
    pub quality_fallback: QualityFallbackPolicy,
    #[serde(default)]
    pub fit_mode: FitMode,
    #[serde(default)]
    pub aspect_ratio: Option<AspectRatio>,
    #[serde(default)]
    pub allow_upscale: bool,
    #[serde(default)]
    pub effort: EncodeEffort,
    #[serde(default)]
    pub metadata: MetadataRetention,
    #[serde(default)]
    pub sharpen_amount: Option<f32>,
    #[serde(default)]
    pub workspace_policy: WorkspaceAdmission,
    #[serde(default = "default_true")]
    pub generate_video_posters: bool,
    #[serde(default)]
    pub video_formats: Vec<OutputFormat>,
}

fn default_widths() -> Vec<u32> {
    vec![400, 800, 1600]
}
fn default_formats() -> Vec<OutputFormat> {
    vec![OutputFormat::Avif, OutputFormat::Webp, OutputFormat::Jpeg]
}
fn default_true() -> bool {
    true
}

impl Default for ResponsivePreset {
    fn default() -> Self {
        Self::web_responsive()
    }
}

impl ResponsivePreset {
    pub fn web_responsive() -> Self {
        Self {
            widths_px: vec![400, 800, 1600],
            formats: vec![OutputFormat::Avif, OutputFormat::Webp, OutputFormat::Jpeg],
            quality: QualityTarget::VerifyZensimA { target: 85.0 },
            quality_fallback: QualityFallbackPolicy::Calibrated,
            fit_mode: FitMode::Within,
            aspect_ratio: None,
            allow_upscale: false,
            effort: EncodeEffort::Balanced,
            metadata: MetadataRetention::Web,
            sharpen_amount: None,
            workspace_policy: WorkspaceAdmission::BestEffort,
            generate_video_posters: true,
            video_formats: Vec::new(),
        }
    }

    pub fn gallery_high_quality() -> Self {
        Self {
            widths_px: vec![480, 960, 1600, 2400],
            formats: vec![OutputFormat::Avif, OutputFormat::Webp, OutputFormat::Jpeg],
            quality: QualityTarget::VerifyZensimA { target: 90.0 },
            ..Self::web_responsive()
        }
    }

    pub fn thumbnail_grid() -> Self {
        Self {
            widths_px: vec![320, 640],
            formats: vec![OutputFormat::Avif, OutputFormat::Webp, OutputFormat::Jpeg],
            quality: QualityTarget::VerifyZensimA { target: 82.0 },
            fit_mode: FitMode::FitCrop,
            aspect_ratio: Some(AspectRatio { width: 1, height: 1 }),
            ..Self::web_responsive()
        }
    }

    pub fn compact_mobile() -> Self {
        Self {
            widths_px: vec![320, 640, 1024],
            formats: vec![OutputFormat::Avif, OutputFormat::Webp, OutputFormat::Jpeg],
            quality: QualityTarget::VerifyZensimA { target: 76.0 },
            ..Self::web_responsive()
        }
    }

    pub fn archival() -> Self {
        Self {
            widths_px: vec![4096],
            formats: vec![OutputFormat::Avif, OutputFormat::Jpeg],
            quality: QualityTarget::VerifyZensimA { target: 94.0 },
            metadata: MetadataRetention::Preserve,
            ..Self::web_responsive()
        }
    }

    pub fn social_cards() -> Self {
        Self {
            widths_px: vec![1200],
            formats: vec![OutputFormat::Jpeg, OutputFormat::Webp],
            quality: QualityTarget::VerifyZensimA { target: 86.0 },
            fit_mode: FitMode::FitCrop,
            aspect_ratio: Some(AspectRatio { width: 40, height: 21 }),
            ..Self::web_responsive()
        }
    }

    pub fn from_preset_name(name: &str) -> Result<Self, String> {
        match name.trim().to_ascii_lowercase().as_str() {
            "web-responsive" | "web_responsive" | "web" => Ok(Self::web_responsive()),
            "gallery-hq" | "gallery_hq" | "gallery" => Ok(Self::gallery_high_quality()),
            "thumb-grid" | "thumb_grid" | "thumbs" => Ok(Self::thumbnail_grid()),
            "compact-mobile" | "compact_mobile" | "compact" => Ok(Self::compact_mobile()),
            "archival" => Ok(Self::archival()),
            "social-cards" | "social_cards" | "social" => Ok(Self::social_cards()),
            other => Err(format!(
                "unknown preset '{other}' (expected web-responsive, gallery-hq, thumb-grid, compact-mobile, archival, social-cards)"
            )),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.widths_px.is_empty() {
            return Err("widths_px must contain at least one positive width".into());
        }
        for &w in &self.widths_px {
            if w == 0 || w > 16_384 {
                return Err(format!("width {w} is out of range 1..=16384"));
            }
        }
        if self.formats.is_empty() {
            return Err("formats must contain at least one still output format".into());
        }
        if self.formats.iter().any(|f| f.is_video()) {
            return Err("still formats list must not contain video formats (use video_formats)".into());
        }
        if self.video_formats.iter().any(|f| !f.is_video()) {
            return Err("video_formats must only contain video formats (mp4_av1, webm_vp9, webm_vp8)".into());
        }
        if let Some(ar) = self.aspect_ratio {
            AspectRatio::new(ar.width, ar.height)?;
        }
        if let Some(s) = self.sharpen_amount {
            if !s.is_finite() || !(0.0..=10.0).contains(&s) {
                return Err(format!("sharpen_amount must be finite within 0.0..=10.0 (got {s})"));
            }
        }
        self.quality.validate()?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputPathLayout {
    #[default]
    ContentAddressed,
    PreserveTree,
    Flatten,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailurePolicy {
    #[default]
    Continue,
    FailFast,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkLimits {
    #[serde(default = "default_max_memory")]
    pub max_memory_bytes: u64,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_max_dimension")]
    pub max_dimension: u32,
}

fn default_max_memory() -> u64 {
    1024 * 1024 * 1024 // 1 GiB per job
}
fn default_timeout_ms() -> u64 {
    120_000
}
fn default_max_dimension() -> u32 {
    16_384
}

impl Default for BulkLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: default_max_memory(),
            timeout_ms: default_timeout_ms(),
            max_dimension: default_max_dimension(),
        }
    }
}

/// Complete bulk job configuration (`schema = "imageflow-bulk-v1"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BulkConfig {
    #[serde(default = "default_bulk_schema")]
    pub schema: String,
    pub inputs: Vec<PathBuf>,
    pub output_dir: PathBuf,
    #[serde(default = "default_true")]
    pub recursive: bool,
    #[serde(default)]
    pub include_globs: Vec<String>,
    #[serde(default)]
    pub exclude_globs: Vec<String>,
    #[serde(default)]
    pub follow_symlinks: bool,
    #[serde(default)]
    pub output_layout: OutputPathLayout,
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
    #[serde(default = "default_true")]
    pub incremental_cache: bool,
    #[serde(default)]
    pub force_rebuild: bool,
    #[serde(default)]
    pub on_error: FailurePolicy,
    #[serde(default = "default_true")]
    pub write_manifest: bool,
    #[serde(default)]
    pub write_snippets: bool,
    #[serde(default)]
    pub preset: ResponsivePreset,
    #[serde(default)]
    pub limits: BulkLimits,
}

fn default_bulk_schema() -> String {
    BULK_SCHEMA_VERSION.to_string()
}

fn default_concurrency() -> usize {
    std::thread::available_parallelism().map_or(2, |n| n.get().clamp(1, 4))
}

impl BulkConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != BULK_SCHEMA_VERSION {
            return Err(format!(
                "unsupported bulk schema '{}' (expected '{BULK_SCHEMA_VERSION}')",
                self.schema
            ));
        }
        if self.inputs.is_empty() {
            return Err("inputs must contain at least one file or directory".into());
        }
        if self.concurrency == 0 || self.concurrency > 64 {
            return Err(format!("concurrency must be 1..=64 (got {})", self.concurrency));
        }
        self.preset.validate()?;
        Ok(())
    }

    pub fn load_file(path: &Path) -> Result<Self, String> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read config '{}': {e}", path.display()))?;
        let is_json = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("json"));
        let cfg: Self = if is_json {
            serde_json::from_str(&raw).map_err(|e| format!("invalid JSON bulk config: {e}"))?
        } else {
            toml::from_str(&raw).map_err(|e| format!("invalid TOML bulk config: {e}"))?
        };
        cfg.validate()?;
        Ok(cfg)
    }
}

/// Machine-readable responsive asset manifest (`schema = "imageflow-manifest-v1"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BulkManifest {
    pub schema: String,
    pub generated_at_utc: String,
    pub generator: String,
    pub preset: ResponsivePreset,
    pub total_input_bytes: u64,
    pub total_output_bytes: u64,
    pub assets: Vec<ManifestAsset>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestAsset {
    pub id: String,
    pub source_relative_path: String,
    pub source_sha256: String,
    pub source_bytes: u64,
    pub media_kind: String,
    pub source_format: String,
    pub original_width: u32,
    pub original_height: u32,
    pub variants: Vec<ManifestVariant>,
    #[serde(default)]
    pub video_variants: Vec<ManifestVariant>,
    pub picture_html: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestVariant {
    pub relative_path: String,
    pub format: OutputFormat,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub requested_coding: String,
    pub effective_coding: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_metric: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_score: Option<f64>,
}

// =============================================================================
// Gallery Design Schema (`imageflow-gallery-v1`)
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GalleryLayout {
    #[default]
    Justified,
    Masonry,
    Grid,
    Editorial,
    Filmstrip,
}

impl FromStr for GalleryLayout {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "justified" => Ok(Self::Justified),
            "masonry" => Ok(Self::Masonry),
            "grid" => Ok(Self::Grid),
            "editorial" => Ok(Self::Editorial),
            "filmstrip" => Ok(Self::Filmstrip),
            other => Err(format!(
                "unknown gallery layout '{other}' (expected justified, masonry, grid, editorial, filmstrip)"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GalleryTheme {
    #[default]
    GalleryDark,
    PaperLight,
    MinimalWhite,
    WarmLinen,
    MidnightSlate,
}

impl FromStr for GalleryTheme {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "gallery_dark" | "dark" => Ok(Self::GalleryDark),
            "paper_light" | "paper" => Ok(Self::PaperLight),
            "minimal_white" | "light" | "white" => Ok(Self::MinimalWhite),
            "warm_linen" | "linen" => Ok(Self::WarmLinen),
            "midnight_slate" | "slate" => Ok(Self::MidnightSlate),
            other => Err(format!(
                "unknown gallery theme '{other}' (expected gallery_dark, paper_light, minimal_white, warm_linen, midnight_slate)"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GalleryFontFamily {
    #[default]
    Sans,
    Serif,
    Mono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GalleryCaptionStyle {
    #[default]
    Below,
    HoverOverlay,
    LightboxOnly,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GallerySortOrder {
    #[default]
    FilenameAsc,
    FilenameDesc,
    AspectRatio,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GalleryDesign {
    #[serde(default)]
    pub layout: GalleryLayout,
    #[serde(default)]
    pub theme: GalleryTheme,
    #[serde(default)]
    pub font_family: GalleryFontFamily,
    #[serde(default = "default_row_height")]
    pub target_row_height_px: u32,
    #[serde(default = "default_cols_desktop")]
    pub columns_desktop: u32,
    #[serde(default = "default_cols_tablet")]
    pub columns_tablet: u32,
    #[serde(default = "default_cols_mobile")]
    pub columns_mobile: u32,
    #[serde(default)]
    pub grid_aspect_ratio: Option<AspectRatio>,
    #[serde(default = "default_gap")]
    pub gap_px: u32,
    #[serde(default = "default_radius")]
    pub corner_radius_px: u32,
    #[serde(default = "default_max_width")]
    pub max_width_px: u32,
    #[serde(default = "default_padding")]
    pub padding_px: u32,
    #[serde(default)]
    pub caption_style: GalleryCaptionStyle,
    #[serde(default = "default_true")]
    pub show_exif_badges: bool,
    #[serde(default = "default_true")]
    pub show_download_button: bool,
    #[serde(default = "default_true")]
    pub enable_lightbox: bool,
    #[serde(default)]
    pub accent_color_hex: Option<String>,
}

fn default_row_height() -> u32 {
    280
}
fn default_cols_desktop() -> u32 {
    3
}
fn default_cols_tablet() -> u32 {
    2
}
fn default_cols_mobile() -> u32 {
    1
}
fn default_gap() -> u32 {
    14
}
fn default_radius() -> u32 {
    8
}
fn default_max_width() -> u32 {
    1440
}
fn default_padding() -> u32 {
    32
}

impl Default for GalleryDesign {
    fn default() -> Self {
        Self {
            layout: GalleryLayout::Justified,
            theme: GalleryTheme::GalleryDark,
            font_family: GalleryFontFamily::Sans,
            target_row_height_px: default_row_height(),
            columns_desktop: default_cols_desktop(),
            columns_tablet: default_cols_tablet(),
            columns_mobile: default_cols_mobile(),
            grid_aspect_ratio: Some(AspectRatio { width: 4, height: 3 }),
            gap_px: default_gap(),
            corner_radius_px: default_radius(),
            max_width_px: default_max_width(),
            padding_px: default_padding(),
            caption_style: GalleryCaptionStyle::Below,
            show_exif_badges: true,
            show_download_button: true,
            enable_lightbox: true,
            accent_color_hex: None,
        }
    }
}

impl GalleryDesign {
    pub fn validate(&self) -> Result<(), String> {
        if !(80..=1200).contains(&self.target_row_height_px) {
            return Err("target_row_height_px must be within 80..=1200".into());
        }
        if !(1..=8).contains(&self.columns_desktop)
            || !(1..=6).contains(&self.columns_tablet)
            || !(1..=4).contains(&self.columns_mobile)
        {
            return Err("column counts out of range".into());
        }
        if self.gap_px > 64 {
            return Err("gap_px must be <= 64".into());
        }
        if self.corner_radius_px > 48 {
            return Err("corner_radius_px must be <= 48".into());
        }
        if !(320..=3840).contains(&self.max_width_px) {
            return Err("max_width_px must be within 320..=3840".into());
        }
        if self.padding_px > 128 {
            return Err("padding_px must be <= 128".into());
        }
        if let Some(ar) = self.grid_aspect_ratio {
            AspectRatio::new(ar.width, ar.height)?;
        }
        if let Some(ref hex) = self.accent_color_hex {
            validate_hex_color(hex)?;
        }
        Ok(())
    }
}

pub fn validate_hex_color(s: &str) -> Result<(), String> {
    let bytes = s.as_bytes();
    let valid_len = bytes.len() == 4 || bytes.len() == 7;
    if !valid_len || bytes[0] != b'#' || !bytes[1..].iter().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("invalid hex color '{s}' (expected #RGB or #RRGGBB)"));
    }
    Ok(())
}

/// Per-photo metadata & curation override inside a gallery config.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GalleryItemOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt_text: Option<String>,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub featured: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<u32>,
}

/// Complete gallery design & encoding configuration (`schema = "imageflow-gallery-v1"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GalleryConfig {
    #[serde(default = "default_gallery_schema")]
    pub schema: String,
    #[serde(default = "default_gallery_title")]
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub footer_text: Option<String>,
    #[serde(default)]
    pub sort_by: GallerySortOrder,
    #[serde(default)]
    pub design: GalleryDesign,
    #[serde(default = "ResponsivePreset::gallery_high_quality")]
    pub encoding: ResponsivePreset,
    #[serde(default)]
    pub items: BTreeMap<String, GalleryItemOverride>,
}

fn default_gallery_schema() -> String {
    GALLERY_SCHEMA_VERSION.to_string()
}
fn default_gallery_title() -> String {
    "Photo Gallery".to_string()
}

impl Default for GalleryConfig {
    fn default() -> Self {
        Self {
            schema: default_gallery_schema(),
            title: default_gallery_title(),
            subtitle: None,
            author: None,
            footer_text: None,
            sort_by: GallerySortOrder::FilenameAsc,
            design: GalleryDesign::default(),
            encoding: ResponsivePreset::gallery_high_quality(),
            items: BTreeMap::new(),
        }
    }
}

impl GalleryConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != GALLERY_SCHEMA_VERSION {
            return Err(format!(
                "unsupported gallery schema '{}' (expected '{GALLERY_SCHEMA_VERSION}')",
                self.schema
            ));
        }
        if self.title.len() > 256 {
            return Err("gallery title must be <= 256 bytes".into());
        }
        self.design.validate()?;
        self.encoding.validate()?;
        Ok(())
    }

    pub fn load_file(path: &Path) -> Result<Self, String> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read gallery config '{}': {e}", path.display()))?;
        let is_json = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("json"));
        let cfg: Self = if is_json {
            serde_json::from_str(&raw).map_err(|e| format!("invalid JSON gallery config: {e}"))?
        } else {
            toml::from_str(&raw).map_err(|e| format!("invalid TOML gallery config: {e}"))?
        };
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn to_toml_string(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| format!("TOML serialization failed: {e}"))
    }

    pub fn to_json_string(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| format!("JSON serialization failed: {e}"))
    }
}
