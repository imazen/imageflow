use super::internal_prelude::*;

pub static SCALE_2D_RENDER_TO_CANVAS_1D: Scale2dDef = Scale2dDef{};
pub static SCALE: ScaleDef = ScaleDef{};
pub static DRAW_IMAGE_EXACT: DrawImageDef = DrawImageDef{};


#[derive(Debug,Clone)]
pub struct ScaleDef;
impl NodeDef for ScaleDef{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for ScaleDef {
    fn fqn(&self) -> &'static str {
        "imazen.scale"
    }
    fn estimate(&self, params: &NodeParams, input: FrameEstimate) -> Result<FrameEstimate> {
        if let NodeParams::Json(s::Node::Resample2D { w, h, .. }) = *params {
            input.map_frame(|info| {
                Ok(FrameInfo {
                    w: w as i32,
                    h: h as i32,
                    fmt: ffi::PixelFormat::from(info.fmt),
                })
            })
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need Resample2D, got {:?}",params))
        }
    }
    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, params: NodeParams, parent: FrameInfo) -> Result<()> {
        if let NodeParams::Json(s::Node::Resample2D { w, h, hints }) =
        params {
            let resample_when = hints.as_ref().and_then(|ref h| h.resample_when).unwrap_or(s::ResampleWhen::SizeDiffersOrSharpeningRequested);


            let size_differs = w != parent.w as u32 || h != parent.h as u32;
            let downscaling = w < parent.w as u32 || h < parent.h as u32;
            let upscaling = w > parent.w as u32 || h > parent.h as u32;

            let sharpen_percent_raw = hints.as_ref().and_then(|h| h.sharpen_percent).unwrap_or(0f32);

            let sharpen_percent = match hints.as_ref()
                .and_then(|h| h.sharpen_when)
                .unwrap_or(s::SharpenWhen::Always){
                imageflow_types::SharpenWhen::Always => sharpen_percent_raw,
                imageflow_types::SharpenWhen::Downscaling if downscaling => sharpen_percent_raw,
                imageflow_types::SharpenWhen::Upscaling if upscaling => sharpen_percent_raw,
                imageflow_types::SharpenWhen::SizeDiffers if size_differs => sharpen_percent_raw,
                _ => 0f32
            };

            let sharpen_requested = sharpen_percent != 0f32;

            let resample = match resample_when {
                s::ResampleWhen::Always => true,
                s::ResampleWhen::SizeDiffers if size_differs => true,
                s::ResampleWhen::SizeDiffersOrSharpeningRequested if size_differs || sharpen_requested => true,
                _ => false
            };

            if resample {
                let scale2d_params = imageflow_types::Node::Resample2D {
                    w,
                    h,
                    hints: Some(imageflow_types::ResampleHints {
                        sharpen_percent: Some(sharpen_percent),
                        down_filter: hints.as_ref().and_then(|h| h.down_filter),
                        up_filter: hints.as_ref().and_then(|h| h.up_filter),
                        scaling_colorspace: hints.as_ref().and_then(|h| h.scaling_colorspace),
                        background_color: hints.as_ref().and_then(|h| h.background_color.clone()),
                        resample_when: Some(s::ResampleWhen::Always),
                        sharpen_when: hints.as_ref().and_then(|h| h.sharpen_when)
                    }),
                };

                let canvas_params = s::Node::CreateCanvas {
                    w: w as usize,
                    h: h as usize,
                    format: s::PixelFormat::from(parent.fmt),
                    color: hints.as_ref().and_then(|h| h.background_color.clone()).unwrap_or(s::Color::Transparent),
                };

                let canvas = ctx.graph
                    .add_node(Node::n(&CREATE_CANVAS,
                                      NodeParams::Json(canvas_params)));
                let scale2d = ctx.graph
                    .add_node(Node::n(&SCALE_2D_RENDER_TO_CANVAS_1D,
                                      NodeParams::Json(scale2d_params)));
                ctx.graph.add_edge(canvas, scale2d, EdgeKind::Canvas).unwrap();
                ctx.replace_node_with_existing(ix, scale2d);
            } else {
                ctx.delete_node_and_snap_together(ix);
            }
            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need Resample2D, got {:?}",params))
        }
    }
}


#[derive(Debug, Clone)]
pub struct Scale2dDef;

impl NodeDef for Scale2dDef {
    fn as_one_input_one_canvas_expand(&self) -> Option<&dyn NodeDefOneInputOneCanvasExpand> {
        Some(self)
    }
}
impl NodeDefOneInputOneCanvasExpand for Scale2dDef {
    fn fqn(&self) -> &'static str {
        "imazen.scale_2d_to_canvas"
    }
    fn validate_params(&self, p: &NodeParams) -> Result<()> {
        Ok(())
    }

    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, params: NodeParams, input: FrameInfo, canvas: FrameInfo) -> Result<()> {
        if let NodeParams::Json(imageflow_types::Node::Resample2D { w, h, hints }) =
        params {

            if w != canvas.w as u32 || h != canvas.h as u32 {
                return Err(nerror!(crate::ErrorKind::InvalidNodeParams, "Resample2D target size {}x{} does not match canvas size {}x{}.", w, h, canvas.w, canvas.h));
            }
            if input.fmt.bytes() != 4 || canvas.fmt.bytes() != 4 {
                return Err(nerror!(crate::ErrorKind::InvalidNodeConnections, "Resample2D can only operate on Rgb32 and Rgba32 bitmaps. Input pixel format {:?}. Canvas pixel format {:?}.", input.fmt, canvas.fmt));
            }

            match hints.as_ref().and_then(|h| h.resample_when){
                Some(s::ResampleWhen::Always) | None => {},
                v => {
                    return Err(nerror!(crate::ErrorKind::InvalidNodeParams, "Resample2D already has a canvas and cannot honor ResampleWhen value {:?}, ", v));
                }
            }

            let bgcolor = hints.as_ref().and_then(|h| h.background_color.clone()).unwrap_or(s::Color::Transparent);
            let new_params = s::Node::DrawImageExact { x: 0, y: 0, w: canvas.w as u32, h: canvas.h as u32,
                blend: if bgcolor.is_transparent() {
                    Some(::imageflow_types::CompositingMode::Overwrite)
                }else{
                    None
                },
                hints: Some(::imageflow_types::ResampleHints {
                    sharpen_percent: hints.as_ref().and_then(|h| h.sharpen_percent),
                    down_filter: hints.as_ref().and_then(|h| h.down_filter),
                    up_filter: hints.as_ref().and_then(|h| h.up_filter),
                    scaling_colorspace: hints.as_ref().and_then(|h| h.scaling_colorspace),
                    background_color: Some(bgcolor),
                    resample_when: None, //We already threw and error if this wasn't none or always
                    sharpen_when: hints.as_ref().and_then(|h| h.sharpen_when)
                })};

            let new_draw_image = ctx.graph
                .add_node(Node::n(&DRAW_IMAGE_EXACT,
                                  NodeParams::Json(new_params)));
            ctx.replace_node_with_existing(ix, new_draw_image);
            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need Resample2D, got {:?}",params))
        }
    }

}


#[derive(Debug, Clone)]
pub struct DrawImageDef;

impl NodeDef for DrawImageDef {
    fn as_one_input_one_canvas(&self) -> Option<&dyn NodeDefOneInputOneCanvas> {
        Some(self)
    }
}

impl NodeDefOneInputOneCanvas for DrawImageDef {
    fn fqn(&self) -> &'static str {
        "imazen.draw_image_to_canvas"
    }
    fn validate_params(&self, p: &NodeParams) -> Result<()> {
        Ok(())
    }

    fn render(&self, c: &Context, canvas: &mut BitmapBgra, input: &mut BitmapBgra, p: &NodeParams) -> Result<()> {
        if let NodeParams::Json(s::Node::DrawImageExact { x, y, w, h, ref hints, blend }) = *p {

            let hints = hints.as_ref();
            if x + w > canvas.w || y + h > canvas.h {

                return Err(nerror!(crate::ErrorKind::InvalidNodeParams, "DrawImageExact target rect x1={},y1={},w={},h={} does not fit canvas size {}x{}.", x,y,  w, h, canvas.w, canvas.h));
            }
            if input.fmt.bytes() != 4 || canvas.fmt.bytes() != 4 {
                return Err(nerror!(crate::ErrorKind::InvalidNodeConnections, "DrawImageExact can only operate on Rgb32 and Rgba32 bitmaps. Input pixel format {:?}. Canvas pixel format {:?}.", input.fmt, canvas.fmt));
            }
            match hints.and_then(|h| h.resample_when){
                Some(s::ResampleWhen::Always) | None => {},
                v => {
                    return Err(nerror!(crate::ErrorKind::InvalidNodeParams, "DrawImageExact already has a canvas and cannot honor ResampleWhen value {:?}, ", v));
                }
            }

            let upscaling = w > input.w || h > input.h;
            let downscaling = w < input.w || h < input.h;
            let size_differs = w != input.w || h != input.h;

            let picked_filter = if upscaling {
                hints.and_then(|h| h.up_filter).unwrap_or(s::Filter::Ginseng)
            } else {
                hints.and_then(|h| h.down_filter).unwrap_or(s::Filter::Robidoux)
            };


            let sharpen_percent_raw = hints.and_then(|h| h.sharpen_percent).unwrap_or(0f32);

            let sharpen_percent = match hints
                .and_then(|h| h.sharpen_when)
                .unwrap_or(s::SharpenWhen::Always){
                imageflow_types::SharpenWhen::Always => sharpen_percent_raw,
                imageflow_types::SharpenWhen::Downscaling if downscaling => sharpen_percent_raw,
                imageflow_types::SharpenWhen::Upscaling if upscaling => sharpen_percent_raw,
                imageflow_types::SharpenWhen::SizeDiffers if size_differs => sharpen_percent_raw,
                _ => 0f32
            };

            let sharpen_requested = sharpen_percent != 0f32;



            let floatspace = hints
                .and_then(|h| h.scaling_colorspace)
                .unwrap_or( s::ScalingFloatspace::Linear); //TODO: reconsider upscaling in srgb by default //if downscaling { s::ScalingFloatspace::Linear} else {s::ScalingFloatspace::Srgb});

            let compose = blend.unwrap_or(::imageflow_types::CompositingMode::Compose) == s::CompositingMode::Compose;

            if canvas.compositing_mode == crate::ffi::BitmapCompositingMode::ReplaceSelf && compose{
                canvas.compositing_mode = crate::ffi::BitmapCompositingMode::BlendWithSelf;
            }
            if canvas.compositing_mode == crate::ffi::BitmapCompositingMode::BlendWithMatte && !compose && canvas.fmt == PixelFormat::Bgra32 {
                canvas.compositing_mode = crate::ffi::BitmapCompositingMode::ReplaceSelf;
            }


            let ffi_struct = ffi::Scale2dRenderToCanvas1d {
                interpolation_filter:                ffi::Filter::from(picked_filter),
                x,
                y,
                w,
                h,
                sharpen_percent_goal: sharpen_percent,
                scale_in_colorspace: crate::ffi::Floatspace::from(floatspace)
            };

            unsafe {
                if !crate::ffi::flow_node_execute_scale2d_render1d(c.flow_c(),
                                                              input, canvas, &ffi_struct as *const ffi::Scale2dRenderToCanvas1d) {
                    return Err(cerror!(c, "Failed to execute Scale2D:  "));
                }
            }

            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need DrawImageExact, got {:?}",p))
        }
    }
}
