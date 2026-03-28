//! Translate imageflow v2 [`Node`] variants into zennode [`NodeInstance`] objects.
//!
//! Each v2 Node variant maps to one or more zennode node instances via
//! `NodeDef::create_default()` + `set_param()`. The translation is mechanical:
//! extract fields from the Node variant, set corresponding params on the
//! zennode instance.
//!
//! Encode/Decode nodes are handled separately — they produce configuration
//! rather than pixel-processing nodes.

use std::collections::HashMap;
use std::fmt;

use imageflow_types::{
    self as s, Color, ColorFilterSrgb, CommandStringKind, CompositingMode, Constraint,
    ConstraintMode, EncoderPreset, Node, RoundCornersMode, Watermark,
};
use zennode::{NodeDef, NodeInstance, NodeRegistry, ParamValue};

use super::preset_map::PresetMapping;

/// Error during v2 Node → zennode translation.
#[derive(Debug)]
pub enum TranslateError {
    /// A node variant that isn't yet supported in the zen pipeline.
    Unsupported(String),
    /// A parameter value couldn't be converted.
    InvalidParam(String),
    /// Zenode node creation failed.
    NodeCreation(String),
}

impl fmt::Display for TranslateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(msg) => write!(f, "unsupported node: {msg}"),
            Self::InvalidParam(msg) => write!(f, "invalid param: {msg}"),
            Self::NodeCreation(msg) => write!(f, "node creation failed: {msg}"),
        }
    }
}

impl std::error::Error for TranslateError {}

/// Parameters for CreateCanvas (synthetic solid-color source).
#[derive(Clone, Debug)]
pub struct CreateCanvasParams {
    pub w: u32,
    pub h: u32,
    pub color: s::Color,
}

/// Result of translating a v2 framewise pipeline.
pub struct TranslatedPipeline {
    /// Zennode instances for pixel-processing operations (in user-declared order).
    pub nodes: Vec<Box<dyn NodeInstance>>,
    /// Encoder configuration derived from the Encode node.
    pub preset: Option<PresetMapping>,
    /// Decode io_id (which input buffer to decode).
    pub decode_io_id: Option<i32>,
    /// Encode io_id (which output buffer to write to).
    pub encode_io_id: Option<i32>,
    /// Decoder commands from the Decode node.
    pub decoder_commands: Option<Vec<s::DecoderCommand>>,
    /// If present, create a solid-color canvas instead of decoding.
    pub create_canvas: Option<CreateCanvasParams>,
}

/// Translate a sequence of v2 [`Node`] values into zennode instances.
///
/// `io_buffers` is needed for Watermark nodes which must decode a secondary image.
pub fn translate_nodes(
    nodes: &[Node],
    io_buffers: &HashMap<i32, Vec<u8>>,
) -> Result<TranslatedPipeline, TranslateError> {
    let mut result = TranslatedPipeline {
        nodes: Vec::new(),
        preset: None,
        decode_io_id: None,
        encode_io_id: None,
        decoder_commands: None,
        create_canvas: None,
    };

    for node in nodes {
        translate_one(node, &mut result, io_buffers)?;
    }

    Ok(result)
}

fn translate_one(
    node: &Node,
    result: &mut TranslatedPipeline,
    io_buffers: &HashMap<i32, Vec<u8>>,
) -> Result<(), TranslateError> {
    match node {
        // ─── I/O: handled as config, not pixel nodes ───
        Node::Decode { io_id, commands } => {
            result.decode_io_id = Some(*io_id);
            result.decoder_commands = commands.clone();
            Ok(())
        }

        Node::Encode { io_id, preset } => {
            result.encode_io_id = Some(*io_id);
            result.preset = Some(super::preset_map::map_preset(preset)?);
            Ok(())
        }

        // ─── Geometry: zenlayout nodes ───
        Node::FlipV => push_layout_node(&mut result.nodes, "zenlayout.flip_v", &[]),

        Node::FlipH => push_layout_node(&mut result.nodes, "zenlayout.flip_h", &[]),

        Node::Rotate90 => push_layout_node(&mut result.nodes, "zenlayout.rotate_90", &[]),

        Node::Rotate180 => push_layout_node(&mut result.nodes, "zenlayout.rotate_180", &[]),

        Node::Rotate270 => push_layout_node(&mut result.nodes, "zenlayout.rotate_270", &[]),

        Node::Transpose => {
            // Transpose = rotate90 + flip_h (equivalent)
            push_layout_node(&mut result.nodes, "zenlayout.rotate_90", &[])?;
            push_layout_node(&mut result.nodes, "zenlayout.flip_h", &[])
        }

        Node::ApplyOrientation { flag } => push_layout_node(
            &mut result.nodes,
            "zenlayout.orient",
            &[("orientation", ParamValue::I32(*flag))],
        ),

        Node::Crop { x1, y1, x2, y2 } => {
            // v2 Crop uses x1,y1,x2,y2 (corners). zenlayout uses x,y,w,h.
            let w = x2.saturating_sub(*x1);
            let h = y2.saturating_sub(*y1);
            push_layout_node(
                &mut result.nodes,
                "zenlayout.crop",
                &[
                    ("x", ParamValue::U32(*x1)),
                    ("y", ParamValue::U32(*y1)),
                    ("w", ParamValue::U32(w)),
                    ("h", ParamValue::U32(h)),
                ],
            )
        }

        Node::Constrain(c) => translate_constrain(c, &mut result.nodes),

        Node::Resample2D { w, h, hints } => {
            let filter = hints
                .as_ref()
                .and_then(|h| h.down_filter.as_ref())
                .map(|f| filter_to_str(f).to_string())
                .unwrap_or_default();
            let sharpen = hints.as_ref().and_then(|h| h.sharpen_percent).unwrap_or(0.0);
            push_layout_node(
                &mut result.nodes,
                "zenresize.resize",
                &[
                    ("w", ParamValue::U32(*w)),
                    ("h", ParamValue::U32(*h)),
                    ("filter", ParamValue::Str(filter)),
                    ("sharpen", ParamValue::F32(sharpen)),
                ],
            )?;
            // If an opaque background_color (matte) is specified, add RemoveAlpha after resize.
            if let Some(hints) = hints {
                if let Some(ref bg) = hints.background_color {
                    if !matches!(bg, Color::Transparent) {
                        let rgba = color_to_rgba(bg);
                        if rgba[3] > 0 {
                            push_layout_node(
                                &mut result.nodes,
                                "zenpipe.remove_alpha",
                                &[
                                    ("matte_r", ParamValue::U32(rgba[0] as u32)),
                                    ("matte_g", ParamValue::U32(rgba[1] as u32)),
                                    ("matte_b", ParamValue::U32(rgba[2] as u32)),
                                ],
                            )?;
                        }
                    }
                }
            }
            Ok(())
        }

        Node::Region { x1, y1, x2, y2, background_color } => push_layout_node(
            &mut result.nodes,
            "zenlayout.region",
            &[
                ("x1", ParamValue::I32(*x1)),
                ("y1", ParamValue::I32(*y1)),
                ("x2", ParamValue::I32(*x2)),
                ("y2", ParamValue::I32(*y2)),
            ],
        ),

        Node::RegionPercent { x1, y1, x2, y2, background_color } => push_layout_node(
            &mut result.nodes,
            "zenlayout.crop_percent",
            &[
                ("x1", ParamValue::F32(*x1)),
                ("y1", ParamValue::F32(*y1)),
                ("x2", ParamValue::F32(*x2)),
                ("y2", ParamValue::F32(*y2)),
            ],
        ),

        Node::ExpandCanvas { left, top, right, bottom, color } => {
            let color_str = color_to_css_string(color);
            push_layout_node(
                &mut result.nodes,
                "zenlayout.expand_canvas",
                &[
                    ("left", ParamValue::U32(*left)),
                    ("top", ParamValue::U32(*top)),
                    ("right", ParamValue::U32(*right)),
                    ("bottom", ParamValue::U32(*bottom)),
                    ("color", ParamValue::Str(color_str)),
                ],
            )
        }

        // ─── Filters: zenfilters nodes ───
        Node::ColorFilterSrgb(filter) => translate_color_filter(filter, &mut result.nodes),

        Node::ColorMatrixSrgb { matrix } => {
            // Transpose v2 [[f32; 5]; 5] (m[input][output]) to flat [f32; 25]
            // (row-per-output-channel) for the ColorMatrixSrgbConverter.
            let flat = v2_matrix_to_flat(matrix);
            result.nodes.push(Box::new(super::nodes::ColorMatrixSrgbNode { matrix: flat }));
            Ok(())
        }

        Node::WhiteBalanceHistogramAreaThresholdSrgb { threshold } => {
            // Default threshold is 0.006 (0.6%) matching v2 behavior.
            let t = threshold.unwrap_or(0.006);
            push_layout_node(
                &mut result.nodes,
                "imageflow.white_balance_srgb",
                &[("threshold", ParamValue::F32(t))],
            )
        }

        // ─── Canvas operations ───
        Node::CreateCanvas { format, w, h, color } => {
            // CreateCanvas is a decode-replacement: produces a solid-color image.
            // In the zen pipeline, we handle it as a special source in execute.rs.
            // Store it as a decode with a synthetic marker.
            result.decode_io_id = Some(-1); // sentinel: create_canvas, not a real io_id
            result.create_canvas =
                Some(CreateCanvasParams { w: *w as u32, h: *h as u32, color: color.clone() });
            Ok(())
        }

        Node::FillRect { x1, y1, x2, y2, color } => {
            let rgba = color_to_rgba(color);
            push_layout_node(
                &mut result.nodes,
                "zenpipe.fill_rect",
                &[
                    ("x1", ParamValue::U32(*x1)),
                    ("y1", ParamValue::U32(*y1)),
                    ("x2", ParamValue::U32(*x2)),
                    ("y2", ParamValue::U32(*y2)),
                    ("color_r", ParamValue::U32(rgba[0] as u32)),
                    ("color_g", ParamValue::U32(rgba[1] as u32)),
                    ("color_b", ParamValue::U32(rgba[2] as u32)),
                    ("color_a", ParamValue::U32(rgba[3] as u32)),
                ],
            )
        }

        // ─── Composition ───
        Node::DrawImageExact { x, y, w, h, blend, hints } => {
            Err(TranslateError::Unsupported("draw_image_exact".into()))
        }

        Node::Watermark(wm) => {
            // Decode the watermark image and create an overlay node.
            let overlay_node = super::watermark::decode_watermark(wm, io_buffers)
                .map_err(|e| TranslateError::NodeCreation(format!("watermark decode: {e}")))?;
            result.nodes.push(Box::new(overlay_node));
            Ok(())
        }

        Node::CopyRectToCanvas { from_x, from_y, w, h, x, y } => {
            Err(TranslateError::Unsupported("copy_rect_to_canvas".into()))
        }

        // ─── Misc ───
        Node::CropWhitespace { threshold, percent_padding } => push_layout_node(
            &mut result.nodes,
            "zenpipe.crop_whitespace",
            &[
                ("threshold", ParamValue::U32(*threshold as u32)),
                ("percent_padding", ParamValue::F32(*percent_padding)),
            ],
        ),

        Node::RoundImageCorners { radius, background_color } => {
            let bg = color_to_rgba(background_color);
            let mut params: Vec<(&str, ParamValue)> = Vec::new();
            match radius {
                RoundCornersMode::Percentage(p) => {
                    params.push(("radius", ParamValue::F32(*p)));
                    params.push(("mode", ParamValue::Str("percentage".to_string())));
                }
                RoundCornersMode::Pixels(px) => {
                    params.push(("radius", ParamValue::F32(*px)));
                    params.push(("mode", ParamValue::Str("pixels".to_string())));
                }
                RoundCornersMode::Circle => {
                    params.push(("radius", ParamValue::F32(50.0)));
                    params.push(("mode", ParamValue::Str("circle".to_string())));
                }
                RoundCornersMode::PercentageCustom {
                    top_left,
                    top_right,
                    bottom_right,
                    bottom_left,
                } => {
                    params.push(("mode", ParamValue::Str("percentage_custom".to_string())));
                    params.push(("radius_tl", ParamValue::F32(*top_left)));
                    params.push(("radius_tr", ParamValue::F32(*top_right)));
                    params.push(("radius_bl", ParamValue::F32(*bottom_left)));
                    params.push(("radius_br", ParamValue::F32(*bottom_right)));
                }
                RoundCornersMode::PixelsCustom {
                    top_left,
                    top_right,
                    bottom_right,
                    bottom_left,
                } => {
                    params.push(("mode", ParamValue::Str("pixels_custom".to_string())));
                    params.push(("radius_tl", ParamValue::F32(*top_left)));
                    params.push(("radius_tr", ParamValue::F32(*top_right)));
                    params.push(("radius_bl", ParamValue::F32(*bottom_left)));
                    params.push(("radius_br", ParamValue::F32(*bottom_right)));
                }
            }
            params.push(("bg_r", ParamValue::U32(bg[0] as u32)));
            params.push(("bg_g", ParamValue::U32(bg[1] as u32)));
            params.push(("bg_b", ParamValue::U32(bg[2] as u32)));
            params.push(("bg_a", ParamValue::U32(bg[3] as u32)));
            let param_refs: Vec<(&str, ParamValue)> = params.into_iter().collect();
            push_layout_node(&mut result.nodes, "zenpipe.round_corners", &param_refs)
        }

        Node::CommandString { kind, value, decode, encode, watermarks } => {
            // RIAPI querystring — delegate to imageflow_riapi at a higher level.
            // The caller should expand CommandString before calling translate_nodes.
            Err(TranslateError::Unsupported(
                "command_string must be expanded before translation".into(),
            ))
        }

        Node::WatermarkRedDot => {
            result.nodes.push(Box::new(super::watermark::RedDotNode));
            Ok(())
        }

        // Internal test node — no-op in zen pipeline.
        Node::CaptureBitmapKey { .. } => Ok(()),

        #[allow(unreachable_patterns)]
        _ => Err(TranslateError::Unsupported(format!("{node:?}"))),
    }
}

// ─── Constrain translation ───

fn translate_constrain(
    c: &Constraint,
    nodes: &mut Vec<Box<dyn NodeInstance>>,
) -> Result<(), TranslateError> {
    let mode_str = constraint_mode_to_str(&c.mode);
    let mut params: Vec<(&str, ParamValue)> = vec![("mode", ParamValue::Str(mode_str.into()))];
    if let Some(w) = c.w {
        params.push(("w", ParamValue::U32(w)));
    }
    if let Some(h) = c.h {
        params.push(("h", ParamValue::U32(h)));
    }
    if let Some(ref hints) = c.hints {
        if let Some(ref filter) = hints.down_filter {
            params.push(("down_filter", ParamValue::Str(filter_to_str(filter).into())));
        }
    }
    push_constrain_node(nodes, &params)?;
    // If an opaque background_color (matte) is specified, add RemoveAlpha after constrain.
    if let Some(ref hints) = c.hints {
        if let Some(ref bg) = hints.background_color {
            if !matches!(bg, Color::Transparent) {
                let rgba = color_to_rgba(bg);
                if rgba[3] > 0 {
                    push_layout_node(
                        nodes,
                        "zenpipe.remove_alpha",
                        &[
                            ("matte_r", ParamValue::U32(rgba[0] as u32)),
                            ("matte_g", ParamValue::U32(rgba[1] as u32)),
                            ("matte_b", ParamValue::U32(rgba[2] as u32)),
                        ],
                    )?;
                }
            }
        }
    }
    // Also check canvas_color as matte for pad modes.
    if let Some(ref canvas) = c.canvas_color {
        let rgba = color_to_rgba(canvas);
        if rgba[3] >= 255 {
            push_layout_node(
                nodes,
                "zenpipe.remove_alpha",
                &[
                    ("matte_r", ParamValue::U32(rgba[0] as u32)),
                    ("matte_g", ParamValue::U32(rgba[1] as u32)),
                    ("matte_b", ParamValue::U32(rgba[2] as u32)),
                ],
            )?;
        }
    }
    Ok(())
}

// ─── Color filter translation ───

/// Translate a v2 ColorFilterSrgb to a sRGB-space color matrix node.
///
/// All ColorFilterSrgb variants are implemented as 5x5 color matrices applied
/// in sRGB gamma space on u8 values. This matches v2 behavior exactly — v2
/// expanded every ColorFilterSrgb into a ColorMatrixSrgb node with the same
/// matrix functions.
fn translate_color_filter(
    filter: &ColorFilterSrgb,
    nodes: &mut Vec<Box<dyn NodeInstance>>,
) -> Result<(), TranslateError> {
    // All ColorFilterSrgb variants (including Alpha) use the v2 color matrix
    // approach in sRGB gamma space. This avoids zenfilters' Oklab conversion
    // and matches v2 behavior exactly.
    let v2_matrix = color_filter_to_v2_matrix(filter);
    let flat = v2_matrix_to_flat(&v2_matrix);
    nodes.push(Box::new(super::nodes::ColorMatrixSrgbNode { matrix: flat }));
    Ok(())
}

/// Convert a v2-format 5x5 color matrix to the flat [f32; 25] format used by
/// `ColorMatrixSrgbConverter`.
///
/// V2 matrices use `m[input_channel][output_channel]` layout:
///   out_R = m[0][0]*r + m[1][0]*g + m[2][0]*b + m[3][0]*a + m[4][0]*255
///
/// The converter uses row-per-output-channel layout:
///   out_R = flat[0]*r + flat[1]*g + flat[2]*b + flat[3]*a + flat[4]*255
///
/// So we transpose: flat[out*5 + in] = m[in][out].
fn v2_matrix_to_flat(m: &[[f32; 5]; 5]) -> [f32; 25] {
    let mut flat = [0.0f32; 25];
    for out_ch in 0..4 {
        for in_ch in 0..4 {
            flat[out_ch * 5 + in_ch] = m[in_ch][out_ch];
        }
        flat[out_ch * 5 + 4] = m[4][out_ch]; // bias term
    }
    // flat[20..25] is unused by the converter
    flat
}

/// Build the v2-format 5x5 color matrix for a given ColorFilterSrgb variant.
///
/// These matrices are identical to the ones in `imageflow_core::flow::nodes::color`.
/// The matrix format is `m[input_channel][output_channel]` with row 4 as bias.
fn color_filter_to_v2_matrix(filter: &ColorFilterSrgb) -> [[f32; 5]; 5] {
    match filter {
        ColorFilterSrgb::Sepia => srgb_matrix::sepia(),
        ColorFilterSrgb::GrayscaleNtsc => srgb_matrix::grayscale_ntsc(),
        ColorFilterSrgb::GrayscaleRy => srgb_matrix::grayscale_ry(),
        ColorFilterSrgb::GrayscaleFlat => srgb_matrix::grayscale_flat(),
        ColorFilterSrgb::GrayscaleBt709 => srgb_matrix::grayscale_bt709(),
        ColorFilterSrgb::Invert => srgb_matrix::invert(),
        ColorFilterSrgb::Alpha(a) => srgb_matrix::alpha(*a),
        ColorFilterSrgb::Contrast(c) => srgb_matrix::contrast(*c),
        ColorFilterSrgb::Saturation(s) => srgb_matrix::saturation(*s),
        ColorFilterSrgb::Brightness(b) => srgb_matrix::brightness(*b),
    }
}

/// V2-compatible sRGB color matrices.
///
/// Each function returns a 5x5 matrix in v2 layout: `m[input][output]`, row 4 = bias.
/// These are exact copies of the matrices in `imageflow_core::flow::nodes::color`.
mod srgb_matrix {
    pub fn sepia() -> [[f32; 5]; 5] {
        [
            [0.393f32, 0.349f32, 0.272f32, 0f32, 0f32],
            [0.769f32, 0.686f32, 0.534f32, 0f32, 0f32],
            [0.189f32, 0.168f32, 0.131f32, 0f32, 0f32],
            [0f32, 0f32, 0f32, 1f32, 0f32],
            [0f32, 0f32, 0f32, 0f32, 0f32],
        ]
    }

    fn grayscale(r: f32, g: f32, b: f32) -> [[f32; 5]; 5] {
        [
            [r, r, r, 0f32, 0f32],
            [g, g, g, 0f32, 0f32],
            [b, b, b, 0f32, 0f32],
            [0f32, 0f32, 0f32, 1f32, 0f32],
            [0f32, 0f32, 0f32, 0f32, 1f32],
        ]
    }

    pub fn grayscale_flat() -> [[f32; 5]; 5] {
        grayscale(0.5f32, 0.5f32, 0.5f32)
    }

    pub fn grayscale_bt709() -> [[f32; 5]; 5] {
        grayscale(0.2125f32, 0.7154f32, 0.0721f32)
    }

    pub fn grayscale_ry() -> [[f32; 5]; 5] {
        grayscale(0.5f32, 0.419f32, 0.081f32)
    }

    pub fn grayscale_ntsc() -> [[f32; 5]; 5] {
        // NTSC uses the "Y" luma coefficients (same as grayscale_y in v2).
        grayscale(0.229f32, 0.587f32, 0.114f32)
    }

    pub fn invert() -> [[f32; 5]; 5] {
        [
            [-1f32, 0f32, 0f32, 0f32, 0f32],
            [0f32, -1f32, 0f32, 0f32, 0f32],
            [0f32, 0f32, -1f32, 0f32, 0f32],
            [0f32, 0f32, 0f32, 1f32, 0f32],
            [1f32, 1f32, 1f32, 0f32, 1f32],
        ]
    }

    pub fn alpha(alpha: f32) -> [[f32; 5]; 5] {
        [
            [1f32, 0f32, 0f32, 0f32, 0f32],
            [0f32, 1f32, 0f32, 0f32, 0f32],
            [0f32, 0f32, 1f32, 0f32, 0f32],
            [0f32, 0f32, 0f32, alpha, 0f32],
            [0f32, 0f32, 0f32, 0f32, 1f32],
        ]
    }

    pub fn contrast(c: f32) -> [[f32; 5]; 5] {
        let c = c + 1f32; // Stop at -1
        let factor_t = 0.5f32 * (1.0f32 - c);
        [
            [c, 0f32, 0f32, 0f32, 0f32],
            [0f32, c, 0f32, 0f32, 0f32],
            [0f32, 0f32, c, 0f32, 0f32],
            [0f32, 0f32, 0f32, 1f32, 0f32],
            [factor_t, factor_t, factor_t, 0f32, 1f32],
        ]
    }

    pub fn brightness(factor: f32) -> [[f32; 5]; 5] {
        [
            [1f32, 0f32, 0f32, 0f32, 0f32],
            [0f32, 1f32, 0f32, 0f32, 0f32],
            [0f32, 0f32, 1f32, 0f32, 0f32],
            [0f32, 0f32, 0f32, 1f32, 0f32],
            [factor, factor, factor, 0f32, 1f32],
        ]
    }

    pub fn saturation(saturation: f32) -> [[f32; 5]; 5] {
        let saturation = (saturation + 1f32).max(0f32); // Stop at -1
        let complement = 1.0f32 - saturation;
        let complement_r = 0.3086f32 * complement;
        let complement_g = 0.6094f32 * complement;
        let complement_b = 0.0820f32 * complement;
        [
            [complement_r + saturation, complement_r, complement_r, 0.0f32, 0.0f32],
            [complement_g, complement_g + saturation, complement_g, 0.0f32, 0.0f32],
            [complement_b, complement_b, complement_b + saturation, 0.0f32, 0.0f32],
            [0.0f32, 0.0f32, 0.0f32, 1.0f32, 0.0f32],
            [0.0f32, 0.0f32, 0.0f32, 0.0f32, 1.0f32],
        ]
    }
}

// ─── Node construction helpers ───

/// Shared node registry containing all zenlayout, zenresize, zenfilters, and zenpipe nodes.
///
/// Initialized once on first use; avoids re-registering all node definitions on every
/// `push_layout_node` call.
fn zen_registry() -> &'static NodeRegistry {
    use std::sync::OnceLock;
    static REGISTRY: OnceLock<NodeRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut registry = NodeRegistry::new();
        zenlayout::zennode_defs::register(&mut registry);
        zenresize::zennode_defs::register(&mut registry);
        zenfilters::zennode_defs::register(&mut registry);
        zenpipe::zennode_defs::register(&mut registry);
        super::nodes::register(&mut registry);
        registry
    })
}

/// Create a zenlayout node by schema ID and set params.
fn push_layout_node(
    nodes: &mut Vec<Box<dyn NodeInstance>>,
    schema_id: &str,
    params: &[(&str, ParamValue)],
) -> Result<(), TranslateError> {
    let registry = zen_registry();
    let def = registry.get(schema_id).ok_or_else(|| {
        TranslateError::NodeCreation(format!("zenlayout node '{schema_id}' not found in registry"))
    })?;
    let mut node = def
        .create_default()
        .map_err(|e| TranslateError::NodeCreation(format!("{schema_id}: {e}")))?;
    for (name, value) in params {
        node.set_param(name, value.clone());
    }
    nodes.push(node);
    Ok(())
}

/// Create a zenresize constrain node and set params.
fn push_constrain_node(
    nodes: &mut Vec<Box<dyn NodeInstance>>,
    params: &[(&str, ParamValue)],
) -> Result<(), TranslateError> {
    let def: &dyn NodeDef = &zenresize::zennode_defs::CONSTRAIN_NODE;
    let mut node = def
        .create_default()
        .map_err(|e| TranslateError::NodeCreation(format!("zenresize.constrain: {e}")))?;
    for (name, value) in params {
        if !node.set_param(name, value.clone()) {
            eprintln!("warning: set_param({name}, {value:?}) failed on zenresize.constrain");
        }
    }
    nodes.push(node);
    Ok(())
}

// ─── String conversion helpers ───

fn constraint_mode_to_str(mode: &ConstraintMode) -> &'static str {
    match mode {
        ConstraintMode::Distort => "distort",
        ConstraintMode::Within => "within",
        ConstraintMode::Fit => "fit",
        ConstraintMode::FitCrop => "fit_crop",
        ConstraintMode::WithinCrop => "within_crop",
        ConstraintMode::FitPad => "fit_pad",
        ConstraintMode::WithinPad => "within_pad",
        ConstraintMode::AspectCrop => "aspect_crop",
        ConstraintMode::LargerThan => "larger_than",
    }
}

fn filter_to_str(filter: &s::Filter) -> &'static str {
    match filter {
        s::Filter::RobidouxFast => "robidoux_fast",
        s::Filter::Robidoux => "robidoux",
        s::Filter::RobidouxSharp => "robidoux_sharp",
        s::Filter::Ginseng => "ginseng",
        s::Filter::GinsengSharp => "ginseng_sharp",
        s::Filter::Lanczos => "lanczos",
        s::Filter::LanczosSharp => "lanczos_sharp",
        s::Filter::Lanczos2 => "lanczos2",
        s::Filter::Lanczos2Sharp => "lanczos2_sharp",
        s::Filter::Cubic => "cubic",
        s::Filter::CubicSharp => "cubic_sharp",
        s::Filter::CatmullRom => "catmull_rom",
        s::Filter::Mitchell => "mitchell",
        s::Filter::CubicBSpline => "cubic",
        s::Filter::Hermite => "hermite",
        s::Filter::Jinc => "lanczos", // closest equivalent
        s::Filter::Triangle => "triangle",
        s::Filter::Linear => "linear",
        s::Filter::Box => "box",
        s::Filter::Fastest => "box",
        s::Filter::NCubic => "cubic",
        s::Filter::NCubicSharp => "cubic_sharp",
        #[allow(unreachable_patterns)]
        _ => "robidoux", // safe default
    }
}

// Color parsing delegated to super::color module.
use super::color::{color_to_css_string, color_to_rgba};

// Custom NodeInstance wrappers removed — all operations now use native
// zennode definitions in zenpipe::zennode_defs and zenresize::zennode_defs.
// See zenpipe.crop_whitespace, zenpipe.fill_rect, zenpipe.remove_alpha,
// zenpipe.round_corners, zenresize.resize.
