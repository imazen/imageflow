use super::internal_prelude::*;


pub static CREATE_CANVAS: CreateCanvasNodeDef = CreateCanvasNodeDef{};


#[derive(Debug,Clone)]
pub struct CreateCanvasNodeDef{}

impl CreateCanvasNodeDef{
    fn get(&self, n: &NodeParams) -> Result<(usize, usize, PixelFormat, s::Color)>{
        if let &NodeParams::Json(s::Node::CreateCanvas { format,
                                    w,
                                    h,
                                    ref color }) = n{
            let max_dimension = 2000000; // 2million

            if w < 1 || w > max_dimension {
                Err(nerror!(::ErrorKind::InvalidCoordinates, "canvas width ({}) must be greater than zero and less than {}.", w, max_dimension))
            } else if h < 1 || h > max_dimension {
                Err(nerror!(::ErrorKind::InvalidCoordinates, "canvas height ({}) must be greater than zero and less than {}.", w, max_dimension))
            } else if h * w > 100000000 {
                Err(nerror!(::ErrorKind::InvalidCoordinates, "canvas size ({}) cannot exceed 100 megapixels.", w))
            } else if format == ffi::PixelFormat::Gray8 {
                Err(nerror!(::ErrorKind::InvalidNodeParams, "canvas format cannot be grayscale; single-channel grayscale bitmaps are not yet supported in Imageflow"))
            }else if format != ffi::PixelFormat::Bgr24 && format != ffi::PixelFormat::Bgr32 && format != ffi::PixelFormat::Bgra32{
                Err(nerror!(::ErrorKind::InvalidNodeParams, "canvas format {:?} not recognized", format))
            } else {
                Ok((w,h,format,color.clone()))
            }
        }else {
            Err(nerror!(::ErrorKind::NodeParamsMismatch))
        }
    }
}
impl NodeDef for CreateCanvasNodeDef {
    fn fqn(&self) -> &'static str {
        "imazen.create_canvas"
    }

    fn edges_required(&self, p: &NodeParams) -> Result<(EdgesIn, EdgesOut)> {
        Ok((EdgesIn::NoInput, EdgesOut::Any))
    }

    fn validate_params(&self, n: &NodeParams) -> Result<()> {
        self.get(n).map(|_| ())
    }

    fn estimate(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<FrameEstimate> {
        self.get(&ctx.weight(ix).params).map(|(w, h, format, _)| {
            FrameEstimate::Some(FrameInfo {
                w: w as i32,
                h: h as i32,
                fmt: format,
            })
        })
    }
    fn can_execute(&self) -> bool{
        true
    }
    fn execute(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<NodeResult> {
        match self.get(&ctx.weight(ix).params) {
            Ok((w, h, format, color)) => {
                let flow_pointer = ctx.flow_c();
                let weight = &mut ctx.weight_mut(ix);

                unsafe {

                    let ptr =
                        ::ffi::flow_bitmap_bgra_create(flow_pointer, w as i32, h as i32, true, format);
                    if ptr.is_null() {
                        return Err(cerror!(ctx.c, "Failed to allocate {}x{}x{} bitmap ({} bytes). Reduce dimensions or increase RAM.", w, h, format.bytes(), w * h * format.bytes()))
                    }
                    let color_val = color.clone();
                    let color_srgb_argb = color_val.clone().to_u32_bgra().unwrap();
                    (*ptr).compositing_mode = ::ffi::BitmapCompositingMode::ReplaceSelf;
                    if color_val != s::Color::Transparent {
                        if !ffi::flow_bitmap_bgra_fill_rect(flow_pointer,
                                                            ptr,
                                                            0,
                                                            0,
                                                            w as u32,
                                                            h as u32,
                                                            color_srgb_argb) {
                            return Err(cerror!(ctx.c, "Failed to fill rectangle"))

                        }
                        (*ptr).compositing_mode = ::ffi::BitmapCompositingMode::BlendWithMatte;
                    }

                    (*ptr).matte_color = mem::transmute(color_srgb_argb);


                    weight.result = NodeResult::Frame(ptr);

                    Ok(NodeResult::Frame(ptr))
                }
            },
            Err(e) => Err(e)
        }
    }
}


