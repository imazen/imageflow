use super::internal_prelude::*;
use imageflow_riapi::sizing::{Layout, AspectRatio, LayoutError};
use imageflow_types::{ConstraintMode, ImageInfo};
use itertools::Itertools;

pub static COMMAND_STRING: CommandStringDef = CommandStringDef{};
pub static COMMAND_STRING_POST_TRANSLATE: CommandStringPostTranslateDef = CommandStringPostTranslateDef {};
pub static COMMAND_STRING_POST_DECODE: CommandStringPostDecodeDef = CommandStringPostDecodeDef {};



fn get_decoder_mime(ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<Option<String>>{
    let decoders = ctx.get_decoder_io_ids_and_indexes(ix);
    if let Some((io_id, _)) = decoders.first(){
        Ok(Some(ctx.c.get_unscaled_rotated_image_info(*io_id)?.preferred_mime_type))
    }
    else{
        Ok(None)
    }

}

fn get_expand(ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<::imageflow_riapi::ir4::Ir4Expand>{
    let input = ctx.first_parent_frame_info_some(ix).ok_or_else(|| nerror!(crate::ErrorKind::InvalidNodeConnections, "CommandString node requires that its parent nodes be perfectly estimable"))?;

    let mut image_info: Option<ImageInfo> = None;
    if let Some((io_id, decoder_ix)) = ctx.get_decoder_io_ids_and_indexes(ix).first(){
        image_info = Some(ctx.c.get_unscaled_rotated_image_info(*io_id).map_err(|e| e.at(here!()))?);

    }

    let params = &ctx.weight(ix).params;
    if let NodeParams::Json(s::Node::CommandString{ref kind, ref value, ref decode, ref encode, ref watermarks}) =
    *params {
        match *kind {
            s::CommandStringKind::ImageResizer4 => {
                Ok(::imageflow_riapi::ir4::Ir4Expand {
                    i: ::imageflow_riapi::ir4::Ir4Command::QueryString(value.to_owned()),
                    encode_id: *encode,
                    watermarks: watermarks.clone(),
                    source: ::imageflow_riapi::ir4::Ir4SourceFrameInfo {
                        w: input.w,
                        h: input.h,
                        fmt: input.fmt,
                        original_mime: get_decoder_mime(ctx,ix)?
                    },
                    reference_width: image_info.as_ref().map(|i| i.image_width).unwrap_or(input.w),
                    reference_height: image_info.as_ref().map(|i| i.image_height).unwrap_or(input.h),

                })
            }
        }
    }else{
        Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need CommandString, got {:?}", params))
    }
}



#[derive(Debug,Clone)]
pub struct CommandStringPostDecodeDef;



impl NodeDef for CommandStringPostDecodeDef {
    fn fqn(&self) -> &'static str{
        "imazen.command_string_post_decode"
    }
    fn edges_required(&self, p: &NodeParams) -> Result<(EdgesIn, EdgesOut)>{
        Ok((EdgesIn::OneInput, EdgesOut::Any))
    }
    fn validate_params(&self, p: &NodeParams) -> Result<()>{
        Ok(()) //TODO: need way to provide warnings
    }
    fn estimate(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<FrameEstimate>{
        let old_estimate = ctx.weight(ix).frame_est;

        if old_estimate == FrameEstimate::InvalidateGraph{
            Ok(FrameEstimate::Impossible)
        } else {
            let e = get_expand(ctx, ix).map_err(|e| e.at(here!()))?;

            let decode_commands_result = e.get_decode_commands();

            match decode_commands_result{
                Err(LayoutError::ContentDependent) | Ok(None) => {},
                Ok(Some(commands)) => {
                    for command in commands {
                        //Send command to codec
                        for (io_id, decoder_ix) in ctx.get_decoder_io_ids_and_indexes(ix) {
                            ctx.c.tell_decoder(io_id, command.clone()).map_err(|e| e.at(here!()))?;
                        }
                    }
                }
                Err(e) => {
                    return Err(FlowError::from_layout(e).at(here!()));
                }
            }


            Ok(FrameEstimate::InvalidateGraph)
        }
    }
    fn can_expand(&self) -> bool{
        true
    }

    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<()> {

        let e = get_expand(ctx, ix).map_err(|e| e.at(here!()))?;


        match e.expand_steps().map_err(|e| FlowError::from_layout(e).at(here!())) {
            Ok(r) => {
                //TODO: Find a way to expose warnings
                ctx.replace_node(ix, r.steps.unwrap().into_iter().map( Node::from).collect::<>());
                Ok(())
            }
            Err(e) => {
                //TODO: reparse to get warnings
                Err(e)
            }
        }
    }
}





#[derive(Debug,Clone)]
pub struct CommandStringPostTranslateDef;
impl NodeDef for CommandStringPostTranslateDef{

    fn fqn(&self) -> &'static str{
        "imazen.command_string_post_translate"
    }
    fn estimate(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<FrameEstimate>{
        Ok(FrameEstimate::Impossible)
    }
    fn edges_required(&self, p: &NodeParams) -> Result<(EdgesIn, EdgesOut)> {
        Ok((EdgesIn::OneInput, EdgesOut::Any))
    }
    fn validate_params(&self, p: &NodeParams) -> Result<()>{
        Ok(())
    }
    fn can_expand(&self) -> bool{
        true
    }
    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<()> {
        let has_parent = ctx.first_parent_of_kind(ix, EdgeKind::Input).is_some();
        let params = ctx.weight(ix).params.clone();
        let params_copy = ctx.weight(ix).params.clone();

        if let NodeParams::Json(s::Node::CommandString { kind, value, decode, encode, watermarks }) = params_copy {
            if let Some(d_id) = decode {
                if has_parent {
                    return Err(nerror!(crate::ErrorKind::InvalidNodeParams, "CommandString must either have decode: null or have no parent nodes. Specifying a value for decode creates a new decoder node."));
                }
                let decode_node = ::imageflow_riapi::ir4::Ir4Translate {
                    i: ::imageflow_riapi::ir4::Ir4Command::QueryString(value.to_owned()),
                    decode_id: Some(d_id),
                    encode_id: None,
                    watermarks,
                }.get_decode_node_without_commands().unwrap();
                ctx.replace_node(ix, vec![
                    Node::from(decode_node),
                    Node::n(&COMMAND_STRING_POST_DECODE, params)
                ]);
            } else {
                if !has_parent {
                    return Err(nerror!(crate::ErrorKind::InvalidNodeParams,"CommandString must have a parent node unless 'decode' has a numeric value. Otherwise it has no image source. "));
                }
                ctx.replace_node(ix, vec![
                    Node::n(&COMMAND_STRING_POST_DECODE, params)
                ]);
            }
            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need CommandString, got {:?}", params))
        }
    }
}



#[derive(Debug,Clone)]
pub struct CommandStringDef;
impl NodeDef for CommandStringDef{

    fn fqn(&self) -> &'static str{
        "imazen.command_string"
    }
    fn estimate(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<FrameEstimate>{
        Ok(FrameEstimate::Impossible)
    }
    fn edges_required(&self, p: &NodeParams) -> Result<(EdgesIn, EdgesOut)> {
        Ok((EdgesIn::OneInput, EdgesOut::Any))
    }
    fn validate_params(&self, p: &NodeParams) -> Result<()>{
        Ok(())
    }
    fn can_expand(&self) -> bool{
        true
    }
    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<()> {
        let has_parent = ctx.first_parent_of_kind(ix, EdgeKind::Input).is_some();
        let params = ctx.weight(ix).params.clone();
        let params_copy = ctx.weight(ix).params.clone();

        if let NodeParams::Json(s::Node::CommandString { kind, value, decode, encode, watermarks }) = params_copy {
            if let Some(d_id) = decode {
                if has_parent {
                    return Err(nerror!(crate::ErrorKind::InvalidNodeParams, "CommandString must either have decode: null or have no parent nodes. Specifying a value for decode creates a new decoder node."));
                }
            } else {
                if !has_parent {
                    return Err(nerror!(crate::ErrorKind::InvalidNodeParams,"CommandString must have a parent node unless 'decode' has a numeric value. Otherwise it has no image source. "));
                }
            }
            let translation_result = ::imageflow_riapi::ir4::Ir4Translate {
                i: ::imageflow_riapi::ir4::Ir4Command::QueryString(value.to_owned()),
                decode_id: decode,
                encode_id: encode,
                watermarks,
            }.translate().map_err(|e| FlowError::from_layout(e).at(here!()))?;

            let translation_nodes = translation_result.steps.unwrap()
                .into_iter().map(|n| {
                match n{
                    imageflow_types::Node::CommandString {..} =>  Node::n(&COMMAND_STRING_POST_TRANSLATE, NodeParams::Json(n)),
                    other => Node::from(other)
                }
            }).collect_vec();

            ctx.replace_node(ix, translation_nodes);

            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need CommandString, got {:?}", params))
        }
    }
}