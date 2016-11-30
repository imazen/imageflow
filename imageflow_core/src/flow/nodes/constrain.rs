use super::internal_prelude::*;

    fn constrain_size_but_input_format(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
        let input = ctx.first_parent_frame_info_some(ix).unwrap();

        let ref mut weight = ctx.weight_mut(ix);
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
    let result = b_from as f32 * scale_factor / aspect_ratio_a_over_b;
    result.round() as u32
}
fn constrain(old_w: u32, old_h: u32, constraint: &s::Constraint) -> (u32,u32, Option<s::ConstraintResamplingHints>){
    let aspect = old_w as f32 / old_h as f32;
    match constraint.clone(){

        s::Constraint::Within{ w: Some(w), h: None, ref hints} if w < old_w => {
            (w, scale_b_to(aspect, old_w, w, old_h), hints.clone())
        }
        s::Constraint::Within{ w: None, h: Some(h), ref hints} if h < old_h => {
            (scale_b_to(1f32 / aspect, old_h, h, old_w), h, hints.clone())
        }
        s::Constraint::Within{ w: Some(w), h: Some(h), ref hints} if w < old_w || h < old_h => {

            let constraint_aspect = w as f32 / h as f32;
            if constraint_aspect > aspect{
                //height is the constraint
                (scale_b_to(1f32 / aspect, old_h, h, old_w), h, hints.clone())
            }else{
                //width is the constraint
                (w, scale_b_to(aspect, old_w, w, old_h), hints.clone())
            }
        }
        s::Constraint::Within{ ref hints, ..} => (old_w, old_h, hints.clone()),
    }
}

#[test]
fn test_constrain(){
    //let hints = s::ConstraintResamplingHints{down_filter: None, up_filter: None, resample_when: None, sharpen_percent: None};
    {
        let constraint = s::Constraint { w: 100, h: 100, hints: None };
        assert_eq!(constrain(200, 50, &constraint), (100, 25, None));
    }
    {
        let constraint = s::Constraint { w: 100, h: 100, hints: None };
        assert_eq!(constrain(50, 200, &constraint), (25, 100, None));
    }
    {
        let constraint = s::Constraint { w: 640, h: 480, hints: None };
        assert_eq!(constrain(200, 50, &constraint), (200, 50, None));
    }
    {
        let constraint = s::Constraint { w: 100, h: 100, hints: None };
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
                                sharpen_percent: h.sharpen_percent.clone(),
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


lazy_static! {
    pub static ref CONSTRAIN: NodeDefinition = constrain_def();
}
