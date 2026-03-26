//! NodeConverter implementations for zenfilters and zenlayout nodes.
//!
//! The zenpipe bridge has built-in converters for core geometry and resize nodes
//! but doesn't know about zenfilters or zenlayout.expand_canvas. These converters
//! bridge the gap.

use zennode::NodeInstance;
use zenpipe::bridge::NodeConverter;
use zenpipe::PipeError;
use zenpipe::graph::NodeOp;

/// Converter for `zenfilters.*` nodes → `NodeOp::Filter(pipeline)`.
///
/// Converts zenfilters NodeInstance types to a zenfilters::Pipeline wrapped in
/// NodeOp::Filter. Handles single nodes and fused groups.
pub struct ZenFiltersConverter;

impl NodeConverter for ZenFiltersConverter {
    fn can_convert(&self, schema_id: &str) -> bool {
        zenfilters::zennode_defs::is_zenfilters_node(schema_id)
    }

    fn convert(&self, node: &dyn NodeInstance) -> Result<NodeOp, PipeError> {
        let filter = zenfilters::zennode_defs::node_to_filter(node).ok_or_else(|| {
            PipeError::Op(format!(
                "zenfilters converter: unrecognized node '{}'",
                node.schema().id
            ))
        })?;

        let mut pipeline =
            zenfilters::Pipeline::new(zenfilters::PipelineConfig::default()).map_err(|e| {
                PipeError::Op(format!("zenfilters pipeline creation failed: {e:?}"))
            })?;
        pipeline.push(filter);
        Ok(NodeOp::Filter(pipeline))
    }

    fn convert_group(&self, nodes: &[&dyn NodeInstance]) -> Result<NodeOp, PipeError> {
        let mut pipeline =
            zenfilters::Pipeline::new(zenfilters::PipelineConfig::default()).map_err(|e| {
                PipeError::Op(format!("zenfilters pipeline creation failed: {e:?}"))
            })?;

        for node in nodes {
            let filter = zenfilters::zennode_defs::node_to_filter(*node).ok_or_else(|| {
                PipeError::Op(format!(
                    "zenfilters converter: unrecognized node '{}'",
                    node.schema().id
                ))
            })?;
            pipeline.push(filter);
        }

        Ok(NodeOp::Filter(pipeline))
    }

    fn fuse_group(&self, nodes: &[&dyn NodeInstance]) -> Result<Option<NodeOp>, PipeError> {
        // All zenfilters nodes in the fused_adjust coalesce group can be fused
        // into a single pipeline. Try to build a pipeline from all nodes.
        if nodes.len() < 2 {
            return Ok(None);
        }

        let mut pipeline =
            zenfilters::Pipeline::new(zenfilters::PipelineConfig::default()).map_err(|e| {
                PipeError::Op(format!("zenfilters pipeline creation failed: {e:?}"))
            })?;

        for node in nodes {
            if let Some(filter) = zenfilters::zennode_defs::node_to_filter(*node) {
                pipeline.push(filter);
            } else {
                return Ok(None); // Can't fuse if any node is unknown
            }
        }

        Ok(Some(NodeOp::Filter(pipeline)))
    }
}

/// Converter for `zenlayout.expand_canvas` → `NodeOp::ExpandCanvas`.
pub struct ExpandCanvasConverter;

impl NodeConverter for ExpandCanvasConverter {
    fn can_convert(&self, schema_id: &str) -> bool {
        schema_id == "zenlayout.expand_canvas"
    }

    fn convert(&self, node: &dyn NodeInstance) -> Result<NodeOp, PipeError> {
        use zennode::ParamValue;
        let left = match node.get_param("left") {
            Some(ParamValue::U32(v)) => v,
            _ => 0,
        };
        let top = match node.get_param("top") {
            Some(ParamValue::U32(v)) => v,
            _ => 0,
        };
        let right = match node.get_param("right") {
            Some(ParamValue::U32(v)) => v,
            _ => 0,
        };
        let bottom = match node.get_param("bottom") {
            Some(ParamValue::U32(v)) => v,
            _ => 0,
        };
        // TODO: extract background color from node params when available.
        let bg_color = [0u8, 0, 0, 0]; // transparent black

        Ok(NodeOp::ExpandCanvas {
            left,
            top,
            right,
            bottom,
            bg_color,
        })
    }

    fn convert_group(&self, nodes: &[&dyn NodeInstance]) -> Result<NodeOp, PipeError> {
        if let Some(node) = nodes.first() {
            self.convert(*node)
        } else {
            Err(PipeError::Op("empty expand_canvas group".into()))
        }
    }
}

/// Converter for `zenlayout.region` — viewport with crop + expand.
///
/// Region defines a viewport in source coordinates. Negative coords extend
/// beyond the source (padding), positive coords within the source (crop).
/// Output is a Materialize that places the source onto a canvas at the
/// correct offset using ExpandCanvasSource.
pub struct RegionConverter;

impl NodeConverter for RegionConverter {
    fn can_convert(&self, schema_id: &str) -> bool {
        schema_id == "zenlayout.region"
    }

    fn convert(&self, node: &dyn NodeInstance) -> Result<NodeOp, PipeError> {
        use zennode::ParamValue;
        let x1 = match node.get_param("x1") { Some(ParamValue::I32(v)) => v, _ => 0 };
        let y1 = match node.get_param("y1") { Some(ParamValue::I32(v)) => v, _ => 0 };
        let x2 = match node.get_param("x2") { Some(ParamValue::I32(v)) => v, _ => 0 };
        let y2 = match node.get_param("y2") { Some(ParamValue::I32(v)) => v, _ => 0 };

        // Region(x1,y1,x2,y2) defines a viewport. The output canvas is
        // (x2-x1) × (y2-y1) pixels. The source image is placed at (-x1,-y1)
        // in the canvas. ExpandCanvasSource handles clipping and padding.
        let canvas_w = (x2 - x1).max(1) as u32;
        let canvas_h = (y2 - y1).max(1) as u32;
        let place_x = -x1; // source's (0,0) goes to canvas position (-x1, -y1)
        let place_y = -y1;

        // Use Materialize to apply the region viewport via ExpandCanvasSource.
        // ExpandCanvasSource handles negative placement (crop) and positive
        // placement (padding) transparently.
        Ok(NodeOp::Materialize(Box::new(move |data: &mut Vec<u8>, w: &mut u32, h: &mut u32, fmt: &mut zenpipe::PixelFormat| {
            let src_w = *w;
            let src_h = *h;
            let bpp = fmt.bytes_per_pixel();
            let src_stride = src_w as usize * bpp;

            // Build the output canvas
            let out_stride = canvas_w as usize * bpp;
            let mut out = vec![0u8; canvas_h as usize * out_stride];

            for out_y in 0..canvas_h as i32 {
                let src_y = out_y - place_y;
                if src_y < 0 || src_y >= src_h as i32 { continue; }
                for out_x in 0..canvas_w as i32 {
                    let src_x = out_x - place_x;
                    if src_x < 0 || src_x >= src_w as i32 { continue; }
                    let src_off = src_y as usize * src_stride + src_x as usize * bpp;
                    let dst_off = out_y as usize * out_stride + out_x as usize * bpp;
                    if src_off + bpp <= data.len() && dst_off + bpp <= out.len() {
                        out[dst_off..dst_off + bpp].copy_from_slice(&data[src_off..src_off + bpp]);
                    }
                }
            }

            *data = out;
            *w = canvas_w;
            *h = canvas_h;
        })))
    }

    fn convert_group(&self, nodes: &[&dyn NodeInstance]) -> Result<NodeOp, PipeError> {
        if let Some(node) = nodes.first() {
            self.convert(*node)
        } else {
            Err(PipeError::Op("empty region group".into()))
        }
    }
}

/// Converter for imageflow-specific custom nodes (fill_rect, crop_whitespace, round_corners).
pub struct ImageflowNodeConverter;

impl NodeConverter for ImageflowNodeConverter {
    fn can_convert(&self, schema_id: &str) -> bool {
        matches!(
            schema_id,
            "imageflow.fill_rect" | "imageflow.crop_whitespace" | "imageflow.round_corners" | "imageflow.remove_alpha" | "imageflow.resample2d"
        )
    }

    fn convert(&self, node: &dyn NodeInstance) -> Result<NodeOp, PipeError> {
        use zennode::ParamValue;
        match node.schema().id {
            "imageflow.fill_rect" => {
                let x1 = match node.get_param("x1") { Some(ParamValue::U32(v)) => v, _ => 0 };
                let y1 = match node.get_param("y1") { Some(ParamValue::U32(v)) => v, _ => 0 };
                let x2 = match node.get_param("x2") { Some(ParamValue::U32(v)) => v, _ => 0 };
                let y2 = match node.get_param("y2") { Some(ParamValue::U32(v)) => v, _ => 0 };
                // Extract color from the concrete type via downcast.
                let color = node
                    .as_any()
                    .downcast_ref::<super::translate::FillRectNode>()
                    .map(|n| n.color)
                    .unwrap_or([0, 0, 0, 255]);
                Ok(NodeOp::FillRect { x1, y1, x2, y2, color })
            }
            "imageflow.crop_whitespace" => {
                let threshold = match node.get_param("threshold") {
                    Some(ParamValue::U32(v)) => v as u8,
                    _ => 80,
                };
                let percent_padding = match node.get_param("percent_padding") {
                    Some(ParamValue::F32(v)) => v,
                    _ => 0.0,
                };
                Ok(NodeOp::CropWhitespace { threshold, percent_padding })
            }
            "imageflow.round_corners" => {
                let radius = match node.get_param("radius") {
                    Some(ParamValue::F32(v)) => v,
                    _ => 0.0,
                };
                let bg = node
                    .as_any()
                    .downcast_ref::<super::translate::RoundCornersNode>()
                    .map(|n| n.bg_color)
                    .unwrap_or([0, 0, 0, 0]);

                // RoundCorners materializes: apply rounded mask to each pixel.
                Ok(NodeOp::Materialize(Box::new(move |data: &mut Vec<u8>, w: &mut u32, h: &mut u32, fmt: &mut zenpipe::PixelFormat| {
                    let bpp = fmt.bytes_per_pixel();
                    let stride = *w as usize * bpp;
                    let r = radius.min(*w as f32 / 2.0).min(*h as f32 / 2.0);

                    for y in 0..*h {
                        for x in 0..*w {
                            let dx = if x < r as u32 {
                                r - x as f32 - 0.5
                            } else if x >= *w - r as u32 {
                                x as f32 + 0.5 - (*w as f32 - r)
                            } else {
                                0.0
                            };
                            let dy = if y < r as u32 {
                                r - y as f32 - 0.5
                            } else if y >= *h - r as u32 {
                                y as f32 + 0.5 - (*h as f32 - r)
                            } else {
                                0.0
                            };
                            if dx > 0.0 && dy > 0.0 && dx * dx + dy * dy > r * r {
                                let off = y as usize * stride + x as usize * bpp;
                                if off + bpp <= data.len() {
                                    for c in 0..bpp.min(4) {
                                        data[off + c] = bg[c];
                                    }
                                }
                            }
                        }
                    }
                })))
            }
            "imageflow.resample2d" => {
                let n = node.as_any()
                    .downcast_ref::<super::translate::Resample2DNode>()
                    .ok_or_else(|| PipeError::Op("resample2d: downcast failed".into()))?;
                let filter = n.filter.as_deref().and_then(|s| match s {
                    "robidoux" => Some(zenresize::Filter::Robidoux),
                    "robidoux_sharp" => Some(zenresize::Filter::RobidouxSharp),
                    "robidoux_fast" => Some(zenresize::Filter::RobidouxFast),
                    "hermite" => Some(zenresize::Filter::Hermite),
                    "lanczos" | "lanczos3" => Some(zenresize::Filter::Lanczos),
                    "mitchell" => Some(zenresize::Filter::Mitchell),
                    "catmull_rom" => Some(zenresize::Filter::CatmullRom),
                    "box" => Some(zenresize::Filter::Box),
                    "triangle" | "linear" => Some(zenresize::Filter::Triangle),
                    _ => None,
                });
                Ok(NodeOp::Resize {
                    w: n.w,
                    h: n.h,
                    filter,
                    sharpen_percent: None,
                })
            }
            "imageflow.remove_alpha" => {
                let matte = node
                    .as_any()
                    .downcast_ref::<super::translate::RemoveAlphaNode>()
                    .map(|n| n.matte)
                    .unwrap_or([255, 255, 255]);
                // v2 blends matte in linear light space, not sRGB.
                // Use Materialize to do the blend manually in linear.
                let [mr, mg, mb] = matte;
                Ok(NodeOp::Materialize(Box::new(move |data: &mut Vec<u8>, w: &mut u32, h: &mut u32, fmt: &mut zenpipe::PixelFormat| {
                    let bpp = fmt.bytes_per_pixel();
                    if bpp < 4 { return; } // No alpha to remove
                    let pw = *w as usize;
                    let ph = *h as usize;
                    // sRGB matte → linear
                    let ml_r = srgb_byte_to_linear(mr);
                    let ml_g = srgb_byte_to_linear(mg);
                    let ml_b = srgb_byte_to_linear(mb);

                    // In-place: RGBA → RGB with linear-space matte blend
                    let mut out = Vec::with_capacity(pw * ph * 3);
                    for y in 0..ph {
                        for x in 0..pw {
                            let off = (y * pw + x) * bpp;
                            let sr = data[off] as f32 / 255.0;
                            let sg = data[off + 1] as f32 / 255.0;
                            let sb = data[off + 2] as f32 / 255.0;
                            let a = data[off + 3] as f32 / 255.0;

                            // sRGB → linear
                            let lr = srgb_to_linear(sr);
                            let lg = srgb_to_linear(sg);
                            let lb = srgb_to_linear(sb);

                            // Composite in linear (straight alpha over matte)
                            let or = lr * a + ml_r * (1.0 - a);
                            let og = lg * a + ml_g * (1.0 - a);
                            let ob = lb * a + ml_b * (1.0 - a);

                            // Linear → sRGB
                            out.push(linear_to_srgb_byte(or));
                            out.push(linear_to_srgb_byte(og));
                            out.push(linear_to_srgb_byte(ob));
                        }
                    }
                    *data = out;
                    *fmt = zenpipe::format::RGB8_SRGB;
                })))
            }
            _ => Err(PipeError::Op(format!(
                "imageflow converter: unknown node '{}'",
                node.schema().id
            ))),
        }
    }

    fn convert_group(&self, nodes: &[&dyn NodeInstance]) -> Result<NodeOp, PipeError> {
        if let Some(node) = nodes.first() {
            self.convert(*node)
        } else {
            Err(PipeError::Op("empty imageflow node group".into()))
        }
    }
}

/// Build the standard set of converters for the imageflow zen pipeline.
pub fn imageflow_converters() -> [&'static dyn NodeConverter; 4] {
    static FILTERS: ZenFiltersConverter = ZenFiltersConverter;
    static EXPAND: ExpandCanvasConverter = ExpandCanvasConverter;
    static REGION: RegionConverter = RegionConverter;
    static IMAGEFLOW: ImageflowNodeConverter = ImageflowNodeConverter;
    [&FILTERS, &EXPAND, &REGION, &IMAGEFLOW]
}

fn srgb_to_linear(s: f32) -> f32 {
    linear_srgb::iec::srgb_to_linear(s)
}

fn srgb_byte_to_linear(b: u8) -> f32 {
    linear_srgb::iec::srgb_to_linear(b as f32 / 255.0)
}

fn linear_to_srgb_byte(l: f32) -> u8 {
    let s = linear_srgb::iec::linear_to_srgb(l);
    (s * 255.0 + 0.5).min(255.0).max(0.0) as u8
}
