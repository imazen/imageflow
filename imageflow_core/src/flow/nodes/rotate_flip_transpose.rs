use super::internal_prelude::*;

pub static FLIP_V_PRIMITIVE: FlipVerticalMutNodeDef = FlipVerticalMutNodeDef{} ;
pub static FLIP_H_PRIMITIVE: FlipHorizontalMutNodeDef = FlipHorizontalMutNodeDef{};
pub static FLIP_V: MutProtect<FlipVerticalMutNodeDef> = MutProtect{ node: &FLIP_V_PRIMITIVE, fqn: "imazen.flip_vertical"};
pub static FLIP_H: MutProtect<FlipHorizontalMutNodeDef> = MutProtect{ node: &FLIP_H_PRIMITIVE, fqn: "imazen.flip_horizontal"};
pub static APPLY_ORIENTATION: ApplyOrientationDef = ApplyOrientationDef{};
pub static NO_OP: NoOpDef = NoOpDef{};
pub static ROTATE_90: Rotate90Def = Rotate90Def{};
pub static ROTATE_180: Rotate180Def = Rotate180Def{};
pub static ROTATE_270: Rotate270Def = Rotate270Def{};
pub static TRANSPOSE: TransposeDef = TransposeDef{};
pub static TRANSPOSE_MUT: TransposeMutDef = TransposeMutDef{};



#[derive(Debug,Clone)]
pub struct ApplyOrientationDef;
impl NodeDef for ApplyOrientationDef{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for ApplyOrientationDef{
    fn fqn(&self) -> &'static str{
        "imazen.apply_orientation"
    }
    fn estimate(&self, p: &NodeParams, input: FrameEstimate) -> Result<FrameEstimate> {
        if let NodeParams::Json(s::Node::ApplyOrientation { flag }) = *p {
            input.map_frame(|info| {
                let swap = flag >= 5 && flag <= 8;
                Ok(FrameInfo {
                    w: if swap {
                        info.h
                    } else { info.w },
                    h: if swap {
                        info.w
                    } else { info.h },
                    ..info
                })
            })
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need ApplyOrientation, got {:?}", p))
        }
    }

    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, p: NodeParams, parent: FrameInfo) -> Result<()>{
        if let NodeParams::Json(s::Node::ApplyOrientation { flag }) = p {
            let replacement_nodes: Vec<&'static dyn NodeDef> = match flag {
                7 => vec![&ROTATE_180, &TRANSPOSE],
                8 => vec![&ROTATE_90],
                6 => vec![&ROTATE_270],
                5 => vec![&TRANSPOSE],
                4 => vec![&FLIP_V],
                3 => vec![&ROTATE_180],
                2 => vec![&FLIP_H],
                _ => vec![],
            };
            ctx.replace_node(ix,
                             replacement_nodes.iter()
                                 .map(|v| Node::n(*v, NodeParams::None))
                                 .collect());
            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need ApplyOrientation, got {:?}", p))
        }
    }
}


#[derive(Debug,Clone)]
pub struct TransposeDef;
impl NodeDef for TransposeDef{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for TransposeDef {
    fn fqn(&self) -> &'static str {
        "imazen.transpose"
    }
    fn estimate(&self, p: &NodeParams, input: FrameEstimate) -> Result<FrameEstimate> {
        Ok(input.transpose())
    }

    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, p: NodeParams, parent: FrameInfo) -> Result<()> {
        let canvas_params = s::Node::CreateCanvas {
            w: parent.h as usize,
            h: parent.w as usize,
            format: s::PixelFormat::from(parent.fmt),
            color: s::Color::Transparent,
        };
        let canvas = ctx.graph
            .add_node(Node::n(&CREATE_CANVAS,
                              NodeParams::Json(canvas_params)));
        let copy = ctx.graph
            .add_node(Node::n(&TRANSPOSE_MUT, NodeParams::None));
        ctx.graph.add_edge(canvas, copy, EdgeKind::Canvas).unwrap();
        ctx.replace_node_with_existing(ix, copy);
        Ok(())
    }
}

#[derive(Debug,Clone)]
pub struct NoOpDef;
impl NodeDef for NoOpDef{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for NoOpDef {
    fn fqn(&self) -> &'static str {
        "imazen.noop"
    }
    fn estimate(&self, p: &NodeParams, input: FrameEstimate) -> Result<FrameEstimate> {
        Ok(input)
    }
    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, p: NodeParams, parent: FrameInfo) -> Result<()> {
        ctx.replace_node(ix, vec![]);
        Ok(())
    }
}


#[derive(Debug, Clone)]
pub struct TransposeMutDef;

impl NodeDef for TransposeMutDef {
    fn as_one_input_one_canvas(&self) -> Option<&dyn NodeDefOneInputOneCanvas> {
        Some(self)
    }
}

impl NodeDefOneInputOneCanvas for TransposeMutDef {
    fn fqn(&self) -> &'static str {
        "imazen.transpose_mut"
    }
    fn validate_params(&self, p: &NodeParams) -> Result<()> {
        Ok(())
    }

    fn render(&self, c: &Context, canvas: &mut BitmapBgra, input: &mut BitmapBgra, p: &NodeParams) -> Result<()> {
        unsafe {
            if input.fmt != canvas.fmt {
                panic!("Can't copy between bitmaps with different pixel formats")
            }
            if input == canvas {
                panic!("Canvas and input must be different bitmaps for transpose to work!")
            }

            if !crate::ffi::flow_bitmap_bgra_transpose(c.flow_c(), input as *mut BitmapBgra, canvas as *mut BitmapBgra) {
                panic!("Failed to transpose bitmap")
            }
        }
        Ok(())
    }
}


#[derive(Debug, Clone)]
pub struct FlipVerticalMutNodeDef;
impl NodeDef for FlipVerticalMutNodeDef{
    fn as_one_mutate_bitmap(&self) -> Option<&dyn NodeDefMutateBitmap>{
        Some(self)
    }
}
impl NodeDefMutateBitmap for FlipVerticalMutNodeDef{
    fn fqn(&self) -> &'static str{
        "imazen.flip_vertical_mutate"
    }
    fn mutate(&self, c: &Context, bitmap: &mut BitmapBgra,  p: &NodeParams) -> Result<()>{
        unsafe {
            if !crate::ffi::flow_bitmap_bgra_flip_vertical(c.flow_c(), bitmap as *mut BitmapBgra){
                return Err(cerror!(c))
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct FlipHorizontalMutNodeDef;
impl NodeDef for FlipHorizontalMutNodeDef{
    fn as_one_mutate_bitmap(&self) -> Option<&dyn NodeDefMutateBitmap>{
        Some(self)
    }
}
impl NodeDefMutateBitmap for FlipHorizontalMutNodeDef{
    fn fqn(&self) -> &'static str{
        "imazen.flip_vertical_mutate"
    }
    fn mutate(&self, c: &Context, bitmap: &mut BitmapBgra,  p: &NodeParams) -> Result<()>{
        unsafe {
            if !crate::ffi::flow_bitmap_bgra_flip_horizontal(c.flow_c(), bitmap as *mut BitmapBgra){
                return Err(cerror!(c))
            }
        }
        Ok(())
    }
}

#[derive(Debug,Clone)]
pub struct Rotate90Def;
impl NodeDef for Rotate90Def{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for Rotate90Def {
    fn fqn(&self) -> &'static str {
        "imazen.rotate_90"
    }
    fn estimate(&self, p: &NodeParams, input: FrameEstimate) -> Result<FrameEstimate> {
        Ok(input.transpose())
    }

    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, p: NodeParams, parent: FrameInfo) -> Result<()> {
        ctx.replace_node(ix,
                         vec![
                             Node::n(&TRANSPOSE, NodeParams::None),
                             Node::n(&FLIP_V, NodeParams::None),
                         ]);
        Ok(())
    }
}


#[derive(Debug,Clone)]
pub struct Rotate270Def;
impl NodeDef for Rotate270Def{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for Rotate270Def {
    fn fqn(&self) -> &'static str {
        "imazen.rotate_270"
    }
    fn estimate(&self, p: &NodeParams, input: FrameEstimate) -> Result<FrameEstimate> {
        Ok(input.transpose())
    }

    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, p: NodeParams, parent: FrameInfo) -> Result<()> {
        ctx.replace_node(ix,
                         vec![
                             Node::n(&FLIP_V, NodeParams::None),
                             Node::n(&TRANSPOSE, NodeParams::None),
                         ]);
        Ok(())
    }
}

#[derive(Debug,Clone)]
pub struct Rotate180Def;
impl NodeDef for Rotate180Def{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for Rotate180Def {
    fn fqn(&self) -> &'static str {
        "imazen.rotate_180"
    }
    fn estimate(&self, p: &NodeParams, input: FrameEstimate) -> Result<FrameEstimate> {
        Ok(input)
    }

    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, p: NodeParams, parent: FrameInfo) -> Result<()> {
        ctx.replace_node(ix,
                         vec![
                             Node::n(&FLIP_V as &dyn NodeDef, NodeParams::None),
                             Node::n(&FLIP_H, NodeParams::None),
                         ]);
        Ok(())
    }
}
