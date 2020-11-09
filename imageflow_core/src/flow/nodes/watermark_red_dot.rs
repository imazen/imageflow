use super::internal_prelude::*;

pub static WATERMARK_RED_DOT: WatermarkRedDotDef = WatermarkRedDotDef{};

#[derive(Debug,Clone)]
pub struct WatermarkRedDotDef;
impl WatermarkRedDotDef{
}

impl NodeDef for WatermarkRedDotDef{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for WatermarkRedDotDef {
    fn fqn(&self) -> &'static str {
        "imazen.watermark"
    }
    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, params: NodeParams, input: FrameInfo) -> Result<()> {
        if let NodeParams::Json(imageflow_types::Node::WatermarkRedDot) = params {
            let canvas_sufficient = input.w > 3 &&
                input.h > 3;
            if canvas_sufficient {

                ctx.replace_node(ix, vec![
                    Node::from(imageflow_types::Node::FillRect {
                        x1: input.w as u32 - 3,
                        y1: input.h as u32 - 3,
                        x2: input.w as u32,
                        y2: input.h as u32,
                        color: imageflow_types::Color::Srgb(imageflow_types::ColorSrgb::Hex("FF0000".to_owned()))
                    })
                ]);

            }
            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need WatermarkRedDot, got {:?}", params))
        }
    }
}
