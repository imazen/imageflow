use imageflow_types::Color;
use super::internal_prelude::*;

pub static ROUND_IMAGE_CORNERS: RoundImageCorners = RoundImageCorners{};

pub static ROUND_IMAGE_CORNERS_SRGB: MutProtect<RoundImageCornersMut> = MutProtect{node: &ROUND_IMAGE_CORNERS_SRGB_MUTATE, fqn: "imazen.round_image_corners_srgb"};

pub static ROUND_IMAGE_CORNERS_SRGB_MUTATE: RoundImageCornersMut = RoundImageCornersMut{};

#[derive(Debug,Clone)]
pub struct RoundImageCorners;
impl NodeDef for RoundImageCorners{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for RoundImageCorners{
    fn fqn(&self) -> &'static str{
        "imazen.round_image_corners"
    }
    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, p: NodeParams, parent: FrameInfo) -> Result<()> {
        if let NodeParams::Json(s::Node::RoundImageCorners{ref background_color, ref radius})= p {

            let mut nodes = Vec::new();
            if !background_color.is_opaque(){
                nodes.push(Node::n(&ENABLE_TRANSPARENCY, NodeParams::None));

            }
            nodes.push(Node::n(&ROUND_IMAGE_CORNERS_SRGB,
                               NodeParams::Json(s::Node::RoundImageCorners { background_color: background_color.to_owned(), radius: *radius })));

            ctx.replace_node(ix, nodes);
            Ok(())

        }else{
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need RoundImageCorners, got {:?}", p))
        }
    }
}



#[derive(Debug, Clone)]
pub struct RoundImageCornersMut;
impl NodeDef for RoundImageCornersMut{
    fn as_one_mutate_bitmap(&self) -> Option<&dyn NodeDefMutateBitmap>{
        Some(self)
    }
}
impl NodeDefMutateBitmap for RoundImageCornersMut{
    fn fqn(&self) -> &'static str{
        "imazen.round_image_corners_mut"
    }
    fn mutate(&self, c: &Context, bitmap_key: BitmapKey,  p: &NodeParams) -> Result<()> {
        if let NodeParams::Json(s::Node::RoundImageCorners{ref background_color, ref radius}) = p {

            let bitmaps = c.borrow_bitmaps()
                .map_err(|e| e.at(here!()))?;
            let mut bitmap_bitmap = bitmaps.try_borrow_mut(bitmap_key)
                .map_err(|e| e.at(here!()))?;

            bitmap_bitmap.set_compositing(crate::graphics::bitmaps::BitmapCompositing::BlendWithSelf);

            let mut bitmap_window = bitmap_bitmap.get_window_bgra32().unwrap();

            crate::graphics::rounded_corners::flow_bitmap_bgra_clear_around_rounded_corners(&mut bitmap_window, *radius, background_color.to_owned())
                .map_err(|e| e.at(here!()))?;


            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need RoundImageCorners, got {:?}", p))
        }
    }
}
