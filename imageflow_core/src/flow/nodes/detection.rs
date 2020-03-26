use super::internal_prelude::*;

pub static CROP_FACES: CropFacesDef = CropFacesDef{};

#[derive(Debug,Clone)]
pub struct CropFacesDef;
impl NodeDef for CropFacesDef{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for CropFacesDef {
    fn fqn(&self) -> &'static str {
        "imazen.crop_faces"
    }
    fn estimate(&self, p: &NodeParams, input: FrameEstimate) -> Result<FrameEstimate> {
        Ok(FrameEstimate::Impossible)
    }

    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, p: NodeParams, b: FrameInfo) -> Result<()> {
        // detect bounds, increase, and replace with crop
        if let NodeParams::Json(s::Node::CropWhitespace { threshold, percent_padding }) = p {
            // detect bounds, increase, and replace with crop
            let (x1, y1, x2, y2) = match ctx.first_parent_input_weight(ix).unwrap().result {
                NodeResult::Frame(b) => {
                    let bit_ref = unsafe{&*b};
                    // if let Some(rect) = ::graphics::whitespace::detect_content(bit_ref, threshold) {
                    //                     //     if rect.x2 <= rect.x1 || rect.y2 <= rect.y1 {
                    //                     //         return Err(nerror!(::ErrorKind::InvalidState, "Whitespace detection returned invalid rectangle"));
                    //                     //     }
                    //                     //     let padding = (percent_padding / 100f32 * (rect.x2 - rect.x1 + rect.y2 - rect.y1) as f32 / 2f32).ceil() as i64;
                    //                     //     Ok((cmp::max(0, rect.x1 as i64  - padding) as u32, cmp::max(0, rect.y1 as i64 - padding) as u32,
                    //                     //         cmp::min(bit_ref.w as i64, rect.x2 as i64 + padding) as u32, cmp::min(bit_ref.h as i64, rect.y2 as i64 + padding) as u32))
                    //                     // } else {
                    //                     //     return Err(nerror!(::ErrorKind::InvalidState, "Failed to complete whitespace detection"));
                    //                     // }
                },
                other => { Err(nerror!(::ErrorKind::InvalidOperation, "Cannot CropWhitespace without a parent bitmap; got {:?}", other)) }
            }?;
            ctx.replace_node(ix, vec![
                Node::n(&CROP,
                        NodeParams::Json(s::Node::Crop { x1, y1, x2, y2 }))
            ]);


            Ok(())
        } else {
            Err(nerror!(::ErrorKind::NodeParamsMismatch, "Need CropWhitespace, got {:?}", p))
        }
    }
}

