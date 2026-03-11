use alloc::string::String;
use serde::{Deserialize, Serialize};

/// Node identity for graph-based execution.
///
/// In sequential pipelines, nodes are implicitly ordered. In graph mode,
/// `NodeId` provides explicit edge targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u32);

/// Color specification.
///
/// Supports hex strings (`"#rrggbbaa"` or `"#rrggbb"`) and explicit sRGB components.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Color {
    /// Hex color string (e.g., `"#ff000080"`).
    Hex(String),
    /// Explicit sRGB components.
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

/// Gravity / anchor point for positioning.
///
/// Used by crop, pad, and watermark operations.
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
    /// Explicit (x%, y%) anchor. 0.0 = top/left, 1.0 = bottom/right.
    Percent {
        x: f32,
        y: f32,
    },
}

/// Blend / compositing mode.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    /// Porter-Duff source-over (default).
    #[default]
    Normal,
}

fn default_255() -> u8 {
    255
}
