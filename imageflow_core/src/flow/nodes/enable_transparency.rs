use super::internal_prelude::*;
use imageflow_types::{PixelFormat, CompositingMode};
use crate::ffi::BitmapCompositingMode;
use crate::graphics::bitmaps::BitmapCompositing;

pub static ENABLE_TRANSPARENCY: EnableTransparencyDef = EnableTransparencyDef{};
pub static ENABLE_TRANSPARENCY_MUT: EnableTransparencyMutDef = EnableTransparencyMutDef{};

#[derive(Debug,Clone)]
pub struct EnableTransparencyDef;
impl NodeDef for EnableTransparencyDef{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for EnableTransparencyDef{
    fn fqn(&self) -> &'static str{
        "imazen.enable_transparency"
    }
    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, p: NodeParams, parent: FrameInfo) -> Result<()> {
        if parent.fmt == PixelFormat::Bgra32{
            ctx.delete_node_and_snap_together(ix);
            Ok(())
        }else if parent.fmt == PixelFormat::Bgr32 {
            let mutate = ctx.graph
                .add_node(Node::n(&ENABLE_TRANSPARENCY_MUT, NodeParams::None));
            ctx.replace_node_with_existing(ix, mutate);
            Ok(())
        } else {
            let canvas_params = imageflow_types::Node::CreateCanvas {
                w: parent.h as usize,
                h: parent.w as usize,
                format: PixelFormat::Bgra32,
                color: imageflow_types::Color::Transparent,
            };
            let copy_rect_params = imageflow_types::Node::CopyRectToCanvas {
                from_x: 0,
                from_y: 0,
                w: parent.w as u32,
                h: parent.h as u32,
                x: 0,
                y: 0
            };
            let canvas = ctx.graph
                .add_node(Node::n(&CREATE_CANVAS,
                                  NodeParams::Json(canvas_params)));
            let copy = ctx.graph
                .add_node(Node::n(&COPY_RECT, NodeParams::Json(copy_rect_params)));
            ctx.graph.add_edge(canvas, copy, EdgeKind::Canvas).unwrap();
            ctx.replace_node_with_existing(ix, copy);
            Ok(())
        }
    }
}



#[derive(Debug, Clone)]
pub struct EnableTransparencyMutDef;
impl NodeDef for EnableTransparencyMutDef{
    fn as_one_mutate_bitmap(&self) -> Option<&dyn NodeDefMutateBitmap>{
        Some(self)
    }
}
impl NodeDefMutateBitmap for EnableTransparencyMutDef{
    fn fqn(&self) -> &'static str{
        "imazen.enable_transparency_mut"
    }
    fn mutate(&self, c: &Context, bitmap_key: BitmapKey,  p: &NodeParams) -> Result<()> {

        let bitmaps = c.borrow_bitmaps()
            .map_err(|e| e.at(here!()))?;
        let mut bitmap_bitmap = bitmaps.try_borrow_mut(bitmap_key)
            .map_err(|e| e.at(here!()))?;

        if bitmap_bitmap.info().alpha_meaningful(){
            Err(nerror!(crate::ErrorKind::InvalidNodeConnections, "Need Bgr32 input image to convert to bgra32"))
        }else{
            let mut bitmap = unsafe { bitmap_bitmap.get_window_u8().unwrap().to_bitmap_bgra()? };

            bitmap.normalize_alpha()?;

            bitmap_bitmap.set_alpha_meaningful(true);
            bitmap_bitmap.set_compositing(BitmapCompositing::BlendWithSelf);
            Ok(())
        }

    }
}