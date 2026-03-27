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
        let get_u32 = |name| match node.get_param(name) {
            Some(ParamValue::U32(v)) => v,
            _ => 0,
        };
        let left = get_u32("left");
        let top = get_u32("top");
        let right = get_u32("right");
        let bottom = get_u32("bottom");

        // Extract background color from params (set by translate.rs).
        // Only use non-default color when bg_a is explicitly present.
        let bg_color = if node.get_param("bg_a").is_some() {
            [
                get_u32("bg_r") as u8,
                get_u32("bg_g") as u8,
                get_u32("bg_b") as u8,
                get_u32("bg_a") as u8,
            ]
        } else {
            [0u8, 0, 0, 0] // transparent (legacy default)
        };

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

// ImageflowNodeConverter removed — all operations now use native zennode
// definitions with built-in bridge converters in zenpipe::bridge::convert.

/// Build the standard set of converters for the imageflow zen pipeline.
///
/// Only zenfilters, expand_canvas, and region need external converters.
/// All other operations (fill_rect, crop_whitespace, remove_alpha,
/// round_corners, resize) are handled by zenpipe's built-in bridge converters.
pub fn imageflow_converters() -> [&'static dyn NodeConverter; 3] {
    static FILTERS: ZenFiltersConverter = ZenFiltersConverter;
    static EXPAND: ExpandCanvasConverter = ExpandCanvasConverter;
    static REGION: RegionConverter = RegionConverter;
    [&FILTERS, &EXPAND, &REGION]
}

