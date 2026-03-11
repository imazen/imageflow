use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Direction of an I/O binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IoDirection {
    In,
    Out,
}

/// I/O data source or destination.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IoEnum {
    /// Hex-encoded bytes.
    BytesHex(String),
    /// Base64-encoded bytes.
    Base64(String),
    /// Raw byte array.
    ByteArray(Vec<u8>),
    /// Filesystem path.
    Filename(String),
    /// Output buffer (allocated by engine).
    OutputBuffer,
    /// Output as base64 string.
    OutputBase64,
    /// Placeholder (bound later at execution time).
    Placeholder,
}

/// Named I/O binding.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IoObject {
    pub io_id: i32,
    pub direction: IoDirection,
    #[serde(flatten)]
    pub io: IoEnum,
}
