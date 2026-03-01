//! JSON sidecar writer for bitmap-level metadata.
//!
//! Each test output can have a `.json` sidecar alongside its PNG,
//! recording format details, dimensions, and diff statistics for
//! forensic analysis and cross-referencing.

use serde::Serialize;
use std::io;
use std::path::Path;

/// Metadata about a test bitmap output, written as a JSON sidecar.
#[derive(Serialize)]
pub struct BitmapMetadata {
    pub test_name: String,
    pub checksum: String,
    pub width: u32,
    pub height: u32,
    /// Imageflow pixel format name (e.g., "Bgra32", "Bgr32", "Bgr24").
    pub pixel_format: String,
    /// Imageflow pixel layout name (e.g., "BGRA", "BGR", "Gray").
    pub pixel_layout: String,
    pub stride_bytes: u32,
    pub alpha_meaningful: bool,
    pub arch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s3_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_stats: Option<DiffStatsJson>,
}

/// Summary diff statistics for the sidecar.
#[derive(Serialize)]
pub struct DiffStatsJson {
    pub pixels_differing: i64,
    pub max_channel_delta: i64,
    pub values_differing_by_more_than_1: i64,
}

/// Write a JSON sidecar file alongside a test output image.
///
/// The sidecar is named `{checksum}.json` in the visuals directory.
pub fn write_sidecar(
    visuals_dir: &Path,
    checksum: &str,
    metadata: &BitmapMetadata,
) -> io::Result<()> {
    let path = visuals_dir.join(format!("{checksum}.json"));
    let json = serde_json::to_string_pretty(metadata)?;
    std::fs::write(&path, json)
}
