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

/// Result of translating a v2 framewise pipeline.
pub struct TranslatedPipeline {
    /// Zenode instances for pixel-processing operations (in user-declared order).
    pub nodes: Vec<Box<dyn NodeInstance>>,
    /// Encoder configuration derived from the Encode node.
    pub preset: Option<PresetMapping>,
    /// Decode io_id (which input buffer to decode).
    pub decode_io_id: Option<i32>,
    /// Encode io_id (which output buffer to write to).
    pub encode_io_id: Option<i32>,
    /// Decoder commands from the Decode node.
    pub decoder_commands: Option<Vec<s::DecoderCommand>>,
}

/// Translate a sequence of v2 [`Node`] values into zennode instances.
pub fn translate_nodes(nodes: &[Node]) -> Result<TranslatedPipeline, TranslateError> {
    let mut result = TranslatedPipeline {
        nodes: Vec::new(),
        preset: None,
        decode_io_id: None,
        encode_io_id: None,
        decoder_commands: None,
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

        Node::FlipV => {
            push_layout_node(&mut result.nodes, "zenlayout.flip_v", &[])
        }

        Node::FlipH => {
            push_layout_node(&mut result.nodes, "zenlayout.flip_h", &[])
        }

        Node::Rotate90 => {
            push_layout_node(&mut result.nodes, "zenlayout.rotate_90", &[])
        }

        Node::Rotate180 => {
            push_layout_node(&mut result.nodes, "zenlayout.rotate_180", &[])
        }

        Node::Rotate270 => {
            push_layout_node(&mut result.nodes, "zenlayout.rotate_270", &[])
        }

        Node::Transpose => {
            // Transpose = rotate90 + flip_h (equivalent)
            push_layout_node(&mut result.nodes, "zenlayout.rotate_90", &[])?;
            push_layout_node(&mut result.nodes, "zenlayout.flip_h", &[])
        }

        Node::ApplyOrientation { flag } => {
            push_layout_node(&mut result.nodes, "zenlayout.orient", &[
                ("orientation", ParamValue::I32(*flag)),
            ])
        }

        Node::Crop { x1, y1, x2, y2 } => {
            // v2 Crop uses x1,y1,x2,y2 (corners). zenlayout uses x,y,w,h.
            let w = x2.saturating_sub(*x1);
            let h = y2.saturating_sub(*y1);
            push_layout_node(&mut result.nodes, "zenlayout.crop", &[
                ("x", ParamValue::U32(*x1)),
                ("y", ParamValue::U32(*y1)),
                ("w", ParamValue::U32(w)),
                ("h", ParamValue::U32(h)),
            ])
        }

        Node::Constrain(c) => translate_constrain(c, &mut result.nodes),

        Node::Resample2D { w, h, hints } => {
            // Resample2D is a forced resize to exact dimensions.
            let mut params: Vec<(&str, ParamValue)> = vec![
                ("w", ParamValue::U32(*w)),
                ("h", ParamValue::U32(*h)),
                ("mode", ParamValue::Str("distort".into())),
            ];
            if let Some(hints) = hints {
                if let Some(ref filter) = hints.down_filter {
                    params.push(("down_filter", ParamValue::Str(filter_to_str(filter).into())));
                }
            }
            push_constrain_node(&mut result.nodes, &params)
        }

        Node::Region { x1, y1, x2, y2, background_color } => {
            push_layout_node(&mut result.nodes, "zenlayout.region", &[
                ("x1", ParamValue::I32(*x1)),
                ("y1", ParamValue::I32(*y1)),
                ("x2", ParamValue::I32(*x2)),
                ("y2", ParamValue::I32(*y2)),
            ])
        }

        Node::RegionPercent { x1, y1, x2, y2, background_color } => {
            push_layout_node(&mut result.nodes, "zenlayout.crop_percent", &[
                ("x1", ParamValue::F32(*x1)),
                ("y1", ParamValue::F32(*y1)),
                ("x2", ParamValue::F32(*x2)),
                ("y2", ParamValue::F32(*y2)),
            ])
        }

        Node::ExpandCanvas { left, top, right, bottom, color } => {
            push_layout_node(&mut result.nodes, "zenlayout.expand_canvas", &[
                ("left", ParamValue::U32(*left)),
                ("top", ParamValue::U32(*top)),
                ("right", ParamValue::U32(*right)),
                ("bottom", ParamValue::U32(*bottom)),
            ])
        }

        // ─── Filters: zenfilters nodes ───

        Node::ColorFilterSrgb(filter) => translate_color_filter(filter, &mut result.nodes),

        Node::ColorMatrixSrgb { matrix } => {
            // Flatten 5x5 matrix to flat f32 array.
            // zenfilters color_matrix node takes a flat [f32; 25] or similar.
            // For now, mark as unsupported until zenfilters has a color matrix node.
            Err(TranslateError::Unsupported("color_matrix_srgb".into()))
        }

        Node::WhiteBalanceHistogramAreaThresholdSrgb { threshold } => {
            // Requires full-frame materialization for histogram analysis.
            Err(TranslateError::Unsupported(
                "white_balance_histogram_area_threshold_srgb".into(),
            ))
        }

        // ─── Canvas operations ───

        Node::CreateCanvas { format, w, h, color } => {
            Err(TranslateError::Unsupported("create_canvas".into()))
        }

        Node::FillRect { x1, y1, x2, y2, color } => {
            Err(TranslateError::Unsupported("fill_rect".into()))
        }

        // ─── Composition ───

        Node::DrawImageExact { x, y, w, h, blend, hints } => {
            Err(TranslateError::Unsupported("draw_image_exact".into()))
        }

        Node::Watermark(wm) => {
            Err(TranslateError::Unsupported("watermark".into()))
        }

        Node::CopyRectToCanvas { from_x, from_y, w, h, x, y } => {
            Err(TranslateError::Unsupported("copy_rect_to_canvas".into()))
        }

        // ─── Misc ───

        Node::CropWhitespace { threshold, percent_padding } => {
            Err(TranslateError::Unsupported("crop_whitespace".into()))
        }

        Node::RoundImageCorners { radius, background_color } => {
            Err(TranslateError::Unsupported("round_image_corners".into()))
        }

        Node::CommandString { kind, value, decode, encode, watermarks } => {
            // RIAPI querystring — delegate to imageflow_riapi at a higher level.
            // The caller should expand CommandString before calling translate_nodes.
            Err(TranslateError::Unsupported(
                "command_string must be expanded before translation".into(),
            ))
        }

        Node::WatermarkRedDot => {
            // Debug feature, not worth porting.
            Err(TranslateError::Unsupported("watermark_red_dot".into()))
        }

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
    let mut params: Vec<(&str, ParamValue)> = vec![
        ("mode", ParamValue::Str(mode_str.into())),
    ];
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
    push_constrain_node(nodes, &params)
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
            push_filter_node(nodes, "zenfilters.saturation", &[
                ("amount", ParamValue::F32(-1.0)),
            ])
        }
        ColorFilterSrgb::Sepia => {
            // Desaturate then warm tint. Simplified version.
            push_filter_node(nodes, "zenfilters.saturation", &[
                ("amount", ParamValue::F32(-1.0)),
            ])?;
            push_filter_node(nodes, "zenfilters.temperature", &[
                ("amount", ParamValue::F32(0.3)),
            ])
        }
        ColorFilterSrgb::Invert => {
            push_filter_node(nodes, "zenfilters.invert", &[])
        }
        ColorFilterSrgb::Alpha(a) => {
            // v2 Alpha filter sets global opacity.
            // This is handled differently in zenpipe (ScaleAlphaOp).
            Err(TranslateError::Unsupported("color_filter_srgb::alpha".into()))
        }
        ColorFilterSrgb::Contrast(c) => {
            // v2 contrast is centered at 1.0 (no change), range ~0..2.
            // zenfilters contrast is centered at 0.0, range -1..1.
            let normalized = c - 1.0;
            push_filter_node(nodes, "zenfilters.contrast", &[
                ("amount", ParamValue::F32(normalized)),
            ])
        }
        ColorFilterSrgb::Brightness(b) => {
            // v2 brightness is centered at 1.0, range ~0..2.
            // Map to zenfilters exposure in stops.
            let stops = (*b - 1.0) * 2.0; // rough mapping
            push_filter_node(nodes, "zenfilters.exposure", &[
                ("stops", ParamValue::F32(stops)),
            ])
        }
        ColorFilterSrgb::Saturation(sat) => {
            // v2 saturation is centered at 1.0, range ~0..2.
            // zenfilters saturation is centered at 0.0, range -1..1.
            let normalized = sat - 1.0;
            push_filter_node(nodes, "zenfilters.saturation", &[
                ("amount", ParamValue::F32(normalized)),
            ])
        }
        #[allow(unreachable_patterns)]
        _ => Err(TranslateError::Unsupported(format!(
            "color_filter_srgb::{filter:?}"
        ))),
    }
}

// ─── Node construction helpers ───

/// Build a shared node registry containing all zenlayout, zenresize, and zenfilters nodes.
fn zen_registry() -> NodeRegistry {
    let mut registry = NodeRegistry::new();
    zenlayout::zennode_defs::register(&mut registry);
    zenresize::zennode_defs::register(&mut registry);
    zenfilters::zennode_defs::register(&mut registry);
    registry
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
    let mut node = def.create_default().map_err(|e| {
        TranslateError::NodeCreation(format!("{schema_id}: {e}"))
    })?;
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
    let mut node = def.create_default().map_err(|e| {
        TranslateError::NodeCreation(format!("zenresize.constrain: {e}"))
    })?;
    for (name, value) in params {
        node.set_param(name, value.clone());
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
        TranslateError::NodeCreation(format!(
            "zenfilters node '{schema_id}' not found in registry"
        ))
    })?;
    let mut node = def.create_default().map_err(|e| {
        TranslateError::NodeCreation(format!("{schema_id}: {e}"))
    })?;
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
