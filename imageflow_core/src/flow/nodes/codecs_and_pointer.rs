use super::internal_prelude::*;

pub static BITMAP_BGRA_POINTER: BitmapBgraDef = BitmapBgraDef{};

lazy_static! {
    pub static ref DECODER: NodeDefinition = decoder_def();
    pub static ref ENCODE: NodeDefinition = encoder_def();
    pub static ref PRIMITIVE_DECODER: NodeDefinition = primitive_decoder_def();
}

#[derive(Debug,Clone)]
pub struct BitmapBgraDef{}

impl BitmapBgraDef{
    fn get(&self, p: &NodeParams) -> Result<*mut *mut BitmapBgra> {
        if let &NodeParams::Json(s::Node::FlowBitmapBgraPtr { ptr_to_flow_bitmap_bgra_ptr }) = p {
            let ptr: *mut *mut BitmapBgra = ptr_to_flow_bitmap_bgra_ptr as *mut *mut BitmapBgra;
            if ptr.is_null() {
                return Err(nerror!(::ErrorKind::InvalidNodeParams, "The pointer to the bitmap bgra pointer is null! Must be a valid reference to a pointer's location."));
            } else {
                Ok(ptr)
            }
        }else{
            Err(nerror!(::ErrorKind::NodeParamsMismatch, "Need FlowBitmapBgraPtr, got {:?}", p))
        }
    }
}

impl NodeDef for BitmapBgraDef {
    fn fqn(&self) -> &'static str {
        "imazen.bitmap_bgra_pointer"
    }
    fn edges_required(&self, p: &NodeParams) -> Result<(EdgesIn, EdgesOut)> {
        Ok((EdgesIn::OneOptionalInput, EdgesOut::Any))
    }

    fn validate_params(&self, p: &NodeParams) -> Result<()> {
        self.get(p).map_err(|e| e.at(here!())).map(|_| ())
    }

    fn estimate(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<FrameEstimate> {
        let params = &ctx.weight(ix).params;

        let ptr = self.get(params).map_err(|e| e.at(here!()))?;

        unsafe {
            if (*ptr).is_null() {
                let input = ctx.frame_est_from(ix, EdgeKind::Input).map_err(|e| e.at(here!()))?;
                Ok((input))
            } else {
                let b = &(**ptr);
                Ok(FrameEstimate::Some(FrameInfo {
                    w: b.w as i32,
                    h: b.h as i32,
                    fmt: b.fmt,
                }))
            }
        }
    }

    fn can_execute(&self) -> bool {
        true
    }

    fn execute(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<NodeResult> {
        let ptr = self.get(&ctx.weight(ix).params).map_err(|e| e.at(here!()))?;

        let frame = ctx.first_parent_result_frame(ix, EdgeKind::Input);
        if let Some(input_ptr) = frame {
            unsafe { *ptr = input_ptr };
            ctx.consume_parent_result(ix, EdgeKind::Input)?;
            Ok(NodeResult::Frame(input_ptr))
        } else {
            unsafe {
                if (*ptr).is_null() {
                    return Err(nerror!(::ErrorKind::InvalidNodeParams, "When serving as an input node (no parent), FlowBitmapBgraPtr must point to a pointer to a valid BitmapBgra struct."));
                }
                Ok(NodeResult::Frame(*ptr))
            }
        }
    }
}
//
//#[derive(Debug,Clone)]
//pub struct DecoderDef{}
//
//impl DecoderDef{
//    fn get(&self, p: &NodeParams) -> NResult<(i32, Vec<s::DecoderCommand>)> {
//        match ctx.weight(ix).params {
//            NodeParams::Json(s::Node::Decode { io_id, commands }) => Ok(io_id),
//            _ => Err(nerror!(::ErrorKind::NodeParamsMismatch, "Need Decode, got {:?}", p)),
//        }
//    }
//}
//
//impl NodeDef for DecoderDef {
//    fn fqn(&self) -> &'static str {
//        "imazen.decoder"
//    }
//    fn edges_required(&self, p: &NodeParams) -> NResult<(EdgesIn, EdgesOut)> {
//        Ok((EdgesIn::None, EdgesOut::Any))
//    }
//
//    fn validate_params(&self, p: &NodeParams) -> NResult<()> {
//        self.get(p).map_err(|e| e.at(here!())).map(|_| ())
//    }
//
//    fn tell_decoder(&self, p: &NodeParams) -> Option<(i32, Vec<s::DecoderCommand>)> {
//        None
//    }
//
//
//    fn estimate(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> NResult<FrameEstimate> {
//        let params = &ctx.weight(ix).params;
//
//        let ptr = self.get(p).map_err(|e| e.at(here!()))?;
//
//        unsafe {
//            if (*ptr).is_null() {
//                let input = ctx.frame_est_from(ix, EdgeKind::Input).map_err(|e| e.at(here!()))?;
//                Ok((input))
//            } else {
//                let b = &(**ptr);
//                Ok(FrameEstimate::Some(FrameInfo {
//                    w: b.w as i32,
//                    h: b.h as i32,
//                    fmt: b.fmt,
//                    alpha_meaningful: b.alpha_meaningful
//                }));
//            }
//        }
//    }
//    fn can_expand(&self) -> bool {
//        true
//    }
//
//    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> NResult<()> {
//        // Mutate instead of replace
//        ctx.weight_mut(ix).def = PRIMITIVE_DECODER.as_node_def();
//
//        let io_id = decoder_encoder_io_id(ctx, ix).unwrap();
//
//        if let Ok(exif_flag) = ctx.job.get_exif_rotation_flag(io_id){
//            if exif_flag > 0 {
//                let new_node = ctx.graph
//                    .add_node(Node::n(&APPLY_ORIENTATION,
//                                      NodeParams::Json(s::Node::ApplyOrientation {
//                                          flag: exif_flag,
//                                      })));
//                ctx.copy_edges_to(ix, new_node, EdgeDirection::Outgoing);
//                ctx.delete_child_edges_for(ix);
//                ctx.graph.add_edge(ix, new_node, EdgeKind::Input).unwrap();
//            }
//        }
//    }
//}




    fn decoder_encoder_io_id(ctx: &mut OpCtxMut, ix: NodeIndex) -> Option<i32> {
    match ctx.weight(ix).params {
        NodeParams::Json(s::Node::Decode { io_id, .. }) |
        NodeParams::Json(s::Node::Encode { io_id, .. }) => Some(io_id),
        _ => None,
    }
}

fn decoder_estimate(ctx: &mut OpCtxMut, ix: NodeIndex) {
    let io_id = decoder_encoder_io_id(ctx, ix).unwrap();
    let frame_info: s::ImageInfo = ctx.job.get_image_info(io_id).unwrap();

    ctx.weight_mut(ix).frame_est = FrameEstimate::Some(FrameInfo {
        fmt: frame_info.frame_decodes_into,
        w: frame_info.image_width,
        h: frame_info.image_height
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
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex) {

                // Mutate instead of replace
                ctx.weight_mut(ix).def = PRIMITIVE_DECODER.as_node_def();

                let io_id = decoder_encoder_io_id(ctx, ix).unwrap();

                if let Ok(exif_flag) = ctx.job.get_exif_rotation_flag(io_id){
                    if exif_flag > 0 {
                        let new_node = ctx.graph
                            .add_node(Node::n(&APPLY_ORIENTATION,
                                                NodeParams::Json(s::Node::ApplyOrientation {
                                                    flag: exif_flag,
                                                })));
                        ctx.copy_edges_to(ix, new_node, EdgeDirection::Outgoing);
                        ctx.delete_child_edges_for(ix);
                        ctx.graph.add_edge(ix, new_node, EdgeKind::Input).unwrap();
                    }
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
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex) {
                let io_id = decoder_encoder_io_id(ctx, ix).unwrap();

                let result = ctx.job.get_codec(io_id).unwrap().get_decoder().unwrap().read_frame(ctx.c, ctx.job, &mut *ctx.job.get_io(io_id).unwrap()).unwrap();
                ctx.weight_mut(ix).result = NodeResult::Frame(result);
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
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex) {
                let io_id = decoder_encoder_io_id(ctx, ix).unwrap();

                if let Some(input_bitmap) = ctx.first_parent_result_frame(ix, EdgeKind::Input) {
                    let result;
                    {
                        let weight = ctx.weight(ix);
                        if let NodeParams::Json(s::Node::Encode { ref preset, ref io_id, .. }) =
                               weight.params {

                            result = NodeResult::Encoded(
                                ctx.job.get_codec(*io_id).unwrap().write_frame(ctx.c, ctx.job, preset,
                                                  unsafe{ &mut *input_bitmap } ).unwrap());

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

