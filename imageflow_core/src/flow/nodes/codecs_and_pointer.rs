use super::internal_prelude::*;

fn bitmap_bgra_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.bitmap_bgra_pointer",
        name: "bitmap_bgra_pointer",
        outbound_edges: true,
        inbound_edges: EdgesIn::OneOptionalInput,

        fn_estimate: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                match ctx.weight(ix).params {
                    NodeParams::Json(s::Node::FlowBitmapBgraPtr{ptr_to_flow_bitmap_bgra_ptr}) => {
                        let ptr: *mut *mut BitmapBgra = ptr_to_flow_bitmap_bgra_ptr as *mut *mut BitmapBgra;
                        unsafe {
                            if ptr.is_null() {
                                panic!("Must be a valid pointer to a pointer to BitmapBgra");
                            }

                            if (*ptr).is_null() {
                                NodeDefHelpers::copy_frame_est_from_first_input(ctx, ix);
                            } else {
                                let weight = &mut ctx.weight_mut(ix);
                                let b = &(**ptr);
                                weight.frame_est = FrameEstimate::Some(FrameInfo {
                                    w: b.w as i32,
                                    h: b.h as i32,
                                    fmt: b.fmt,
                                    alpha_meaningful: b.alpha_meaningful
                                });
                            }
                        }

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
                // let weight = &mut ctx.weight_mut(ix);
                match ctx.weight(ix).params {
                    NodeParams::Json(s::Node::FlowBitmapBgraPtr{ptr_to_flow_bitmap_bgra_ptr}) => {
                        let ptr: *mut *mut BitmapBgra = ptr_to_flow_bitmap_bgra_ptr as *mut *mut BitmapBgra;
                        unsafe {
                            if ptr.is_null() {
                                panic!("Must be a valid pointer to a pointer to BitmapBgra");
                            }

                            let frame =     ctx.first_parent_result_frame(ix, EdgeKind::Input);
                            let weight = &mut ctx.weight_mut(ix);
                            match frame {
                                Some(input_ptr) => {
                                    *ptr = input_ptr;
                                    weight.result = NodeResult::Frame(input_ptr);
                                },
                                None => {
                                    if (*ptr).is_null() {
                                        panic!("When serving as an input node, FlowBitmapBgraPtr must point to a valid BitmapBgra. Found null.");
                                    }
                                    weight.result = NodeResult::Frame(*ptr);
                                }
                            }
                        }
                    }
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

fn decoder_encoder_io_id(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) -> Option<i32> {
    match ctx.weight(ix).params {
        NodeParams::Json(s::Node::Decode { io_id, .. }) |
        NodeParams::Json(s::Node::Encode { io_id, .. }) => Some(io_id),
        _ => None,
    }
}

fn decoder_estimate(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
    let io_id = decoder_encoder_io_id(ctx, ix).unwrap();
    let frame_info: s::ImageInfo = ctx.job.get_image_info(io_id).unwrap();

    ctx.weight_mut(ix).frame_est = FrameEstimate::Some(FrameInfo {
        fmt: frame_info.frame_decodes_into,
        w: frame_info.image_width,
        h: frame_info.image_height,
        alpha_meaningful: true, // WRONG
    });
}

// Todo list codec name in stringify

fn decoder_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.decoder",
        name: "decoder",
        outbound_edges: true,
        inbound_edges: EdgesIn::NoInput,
        fn_estimate: Some(decoder_estimate),

        // Allow link-up
        fn_link_state_to_this_io_id: Some(decoder_encoder_io_id),
        fn_flatten_pre_optimize: {
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {

//                if ctx.weight(ix).frame_est == FrameEstimate::None{
//                    decoder_estimate(ctx,ix);
//                }

                // Mutate instead of replace (custom_state is populated)
                ctx.weight_mut(ix).def = &PRIMITIVE_DECODER;


                let exif_flag = unsafe {ffi::flow_codecs_jpg_decoder_get_exif(ctx.flow_c(), ctx.weight(ix).custom_state as *mut ffi::CodecInstance) };
                if exif_flag > 0 {
                    let new_node = ctx.graph
                        .add_node(Node::new(&APPLY_ORIENTATION,
                                            NodeParams::Json(s::Node::ApplyOrientation {
                                                flag: exif_flag,
                                            })));
                    ctx.copy_edges_to(ix, new_node, EdgeDirection::Outgoing);
                    ctx.delete_child_edges_for(ix);
                    ctx.graph.add_edge(ix, new_node, EdgeKind::Input).unwrap();
                }
            }
            Some(f)
        },
        ..Default::default()
    }
}
fn primitive_decoder_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.primitive_decoder",
        name: "primitive_decoder",
        outbound_edges: true,
        inbound_edges: EdgesIn::NoInput,
        fn_estimate: Some(decoder_estimate),
        fn_link_state_to_this_io_id: Some(decoder_encoder_io_id),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {

                // TODO______
                let codec = ctx.weight(ix).custom_state as *mut ffi::CodecInstance;
                if codec.is_null(){
                    panic!("custom_state: CodecInstance is null. Codec failed to link?")
                }
                unsafe {
                    let result = ffi::flow_codec_execute_read_frame(ctx.flow_c(), codec);
                    if result.is_null() {
                        ctx.assert_ok();
                    } else {
                        ctx.weight_mut(ix).result = NodeResult::Frame(result);
                    }
                }
            }
            f
        }),
        ..Default::default()
    }
}

fn encoder_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.primitive_encoder",
        name: "primitive_encoder",
        outbound_edges: false,
        inbound_edges: EdgesIn::OneInput,
        // Allow link-up
        fn_link_state_to_this_io_id: Some(decoder_encoder_io_id),
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_input),

        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                if let Some(input_bitmap) = ctx.first_parent_result_frame(ix, EdgeKind::Input) {
                    let result;
                    {
                        let weight = ctx.weight(ix);
                        if let NodeParams::Json(s::Node::Encode { ref preset, ref io_id, .. }) =
                               weight.params {
                            let codec = weight.custom_state as *mut ffi::CodecInstance;

                            if codec.is_null(){
                                panic!("custom_state: CodecInstance is null")
                            }

                            let (wanted_id, hints) = match *preset {
                                s::EncoderPreset::LibjpegTurbo { quality } => {
                                    (ffi::CodecType::EncodeJpeg as i64,
                                     ffi::EncoderHints {
                                        jpeg_encode_quality: quality.unwrap_or(90),
                                        disable_png_alpha: false,
                                    })
                                }
                                s::EncoderPreset::Libpng { ref matte,
                                                           zlib_compression,
                                                           ref depth } => {
                                    (ffi::CodecType::EncodePng as i64,
                                     ffi::EncoderHints {
                                        jpeg_encode_quality: -1,
                                        disable_png_alpha: match *depth {
                                            Some(s::PngBitDepth::Png24) => true,
                                            _ => false,
                                        },
                                    })
                                }
                            };

                            unsafe {
                                let (result_mime, result_ext) = match *preset {
                                    s::EncoderPreset::Libpng { .. } => ("image/png", "png"),
                                    s::EncoderPreset::LibjpegTurbo { .. } => ("image/jpeg", "jpg"),
                                };
                                result = NodeResult::Encoded(s::EncodeResult {
                                    w: (*input_bitmap).w as i32,
                                    h: (*input_bitmap).h as i32,
                                    preferred_mime_type: result_mime.to_owned(),
                                    preferred_extension: result_ext.to_owned(),
                                    io_id: *io_id,
                                    bytes: s::ResultBytes::Elsewhere,
                                });



                                (*codec).codec_id = wanted_id;
                                if !ffi::flow_codec_initialize(ctx.flow_c(), codec) {
                                    ctx.assert_ok();
                                }
                                let codec_def = ffi::flow_codec_get_definition(ctx.flow_c(), wanted_id);
                                if codec_def.is_null() {
                                    ctx.assert_ok();
                                }
                                let write_fn = (*codec_def).write_frame;
                                if write_fn == None {
                                    panic!("Codec didn't implement write_frame");
                                }

                                if !write_fn.unwrap()(ctx.flow_c(),
                                                      (*codec).codec_state,
                                                      input_bitmap,
                                                      &hints as *const ffi::EncoderHints) {
                                    ctx.assert_ok();
                                }
                            }
                        } else {
                            panic!("");
                        }
                    }
                    {
                        ctx.weight_mut(ix).result = result;
                    }
                } else {
                    panic!("");
                }

            }
            f
        }),

        ..Default::default()
    }
}


lazy_static! {
    pub static ref BITMAP_BGRA_POINTER: NodeDefinition = bitmap_bgra_def();
    pub static ref DECODER: NodeDefinition = decoder_def();
    pub static ref ENCODE: NodeDefinition = encoder_def();
    pub static ref PRIMITIVE_DECODER: NodeDefinition = primitive_decoder_def();
}
