use super::internal_prelude::*;

pub static BITMAP_BGRA_POINTER: BitmapBgraDef = BitmapBgraDef{};

pub static DECODER: DecoderDef = DecoderDef{};
pub static ENCODE: EncoderDef = EncoderDef{};
pub static PRIMITIVE_DECODER: DecoderPrimitiveDef = DecoderPrimitiveDef{};


#[derive(Debug,Clone)]
pub struct BitmapBgraDef{}

impl BitmapBgraDef{
    fn get(&self, p: &NodeParams) -> Result<*mut *mut BitmapBgra> {
        if let NodeParams::Json(s::Node::FlowBitmapBgraPtr { ptr_to_flow_bitmap_bgra_ptr }) = *p {
            let ptr: *mut *mut BitmapBgra = ptr_to_flow_bitmap_bgra_ptr as *mut *mut BitmapBgra;
            if ptr.is_null() {
                return Err(nerror!(crate::ErrorKind::InvalidNodeParams, "The pointer to the bitmap bgra pointer is null! Must be a valid reference to a pointer's location."));
            } else {
                Ok(ptr)
            }
        }else{
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need FlowBitmapBgraPtr, got {:?}", p))
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
                Ok(input)
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
                    return Err(nerror!(crate::ErrorKind::InvalidNodeParams, "When serving as an input node (no parent), FlowBitmapBgraPtr must point to a pointer to a valid BitmapBgra struct."));
                }
                Ok(NodeResult::Frame(*ptr))
            }
        }
    }
}

#[derive(Debug,Clone)]
pub struct DecoderDef{}

fn decoder_get_io_id(params: &NodeParams) -> Result<i32> {
    if let NodeParams::Json(s::Node::Decode { io_id, .. }) = *params {
        Ok(io_id)
    }else{
        Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need Decode, got {:?}", params))
    }
}
fn decoder_estimate(ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<FrameEstimate> {
    let io_id = decoder_get_io_id(&ctx.weight(ix).params).map_err(|e| e.at(here!()))?;
    let frame_info = ctx.job.get_scaled_image_info(io_id).map_err(|e| e.at(here!()))?;

    Ok(FrameEstimate::Some(FrameInfo {
        fmt: frame_info.frame_decodes_into,
        w: frame_info.image_width,
        h: frame_info.image_height
    }))
}

impl NodeDef for DecoderDef {
    fn fqn(&self) -> &'static str {
        "imazen.decoder"
    }
    fn edges_required(&self, p: &NodeParams) -> Result<(EdgesIn, EdgesOut)> {
        Ok((EdgesIn::NoInput, EdgesOut::Any))
    }

    fn validate_params(&self, p: &NodeParams) -> Result<()> {
        decoder_get_io_id(p).map_err(|e| e.at(here!())).map(|_| ())
    }

    fn tell_decoder(&self, p: &NodeParams) -> Result<Option<(i32, Vec<s::DecoderCommand>)>> {
        Ok(None)
    }


    fn estimate(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<FrameEstimate> {
        decoder_estimate(ctx, ix).map_err(|e| e.at(here!()))
    }
    fn can_expand(&self) -> bool {
        true
    }

    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<()> {
        let io_id = decoder_get_io_id(&ctx.weight(ix).params)?;

        // Add the necessary rotation step afterwards
        if let Some(exif_flag) = ctx.job.get_exif_rotation_flag(io_id).map_err(|e| e.at(here!()))?{
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
        // Mutate instead of replace
        ctx.weight_mut(ix).def = &PRIMITIVE_DECODER;
        Ok(())

    }
}





#[derive(Debug,Clone)]
pub struct DecoderPrimitiveDef{}

impl DecoderPrimitiveDef{
    fn get(&self, params: &NodeParams) -> Result<(i32, Option<Vec<s::DecoderCommand>>)> {
        if let NodeParams::Json(s::Node::Decode { io_id, ref commands }) = *params {
            Ok((io_id, commands.clone()))
        }else{
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need Decode, got {:?}", params))
        }
    }
}

impl NodeDef for DecoderPrimitiveDef {
    fn fqn(&self) -> &'static str {
        "imazen.primitive_decoder"
    }
    fn edges_required(&self, p: &NodeParams) -> Result<(EdgesIn, EdgesOut)> {
        Ok((EdgesIn::NoInput, EdgesOut::Any))
    }

    fn validate_params(&self, p: &NodeParams) -> Result<()> {
        // TODO: validate DecoderCommands?
        decoder_get_io_id(p).map_err(|e| e.at(here!())).map(|_| ())
    }

    fn tell_decoder(&self, p: &NodeParams) -> Result<Option<(i32, Vec<s::DecoderCommand>)>> {
        let (io_id, commands) = self.get(p)?;
        if let Some(v) = commands{
            Ok(Some((io_id, v)))
        }else{
            Ok(None)
        }

    }

    fn estimate(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<FrameEstimate> {
        decoder_estimate(ctx, ix).map_err(|e| e.at(here!()))
    }
    fn can_execute(&self) -> bool {
        true
    }

    fn execute(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<NodeResult> {
        let io_id = decoder_get_io_id(&ctx.weight(ix).params)?;

        let estimate = self.estimate(ctx, ix)?;

        validate_frame_size(estimate, &ctx.c.security.max_decode_size, "max_decode_size")?;

        let mut codec = ctx.c.get_codec(io_id).map_err(|e| e.at(here!()))?;
        let decoder = codec.get_decoder().map_err(|e| e.at(here!()))?;

        let result = decoder.read_frame(ctx.c).map_err(|e| e.at(here!()))?;

        if decoder.has_more_frames()?{
            ctx.set_more_frames(true);
        }

        Ok(NodeResult::Frame(result))
    }
}

fn validate_frame_size(est: FrameEstimate, limit_maybe: &Option<imageflow_types::FrameSizeLimit>, limit_name: &'static str) -> Result<()>{
    if let Some(limit)= limit_maybe {
        // Validate frame size
        let info = match est {
            FrameEstimate::Some(info) => Some(info),
            FrameEstimate::UpperBound(info) => Some(info),
            _ => None
        };
        if let Some(frame_info) = info {
            if limit.w.leading_zeros() == 0 ||
                limit.h.leading_zeros() == 0 {
                return Err(nerror!(ErrorKind::SizeLimitExceeded, "{} values overflow an i32", limit_name));
            }
            if frame_info.w > limit.w as i32 {
                return Err(nerror!(ErrorKind::SizeLimitExceeded, "Frame width {} exceeds {}.w {}", frame_info.w, limit_name, limit.w))
            }
            if frame_info.h > limit.h as i32 {
                return Err(nerror!(ErrorKind::SizeLimitExceeded, "Frame height {} exceeds {}.h {}", frame_info.h, limit_name, limit.h))
            }
            let megapixels = frame_info.w as f32 * frame_info.h as f32  / 1000000f32;
            if megapixels > limit.megapixels {
                return Err(nerror!(ErrorKind::SizeLimitExceeded, "Frame megapixels {} exceeds {}.megapixels {}", megapixels, limit_name, limit.megapixels))
            }
        }
    }
    Ok(())
}



#[derive(Debug,Clone)]
pub struct EncoderDef{}

impl EncoderDef{
    fn get(&self, params: &NodeParams) -> Result<(i32, s::EncoderPreset)> {
        if let NodeParams::Json(s::Node::Encode { io_id, ref preset }) = *params {
            Ok((io_id, preset.clone()))
        }else{
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need Encode, got {:?}", params))
        }
    }
}

impl NodeDef for EncoderDef {
    fn fqn(&self) -> &'static str {
        "imazen.primitive_encoder"
    }
    fn edges_required(&self, p: &NodeParams) -> Result<(EdgesIn, EdgesOut)> {
        Ok((EdgesIn::OneInput, EdgesOut::None))
    }

    fn validate_params(&self, p: &NodeParams) -> Result<()> {
        // TODO: validate Presets?
        self.get(p).map_err(|e| e.at(here!())).map(|_| ())
    }

    fn estimate(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<FrameEstimate> {
        ctx.frame_est_from(ix, EdgeKind::Input).map_err(|e| e.at(here!()))
    }
    fn can_execute(&self) -> bool {
        true
    }

    fn execute(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<NodeResult> {
        let (io_id, preset) = self.get(&ctx.weight(ix).params)?;
        let input_bitmap = ctx.bitmap_bgra_from(ix, EdgeKind::Input).map_err(|e| e.at(here!()))?;

        // Validate max encode size
        let estimate = self.estimate(ctx, ix)?;
        validate_frame_size(estimate, &ctx.c.security.max_encode_size, "max_encode_size")?;

        let decoders = ctx.get_decoder_io_ids_and_indexes(ix).into_iter().map(|(io_id, ix)| io_id).collect::<Vec<i32>>();

        let mut codec = ctx.job.get_codec(io_id).map_err(|e| e.at(here!()))?;
        let result = codec.write_frame(ctx.c, &preset,unsafe{ &mut *input_bitmap }, &decoders ).map_err(|e| e.at(here!()))?;


        Ok(NodeResult::Encoded(result))
    }
}



