//! Watermark support for the zen streaming pipeline.
//!
//! Two watermark features:
//!
//! - **WatermarkRedDot**: 3x3 red pixels at the bottom-right corner (debug marker).
//! - **Watermark overlay**: Decode a secondary image (by io_id), resize it to fit
//!   a bounding box on the canvas, and composite with opacity.
//!
//! Both are implemented as custom `NodeInstance` types that carry their data through
//! the zennode/bridge pipeline, then converted to `NodeOp::Materialize` by
//! `WatermarkConverter`.

use std::any::Any;
use std::collections::HashMap;

use imageflow_types::{
    ConstraintGravity, Watermark, WatermarkConstraintBox, WatermarkConstraintMode,
};
use zennode::{NodeInstance, NodeSchema, ParamMap, ParamValue};
use zenpipe::bridge::NodeConverter;
use zenpipe::graph::NodeOp;
use zenpipe::PipeError;

use super::execute::ZenError;

// ─── NodeInstance: WatermarkRedDot ───

/// Schema for the red dot watermark node.
static RED_DOT_SCHEMA: NodeSchema = NodeSchema {
    id: "imageflow.watermark_red_dot",
    label: "Watermark Red Dot",
    description: "3x3 red debug marker at bottom-right corner",
    group: zennode::NodeGroup::Other,
    role: zennode::NodeRole::Filter,
    params: &[],
    tags: &["watermark", "debug"],
    inputs: &[],
    coalesce: None,
    format: zennode::FormatHint {
        preferred: zennode::PixelFormatPreference::Srgb8,
        alpha: zennode::AlphaHandling::Process,
        changes_dimensions: false,
        is_neighborhood: false,
    },
    version: 1,
    compat_version: 1,
    json_key: "",
    deny_unknown_fields: false,
};

/// A no-data node that tells the converter to draw a red dot.
#[derive(Clone)]
pub struct RedDotNode;

impl NodeInstance for RedDotNode {
    fn schema(&self) -> &'static NodeSchema {
        &RED_DOT_SCHEMA
    }
    fn to_params(&self) -> ParamMap {
        ParamMap::new()
    }
    fn get_param(&self, _name: &str) -> Option<ParamValue> {
        None
    }
    fn set_param(&mut self, _name: &str, _value: ParamValue) -> bool {
        false
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn clone_boxed(&self) -> Box<dyn NodeInstance> {
        Box::new(self.clone())
    }
}

// ─── NodeInstance: WatermarkOverlay ───

/// Schema for the watermark overlay node.
static OVERLAY_SCHEMA: NodeSchema = NodeSchema {
    id: "imageflow.watermark_overlay",
    label: "Watermark Overlay",
    description: "Composite a decoded watermark image onto the canvas",
    group: zennode::NodeGroup::Other,
    role: zennode::NodeRole::Filter,
    params: &[],
    tags: &["watermark", "overlay", "composite"],
    inputs: &[],
    coalesce: None,
    format: zennode::FormatHint {
        preferred: zennode::PixelFormatPreference::Srgb8,
        alpha: zennode::AlphaHandling::Process,
        changes_dimensions: false,
        is_neighborhood: false,
    },
    version: 1,
    compat_version: 1,
    json_key: "",
    deny_unknown_fields: false,
};

/// Pre-decoded watermark image data + placement parameters.
///
/// Created during `execute_steps` pre-expansion, carried through translation
/// as a NodeInstance, then consumed by `WatermarkConverter` to produce
/// `NodeOp::Materialize`.
#[derive(Clone)]
pub struct WatermarkOverlayNode {
    /// Decoded RGBA8 pixel data of the watermark image.
    pub pixels: Vec<u8>,
    /// Watermark image width (decoded).
    pub width: u32,
    /// Watermark image height (decoded).
    pub height: u32,
    /// Bounding box specification (how to compute position on canvas).
    pub fit_box: Option<WatermarkConstraintBox>,
    /// How the watermark fits within the box.
    pub fit_mode: Option<WatermarkConstraintMode>,
    /// Gravity for positioning within the box.
    pub gravity: Option<ConstraintGravity>,
    /// Minimum canvas width to apply watermark.
    pub min_canvas_width: Option<u32>,
    /// Minimum canvas height to apply watermark.
    pub min_canvas_height: Option<u32>,
    /// Opacity (0.0 = invisible, 1.0 = full).
    pub opacity: f32,
}

impl NodeInstance for WatermarkOverlayNode {
    fn schema(&self) -> &'static NodeSchema {
        &OVERLAY_SCHEMA
    }
    fn to_params(&self) -> ParamMap {
        ParamMap::new()
    }
    fn get_param(&self, _name: &str) -> Option<ParamValue> {
        None
    }
    fn set_param(&mut self, _name: &str, _value: ParamValue) -> bool {
        false
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn clone_boxed(&self) -> Box<dyn NodeInstance> {
        Box::new(self.clone())
    }
}

// ─── NodeConverter ───

/// Converter for `imageflow.watermark_red_dot` and `imageflow.watermark_overlay`.
pub struct WatermarkConverter;

impl NodeConverter for WatermarkConverter {
    fn can_convert(&self, schema_id: &str) -> bool {
        schema_id == "imageflow.watermark_red_dot" || schema_id == "imageflow.watermark_overlay"
    }

    fn convert(&self, node: &dyn NodeInstance) -> Result<NodeOp, PipeError> {
        let schema_id = node.schema().id;
        match schema_id {
            "imageflow.watermark_red_dot" => Ok(make_red_dot_materialize()),
            "imageflow.watermark_overlay" => {
                let overlay =
                    node.as_any().downcast_ref::<WatermarkOverlayNode>().ok_or_else(|| {
                        PipeError::Op("watermark overlay: wrong NodeInstance type".into())
                    })?;
                Ok(make_watermark_materialize(overlay.clone()))
            }
            _ => Err(PipeError::Op(format!("WatermarkConverter: unknown schema '{schema_id}'"))),
        }
    }

    fn convert_group(&self, nodes: &[&dyn NodeInstance]) -> Result<NodeOp, PipeError> {
        if let Some(node) = nodes.first() {
            self.convert(*node)
        } else {
            Err(PipeError::Op("empty watermark group".into()))
        }
    }
}

// ─── Red dot Materialize ───

/// Create a `NodeOp::Materialize` that draws a 3x3 red rectangle at the
/// bottom-right corner of the image.
fn make_red_dot_materialize() -> NodeOp {
    NodeOp::Materialize {
        label: "red_dot",
        transform: Box::new(
            move |data: &mut Vec<u8>, w: &mut u32, h: &mut u32, fmt: &mut zenpipe::PixelFormat| {
                let iw = *w;
                let ih = *h;
                if iw < 3 || ih < 3 {
                    return; // Canvas too small for the dot.
                }
                let bpp = fmt.bytes_per_pixel();
                let stride = fmt.aligned_stride(iw);

                // Red color: RGBA = (255, 0, 0, 255)
                let red: [u8; 4] = [255, 0, 0, 255];

                for dy in 0..3u32 {
                    for dx in 0..3u32 {
                        let x = (iw - 3 + dx) as usize;
                        let y = (ih - 3 + dy) as usize;
                        let off = y * stride + x * bpp;
                        if off + bpp <= data.len() {
                            // Write as many bytes as the format has per pixel.
                            for c in 0..bpp.min(4) {
                                data[off + c] = red[c];
                            }
                        }
                    }
                }
            },
        ),
    }
}

// ─── Watermark overlay Materialize ───

/// Create a `NodeOp::Materialize` that composites the watermark image onto
/// the canvas at the computed position with opacity.
///
/// Uses `zenpipe::watermark::WatermarkLayout` for geometry (bounding box,
/// sizing, gravity) and keeps the pixel compositing inline.
fn make_watermark_materialize(overlay: WatermarkOverlayNode) -> NodeOp {
    NodeOp::Materialize {
        label: "watermark_overlay",
        transform: Box::new(
            move |data: &mut Vec<u8>, w: &mut u32, h: &mut u32, fmt: &mut zenpipe::PixelFormat| {
                let canvas_w = *w;
                let canvas_h = *h;

                // Use WatermarkLayout for geometry resolution.
                let layout = to_watermark_layout(&overlay, canvas_w, canvas_h);
                let Some(placement) = layout.resolve(canvas_w, canvas_h) else {
                    return; // Canvas too small or invalid box.
                };

                let target_w = placement.width;
                let target_h = placement.height;
                let place_x = placement.x;
                let place_y = placement.y;

                // Resize the watermark to target dimensions.
                let resized = resize_rgba8(
                    &overlay.pixels,
                    overlay.width,
                    overlay.height,
                    target_w,
                    target_h,
                );

                // Composite the watermark onto the canvas.
                let bpp = fmt.bytes_per_pixel();
                let canvas_stride = fmt.aligned_stride(canvas_w);
                let wm_stride = target_w as usize * 4; // Watermark is always RGBA8.
                let opacity = overlay.opacity;

                for wy in 0..target_h {
                    let cy = place_y + wy as i32;
                    if cy < 0 || cy >= canvas_h as i32 {
                        continue;
                    }
                    for wx in 0..target_w {
                        let cx = place_x + wx as i32;
                        if cx < 0 || cx >= canvas_w as i32 {
                            continue;
                        }
                        let wm_off = wy as usize * wm_stride + wx as usize * 4;
                        let canvas_off = cy as usize * canvas_stride + cx as usize * bpp;

                        if wm_off + 4 > resized.len() || canvas_off + bpp > data.len() {
                            continue;
                        }

                        // Source pixel (RGBA8 from watermark) → linear.
                        let sa = resized[wm_off + 3] as f32 / 255.0 * opacity;
                        let sr_lin = srgb_to_linear(resized[wm_off]) * sa; // premultiply
                        let sg_lin = srgb_to_linear(resized[wm_off + 1]) * sa;
                        let sb_lin = srgb_to_linear(resized[wm_off + 2]) * sa;

                        // Dest pixel (from canvas) → linear.
                        let da = if bpp >= 4 { data[canvas_off + 3] as f32 / 255.0 } else { 1.0 };
                        let dest_coeff = (1.0 - sa) * da;
                        let dr_lin = srgb_to_linear(data[canvas_off]) * dest_coeff;
                        let dg_lin = srgb_to_linear(data[canvas_off + 1]) * dest_coeff;
                        let db_lin = srgb_to_linear(data[canvas_off + 2]) * dest_coeff;

                        // Porter-Duff source-over in linear space (matches v2).
                        let out_a = sa + dest_coeff;
                        if out_a > 0.0 {
                            data[canvas_off] = linear_to_srgb((sr_lin + dr_lin) / out_a);
                            data[canvas_off + 1] = linear_to_srgb((sg_lin + dg_lin) / out_a);
                            data[canvas_off + 2] = linear_to_srgb((sb_lin + db_lin) / out_a);
                        } else {
                            data[canvas_off] = 0;
                            data[canvas_off + 1] = 0;
                            data[canvas_off + 2] = 0;
                        }
                        if bpp >= 4 {
                            data[canvas_off + 3] = (out_a * 255.0 + 0.5).min(255.0).max(0.0) as u8;
                        }
                    }
                }
            },
        ),
    }
}

// ─── V2 type → WatermarkLayout adapter ───

/// Convert a WatermarkOverlayNode's constraints to a zenpipe WatermarkLayout.
fn to_watermark_layout(
    overlay: &WatermarkOverlayNode,
    _canvas_w: u32,
    _canvas_h: u32,
) -> zenpipe::watermark::WatermarkLayout {
    use zenpipe::watermark::{FitBox, FitMode, Gravity, WatermarkLayout};

    let fit_box = match overlay.fit_box.as_ref() {
        None => FitBox::FullCanvas,
        Some(WatermarkConstraintBox::ImageMargins { left, top, right, bottom })
        | Some(WatermarkConstraintBox::CanvasMargins { left, top, right, bottom }) => {
            FitBox::Margins { left: *left, top: *top, right: *right, bottom: *bottom }
        }
        Some(WatermarkConstraintBox::ImagePercentage { x1, y1, x2, y2 })
        | Some(WatermarkConstraintBox::CanvasPercentage { x1, y1, x2, y2 }) => {
            FitBox::Percentage { x1: *x1, y1: *y1, x2: *x2, y2: *y2 }
        }
    };

    let fit_mode = match overlay.fit_mode {
        None | Some(WatermarkConstraintMode::Within) => FitMode::Within,
        Some(WatermarkConstraintMode::Distort) => FitMode::Distort,
        Some(WatermarkConstraintMode::Fit) => FitMode::Fit,
        Some(WatermarkConstraintMode::FitCrop) => FitMode::FitCrop,
        Some(WatermarkConstraintMode::WithinCrop) => FitMode::WithinCrop,
    };

    let gravity = match overlay.gravity.as_ref() {
        None | Some(ConstraintGravity::Center) => Gravity::Center,
        Some(ConstraintGravity::Percentage { x, y }) => Gravity::Percentage(*x, *y),
    };

    WatermarkLayout {
        wm_width: overlay.width,
        wm_height: overlay.height,
        fit_box,
        fit_mode,
        gravity,
        min_canvas_width: overlay.min_canvas_width,
        min_canvas_height: overlay.min_canvas_height,
    }
}

// ─── Watermark image resize ───

/// Resize RGBA8 pixel data using zenresize (Robidoux filter, linear-light).
///
/// Uses the same high-quality resampling pipeline as the main resize path,
/// ensuring watermark quality matches v2 output.
fn resize_rgba8(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    if src_w == dst_w && src_h == dst_h {
        return src.to_vec();
    }

    let config = zenresize::ResizeConfig::builder(src_w, src_h, dst_w, dst_h)
        .filter(zenresize::Filter::Robidoux)
        .format(zenresize::PixelDescriptor::RGBA8_SRGB)
        .build();

    zenresize::Resizer::new(&config).resize(src)
}

// ─── Pre-expansion: decode watermark from io_buffers ───

/// Decode a watermark image from `io_buffers` and return a `WatermarkOverlayNode`.
///
/// Decodes to full frame, converts to RGBA8 sRGB, and stores as raw pixel data.
pub fn decode_watermark(
    wm: &Watermark,
    io_buffers: &HashMap<i32, Vec<u8>>,
) -> Result<WatermarkOverlayNode, ZenError> {
    let input_data = io_buffers
        .get(&wm.io_id)
        .ok_or_else(|| ZenError::Io(format!("no input buffer for watermark io_id {}", wm.io_id)))?;

    // Decode the watermark image to full frame.
    let registry = zencodecs::AllowedFormats::all();
    let decoded = zencodecs::decode_full_frame(input_data, &registry)
        .map_err(|e| ZenError::Codec(format!("watermark decode: {e}")))?;

    let w = decoded.width();
    let h = decoded.height();
    let descriptor = decoded.descriptor();
    let raw_bytes = decoded.into_buffer().copy_to_contiguous_bytes();

    // Convert to RGBA8 sRGB using zenpipe's format conversion.
    let target_format = zenpipe::format::RGBA8_SRGB;
    let pixels = if descriptor == target_format {
        raw_bytes
    } else {
        // Use MaterializedSource + RowConverterOp to get RGBA8.
        let source: Box<dyn zenpipe::Source> =
            Box::new(zenpipe::sources::MaterializedSource::from_data(raw_bytes, w, h, descriptor));
        let src_format = source.format();
        let converter = zenpipe::ops::RowConverterOp::new(src_format, target_format)
            .ok_or_else(|| {
                ZenError::Codec(format!(
                    "watermark pixel format conversion not supported: {src_format:?} -> {target_format:?}"
                ))
            })?;
        let transform =
            zenpipe::sources::TransformSource::new(source).push_boxed(Box::new(converter));
        let mat_source: Box<dyn zenpipe::Source> = Box::new(transform);
        let mat = zenpipe::sources::MaterializedSource::from_source(mat_source)
            .map_err(ZenError::Pipeline)?;
        mat.data().to_vec()
    };

    Ok(WatermarkOverlayNode {
        pixels,
        width: w,
        height: h,
        fit_box: wm.fit_box.clone(),
        fit_mode: wm.fit_mode,
        gravity: wm.gravity,
        min_canvas_width: wm.min_canvas_width,
        min_canvas_height: wm.min_canvas_height,
        opacity: wm.opacity.unwrap_or(1.0).min(1.0).max(0.0),
    })
}

// ─── sRGB ↔ linear conversion ───

/// Convert an sRGB byte value to linear float (IEC 61966-2-1).
fn srgb_to_linear(b: u8) -> f32 {
    let s = b as f32 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// Convert a linear float to sRGB byte (IEC 61966-2-1).
fn linear_to_srgb(l: f32) -> u8 {
    let s = if l <= 0.0031308 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0 + 0.5).min(255.0).max(0.0) as u8
}
