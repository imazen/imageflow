use super::internal_prelude::*;


pub static CONSTRAIN: ConstrainDef = ConstrainDef{};
pub static COMMAND_STRING: CommandStringDef = CommandStringDef{};

lazy_static! {
    pub static ref EXPANDING_COMMAND_STRING: NodeDefinition = command_string_partially_expanded_def();
}



fn get_expand(ctx: &mut OpCtxMut, ix: NodeIndex) -> ::imageflow_riapi::ir4::Ir4Expand{
    let input = ctx.first_parent_frame_info_some(ix).unwrap();
    if let s::Node::CommandString{ref kind, ref value, ref decode, ref encode} =
    ctx.get_json_params(ix).unwrap() {
        match kind {
            &s::CommandStringKind::ImageResizer4 => {
                ::imageflow_riapi::ir4::Ir4Expand {
                    i: ::imageflow_riapi::ir4::Ir4Command::QueryString(value.to_owned()),
                    encode_id: *encode,
                    source: ::imageflow_riapi::ir4::Ir4SourceFrameInfo {
                        w: input.w,
                        h: input.h,
                        fmt: input.fmt,
                        original_mime: None
                    }
                }
            }
        }
    }else{
        panic!("");
    }
}




fn command_string_partially_expanded_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.expanding_command_string",
        name: "expanding_command_string",
        inbound_edges: EdgesIn::OneInput,
        description: "expanding command string",
        fn_estimate: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex) {

                let old_estimate = ctx.weight(ix).frame_est;


                if old_estimate == FrameEstimate::Invalidated{
                    ctx.weight_mut(ix).frame_est = FrameEstimate::Impossible;
                } else {
                    let e = get_expand(ctx, ix);

                    if let Some(command) = e.get_decode_commands().unwrap() {
                        //Send command to codec
                        for (io_id, decoder_ix) in ctx.get_decoder_io_ids_and_indexes(ix) {
                            ctx.job.tell_decoder(io_id, command.clone()).unwrap();
                            ctx.weight_mut(decoder_ix).frame_est = FrameEstimate::Invalidated;
                        }
                    }

                    //                let canvas = e.get_canvas_size().unwrap();
                    ctx.weight_mut(ix).frame_est = FrameEstimate::Invalidated;
                }


                //                FrameEstimate::UpperBound(FrameInfo {
                //                    w: canvas.width(),
                //                    h: canvas.height(),
                //                    fmt: PixelFormat::Bgra32,
                //                    alpha_meaningful: true
                //                });
            }
            f
        }),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex) {
                let e = get_expand(ctx, ix);


                match e.expand_steps() {
                    Ok(r) => {
                        //TODO: Find a way to expose warnings
                        ctx.replace_node(ix, r.steps.unwrap().into_iter().map(|n| Node::from(n)).collect::<>());
                    }
                    Err(e) => {
                        //TODO: reparse to get warnings
                        panic!("{:?}", e);
                    }
                }
            }
            f
        }),
        ..Default::default()
    }
}



#[derive(Debug,Clone)]
pub struct ConstrainDef;
impl NodeDef for ConstrainDef{
    fn as_one_input_expand(&self) -> Option<&NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for ConstrainDef{
    fn fqn(&self) -> &'static str{
        "imazen.constrain"
    }
    fn estimate(&self, params: &NodeParams, input: FrameEstimate) -> Result<FrameEstimate>{
        if let &NodeParams::Json(s::Node::Constrain(ref constraint)) = params {
            input.map_frame(|input| {
                let (w, h, _) = constrain(input.w as u32, input.h as u32, constraint);
                Ok(FrameInfo {
                    w: w as i32,
                    h: h as i32,
                    fmt: ffi::PixelFormat::from(input.fmt),
                })
            })
        }else{
            Err(nerror!(::ErrorKind::NodeParamsMismatch, "Need Constrain, got {:?}", params))
        }
    }

    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, params: NodeParams, parent: FrameInfo) -> Result<()> {
        if let NodeParams::Json(s::Node::Constrain(constraint)) = params {
            let input_w = parent.w as u32;
            let input_h = parent.h as u32;

            let (new_w, new_h, hints_val) = constrain(input_w, input_h, &constraint);

            let hints = &hints_val;

            let resample_when = hints.and_then(|ref h| h.resample_when).unwrap_or(s::ResampleWhen::SizeDiffers);
            let size_differs = new_w != input_w || new_h != input_h;
            let sharpen_requested = hints.and_then(|h| h.sharpen_percent).unwrap_or(0f32) > 0f32;

            let resample = match resample_when {
                s::ResampleWhen::Always => true,
                s::ResampleWhen::SizeDiffers if size_differs => true,
                s::ResampleWhen::SizeDiffersOrSharpeningRequested if size_differs || sharpen_requested => true,
                _ => false
            };

            if resample {
                let scale2d_params = s::Node::Resample2D {
                    w: new_w as usize,
                    h: new_h as usize,
                    up_filter: hints.and_then(|h| h.up_filter),
                    down_filter: hints.and_then(|h| h.down_filter),
                    scaling_colorspace: hints.and_then(|h| h.scaling_colorspace),
                    hints: hints.map(|h| s::ResampleHints {
                        sharpen_percent: h.sharpen_percent,
                        prefer_1d_twice: None
                    }),
                };

                let scale2d = ctx.graph
                    .add_node(Node::new(&super::SCALE,
                                        NodeParams::Json(scale2d_params)));
                ctx.replace_node_with_existing(ix, scale2d);
            } else {
                ctx.delete_node_and_snap_together(ix);
            }
            Ok(())
        } else {
            Err(nerror!(::ErrorKind::NodeParamsMismatch, "Need Constrain, got {:?}", params))
        }
    }
}



fn scale_b_to(aspect_ratio_a_over_b: f32, a_from: u32, a_to: u32, b_from: u32) -> u32{
    let scale_factor = a_to as f32 / a_from as f32;
    let result = b_from as f32 * scale_factor;// * aspect_ratio_a_over_b;
    result.round() as u32
}
fn constrain(old_w: u32, old_h: u32, constraint: &s::Constraint) -> (u32,u32, Option<s::ConstraintResamplingHints>){
    let aspect = old_w as f32 / old_h as f32;
    match constraint.clone(){

        s::Constraint::Within{ w: Some(w), h: None, ref hints} if w < old_w => {
            (w, scale_b_to(aspect, old_w, w, old_h), *hints)
        }
        s::Constraint::Within{ w: None, h: Some(h), ref hints} if h < old_h => {
            (scale_b_to(1f32 / aspect, old_h, h, old_w), h, *hints)
        }
        s::Constraint::Within{ w: Some(w), h: Some(h), ref hints} if w < old_w || h < old_h => {

            let constraint_aspect = w as f32 / h as f32;
            if constraint_aspect > aspect{
                //height is the constraint
                (scale_b_to(1f32 / aspect, old_h, h, old_w), h, *hints)
            }else{
                //width is the constraint
                (w, scale_b_to(aspect, old_w, w, old_h), *hints)
            }
        }
        s::Constraint::Within{ ref hints, ..} => (old_w, old_h, *hints),
    }
}

#[test]
fn test_constrain(){
    //let hints = s::ConstraintResamplingHints{down_filter: None, up_filter: None, resample_when: None, sharpen_percent: None};
    {
        let constraint = s::Constraint::Within { w: Some(100), h: Some(100), hints: None };
        assert_eq!(constrain(200, 50, &constraint), (100, 25, None));
    }
    {
        let constraint = s::Constraint::Within { w: Some(100), h: Some(100), hints: None };
        assert_eq!(constrain(50, 200, &constraint), (25, 100, None));
    }
    {
        let constraint = s::Constraint::Within { w: Some(640), h: Some(480), hints: None };
        assert_eq!(constrain(200, 50, &constraint), (200, 50, None));
    }
    {
        let constraint = s::Constraint::Within { w: Some(100), h: Some(100), hints: None };
        assert_eq!(constrain(100, 100, &constraint), (100, 100, None));
    }
    {
        let constraint = s::Constraint::Within { w: Some(100), h: Some(100), hints: None };
        assert_eq!(constrain(100, 100, &constraint), (100, 100, None));
    }

}

#[derive(Debug,Clone)]
pub struct CommandStringDef;
impl NodeDef for CommandStringDef{

    fn fqn(&self) -> &'static str{
        "imazen.command_string"
    }
    fn estimate(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<FrameEstimate>{
        Ok((FrameEstimate::Impossible))
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
                    return Err(nerror!(::ErrorKind::InvalidNodeParams, "CommandString must either have decode: null or have no parent nodes. Specifying a value for decode creates a new decoder node."));
                }
                let decode_node = ::imageflow_riapi::ir4::Ir4Translate {
                    i: ::imageflow_riapi::ir4::Ir4Command::QueryString(value.to_owned()),
                    decode_id: Some(d_id),
                    encode_id: None,
                }.get_decode_node().unwrap();
                ctx.replace_node(ix, vec![
                    Node::from(decode_node),
                    Node::new(&EXPANDING_COMMAND_STRING, params)
                ]);
            } else {
                if !has_parent {
                    return Err(nerror!(::ErrorKind::InvalidNodeParams,"CommandString must have a parent node unless 'decode' has a numeric value. Otherwise it has no image source. "));
                }
                ctx.replace_node(ix, vec![
                    Node::new(&EXPANDING_COMMAND_STRING, params)
                ]);
            }
            Ok(())
        } else {
            Err(nerror!(::ErrorKind::NodeParamsMismatch, "Need Constrain, got {:?}", params))
        }
    }
}
