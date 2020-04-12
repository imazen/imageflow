use super::internal_prelude::*;


pub static CREATE_CANVAS: CreateCanvasNodeDef = CreateCanvasNodeDef{};


#[derive(Debug,Clone)]
pub struct CreateCanvasNodeDef{}

impl CreateCanvasNodeDef{
    fn get(&self, n: &NodeParams) -> Result<(usize, usize, PixelFormat, s::Color)>{
        if let NodeParams::Json(s::Node::CreateCanvas { format,
                                    w,
                                    h,
                                    ref color }) = *n{
            let max_dimension = 2_000_000; // 2million

            if w < 1 || w > max_dimension {
                Err(nerror!(crate::ErrorKind::InvalidCoordinates, "canvas width ({}) must be greater than zero and less than {}.", w, max_dimension))
            } else if h < 1 || h > max_dimension {
                Err(nerror!(crate::ErrorKind::InvalidCoordinates, "canvas height ({}) must be greater than zero and less than {}.", w, max_dimension))
            } else if h * w > 100_000_000 {
                Err(nerror!(crate::ErrorKind::InvalidCoordinates, "canvas size ({}) cannot exceed 100 megapixels.", w))
            } else if format == ffi::PixelFormat::Gray8 {
                Err(nerror!(crate::ErrorKind::InvalidNodeParams, "canvas format cannot be grayscale; single-channel grayscale bitmaps are not yet supported in Imageflow"))
            }else if format == ffi::PixelFormat::Bgr24{
                Err(nerror!(crate::ErrorKind::InvalidNodeParams, "canvas format {:?} not permitted. Use Bgr32 instead", format))
            }else if format != ffi::PixelFormat::Bgr24 && format != ffi::PixelFormat::Bgr32 && format != ffi::PixelFormat::Bgra32 {
                Err(nerror!(crate::ErrorKind::InvalidNodeParams, "canvas format {:?} not recognized", format))
            } else {
                Ok((w,h,format,color.clone()))
            }
        }else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch))
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
                let ptr = BitmapBgra::create(ctx.c, w as u32, h as u32, format, color)?;

                let weight = &mut ctx.weight_mut(ix);
                weight.result = NodeResult::Frame(ptr);
                Ok(NodeResult::Frame(ptr))
            },
            Err(e) => Err(e)
        }
    }
}


