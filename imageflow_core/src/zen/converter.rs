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

/// Converter for `zenlayout.crop_percent` → `NodeOp::Crop` (approximate).
pub struct CropPercentConverter;

impl NodeConverter for CropPercentConverter {
    fn can_convert(&self, schema_id: &str) -> bool {
        schema_id == "zenlayout.crop_percent" || schema_id == "zenlayout.region"
    }

    fn convert(&self, node: &dyn NodeInstance) -> Result<NodeOp, PipeError> {
        // Region and crop_percent nodes are handled by the geometry fusion path
        // when source dimensions are known. If we get here, source dims weren't
        // available. Return an error — these need geometry context.
        Err(PipeError::Op(format!(
            "{} requires source dimensions (geometry fusion failed)",
            node.schema().id
        )))
    }

    fn convert_group(&self, nodes: &[&dyn NodeInstance]) -> Result<NodeOp, PipeError> {
        if let Some(node) = nodes.first() {
            self.convert(*node)
        } else {
            Err(PipeError::Op("empty crop_percent group".into()))
        }
    }
}

/// Converter for imageflow-specific custom nodes (fill_rect, crop_whitespace, round_corners).
pub struct ImageflowNodeConverter;

impl NodeConverter for ImageflowNodeConverter {
    fn can_convert(&self, schema_id: &str) -> bool {
        matches!(
            schema_id,
            "imageflow.fill_rect" | "imageflow.crop_whitespace" | "imageflow.round_corners"
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
pub fn imageflow_converters() -> [&'static dyn NodeConverter; 3] {
    static FILTERS: ZenFiltersConverter = ZenFiltersConverter;
    static EXPAND: ExpandCanvasConverter = ExpandCanvasConverter;
    static IMAGEFLOW: ImageflowNodeConverter = ImageflowNodeConverter;
    [&FILTERS, &EXPAND, &IMAGEFLOW]
}
