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

/// Build the standard set of converters for the imageflow zen pipeline.
pub fn imageflow_converters() -> [&'static dyn NodeConverter; 2] {
    static FILTERS: ZenFiltersConverter = ZenFiltersConverter;
    static EXPAND: ExpandCanvasConverter = ExpandCanvasConverter;
    [&FILTERS, &EXPAND]
}
