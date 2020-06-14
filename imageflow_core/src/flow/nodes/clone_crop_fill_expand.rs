use super::internal_prelude::*;


pub static COPY_RECT: CopyRectNodeDef = CopyRectNodeDef{};
pub static FILL_RECT: FillRectNodeDef = FillRectNodeDef{};
pub static CROP: MutProtect<CropMutNodeDef> = MutProtect{node: &CROP_MUTATE, fqn: "imazen.crop"};
pub static CROP_MUTATE: CropMutNodeDef = CropMutNodeDef{};
pub static CLONE: CloneDef = CloneDef{};
pub static EXPAND_CANVAS: ExpandCanvasDef = ExpandCanvasDef{};
pub static CROP_WHITESPACE: CropWhitespaceDef = CropWhitespaceDef{};
pub static REGION_PERCENT: RegionPercentDef = RegionPercentDef {};
pub static REGION: RegionDef = RegionDef {};

#[derive(Debug, Clone)]
pub struct CopyRectNodeDef;

impl NodeDef for CopyRectNodeDef {
    fn as_one_input_one_canvas(&self) -> Option<&dyn NodeDefOneInputOneCanvas> {
        Some(self)
    }
}

impl NodeDefOneInputOneCanvas for CopyRectNodeDef{
    fn fqn(&self) -> &'static str{
        "imazen.copy_rect_to_canvas"
    }
    fn validate_params(&self, p: &NodeParams) -> Result<()>{
        Ok(())
    }

    fn render(&self, c: &Context, canvas: &mut BitmapBgra, input: &mut BitmapBgra,  p: &NodeParams) -> Result<()> {
        if let NodeParams::Json(s::Node::CopyRectToCanvas { from_x, from_y,  w, h, x, y }) = *p {

            if input == canvas {
                return Err(nerror!(crate::ErrorKind::InvalidNodeConnections, "Canvas and Input are the same bitmap!"));
            }
            if input.w <= from_x || input.h <= from_y ||
                input.w < from_x + w ||
                input.h < from_y + h ||
                canvas.w < x + w ||
                canvas.h < y + h {
                return Err(nerror!(crate::ErrorKind::InvalidNodeParams, "Invalid coordinates. Canvas is {}x{}, Input is {}x{}, Params provided: {:?}",
                         canvas.w,
                         canvas.h,
                         input.w,
                         input.h,
                         p));
            }
            crate::graphics::copy_rect::copy_rect(input, canvas, from_x, from_y, x, y, w, h)?;
            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need CopyRectToCanvas, got {:?}", p))
        }
    }
}


#[derive(Debug, Clone)]
pub struct FillRectNodeDef;
impl NodeDef for FillRectNodeDef{
    fn as_one_mutate_bitmap(&self) -> Option<&dyn NodeDefMutateBitmap>{
        Some(self)
    }
}
impl NodeDefMutateBitmap for FillRectNodeDef {
    fn fqn(&self) -> &'static str {
        "imazen.fill_rect_mutate"
    }
    fn validate_params(&self, p: &NodeParams) -> Result<()> {
        Ok(())
    }
    fn mutate(&self, c: &Context, bitmap: &mut BitmapBgra,  p: &NodeParams) -> Result<()>{
        if let NodeParams::Json(s::Node::FillRect { x1, x2, y1, y2, ref color }) = *p{

            if x2 <= x1 || y2 <= y1 || (x1 as i32) < 0 || (y1 as i32) < 0 || x2 > bitmap.w || y2 > bitmap.h{
               return Err(nerror!(crate::ErrorKind::InvalidCoordinates, "Invalid coordinates for {}x{} bitmap: {:?}", bitmap.w, bitmap.h, p));
            }

            bitmap.compositing_mode = crate::ffi::BitmapCompositingMode::BlendWithSelf;
            unsafe {

                if !ffi::flow_bitmap_bgra_fill_rect(c.flow_c(),
                                                    bitmap as *mut BitmapBgra,
                                                    x1,
                                                    y1,
                                                    x2,
                                                    y2,
                                                    color.clone().to_u32_bgra().unwrap()) {
                    return Err(cerror!(c, "Failed to fill rectangle"))
                }else{
                    Ok(())
                }

            }

        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need FillRect, got {:?}", p))
        }
    }
}


#[derive(Debug,Clone)]
pub struct CloneDef;
impl NodeDef for CloneDef{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for CloneDef{
    fn fqn(&self) -> &'static str{
        "imazen.clone"
    }
    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, params: NodeParams, parent: FrameInfo) -> Result<()> {
        let parent = ctx.frame_info_from(ix, EdgeKind::Input).map_err(|e| e.at(here!()))?;

        let canvas_params = s::Node::CreateCanvas {
            w: parent.w as usize,
            h: parent.h as usize,
            format: s::PixelFormat::from(parent.fmt),
            color: s::Color::Transparent,
        };
        let copy_params = s::Node::CopyRectToCanvas {
            from_x: 0,
            from_y: 0,
            x: 0,
            y: 0,
            w: parent.w as u32,
            h: parent.h as u32,
        };
        let canvas = ctx.graph
            .add_node(Node::n(&CREATE_CANVAS, NodeParams::Json(canvas_params)));
        let copy = ctx.graph
            .add_node(Node::n(&COPY_RECT, NodeParams::Json(copy_params)));
        ctx.graph.add_edge(canvas, copy, EdgeKind::Canvas).unwrap();
        ctx.replace_node_with_existing(ix, copy);
        Ok(())
    }
}


#[derive(Debug,Clone)]
pub struct ExpandCanvasDef;
impl NodeDef for ExpandCanvasDef{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for ExpandCanvasDef{
    fn fqn(&self) -> &'static str{
        "imazen.expand_canvas"
    }
    fn estimate(&self, p: &NodeParams, input: FrameEstimate) -> Result<FrameEstimate> {
        if let NodeParams::Json(imageflow_types::Node::ExpandCanvas { left, top, bottom, right, ref color }) = *p {
            input.map_frame( |info| {
                Ok(FrameInfo {
                    w: info.w + left as i32 + right as i32,
                    h: info.h + top as i32 + bottom as i32,
                    fmt: if color.is_opaque() {
                        ffi::PixelFormat::from(info.fmt)
                    } else{
                        PixelFormat::Bgra32
                    }
                })
            })
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need ExpandCanvas, got {:?}", p))
        }
    }

    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, p: NodeParams, parent: FrameInfo) -> Result<()>{
        if let NodeParams::Json(s::Node::ExpandCanvas { left, top, bottom, right, color }) = p {
            let FrameInfo { w, h, fmt } = parent;

            let new_w = w as usize + left as usize + right as usize;
            let new_h = h as usize + top as usize + bottom as usize;
            let canvas_params = s::Node::CreateCanvas {
                w: new_w as usize,
                h: new_h as usize,
                format: if color.is_opaque() {
                    ffi::PixelFormat::from(fmt)
                } else{
                    PixelFormat::Bgra32
                },
                color: color.clone(),
            };
            let copy_params = s::Node::CopyRectToCanvas {
                from_x: 0,
                from_y: 0,
                x: left,
                y: top,
                w: w as u32,
                h: h as u32,
            };
            let canvas = ctx.graph
                .add_node(Node::n(&CREATE_CANVAS,
                                  NodeParams::Json(canvas_params)));
            let copy = ctx.graph
                .add_node(Node::n(&COPY_RECT, NodeParams::Json(copy_params)));
            ctx.graph.add_edge(canvas, copy, EdgeKind::Canvas).unwrap();
            ctx.replace_node_with_existing(ix, copy);
            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need ExpandCanvas, got {:?}", p))
        }
    }
}

#[derive(Debug,Clone)]
pub struct RegionPercentDef;
impl RegionPercentDef {
    fn get_coords(info:FrameInfo, left: f32, top: f32, right: f32, bottom: f32) -> (i32,i32,i32,i32){
        let (x1, y1, mut x2, mut y2) =
            ((info.w as f32 * left / 100f32).round() as i32,
            (info.h as f32 * top / 100f32).round() as i32,
            (info.w as f32 * right / 100f32).round() as i32,
             (info.h as f32 * bottom / 100f32).round() as i32);
        //Round up to 1px if our percentages land on the same pixel
        if x2 < x1 {
            x2 = x1 + 1;
        }
        if y2 < y1{
            y2 = y1 + 1;
        }
        //eprintln!("{}x{} {},{},{},{} -> {},{},{},{}", info.w, info.h, left, top, right, bottom, x1,y1,x2,y2);
        return (x1,y1,x2,y2);

    }
}
impl NodeDef for RegionPercentDef {
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for RegionPercentDef {
    fn fqn(&self) -> &'static str {
        "imazen.region_percent"
    }
    fn estimate(&self, p: &NodeParams, input: FrameEstimate) -> Result<FrameEstimate> {
        if let NodeParams::Json(imageflow_types::Node::RegionPercent { x1, y1, x2, y2, ref background_color }) = *p {
            input.map_frame(|info| {
                let (x1, y1, x2, y2) = RegionPercentDef::get_coords(info, x1, y1, x2, y2);
                Ok(FrameInfo {
                    w: x2 - x1,
                    h: y2 - y1,
                    fmt: ffi::PixelFormat::from(info.fmt)
                })
            })
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need RegionPercent, got {:?}", p))
        }
    }
    //TODO: If we want to support transparency on jpeg inputs we have to fix expand_canvas and copy_rect too
    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, p: NodeParams, input: FrameInfo) -> Result<()> {
        if let NodeParams::Json(imageflow_types::Node::RegionPercent { x1: left, y1: top, y2: bottom, x2: right, background_color }) = p {
            if bottom <= top || right <= left {
                return Err(nerror!(crate::ErrorKind::InvalidNodeParams, "Invalid coordinates: {},{} {},{} should describe the top-left and bottom-right corners of the region in percentages. Not a rectangle.", left, top, right, bottom));
            }

            let (x1, y1, x2, y2) = RegionPercentDef::get_coords(input, left, top, right, bottom);

            let region_params = imageflow_types::Node::Region {
                x1,
                y1,
                x2,
                y2,
                background_color: background_color.clone()
            };

            //First crop, then expand
            ctx.replace_node(ix, vec![
                Node::n(&REGION,
                        NodeParams::Json(region_params)),
            ]);


            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need RegionPercent, got {:?}", p))
        }
    }
}

#[derive(Debug,Clone)]
pub struct RegionDef;
impl NodeDef for RegionDef {
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for RegionDef {
    fn fqn(&self) -> &'static str{
        "imazen.region"
    }
    fn estimate(&self, p: &NodeParams, input: FrameEstimate) -> Result<FrameEstimate> {
        if let NodeParams::Json(imageflow_types::Node::Region { x1, y1, x2, y2, ref background_color }) = *p {
            input.map_frame( |info| {
                if y2 <= y1 || x2 <= x1 {
                    return Err(nerror!(crate::ErrorKind::InvalidNodeParams, "Invalid coordinates: {},{} {},{} should describe the top-left and bottom-right corners of the region in pixels. Not a rectangle.", x1, y1, x2, y2));
                }
                Ok(FrameInfo {
                    w: x2 - x1,
                    h: y2 - y1,
                    fmt: ffi::PixelFormat::from(info.fmt)
                })
            })
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need Region, got {:?}", p))
        }
    }
    //TODO: If we want to support transparency on jpeg inputs we have to fix expand_canvas and copy_rect too
    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, p: NodeParams, input: FrameInfo) -> Result<()>{
        if let NodeParams::Json(imageflow_types::Node::Region { x1, y1, y2, x2, background_color }) = p {
            if y2 <= y1 || x2 <= x1 {
                return Err(nerror!(crate::ErrorKind::InvalidNodeParams, "Invalid coordinates: {},{} {},{} should describe the top-left and bottom-righ corners of the region in pixels. Not a rectangle.", x1, y1, x2, y2));
            }

            if x1 >= input.w || y1 >= input.h || x2 <= 0 || y2 <= 0{
                // No cropping of input image, we just create a canvas
                let canvas_params = s::Node::CreateCanvas {
                    w: (x2-x1) as usize,
                    h: (y2-y1) as usize,
                    format: s::PixelFormat::from(input.fmt),
                    color: background_color.clone(),
                };
                let canvas = ctx.graph
                    .add_node(Node::n(&CREATE_CANVAS,
                                      NodeParams::Json(canvas_params)));

                ctx.copy_edges_to(ix, canvas, EdgeDirection::Outgoing);
                ctx.graph.remove_node(ix).unwrap();
            }else{
                let crop_params = imageflow_types::Node::Crop {
                    x1: i32::min(input.w, i32::max(0,x1)) as u32,
                    y1: i32::min(input.h, i32::max(0,y1)) as u32,
                    x2: i32::min(input.w, i32::max(0,x2)) as u32,
                    y2: i32::min(input.h, i32::max(0,y2)) as u32
                };
                let expand_params = imageflow_types::Node::ExpandCanvas {
                    left: i32::max(0, 0 - x1)  as u32,
                    top: i32::max(0, 0 - y1)  as u32,
                    right: i32::max(0, x2 - input.w) as u32,
                    bottom: i32::max(0, y2 - input.h) as u32,
                    color: background_color.clone()
                };

                //First crop, then expand
                ctx.replace_node(ix, vec![
                    Node::n(&CROP,
                            NodeParams::Json(crop_params)),
                    Node::n(&EXPAND_CANVAS,
                            NodeParams::Json(expand_params)),
                ]);
            }

            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need Region, got {:?}", p))
        }
    }
}
#[derive(Debug, Clone)]
pub struct CropMutNodeDef;

impl CropMutNodeDef {
    fn est_validate(&self, p: &NodeParams, input_est: FrameEstimate) -> Result<FrameEstimate> {
        if let NodeParams::Json(s::Node::Crop {  x1,  y1,  x2,  y2 }) = *p {
            if (x1 as i32) < 0 || (y1 as i32) < 0 || x2 <= x1 || y2 <= y1 {
                Err(nerror!(crate::ErrorKind::InvalidNodeParams, "Invalid coordinates: {},{} {},{} should describe the top-left and bottom-right corners of a rectangle", x1,y1,x2,y2))
            } else if let FrameEstimate::Some(input) = input_est {
                if x2 > input.w as u32 || y2 > input.h as u32 {
                    Err(nerror!(crate::ErrorKind::InvalidNodeParams, "Crop coordinates {},{} {},{} invalid for {}x{} bitmap", x1,y1,x2,y2, input.w, input.h))
                } else {
                    Ok(FrameEstimate::Some(FrameInfo {
                        w: x2 as i32 - x1 as i32,
                        h: y2 as i32 - y1 as i32,
                        fmt: ffi::PixelFormat::from(input.fmt),
                    }))
                }
                //TODO: we can estimate with other FrameEstimate values
            } else {
                Ok(FrameEstimate::None)
            }
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need Crop, got {:?}", p))
        }
    }
}

impl NodeDef for CropMutNodeDef{
    fn fqn(&self) -> &'static str{
        "imazen.crop_mutate"
    }
    fn edges_required(&self, p: &NodeParams) -> Result<(EdgesIn, EdgesOut)>{
        Ok((EdgesIn::OneInput, EdgesOut::Any))
    }

    fn validate_params(&self, p: &NodeParams) -> Result<()> {
        self.est_validate(p, FrameEstimate::None).map(|_| ()).map_err(|e| e.at(here!()))
    }

    fn estimate(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<FrameEstimate> {
        let input_est = ctx.frame_est_from(ix, EdgeKind::Input).map_err(|e| e.at(here!()))?;
        let p = &ctx.weight(ix).params;
        self.est_validate(p, input_est).map_err(|e| e.at(here!()))
    }

    fn can_execute(&self) -> bool { true }

    fn execute(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<NodeResult> {
        let input = unsafe {
            &mut *ctx.bitmap_bgra_from(ix, EdgeKind::Input).map_err(|e| e.at(here!()).with_ctx_mut(ctx, ix))?
        };
        ctx.consume_parent_result(ix, EdgeKind::Input)?;

        // Validate against actual bitmap
        let _ = self.est_validate(&ctx.weight(ix).params, FrameEstimate::Some(input.frame_info())).map_err(|e| e.at(here!()))?;

        if let NodeParams::Json(s::Node::Crop { x1, x2, y1, y2 }) = ctx.weight(ix).params {
            // println!("Cropping {}x{} to ({},{}) ({},{})", (*input).w, (*input).h, x1, y1, x2, y2);

            let (w, h) = (input.w, input.h);
            if x2 <= x1 || y2 <= y1 || x2 > w || y2 > h {
                panic!("Invalid crop bounds {:?} (image {}x{})", ((x1, y1), (x2, y2)), w, h);
            }


            unsafe {
                let offset = input.stride as isize * y1 as isize +
                    input.fmt.bytes() as isize * x1 as isize;
                input.pixels = input.pixels.offset(offset);
            }
            input.w = x2 - x1;
            input.h = y2 - y1;


            Ok(NodeResult::Frame(input))
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need Crop, got {:?}", &ctx.weight(ix).params))
        }
    }
}

#[derive(Debug,Clone)]
pub struct CropWhitespaceDef;
impl NodeDef for CropWhitespaceDef{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for CropWhitespaceDef {
    fn fqn(&self) -> &'static str {
        "imazen.crop_whitespace"
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
                    if let Some(rect) = crate::graphics::whitespace::detect_content(bit_ref, threshold) {
                        if rect.x2 <= rect.x1 || rect.y2 <= rect.y1 {
                            return Err(nerror!(crate::ErrorKind::InvalidState, "Whitespace detection returned invalid rectangle"));
                        }
                        let padding = (percent_padding * (rect.x2 - rect.x1 + rect.y2 - rect.y1) as f32 / 2f32).ceil() as i64;
                        Ok((cmp::max(0, rect.x1 as i64  - padding) as u32, cmp::max(0, rect.y1 as i64 - padding) as u32,
                            cmp::min(bit_ref.w as i64, rect.x2 as i64 + padding) as u32, cmp::min(bit_ref.h as i64, rect.y2 as i64 + padding) as u32))
                    } else {
                        return Err(nerror!(crate::ErrorKind::InvalidState, "Failed to complete whitespace detection"));
                    }
                },
                other => { Err(nerror!(crate::ErrorKind::InvalidOperation, "Cannot CropWhitespace without a parent bitmap; got {:?}", other)) }
            }?;
            ctx.replace_node(ix, vec![
                Node::n(&CROP,
                        NodeParams::Json(s::Node::Crop { x1, y1, x2, y2 }))
            ]);


            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need CropWhitespace, got {:?}", p))
        }
    }
}

