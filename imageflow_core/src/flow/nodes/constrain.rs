use super::internal_prelude::*;

    fn constrain_size_but_input_format(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
        let input = ctx.first_parent_frame_info_some(ix).unwrap();

        let weight = &mut ctx.weight_mut(ix);
        match weight.params {
            NodeParams::Json(s::Node::Constrain(ref constraint)) => {
                let (w, h, _) = constrain(input.w as u32, input.h as u32, constraint);
                weight.frame_est = FrameEstimate::Some(FrameInfo {
                    w: w as i32,
                    h: h as i32,
                    fmt: ffi::PixelFormat::from(input.fmt),
                    alpha_meaningful: input.alpha_meaningful,
                });
            }
            _ => {
                panic!("Node params missing");
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

fn constrain_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.constrain",
        name: "constrain",
        inbound_edges: EdgesIn::OneInput,
        description: "constrain",
        fn_estimate: Some(constrain_size_but_input_format),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                let input = ctx.first_parent_frame_info_some(ix).unwrap();
                let input_w = input.w as u32;
                let input_h = input.h as u32;
                if let s::Node::Constrain(ref constraint) =
                ctx.get_json_params(ix).unwrap() {

                    let (new_w, new_h, hints_val) = constrain(input_w, input_h, constraint);

                    let hints = &hints_val;

                    let resample_when = hints.and_then(|ref h| h.resample_when).unwrap_or(s::ResampleWhen::SizeDiffers);
                    let size_differs = new_w != input_w || new_h != input_h;
                    let sharpen_requested = hints.and_then(|h| h.sharpen_percent).unwrap_or(0f32) > 0f32;

                    let resample = match resample_when{
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
                            hints: hints.map(|h| s::ResampleHints {
                                sharpen_percent: h.sharpen_percent,
                                prefer_1d_twice: None
                            }),
                        };

                        let scale2d = ctx.graph
                            .add_node(Node::new(&super::SCALE,
                                                NodeParams::Json(scale2d_params)));
                        ctx.replace_node_with_existing(ix, scale2d);
                    }else{
                        ctx.delete_node_and_snap_together(ix);
                    }
                }

            }
            f
        }),
        ..Default::default()
    }
}






fn command_string_partially_expanded_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.expanding_command_string",
        name: "expanding_command_string",
        inbound_edges: EdgesIn::OneInput,
        description: "expanding command string",
        fn_estimate: None,
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
                let input = ctx.first_parent_frame_info_some(ix).unwrap();


                if let s::Node::CommandString{ref kind, ref value, ref decode, ref encode} =
                ctx.get_json_params(ix).unwrap() {

                    match kind {
                        &s::CommandStringKind::ImageResizer4 => {
                            let url = ::url::Url::from_str(&format!("https://fakeurl/img.jpg?{}", value)).expect("Must be a valid querystring, excluding ?");

                            let (ext, mime) = match (input.fmt, input.alpha_meaningful){
                                (PixelFormat::Bgr24, false) => ("jpg", "image/jpeg"),
                                _ => ("png", "image/png")
                            };

                            let (instructions, warnings) = ::imageflow_riapi::ir4::parsing::parse_url(&url);
                            let layout = ::imageflow_riapi::ir4::Ir4Layout::new(
                                s::ImageInfo{
                                    current_frame_index: 0,
                                    frame_count: 1,
                                    frame_decodes_into: input.fmt,
                                    image_height: input.h,
                                    image_width: input.w,
                                    preferred_extension: ext.to_owned(),
                                    preferred_mime_type: mime.to_owned(),
                                },
                                instructions
                            );
                            match layout.produce_steps(None, *encode) {
                                Ok(steps) => {
                                    ctx.replace_node(ix, steps.into_iter().map(|n| Node::from(n)).collect::<>());
                                }
                                Err(e) => {
                                    panic!("{:?} {:?}", e, warnings);
                                }
                            }
                        }
                    }
                }

            }
            f
        }),
        ..Default::default()
    }
}


fn command_string_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.command_string",
        name: "command_string",
        inbound_edges: EdgesIn::OneOptionalInput,
        description: "command string",
        fn_estimate: None,
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {

                let n = ctx.get_json_params(ix).unwrap();

                if let s::Node::CommandString{ref kind, ref value, ref decode, ref encode} = n.clone() {

                    match kind {
                        &s::CommandStringKind::ImageResizer4 => {

                            let input = ctx.first_parent_frame_info_some(ix);

                            if let &Some(d_id) = decode{
                                if input.is_some(){
                                    panic!("CommandString must either have decode: null or have no parent nodes. Specifying a value for decode creates a new decoder node.");
                                }
                                // TODO: decoder commands should be sourced from Instructions
                                ctx.replace_node(ix, vec![
                                    Node::from(s::Node::Decode {io_id: d_id, commands: None}),
                                    Node::new(&EXPANDING_COMMAND_STRING, NodeParams::Json(n))
                                ]);
                            }else{
                                if input.is_some(){
                                    panic!("CommandString must have a parent node unless 'decode' has a numeric value. Otherwise it has no image source. ");
                                }
                                ctx.replace_node(ix, vec![
                                    Node::new(&EXPANDING_COMMAND_STRING, NodeParams::Json(n))
                                ]);
                            }
                        }
                    }
                }

            }
            f
        }),
        ..Default::default()
    }
}


lazy_static! {
    pub static ref CONSTRAIN: NodeDefinition = constrain_def();
    pub static ref COMMAND_STRING: NodeDefinition = command_string_def();
    pub static ref EXPANDING_COMMAND_STRING: NodeDefinition = command_string_partially_expanded_def();
}
