use super::internal_prelude::*;
use imageflow_riapi::sizing::{Layout, AspectRatio, LayoutError};
use imageflow_types::{ConstraintMode, WatermarkConstraintBox, WatermarkConstraintMode};

pub static WATERMARK: WatermarkDef = WatermarkDef{};



#[derive(Debug,Clone)]
pub struct WatermarkDef;
impl WatermarkDef{
    /// Returns Ok(None) if the bounding box is not at least 1x1px
    fn get_bounding_box(w: u32, h: u32, fit_box: Option<imageflow_types::WatermarkConstraintBox>) -> Option<(i32,i32,i32,i32)>{
        match fit_box{
            None => Some((0,0,w as i32,h as i32)),
            Some(imageflow_types::WatermarkConstraintBox::ImageMargins { left, top, right, bottom }) =>
                if left + right < w && top + bottom < h{
                    Some((left as i32, top as i32, w as i32 - right as i32, h as i32 - bottom as i32) )
                }else{
                    None
                }
            Some(imageflow_types::WatermarkConstraintBox::ImagePercentage { x1, y1, x2, y2 }) => {
                fn to_pixels(percent: f32, canvas: u32) -> i32{
                    let ratio = f32::min(100f32, f32::max(0f32,percent)) / 100f32;
                    (ratio * canvas as f32).round() as i32
                }
                let x1 = to_pixels(x1, w);
                let y1 = to_pixels(y1, h);
                let x2 = to_pixels(x2, w);
                let y2 = to_pixels(y2, h);
                if x1 < x2 && y1 < y2 {
                    Some((x1,y1,x2,y2))
                }else{
                    None
                }
            }
        }
    }
    fn gravity1d(align_percentage: f32, inner: i32, outer: i32) -> Result<i32>{
        let ratio = f32::min(100f32, f32::max(0f32,align_percentage)) / 100f32;
        if outer < inner && inner < 1 || outer < 1 {
            Err(nerror!(ErrorKind::InvalidNodeParams, "Watermark fit_box does not work"))
        }else{
            Ok(((outer-inner) as f32 * ratio).round() as i32)
        }
    }

    fn obey_gravity(box_x1: i32, box_y1: i32, box_x2: i32, box_y2: i32,
                    w: i32, h: i32,
                    gravity: Option<imageflow_types::ConstraintGravity>) -> Result<(i32, i32)>{
        let (x,y) = match gravity{
            Some(imageflow_types::ConstraintGravity::Center) | None => (50f32,50f32),
            Some(imageflow_types::ConstraintGravity::Percentage {x,y}) => (x, y)
        };
        Ok((Self::gravity1d(x,w, box_x2 - box_x1)? + box_x1,
            Self::gravity1d(y, h, box_y2 - box_y1)? + box_y1))

    }
}
impl NodeDef for WatermarkDef{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for WatermarkDef{
    fn fqn(&self) -> &'static str{
        "imazen.watermark"
    }
    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, params: NodeParams, input: FrameInfo) -> Result<()> {
        if let NodeParams::Json(s::Node::Watermark(watermark)) = params {
            if let Some((box_x1, box_y1, box_x2, box_y2)) =
                    WatermarkDef::get_bounding_box(input.w as u32, input.h as u32, watermark.fit_box) {
                let box_w = (box_x2 - box_x1) as u32;
                let box_h = (box_y2 - box_y1) as u32;

                let constraint = imageflow_types::Constraint {
                    mode: ConstraintMode::from(watermark.fit_mode.unwrap_or(WatermarkConstraintMode::Within)),
                    w: Some(box_w),
                    h: Some(box_h),
                    hints: None,
                    gravity: watermark.gravity.clone(),
                    canvas_color: None
                };

                let constraint_results = imageflow_riapi::ir4::process_constraint(input.w, input.h, &constraint).unwrap(); //TODO: fix unwrap

                let w = constraint_results.scale_to.width() as u32;
                let h =  constraint_results.scale_to.height() as u32;
                let (x1, y1) = WatermarkDef::obey_gravity(box_x1, box_y1, box_x2, box_y2,
                            w as i32, h as i32, watermark.gravity)?;


                let mut b = Vec::new();

                b.push(Node::from(imageflow_types::Node::Decode { io_id: watermark.io_id, commands: None }));

                if let Some(c) = constraint_results.crop {
                    b.push(Node::from(s::Node::Crop { x1: c[0], y1: c[1], x2: c[2], y2: c[3] }));
                }

                let opacity = f32::max(0f32, f32::min(1f32, watermark.opacity.unwrap_or(1f32)));
                if opacity < 1f32 {
                    //TODO: push EnableTransparency node
                    b.push(Node::from(imageflow_types::Node::ColorFilterSrgb(imageflow_types::ColorFilterSrgb::Alpha(opacity))));
                }

                b.push(Node::from(
                    imageflow_types::Node::DrawImageExact {
                        x: x1 as u32,
                        y: y1 as u32,
                        w,
                        h,
                        blend: Some(imageflow_types::CompositingMode::Compose),
                        hints: constraint.hints,
                    })
                );

                // Add the watermark chain
                let (_, draw_image) = ctx.add_nodes(b).unwrap();
                // Locate and add the canvas edge
                let canvas = ctx.first_parent_input(ix).expect("watermark must have input node");
                ctx.graph.add_edge(canvas, draw_image, EdgeKind::Canvas).unwrap();
                // Add outbound nodes
                ctx.copy_edges_to(ix, draw_image, EdgeDirection::Outgoing);

                // Remove old node and old edges
                ctx.graph.remove_node(ix).unwrap();


                Ok(())
            }else{
                // The bounding box was too small to draw the watermark
                ctx.delete_node_and_snap_together(ix);
                Ok(())
            }
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need Constrain, got {:?}", params))
        }
    }
}
