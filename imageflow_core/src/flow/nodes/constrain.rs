use super::internal_prelude::*;
use imageflow_riapi::sizing::{Layout, AspectRatio, LayoutError};
use imageflow_types::ConstraintMode;

pub static CONSTRAIN: ConstrainDef = ConstrainDef{};
pub static COMMAND_STRING: CommandStringDef = CommandStringDef{};

pub static EXPANDING_COMMAND_STRING: CommandStringPartiallyExpandedDef = CommandStringPartiallyExpandedDef{};



fn get_decoder_mime(ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<Option<String>>{
    let decoders = ctx.get_decoder_io_ids_and_indexes(ix);
    if let Some((io_id, _)) = decoders.first(){
        Ok(Some(ctx.c.get_codec(*io_id)?.get_decoder()?.get_image_info(ctx.c)?.preferred_mime_type))
    }
    else{
        Ok(None)
    }

}

fn get_expand(ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<::imageflow_riapi::ir4::Ir4Expand>{
    let input = ctx.first_parent_frame_info_some(ix).ok_or_else(|| nerror!(crate::ErrorKind::InvalidNodeConnections, "CommandString node requires that its parent nodes be perfectly estimable"))?;
    let params = &ctx.weight(ix).params;
    if let NodeParams::Json(s::Node::CommandString{ref kind, ref value, ref decode, ref encode}) =
    *params {
        match *kind {
            s::CommandStringKind::ImageResizer4 => {
                Ok(::imageflow_riapi::ir4::Ir4Expand {
                    i: ::imageflow_riapi::ir4::Ir4Command::QueryString(value.to_owned()),
                    encode_id: *encode,
                    source: ::imageflow_riapi::ir4::Ir4SourceFrameInfo {
                        w: input.w,
                        h: input.h,
                        fmt: input.fmt,
                        original_mime: get_decoder_mime(ctx,ix)?
                    }
                })
            }
        }
    }else{
        Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need CommandString, got {:?}", params))
    }
}



#[derive(Debug,Clone)]
pub struct CommandStringPartiallyExpandedDef;



impl NodeDef for CommandStringPartiallyExpandedDef{
    fn fqn(&self) -> &'static str{
        "imazen.expanding_command_string"
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

            if let Some(commands) = e.get_decode_commands().map_err(|e|FlowError::from_layout(e).at(here!()))? {
                for command in commands {
                    //Send command to codec
                    for (io_id, decoder_ix) in ctx.get_decoder_io_ids_and_indexes(ix) {
                        ctx.job.tell_decoder(io_id, command.clone()).map_err(|e| e.at(here!()))?;
                    }
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
pub struct ConstrainDef;
impl NodeDef for ConstrainDef{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for ConstrainDef{
    fn fqn(&self) -> &'static str{
        "imazen.constrain"
    }
    fn estimate(&self, params: &NodeParams, input: FrameEstimate) -> Result<FrameEstimate>{
        if let NodeParams::Json(s::Node::Constrain(ref constraint)) = *params {
            input.map_frame(|input| {
                let constraint_results = imageflow_riapi::ir4::process_constraint(input.w, input.h, constraint).unwrap(); //TODO: fix unwrap
                Ok(FrameInfo {
                    w: constraint_results.final_canvas.width() as i32,
                    h: constraint_results.final_canvas.height() as i32,
                    fmt: ffi::PixelFormat::from(input.fmt),
                })
            })
        }else{
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need Constrain, got {:?}", params))
        }
    }

    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, params: NodeParams, input: FrameInfo) -> Result<()> {
        if let NodeParams::Json(s::Node::Constrain(constraint)) = params {
            let constraint_results = imageflow_riapi::ir4::process_constraint(input.w, input.h, &constraint).unwrap(); //TODO: fix unwrap

            let mut b  = Vec::new();
            if let Some(c) = constraint_results.crop{
                b.push(Node::from(s::Node::Crop { x1: c[0], y1: c[1], x2: c[2], y2: c[3] }));
            }
            b.push(Node::from(
                imageflow_types::Node::Resample2D {
                    w: constraint_results.scale_to.width() as u32,
                    h: constraint_results.scale_to.height() as u32,
                    hints: constraint.hints,
                })
            );

            if let Some(pad) = constraint_results.pad{
                b.push(Node::from(
                imageflow_types::Node::ExpandCanvas {
                    left: pad[0],
                    top: pad[1],
                    right: pad[2],
                    bottom: pad[3],
                    color: constraint.canvas_color.unwrap_or(imageflow_types::Color::Transparent)
                }));
            }

            ctx.replace_node(ix, b);

            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need Constrain, got {:?}", params))
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

        if let NodeParams::Json(s::Node::CommandString { kind, value, decode, encode }) = params_copy {
            if let Some(d_id) = decode {
                if has_parent {
                    return Err(nerror!(crate::ErrorKind::InvalidNodeParams, "CommandString must either have decode: null or have no parent nodes. Specifying a value for decode creates a new decoder node."));
                }
                let decode_node = ::imageflow_riapi::ir4::Ir4Translate {
                    i: ::imageflow_riapi::ir4::Ir4Command::QueryString(value.to_owned()),
                    decode_id: Some(d_id),
                    encode_id: None,
                }.get_decode_node().unwrap();
                ctx.replace_node(ix, vec![
                    Node::from(decode_node),
                    Node::n(&EXPANDING_COMMAND_STRING, params)
                ]);
            } else {
                if !has_parent {
                    return Err(nerror!(crate::ErrorKind::InvalidNodeParams,"CommandString must have a parent node unless 'decode' has a numeric value. Otherwise it has no image source. "));
                }
                ctx.replace_node(ix, vec![
                    Node::n(&EXPANDING_COMMAND_STRING, params)
                ]);
            }
            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need Constrain, got {:?}", params))
        }
    }
}

