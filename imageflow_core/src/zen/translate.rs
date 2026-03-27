//! Translate imageflow v2 [`Node`] variants into zennode [`NodeInstance`] objects.
//!
//! Each v2 Node variant maps to one or more zennode node instances via
//! `NodeDef::create_default()` + `set_param()`. The translation is mechanical:
//! extract fields from the Node variant, set corresponding params on the
//! zennode instance.
//!
//! Encode/Decode nodes are handled separately — they produce configuration
//! rather than pixel-processing nodes.

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
pub fn translate_nodes(nodes: &[Node]) -> Result<TranslatedPipeline, TranslateError> {
    let mut result = TranslatedPipeline {
        nodes: Vec::new(),
        preset: None,
        decode_io_id: None,
        encode_io_id: None,
        decoder_commands: None,
        create_canvas: None,
    };

    for node in nodes {
        translate_one(node, &mut result)?;
    }

    Ok(result)
}

fn translate_one(node: &Node, result: &mut TranslatedPipeline) -> Result<(), TranslateError> {
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
            // Flatten [[f32; 5]; 5] → [f32; 25] row-major for zenfilters.color_matrix.
            let flat: Vec<f32> = matrix.iter().flat_map(|row| row.iter().copied()).collect();
            push_filter_node(
                &mut result.nodes,
                "zenfilters.color_matrix",
                &[("matrix", ParamValue::F32Array(flat))],
            )
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

        Node::Watermark(_wm) => {
            Err(TranslateError::Unsupported("watermark (NodeOp::Overlay not yet wired)".into()))
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
            let (r, mode) = match radius {
                RoundCornersMode::Percentage(p) => (*p, "percentage"),
                RoundCornersMode::Pixels(px) => (*px, "pixels"),
                RoundCornersMode::Circle => (50.0, "circle"),
                _ => (10.0, "percentage"),
            };
            let bg = color_to_rgba(background_color);
            push_layout_node(
                &mut result.nodes,
                "zenpipe.round_corners",
                &[
                    ("radius", ParamValue::F32(r)),
                    ("mode", ParamValue::Str(mode.to_string())),
                    ("bg_r", ParamValue::U32(bg[0] as u32)),
                    ("bg_g", ParamValue::U32(bg[1] as u32)),
                    ("bg_b", ParamValue::U32(bg[2] as u32)),
                    ("bg_a", ParamValue::U32(bg[3] as u32)),
                ],
            )
        }

        Node::CommandString { kind, value, decode, encode, watermarks } => {
            // RIAPI querystring — delegate to imageflow_riapi at a higher level.
            // The caller should expand CommandString before calling translate_nodes.
            Err(TranslateError::Unsupported(
                "command_string must be expanded before translation".into(),
            ))
        }

        Node::WatermarkRedDot => Err(TranslateError::Unsupported(
            "watermark_red_dot (debug overlay not yet wired)".into(),
        )),

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

fn translate_color_filter(
    filter: &ColorFilterSrgb,
    nodes: &mut Vec<Box<dyn NodeInstance>>,
) -> Result<(), TranslateError> {
    match filter {
        ColorFilterSrgb::GrayscaleBt709
        | ColorFilterSrgb::GrayscaleNtsc
        | ColorFilterSrgb::GrayscaleFlat
        | ColorFilterSrgb::GrayscaleRy => {
            // Set saturation to -1.0 (full desaturation).
            // Different grayscale modes use different luma weights,
            // but zenfilters saturation=-1 uses the standard model.
            push_filter_node(nodes, "zenfilters.saturation", &[("amount", ParamValue::F32(-1.0))])
        }
        ColorFilterSrgb::Sepia => {
            // Desaturate then warm tint. Simplified version.
            push_filter_node(nodes, "zenfilters.saturation", &[("amount", ParamValue::F32(-1.0))])?;
            push_filter_node(nodes, "zenfilters.temperature", &[("amount", ParamValue::F32(0.3))])
        }
        ColorFilterSrgb::Invert => push_filter_node(nodes, "zenfilters.invert", &[]),
        ColorFilterSrgb::Alpha(a) => {
            push_filter_node(nodes, "zenfilters.alpha", &[("factor", ParamValue::F32(*a))])
        }
        ColorFilterSrgb::Contrast(c) => {
            // v2 contrast is centered at 1.0 (no change), range ~0..2.
            // zenfilters contrast is centered at 0.0, range -1..1.
            let normalized = c - 1.0;
            push_filter_node(
                nodes,
                "zenfilters.contrast",
                &[("amount", ParamValue::F32(normalized))],
            )
        }
        ColorFilterSrgb::Brightness(b) => {
            // v2 brightness is centered at 1.0, range ~0..2.
            // Map to zenfilters exposure in stops.
            let stops = (*b - 1.0) * 2.0; // rough mapping
            push_filter_node(nodes, "zenfilters.exposure", &[("stops", ParamValue::F32(stops))])
        }
        ColorFilterSrgb::Saturation(sat) => {
            // v2 saturation is centered at 1.0, range ~0..2.
            // zenfilters saturation is centered at 0.0, range -1..1.
            let normalized = sat - 1.0;
            push_filter_node(
                nodes,
                "zenfilters.saturation",
                &[("amount", ParamValue::F32(normalized))],
            )
        }
        #[allow(unreachable_patterns)]
        _ => Err(TranslateError::Unsupported(format!("color_filter_srgb::{filter:?}"))),
    }
}

// ─── Node construction helpers ───

/// Shared node registry containing all zenlayout, zenresize, zenfilters, and zenpipe nodes.
///
/// Initialized once on first use; avoids re-registering all node definitions on every
/// `push_layout_node` / `push_filter_node` call.
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

/// Create a zenfilters node by schema ID and set params.
fn push_filter_node(
    nodes: &mut Vec<Box<dyn NodeInstance>>,
    schema_id: &str,
    params: &[(&str, ParamValue)],
) -> Result<(), TranslateError> {
    let registry = zen_registry();
    let def = registry.get(schema_id).ok_or_else(|| {
        TranslateError::NodeCreation(format!("zenfilters node '{schema_id}' not found in registry"))
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
