use serde::{Deserialize, Serialize};

/// Resource and security limits for pipeline execution.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SecurityLimits {
    /// Maximum decode dimensions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_decode_size: Option<SizeLimit>,
    /// Maximum intermediate frame dimensions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_frame_size: Option<SizeLimit>,
    /// Maximum encode dimensions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_encode_size: Option<SizeLimit>,
    /// Pipeline execution timeout in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_timeout_ms: Option<u64>,
    /// Maximum memory usage in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_memory_bytes: Option<u64>,
    /// Maximum encoder thread count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_encoder_threads: Option<u32>,
}

/// Dimension limits.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SizeLimit {
    /// Max width in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<u32>,
    /// Max height in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<u32>,
    /// Max total megapixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub megapixels: Option<f32>,
}
