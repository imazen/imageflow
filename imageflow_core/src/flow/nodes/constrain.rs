use super::internal_prelude::*;
use imageflow_riapi::sizing::{Layout, AspectRatio, LayoutError};
use imageflow_types::{ConstraintMode, ImageInfo};

pub static CONSTRAIN: ConstrainDef = ConstrainDef{};


#[derive(Debug,Clone)]
pub struct ConstrainDef;
impl NodeDef for ConstrainDef{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for ConstrainDef{
    fn fqn(&self) -> &'static str{
        "imazen.constrain"
    }
    fn estimate(&self, params: &NodeParams, input: FrameEstimate) -> Result<FrameEstimate>{
        if let NodeParams::Json(s::Node::Constrain(ref constraint)) = *params {
            input.map_frame(|input| {
                let constraint_results = imageflow_riapi::ir4::process_constraint(input.w, input.h, constraint).unwrap(); //TODO: fix unwrap
                Ok(FrameInfo {
                    w: constraint_results.final_canvas.width() as i32,
                    h: constraint_results.final_canvas.height() as i32,
                    fmt: ffi::PixelFormat::from(input.fmt),
                })
            })
        }else{
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need Constrain, got {:?}", params))
        }
    }

    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, params: NodeParams, input: FrameInfo) -> Result<()> {
        if let NodeParams::Json(s::Node::Constrain(constraint)) = params {
            let constraint_results = imageflow_riapi::ir4::process_constraint(input.w, input.h, &constraint).unwrap(); //TODO: fix unwrap

            let mut b  = Vec::new();
            if let Some(c) = constraint_results.crop{
                b.push(Node::from(s::Node::Crop { x1: c[0], y1: c[1], x2: c[2], y2: c[3] }));
            }

            // Override background_color with canvas_color
            let merged_hints = if constraint.canvas_color.is_some() {
                if constraint.hints.is_some() {
                    Some(imageflow_types::ResampleHints {
                        background_color: constraint.canvas_color.clone(),
                        ..constraint.hints.unwrap().clone()
                    })
                }else {
                    Some(imageflow_types::ResampleHints {
                        background_color: constraint.canvas_color.clone(),
                        ..imageflow_types::ResampleHints::new()
                    })
                }
            }else{
                constraint.hints.clone()
            };


            b.push(Node::from(
                imageflow_types::Node::Resample2D {
                    w: constraint_results.scale_to.width() as u32,
                    h: constraint_results.scale_to.height() as u32,
                    hints: merged_hints,
                })
            );

            if let Some(pad) = constraint_results.pad{
                b.push(Node::from(
                imageflow_types::Node::ExpandCanvas {
                    left: pad[0],
                    top: pad[1],
                    right: pad[2],
                    bottom: pad[3],
                    color: constraint.canvas_color.unwrap_or(imageflow_types::Color::Transparent)
                }));
            }

            ctx.replace_node(ix, b);

            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need Constrain, got {:?}", params))
        }
    }
}
