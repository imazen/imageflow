use super::internal_prelude::*;


pub static COPY_RECT: CopyRectNodeDef = CopyRectNodeDef{};
pub static FILL_RECT: FillRectNodeDef = FillRectNodeDef{};
pub static CROP: MutProtect<CropMutNodeDef> = MutProtect{node: &CROP_MUTATE, fqn: "imazen.crop"};
pub static CROP_MUTATE: CropMutNodeDef = CropMutNodeDef{};
pub static CLONE: CloneDef = CloneDef{};
pub static EXPAND_CANVAS: ExpandCanvasDef = ExpandCanvasDef{};
pub static CROP_WHITESPACE: CropWhitespaceDef = CropWhitespaceDef{};


#[derive(Debug, Clone)]
pub struct CopyRectNodeDef;

impl NodeDef for CopyRectNodeDef {
    fn as_one_input_one_canvas(&self) -> Option<&NodeDefOneInputOneCanvas> {
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
        if let &NodeParams::Json(s::Node::CopyRectToCanvas { from_x, from_y, width, height, x, y }) = p {


            if input.fmt != canvas.fmt {
                return Err(nerror!(::ErrorKind::InvalidNodeConnections, "Canvas pixel format {:?} differs from Input pixel format {:?}.", input.fmt, canvas.fmt));
            }
            if input == canvas {
                return Err(nerror!(::ErrorKind::InvalidNodeConnections, "Canvas and Input are the same bitmap!"));
            }

            if input.w <= from_x || input.h <= from_y ||
                input.w < from_x + width ||
                input.h < from_y + height ||
                canvas.w < x + width ||
                canvas.h < y + height {
                return Err(nerror!(::ErrorKind::InvalidNodeParams, "Invalid coordinates. Canvas is {}x{}, Input is {}x{}, Params provided: {:?}",
                         canvas.w,
                         canvas.h,
                         input.w,
                         input.h,
                         p));
            }

            let bytes_pp = input.fmt.bytes() as u32;
            if from_x == 0 && x == 0 && width == input.w && width == canvas.w &&
                input.stride == canvas.stride {
                //This optimization has the side effect of copying irrelevant data, so we don't want to do it if windowed, only
                // if padded or permanently cropped.
                unsafe {
                    let from_offset = input.stride * from_y;
                    let from_ptr = input.pixels.offset(from_offset as isize);
                    let to_offset = canvas.stride * y;
                    let to_ptr = canvas.pixels.offset(to_offset as isize);
                    ptr::copy_nonoverlapping(from_ptr, to_ptr, (input.stride * height) as usize);
                }
            } else {
                for row in 0..height {
                    unsafe {
                        let from_offset = input.stride * (from_y + row) + bytes_pp * from_x;
                        let from_ptr = input.pixels.offset(from_offset as isize);
                        let to_offset = canvas.stride * (y + row) + bytes_pp * x;
                        let to_ptr = canvas.pixels.offset(to_offset as isize);

                        ptr::copy_nonoverlapping(from_ptr, to_ptr, (width * bytes_pp) as usize);
                    }
                }
            }
            Ok(())
        } else {
            Err(nerror!(::ErrorKind::NodeParamsMismatch, "Need CopyRectToCanvas, got {:?}", p))
        }
    }
}


#[derive(Debug, Clone)]
pub struct FillRectNodeDef;
impl NodeDef for FillRectNodeDef{
    fn as_one_mutate_bitmap(&self) -> Option<&NodeDefMutateBitmap>{
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
        if let &NodeParams::Json(s::Node::FillRect { x1, x2, y1, y2, ref color }) = p{

            if x2 <= x1 || y2 <= y1 || (x1 as i32) < 0 || (y1 as i32) < 0 || x2 > bitmap.w || y2 > bitmap.h{
               return Err(nerror!(::ErrorKind::InvalidCoordinates, "Invalid coordinates for {}x{} bitmap: {:?}", bitmap.w, bitmap.h, p));
            }
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
            Err(nerror!(::ErrorKind::NodeParamsMismatch, "Need FillRect, got {:?}", p))
        }
    }
}


#[derive(Debug,Clone)]
pub struct CloneDef;
impl NodeDef for CloneDef{
    fn as_one_input_expand(&self) -> Option<&NodeDefOneInputExpand>{
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
            width: parent.w as u32,
            height: parent.h as u32,
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
    fn as_one_input_expand(&self) -> Option<&NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for ExpandCanvasDef{
    fn fqn(&self) -> &'static str{
        "imazen.expand_canvas"
    }
    fn estimate(&self, p: &NodeParams, input: FrameEstimate) -> Result<FrameEstimate> {
        if let &NodeParams::Json(s::Node::ExpandCanvas { left, top, bottom, right, ref color }) = p {
            input.map_frame( |info| {
                Ok(FrameInfo {
                    w: info.w + left as i32 + right as i32,
                    h: info.h + top as i32 + bottom as i32,
                    fmt: ffi::PixelFormat::from(info.fmt)
                })
            })
        } else {
            Err(nerror!(::ErrorKind::NodeParamsMismatch, "Need ExpandCanvas, got {:?}", p))
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
                format: s::PixelFormat::from(fmt),
                color: color.clone(),
            };
            let copy_params = s::Node::CopyRectToCanvas {
                from_x: 0,
                from_y: 0,
                x: left,
                y: top,
                width: w as u32,
                height: h as u32,
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
            Err(nerror!(::ErrorKind::NodeParamsMismatch, "Need ExpandCanvas, got {:?}", p))
        }
    }
}


#[derive(Debug, Clone)]
pub struct CropMutNodeDef;

impl CropMutNodeDef {
    fn est_validate(&self, p: &NodeParams, input_est: FrameEstimate) -> Result<FrameEstimate> {
        if let &NodeParams::Json(s::Node::Crop {  x1,  y1,  x2,  y2 }) = p {
            if (x1 as i32) < 0 || (y1 as i32) < 0 || x2 <= x1 || y2 <= y1 {
                Err(nerror!(::ErrorKind::InvalidNodeParams, "Invalid coordinates: {},{} {},{} should describe the top-left and bottom-right corners of a rectangle", x1,y1,x2,y2))
            } else if let FrameEstimate::Some(input) = input_est {
                if x2 > input.w as u32 || y2 > input.h as u32 {
                    Err(nerror!(::ErrorKind::InvalidNodeParams, "Crop coordinates {},{} {},{} invalid for {}x{} bitmap", x1,y1,x2,y2, input.w, input.h))
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
            Err(nerror!(::ErrorKind::NodeParamsMismatch, "Need Crop, got {:?}", p))
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
        let mut input = unsafe {
            &mut *ctx.bitmap_bgra_from(ix, EdgeKind::Input).map_err(|e| e.at(here!()).with_ctx_mut(ctx, ix))?
        };
        ctx.consume_parent_result(ix, EdgeKind::Input)?;

        // Validate against actual bitmap
        let _ = self.est_validate(&ctx.weight(ix).params, FrameEstimate::Some(input.frame_info())).map_err(|e| e.at(here!()))?;

        if let s::Node::Crop { x1, x2, y1, y2 } = ctx.get_json_params(ix).unwrap() {
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
            unreachable!(loc!());
        }
    }
}

#[derive(Debug,Clone)]
pub struct CropWhitespaceDef;
impl NodeDef for CropWhitespaceDef{
    fn as_one_input_expand(&self) -> Option<&NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for CropWhitespaceDef {
    fn fqn(&self) -> &'static str {
        "imazen.crop_whitespace"
    }
    fn estimate(&self, p: &NodeParams, input: FrameEstimate) -> Result<FrameEstimate> {
        Ok((FrameEstimate::Impossible))
    }
//TODO: mark as risky
    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, p: NodeParams, b: FrameInfo) -> Result<()> {
        // detect bounds, increase, and replace with crop
        if let NodeParams::Json(s::Node::CropWhitespace { threshold, percent_padding }) = p {
            // detect bounds, increase, and replace with crop
            let (x1, y1, x2, y2) = match ctx.first_parent_input_weight(ix).unwrap().result {
                NodeResult::Frame(b) => {
                    unsafe {
                        let rect = ::ffi::detect_content(ctx.c.flow_c(), b, threshold);
                        if rect == ::ffi::Rect::failure(){
                            return Err(cerror!(ctx.c, "Failed to complete whitespace detection"));
                        }
                        if rect.x2 <= rect.x1 || rect.y2 <= rect.y1{
                            return Err(nerror!(::ErrorKind::InvalidState, "Whitespace detection returned invalid rectangle"));
                        }
                        let padding = (percent_padding * (rect.x2 - rect.x1 + rect.y2 - rect.y1) as f32 / 2f32).ceil() as i32;
                        Ok((cmp::max(0, rect.x1 - padding) as u32, cmp::max(0, rect.y1 - padding) as u32,
                            cmp::min((*b).w as i32, rect.x2 + padding) as u32, cmp::min((*b).h as i32, rect.y2 + padding) as u32))
                    }
                },
                other => { Err(nerror!(::ErrorKind::InvalidOperation, "Cannot CropWhitespace without a parent bitmap; got {:?}", other)) }
            }?;
            ctx.replace_node(ix, vec![
                Node::n(&CROP,
                        NodeParams::Json(s::Node::Crop { x1: x1, y1: y1, x2: x2, y2: y2 }))
            ]);


            Ok(())
        } else {
            Err(nerror!(::ErrorKind::NodeParamsMismatch, "Need CropWhitespace, got {:?}", p))
        }
    }
}

