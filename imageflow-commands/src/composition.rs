use serde::{Deserialize, Serialize};

use crate::{BlendMode, Color, Gravity, NodeId, ResizeHints};

/// Composite an image at exact pixel coordinates.
///
/// In graph mode, `source` references the overlay node.
/// In sequential mode, `io_id` identifies the overlay I/O source.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrawImageStep {
    /// I/O source for the overlay image (sequential mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_id: Option<i32>,
    /// Graph node reference for the overlay (graph mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<NodeId>,
    /// Destination X.
    pub x: i32,
    /// Destination Y.
    pub y: i32,
    /// Target width.
    pub w: u32,
    /// Target height.
    pub h: u32,
    /// Compositing mode.
    #[serde(default)]
    pub blend: BlendMode,
    /// Resize hints for the overlay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hints: Option<ResizeHints>,
}

/// Watermark overlay with positioning and sizing.
///
/// The watermark image is loaded from `io_id`, resized to fit within
/// `fit_box`, positioned by `gravity`, and composited with `opacity`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WatermarkStep {
    /// I/O source for the watermark image (sequential mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_id: Option<i32>,
    /// Graph node reference for the watermark (graph mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<NodeId>,
    /// Bounding box for watermark placement (percentage of canvas).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit_box: Option<FitBox>,
    /// Anchor position.
    #[serde(default)]
    pub gravity: Gravity,
    /// Watermark opacity (0.0–1.0).
    #[serde(default = "default_one")]
    pub opacity: f32,
    /// Don't show watermark if canvas is narrower than this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_canvas_width: Option<u32>,
    /// Don't show watermark if canvas is shorter than this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_canvas_height: Option<u32>,
    /// Resize hints for the watermark.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hints: Option<ResizeHints>,
}

/// Bounding box for watermark placement.
///
/// Values are percentages of the canvas (0.0–1.0).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FitBox {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// Copy a rectangle from the input to a position on the canvas.
///
/// Two-input node: one input image, one canvas.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CopyRectStep {
    /// Source rectangle X.
    pub from_x: u32,
    /// Source rectangle Y.
    pub from_y: u32,
    /// Rectangle width.
    pub w: u32,
    /// Rectangle height.
    pub h: u32,
    /// Destination X on canvas.
    pub x: u32,
    /// Destination Y on canvas.
    pub y: u32,
}

/// HDR canvas output — writes the processing result as an HDR canvas.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HdrCanvasOutputStep {
    /// Background color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
}

fn default_one() -> f32 {
    1.0
}
