use super::internal_prelude::*;

fn create_canvas_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.create_canvas",
        name: "create_canvas",
        fn_estimate: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                let weight = &mut ctx.weight_mut(ix);
                match weight.params {
                    NodeParams::Json(s::Node::CreateCanvas { ref format,
                                                             ref w,
                                                             ref h,
                                                             ref color }) => {
                        weight.frame_est = FrameEstimate::Some(FrameInfo {
                            w: *w as i32,
                            h: *h as i32,
                            fmt: *format,
                            alpha_meaningful: true,
                        });
                    }
                    _ => {
                        panic!("Node params missing");
                    }
                }
            }
            f
        }),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                let flow_pointer = ctx.flow_c();
                let weight = &mut ctx.weight_mut(ix);
                match weight.params {
                    NodeParams::Json(s::Node::CreateCanvas { ref format,
                                                             ref w,
                                                             ref h,
                                                             ref color }) => unsafe {
                        // TODO: handle creation failure. Most likely OOM in entire codebase
                        let ptr =
                            ::ffi::flow_bitmap_bgra_create(flow_pointer, *w as i32, *h as i32, true, *format);
                        let color_val = color.clone();
                        let color_srgb_argb = color_val.clone().to_u32_bgra().unwrap();
                        (*ptr).compositing_mode = ::ffi::BitmapCompositingMode::ReplaceSelf;
                        if color_val != s::Color::Transparent {
                            if !ffi::flow_bitmap_bgra_fill_rect(flow_pointer,
                                                                ptr,
                                                                0,
                                                                0,
                                                                *w as u32,
                                                                *h as u32,
                                                                color_srgb_argb) {
                                panic!("failed to fill rect. epic.");
                            }

                            (*ptr).compositing_mode = ::ffi::BitmapCompositingMode::BlendWithMatte;
                        }
                        (*ptr).matte_color  = mem::transmute(color_srgb_argb);

                        weight.result = NodeResult::Frame(ptr);
                    },
                    _ => {
                        panic!("Node params missing");
                    }
                }

            }
            f
        }),
        ..Default::default()
    }
}

lazy_static! {
    pub static ref CREATE_CANVAS: NodeDefinition = create_canvas_def();
}
