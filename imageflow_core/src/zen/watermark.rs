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
fn make_watermark_materialize(overlay: WatermarkOverlayNode) -> NodeOp {
    NodeOp::Materialize {
        label: "watermark_overlay",
        transform: Box::new(
            move |data: &mut Vec<u8>, w: &mut u32, h: &mut u32, fmt: &mut zenpipe::PixelFormat| {
                let canvas_w = *w;
                let canvas_h = *h;

                // Check minimum canvas size.
                if overlay.min_canvas_width.unwrap_or(0) > canvas_w
                    || overlay.min_canvas_height.unwrap_or(0) > canvas_h
                {
                    return; // Canvas too small, skip watermark (matches v2 behavior).
                }

                // Compute bounding box on the canvas.
                let bbox = get_bounding_box(canvas_w, canvas_h, overlay.fit_box.as_ref());
                let (box_x1, box_y1, box_x2, box_y2) = match bbox {
                    Some(b) => b,
                    None => return, // Bounding box too small.
                };

                let box_w = (box_x2 - box_x1) as u32;
                let box_h = (box_y2 - box_y1) as u32;

                // Compute the target size for the watermark within the bounding box.
                let (target_w, target_h) = compute_watermark_size(
                    overlay.width,
                    overlay.height,
                    box_w,
                    box_h,
                    overlay.fit_mode.unwrap_or(WatermarkConstraintMode::Within),
                );

                if target_w == 0 || target_h == 0 {
                    return;
                }

                // Resize the watermark to target dimensions.
                let resized = resize_rgba8(
                    &overlay.pixels,
                    overlay.width,
                    overlay.height,
                    target_w,
                    target_h,
                );

                // Compute position within the bounding box using gravity.
                let (place_x, place_y) = compute_gravity_position(
                    box_x1,
                    box_y1,
                    box_x2,
                    box_y2,
                    target_w as i32,
                    target_h as i32,
                    overlay.gravity.as_ref(),
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

                        // Source pixel (RGBA8 from watermark).
                        let sr = resized[wm_off] as f32 / 255.0;
                        let sg = resized[wm_off + 1] as f32 / 255.0;
                        let sb = resized[wm_off + 2] as f32 / 255.0;
                        let sa = resized[wm_off + 3] as f32 / 255.0 * opacity;

                        // Dest pixel (from canvas, assumed RGBA8).
                        let dr = data[canvas_off] as f32 / 255.0;
                        let dg = data[canvas_off + 1] as f32 / 255.0;
                        let db = data[canvas_off + 2] as f32 / 255.0;
                        let da = if bpp >= 4 { data[canvas_off + 3] as f32 / 255.0 } else { 1.0 };

                        // Porter-Duff source-over in sRGB space (matches v2 behavior).
                        let out_a = sa + da * (1.0 - sa);
                        let (out_r, out_g, out_b) = if out_a > 0.0 {
                            (
                                (sr * sa + dr * da * (1.0 - sa)) / out_a,
                                (sg * sa + dg * da * (1.0 - sa)) / out_a,
                                (sb * sa + db * da * (1.0 - sa)) / out_a,
                            )
                        } else {
                            (0.0, 0.0, 0.0)
                        };

                        data[canvas_off] = (out_r * 255.0 + 0.5).min(255.0).max(0.0) as u8;
                        data[canvas_off + 1] = (out_g * 255.0 + 0.5).min(255.0).max(0.0) as u8;
                        data[canvas_off + 2] = (out_b * 255.0 + 0.5).min(255.0).max(0.0) as u8;
                        if bpp >= 4 {
                            data[canvas_off + 3] = (out_a * 255.0 + 0.5).min(255.0).max(0.0) as u8;
                        }
                    }
                }
            },
        ),
    }
}

// ─── Bounding box computation ───

/// Compute the bounding box for the watermark on the canvas.
/// Returns `(x1, y1, x2, y2)` in canvas pixels, or `None` if the box is invalid.
fn get_bounding_box(
    w: u32,
    h: u32,
    fit_box: Option<&WatermarkConstraintBox>,
) -> Option<(i32, i32, i32, i32)> {
    match fit_box {
        None => Some((0, 0, w as i32, h as i32)),
        Some(WatermarkConstraintBox::ImageMargins { left, top, right, bottom })
        | Some(WatermarkConstraintBox::CanvasMargins { left, top, right, bottom }) => {
            if left + right < w && top + bottom < h {
                Some((
                    *left as i32,
                    *top as i32,
                    w as i32 - *right as i32,
                    h as i32 - *bottom as i32,
                ))
            } else {
                None
            }
        }
        Some(WatermarkConstraintBox::ImagePercentage { x1, y1, x2, y2 })
        | Some(WatermarkConstraintBox::CanvasPercentage { x1, y1, x2, y2 }) => {
            fn to_pixels(percent: f32, canvas: u32) -> i32 {
                let ratio = percent.min(100.0).max(0.0) / 100.0;
                (ratio * canvas as f32).round() as i32
            }
            let px1 = to_pixels(*x1, w);
            let py1 = to_pixels(*y1, h);
            let px2 = to_pixels(*x2, w);
            let py2 = to_pixels(*y2, h);
            if px1 < px2 && py1 < py2 {
                Some((px1, py1, px2, py2))
            } else {
                None
            }
        }
    }
}

/// Compute the gravity-based position within a bounding box.
fn compute_gravity_position(
    box_x1: i32,
    box_y1: i32,
    box_x2: i32,
    box_y2: i32,
    wm_w: i32,
    wm_h: i32,
    gravity: Option<&ConstraintGravity>,
) -> (i32, i32) {
    let (gx, gy) = match gravity {
        Some(ConstraintGravity::Center) | None => (50.0f32, 50.0f32),
        Some(ConstraintGravity::Percentage { x, y }) => (*x, *y),
    };
    let box_w = box_x2 - box_x1;
    let box_h = box_y2 - box_y1;
    let x = if box_w > wm_w {
        box_x1 + ((box_w - wm_w) as f32 * gx.min(100.0).max(0.0) / 100.0).round() as i32
    } else {
        box_x1
    };
    let y = if box_h > wm_h {
        box_y1 + ((box_h - wm_h) as f32 * gy.min(100.0).max(0.0) / 100.0).round() as i32
    } else {
        box_y1
    };
    (x, y)
}

/// Compute target watermark size within a bounding box using the constraint mode.
fn compute_watermark_size(
    wm_w: u32,
    wm_h: u32,
    box_w: u32,
    box_h: u32,
    mode: WatermarkConstraintMode,
) -> (u32, u32) {
    if wm_w == 0 || wm_h == 0 || box_w == 0 || box_h == 0 {
        return (0, 0);
    }

    let wm_aspect = wm_w as f64 / wm_h as f64;
    let box_aspect = box_w as f64 / box_h as f64;

    match mode {
        WatermarkConstraintMode::Distort => (box_w, box_h),

        WatermarkConstraintMode::Fit => {
            // Scale to fit within box, upscaling if needed.
            if wm_aspect > box_aspect {
                // Width-constrained.
                let h = (box_w as f64 / wm_aspect).round() as u32;
                (box_w, h.max(1))
            } else {
                // Height-constrained.
                let w = (box_h as f64 * wm_aspect).round() as u32;
                (w.max(1), box_h)
            }
        }

        WatermarkConstraintMode::Within => {
            // Scale to fit within box, no upscaling.
            if wm_w <= box_w && wm_h <= box_h {
                // Already fits, no scaling.
                (wm_w, wm_h)
            } else {
                // Need to downscale.
                if wm_aspect > box_aspect {
                    let h = (box_w as f64 / wm_aspect).round() as u32;
                    (box_w, h.max(1))
                } else {
                    let w = (box_h as f64 * wm_aspect).round() as u32;
                    (w.max(1), box_h)
                }
            }
        }

        WatermarkConstraintMode::FitCrop => {
            // Scale to fill box (may overshoot one dimension), then crop.
            // "Fill" means the smaller ratio determines scale.
            if wm_aspect > box_aspect {
                // Height-constrained fill.
                let w = (box_h as f64 * wm_aspect).round() as u32;
                (w.min(box_w).max(1), box_h)
            } else {
                // Width-constrained fill.
                let h = (box_w as f64 / wm_aspect).round() as u32;
                (box_w, h.min(box_h).max(1))
            }
        }

        WatermarkConstraintMode::WithinCrop => {
            // Like FitCrop but no upscaling.
            if wm_w <= box_w && wm_h <= box_h {
                (wm_w, wm_h)
            } else {
                if wm_aspect > box_aspect {
                    let w = (box_h as f64 * wm_aspect).round() as u32;
                    (w.min(box_w).max(1), box_h.min(wm_h))
                } else {
                    let h = (box_w as f64 / wm_aspect).round() as u32;
                    (box_w.min(wm_w), h.min(box_h).max(1))
                }
            }
        }
    }
}

// ─── Watermark image resize ───

/// Simple bilinear resize of RGBA8 pixel data.
///
/// Not as high quality as zenresize's multi-tap filters, but adequate for
/// watermark overlays which are typically simple logos/icons.
fn resize_rgba8(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    if src_w == dst_w && src_h == dst_h {
        return src.to_vec();
    }

    let mut dst = vec![0u8; dst_w as usize * dst_h as usize * 4];
    let src_stride = src_w as usize * 4;
    let dst_stride = dst_w as usize * 4;

    for dy in 0..dst_h {
        let sy_f = (dy as f64 + 0.5) * src_h as f64 / dst_h as f64 - 0.5;
        let sy0 = sy_f.floor().max(0.0) as u32;
        let sy1 = (sy0 + 1).min(src_h - 1);
        let fy = (sy_f - sy0 as f64).max(0.0).min(1.0);

        for dx in 0..dst_w {
            let sx_f = (dx as f64 + 0.5) * src_w as f64 / dst_w as f64 - 0.5;
            let sx0 = sx_f.floor().max(0.0) as u32;
            let sx1 = (sx0 + 1).min(src_w - 1);
            let fx = (sx_f - sx0 as f64).max(0.0).min(1.0);

            // Four source pixels for bilinear interpolation.
            let off00 = sy0 as usize * src_stride + sx0 as usize * 4;
            let off10 = sy0 as usize * src_stride + sx1 as usize * 4;
            let off01 = sy1 as usize * src_stride + sx0 as usize * 4;
            let off11 = sy1 as usize * src_stride + sx1 as usize * 4;

            let dst_off = dy as usize * dst_stride + dx as usize * 4;

            for c in 0..4 {
                let p00 = src.get(off00 + c).copied().unwrap_or(0) as f64;
                let p10 = src.get(off10 + c).copied().unwrap_or(0) as f64;
                let p01 = src.get(off01 + c).copied().unwrap_or(0) as f64;
                let p11 = src.get(off11 + c).copied().unwrap_or(0) as f64;

                let top = p00 * (1.0 - fx) + p10 * fx;
                let bot = p01 * (1.0 - fx) + p11 * fx;
                let val = top * (1.0 - fy) + bot * fy;
                dst[dst_off + c] = (val + 0.5).min(255.0).max(0.0) as u8;
            }
        }
    }

    dst
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
