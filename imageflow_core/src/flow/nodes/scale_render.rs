use super::internal_prelude::*;

pub static SCALE_2D_RENDER_TO_CANVAS_1D: Scale2dDef = Scale2dDef{};
pub static SCALE: ScaleDef = ScaleDef{};
pub static DRAW_IMAGE_EXACT: DrawImageDef = DrawImageDef{};


#[derive(Debug,Clone)]
pub struct ScaleDef;
impl NodeDef for ScaleDef{
    fn as_one_input_expand(&self) -> Option<&NodeDefOneInputExpand>{
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
            Err(nerror!(::ErrorKind::NodeParamsMismatch, "Need Resample2D, got {:?}",params))
        }
    }
    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, params: NodeParams, parent: FrameInfo) -> Result<()> {
        if let NodeParams::Json(s::Node::Resample2D { w, h, down_filter, up_filter, scaling_colorspace, hints }) =
        params {
            let filter = if parent.w < w as i32 || parent.h < h as i32 {
                up_filter
            } else {
                down_filter
            };

            let canvas_params = s::Node::CreateCanvas {
                w: w as usize,
                h: h as usize,
                format: s::PixelFormat::from(parent.fmt),
                color: hints.as_ref().and_then(|h| h.background_color.clone()).unwrap_or(s::Color::Transparent),
            };
            // TODO: Not the right params! - me later - what??
            let scale2d_params = s::Node::Resample2D {
                w,
                h,
                up_filter,
                down_filter,
                scaling_colorspace,
                hints,
            };
            let canvas = ctx.graph
                .add_node(Node::n(&CREATE_CANVAS,
                                  NodeParams::Json(canvas_params)));
            let scale2d = ctx.graph
                .add_node(Node::n(&SCALE_2D_RENDER_TO_CANVAS_1D,
                                  NodeParams::Json(scale2d_params)));
            ctx.graph.add_edge(canvas, scale2d, EdgeKind::Canvas).unwrap();
            ctx.replace_node_with_existing(ix, scale2d);
            Ok(())
        } else {
            Err(nerror!(::ErrorKind::NodeParamsMismatch, "Need Resample2D, got {:?}",params))
        }
    }
}


#[derive(Debug, Clone)]
pub struct Scale2dDef;

impl NodeDef for Scale2dDef {
    fn as_one_input_one_canvas_expand(&self) -> Option<&NodeDefOneInputOneCanvasExpand> {
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
        if let NodeParams::Json(s::Node::Resample2D { w, h, down_filter, up_filter, scaling_colorspace, hints }) =
        params {

            if w != canvas.w as u32 || h != canvas.h as u32 {
                return Err(nerror!(::ErrorKind::InvalidNodeParams, "Resample2D target size {}x{} does not match canvas size {}x{}.", w, h, canvas.w, canvas.h));
            }
            if input.fmt.bytes() != 4 || canvas.fmt.bytes() != 4 {
                return Err(nerror!(::ErrorKind::InvalidNodeConnections, "Resample2D can only operate on Rgb32 and Rgba32 bitmaps. Input pixel format {:?}. Canvas pixel format {:?}.", input.fmt, canvas.fmt));
            }

            let bgcolor = hints.as_ref().and_then(|h| h.background_color.clone()).unwrap_or(s::Color::Transparent);
            let new_params = s::Node::DrawImageExact { x: 0, y: 0, w: canvas.w as u32, h: canvas.h as u32,
                blend: if bgcolor.is_transparent() {
                    Some(::imageflow_types::CompositingMode::Overwrite)
                }else{
                    None
                },
                hints: Some(::imageflow_types::ConstraintResamplingHints {
                    sharpen_percent: hints.and_then(|h| h.sharpen_percent),
                    down_filter,
                    up_filter,
                    scaling_colorspace,
                    background_color: Some(bgcolor),
                    resample_when: Some(::imageflow_types::ResampleWhen::Always), //TODO: modify once implemented and update tests
                })};

            let new_draw_image = ctx.graph
                .add_node(Node::n(&DRAW_IMAGE_EXACT,
                                  NodeParams::Json(new_params)));
            ctx.replace_node_with_existing(ix, new_draw_image);
            Ok(())
        } else {
            Err(nerror!(::ErrorKind::NodeParamsMismatch, "Need Resample2D, got {:?}",params))
        }
    }

}


#[derive(Debug, Clone)]
pub struct DrawImageDef;

impl NodeDef for DrawImageDef {
    fn as_one_input_one_canvas(&self) -> Option<&NodeDefOneInputOneCanvas> {
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

                return Err(nerror!(::ErrorKind::InvalidNodeParams, "DrawImageExact target rect x1={},y1={},w={},h={} does not fit canvas size {}x{}.", x,y,  w, h, canvas.w, canvas.h));
            }
            if input.fmt.bytes() != 4 || canvas.fmt.bytes() != 4 {
                return Err(nerror!(::ErrorKind::InvalidNodeConnections, "DrawImageExact can only operate on Rgb32 and Rgba32 bitmaps. Input pixel format {:?}. Canvas pixel format {:?}.", input.fmt, canvas.fmt));
            }

            let upscaling = w > input.w || h > input.h;
            let downscaling = w < input.w || h < input.h;

            let picked_filter = if w > input.w || h > input.h {
                hints.and_then(|h| h.up_filter).unwrap_or(s::Filter::Ginseng)
            } else {
                hints.and_then(|h| h.down_filter).unwrap_or(s::Filter::Robidoux)
            };

            let sharpen_percent = hints.and_then(|h| h.sharpen_percent).unwrap_or(0f32);

            let floatspace = hints.and_then(|h| h.scaling_colorspace).unwrap_or(s::ScalingFloatspace::Linear); //  if downscaling { ffi::Floatspace::Linear} else {ffi::Floatspace::Srgb}

            let compose = blend.unwrap_or(::imageflow_types::CompositingMode::Compose) == s::CompositingMode::Compose;

            if canvas.compositing_mode == ::ffi::BitmapCompositingMode::ReplaceSelf && compose{
                canvas.compositing_mode = ::ffi::BitmapCompositingMode::BlendWithSelf;
            }
            if canvas.compositing_mode == ::ffi::BitmapCompositingMode::BlendWithMatte && !compose && canvas.fmt == PixelFormat::Bgra32 {
                canvas.compositing_mode = ::ffi::BitmapCompositingMode::ReplaceSelf;
            }

            let ffi_struct = ffi::Scale2dRenderToCanvas1d {
                interpolation_filter:                ffi::Filter::from(picked_filter),
                x,
                y,
                w,
                h,
                sharpen_percent_goal: sharpen_percent,
                scale_in_colorspace: ::ffi::Floatspace::from(floatspace)
            };

            unsafe {
                if !::ffi::flow_node_execute_scale2d_render1d(c.flow_c(),
                                                              input, canvas, &ffi_struct as *const ffi::Scale2dRenderToCanvas1d) {
                    return Err(cerror!(c, "Failed to execute Scale2D:  "));
                }
            }

            Ok(())
        } else {
            Err(nerror!(::ErrorKind::NodeParamsMismatch, "Need DrawImageExact, got {:?}",p))
        }
    }
}
