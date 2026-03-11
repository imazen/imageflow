use serde::{Deserialize, Serialize};

/// Decode an image from an I/O source.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecodeStep {
    /// I/O identifier for the source.
    pub io_id: i32,
    /// Color management behavior during decode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorHandling>,
    /// Decode-time hints (JPEG downscaling, frame selection).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hints: Option<DecodeHints>,
    /// UltraHDR decode mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultrahdr: Option<UltraHdrDecodeMode>,
}

/// How to handle ICC/CICP color profiles during decode.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ColorHandling {
    /// ICC profile handling strategy.
    #[serde(default)]
    pub icc: IccHandling,
    /// What to do when a color profile can't be parsed.
    #[serde(default)]
    pub profile_errors: ProfileErrorHandling,
    /// Whether to honor PNG gAMA+cHRM chunks.
    #[serde(default)]
    pub honor_gama_chrm: HonorGamaChrm,
}

/// ICC/CICP profile handling strategy.
///
/// Follows PNG 3rd Edition precedence: cICP > iCCP > sRGB > gAMA+cHRM > assume sRGB.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IccHandling {
    /// Preserve ICC/CICP through pipeline and re-embed on encode.
    #[default]
    Preserve,
    /// Convert pixels to sRGB at decode time.
    ConvertToSrgb,
    /// Strip color profile, don't touch pixels.
    Strip,
}

/// How to handle unparseable or corrupt color profiles.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileErrorHandling {
    /// Fail the decode (default).
    #[default]
    Error,
    /// Ignore the broken profile and continue.
    Ignore,
}

/// Whether to honor PNG gAMA and cHRM chunks.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HonorGamaChrm {
    /// Honor gAMA+cHRM when no ICC/cICP/sRGB chunk present (default).
    #[default]
    WhenNoProfile,
    /// Always ignore gAMA+cHRM.
    Never,
    /// Always honor gAMA+cHRM (even if ICC is present).
    Always,
}

/// Decode-time hints.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DecodeHints {
    /// For JPEG: request IDCT downscaling to target size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jpeg_downscale: Option<DownscaleTarget>,
    /// For WebP: request decode at reduced resolution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webp_downscale: Option<DownscaleTarget>,
    /// Select a specific frame from multi-frame images.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<u32>,
}

/// Target dimensions for decode-time downscaling.
///
/// The decoder will choose the smallest native scale factor that
/// produces an image at least this large.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownscaleTarget {
    pub w: u32,
    pub h: u32,
}

/// UltraHDR decode behavior.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UltraHdrDecodeMode {
    /// Decode SDR base only (ignore gain map).
    SdrOnly,
    /// Reconstruct HDR from SDR + gain map.
    HdrReconstruct {
        /// Display boost factor. Default: 1.0 (SDR equivalent).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        boost: Option<f32>,
    },
    /// Keep SDR + gain map as separate layers for downstream processing.
    PreserveLayers,
}
