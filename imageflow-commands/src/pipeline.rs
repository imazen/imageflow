use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::{
    AdjustStep, BlurStep, ColorFilterStep, ColorMatrixStep, ConstrainStep, CopyRectStep,
    CreateCanvasStep, CropStep, CropWhitespaceStep, DecodeStep, DrawImageStep, EncodeStep,
    ExpandCanvasStep, FillRectStep, HdrCanvasOutputStep, IoObject, NodeId, OrientStep,
    RegionPercentStep, RegionStep, ResizeStep, RoundCornersStep, SecurityLimits, SharpenStep,
    WatermarkStep, WhiteBalanceStep,
};

// ─── Pipeline Step ──────────────────────────────────────────────────────

/// A single pipeline step.
///
/// In **sequential mode**, steps execute in order. The output of one step
/// feeds into the next.
///
/// In **graph mode**, steps are placed in a `Graph` with explicit edges
/// connecting them via `NodeId`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    // ── I/O ──
    Decode(DecodeStep),
    Encode(EncodeStep),

    // ── Geometry ──
    Constrain(ConstrainStep),
    Resize(ResizeStep),
    Crop(CropStep),
    CropWhitespace(CropWhitespaceStep),
    Region(RegionStep),
    RegionPercent(RegionPercentStep),
    Orient(OrientStep),
    FlipH,
    FlipV,
    #[serde(alias = "rotate_90")]
    Rotate90,
    #[serde(alias = "rotate_180")]
    Rotate180,
    #[serde(alias = "rotate_270")]
    Rotate270,
    Transpose,

    // ── Canvas ──
    ExpandCanvas(ExpandCanvasStep),
    FillRect(FillRectStep),
    CreateCanvas(CreateCanvasStep),
    RoundCorners(RoundCornersStep),

    // ── Color & Filters ──
    /// Perceptual adjustments (exposure, contrast, saturation, etc.) in Oklab space.
    Adjust(AdjustStep),
    ColorMatrix(ColorMatrixStep),
    ColorFilter(ColorFilterStep),
    Sharpen(SharpenStep),
    Blur(BlurStep),
    WhiteBalance(WhiteBalanceStep),

    // ── Composition ──
    DrawImage(DrawImageStep),
    Watermark(WatermarkStep),
    CopyRect(CopyRectStep),
    HdrCanvasOutput(HdrCanvasOutputStep),

    // ── Legacy ──
    /// RIAPI querystring command (backward compatibility).
    CommandString(CommandStringStep),
}

/// Legacy RIAPI command string step.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandStringStep {
    /// The querystring value (e.g., "w=800&h=600&mode=crop").
    pub value: String,
    /// I/O id to decode from (if not already decoded).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decode: Option<i32>,
    /// I/O id to encode to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encode: Option<i32>,
}

// ─── Graph Representation ───────────────────────────────────────────────

/// A directed acyclic graph of operations.
///
/// Nodes are identified by `NodeId`. Edges are defined inline via
/// `NodeId` references in step parameters, plus explicit `edges`
/// for multi-input operations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Graph {
    /// Named nodes. Keys are string node IDs for JSON ergonomics.
    pub nodes: BTreeMap<String, GraphNode>,
    /// Explicit edges (for operations with multiple inputs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<Edge>,
}

/// A node in the processing graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphNode {
    /// The operation this node performs.
    #[serde(flatten)]
    pub step: Step,
    /// Explicit input node ID (alternative to edge list).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<NodeId>,
}

/// An edge in the processing graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    #[serde(default)]
    pub kind: EdgeKind,
}

/// Edge type in the processing graph.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Primary input.
    #[default]
    Input,
    /// Canvas / background input (for compositing operations).
    Canvas,
}

// ─── Request / Response ─────────────────────────────────────────────────

/// Pipeline execution mode.
///
/// Supports both sequential (ordered list) and graph (DAG) modes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pipeline {
    /// Sequential steps — output of each feeds into the next.
    Steps(Vec<Step>),
    /// Directed acyclic graph of operations.
    Graph(Graph),
}

/// Complete build request: I/O bindings + pipeline + security limits.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildRequest {
    pub io: Vec<IoObject>,
    pub pipeline: Pipeline,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<SecurityLimits>,
}

/// Execute request (I/O already bound to context).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecuteRequest {
    pub pipeline: Pipeline,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<SecurityLimits>,
}

// ─── Response Types ─────────────────────────────────────────────────────

/// Response wrapper.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Response {
    pub code: u32,
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<ResponseData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Response payload variants.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseData {
    BuildResult(BuildResult),
    ImageInfo(ImageInfo),
    VersionInfo(VersionInfo),
}

/// Result of a build/execute request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildResult {
    pub outputs: Vec<EncodeResult>,
}

/// Result of a single encode operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncodeResult {
    pub io_id: i32,
    pub format: String,
    pub mime_type: String,
    pub w: u32,
    pub h: u32,
    pub bytes: u64,
}

/// Image metadata from a decode operation.
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
    /// Number of frames (for animated images).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_count: Option<u32>,
}

/// Color profile metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColorProfileInfo {
    /// CICP codes if present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cicp: Option<CicpInfo>,
    /// Whether an ICC profile is embedded.
    pub has_icc: bool,
    /// Transfer function name (e.g., "srgb", "pq", "hlg").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer: Option<String>,
    /// Color primaries name (e.g., "srgb", "display_p3", "bt2020").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primaries: Option<String>,
}

/// ITU-T H.273 CICP codes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CicpInfo {
    pub color_primaries: u8,
    pub transfer_characteristics: u8,
    pub matrix_coefficients: u8,
    pub full_range: bool,
}

/// Engine version information.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_commit: Option<String>,
    pub codecs: Vec<String>,
}
