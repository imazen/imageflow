use super::internal_prelude::*;
// TODO: someday look into better algorithms - see http://colorconstancy.com/ and http://ipg.fer.hr/ipg/resources/color_constancy
// http://localhost:39876/ir4/proxy_unsplash/photo-1496264057429-6a331647b69e?a.balancewhite=true&w=800
// http://localhost:39876/ir4/proxy_unsplash/photo-1496264057429-6a331647b69e?w=800

fn area_threshold(histogram: &[u64], total_pixels: u64, low_threshold: f64, high_threshold: f64) -> (usize, usize){
    let mut low = 0;
    let mut high = histogram.len() - 1;
    let mut area = 0u64;
    let pixel_count = total_pixels as f64;
    for (ix, value) in histogram.iter().enumerate(){
        area += *value as u64;
        if area as f64 / pixel_count as f64 > low_threshold {
            low = ix;
            break;
        }
    }
    area = 0u64;
    for (ix, value) in histogram.iter().enumerate().rev(){
        area += *value as u64;
        if area as f64 / pixel_count as f64 > low_threshold {
            high = ix;
            break;
        }
    }
    eprintln!("{},{}", low,high);
    (low, high)
}

fn create_byte_mapping(low: usize, high: usize) -> Vec<u8>{
    let scale = 255.0 / ((high - low) as f64);

    (0..256usize).map(|v| (v.saturating_sub(low) as f64 * scale).round().min(255f64).max(0f64) as u8).collect()
}


fn apply_mappings(bitmap: *mut BitmapBgra, map_red: &[u8], map_green: &[u8], map_blue: &[u8]){

    let input: &BitmapBgra = unsafe{ &*bitmap };
    let bytes: &mut [u8] = unsafe { slice::from_raw_parts_mut::<u8>(input.pixels, (input.stride * input.h) as usize) };

    if map_red.len() < 256 || map_green.len() < 256 || map_blue.len() < 256{
        panic!("");
    }

    match input.fmt {
        PixelFormat::Gray8 => {panic!("")},
        PixelFormat::Bgra32 => {
            for row in bytes.chunks_mut(input.stride as usize){
                for pixel in row.chunks_mut(4).take(input.w as usize){
                    //pixel[0] = map_blue[pixel[0]];
                    //pixel[1] = map_green[pixel[1]];
                    //pixel[2] = map_red[pixel[2]];
                    unsafe {
                        *pixel.get_unchecked_mut(0) = *map_blue.get_unchecked(*pixel.get_unchecked(0) as usize);
                        *pixel.get_unchecked_mut(1) = *map_green.get_unchecked(*pixel.get_unchecked(1) as usize);
                        *pixel.get_unchecked_mut(2) = *map_red.get_unchecked(*pixel.get_unchecked(2) as usize);
                    }
                }
            }
        },
        PixelFormat::Bgr24 => {
            for row in bytes.chunks_mut(input.stride as usize){
                for pixel in row.chunks_mut(3).take(input.w as usize){
                    //pixel[0] = *map_blue[pixel[0]];
                    //pixel[1] = *map_green[pixel[1]];
                    //pixel[2] = *map_red[pixel[2]];
                    unsafe {
                        *pixel.get_unchecked_mut(0) = *map_blue.get_unchecked(*pixel.get_unchecked(0) as usize);
                        *pixel.get_unchecked_mut(1) = *map_green.get_unchecked(*pixel.get_unchecked(1) as usize);
                        *pixel.get_unchecked_mut(2) = *map_red.get_unchecked(*pixel.get_unchecked(2) as usize);
                    }
                }
            }
        },
    };
}

fn white_balance_srgb_mut(bitmap: *mut BitmapBgra, histograms: &[u64;768], pixels_sampled: u64, low_threshold: Option<f32>, high_threshold: Option<f32>){
    let low_threshold = low_threshold.unwrap_or(0.006) as f64;
    let high_threshold = high_threshold.unwrap_or(0.006) as f64;

    let (red_low, red_high) = area_threshold(&histograms[0..256], pixels_sampled, low_threshold, high_threshold);
    let (green_low, green_high) = area_threshold(&histograms[256..512], pixels_sampled, low_threshold, high_threshold);
    let (blue_low, blue_high) = area_threshold(&histograms[512..768], pixels_sampled, low_threshold, high_threshold);

    let red_map = create_byte_mapping(red_low, red_high);
    let green_map = create_byte_mapping(green_low, green_high);
    let blue_map = create_byte_mapping(blue_low, blue_high);

    apply_mappings(bitmap, &red_map, &green_map, &blue_map);
}


fn white_balance_srgb_mutate_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.whitebal_srgb_area_mutate",
        name: "Auto white balance",
        description: "Auto white balance",
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_input),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex) {
                let from_node = ctx.first_parent_input_weight(ix).unwrap().clone();
                match from_node.result {
                    NodeResult::Frame(bitmap) => {
                        unsafe {
                            let mut histograms: [u64; 768] = [0; 768];
                            let mut pixels_sampled: u64 = 0;
                            if !::ffi::flow_bitmap_bgra_populate_histogram(ctx.flow_c(), bitmap, histograms.as_mut_ptr(), 256, 3,  &mut pixels_sampled as *mut u64){
                                ctx.panic_time();
                            }

                            match ctx.weight_mut(ix).params {
                                NodeParams::Json(s::Node::WhiteBalanceHistogramAreaThresholdSrgb { ref low_threshold, ref high_threshold}) => {
                                    white_balance_srgb_mut(bitmap, &histograms, pixels_sampled, *low_threshold, *high_threshold);
                                },
                                _ => {
                                    panic!("Node params missing");
                                }
                            }


                        }
                        ctx.weight_mut(ix).result = NodeResult::Frame(bitmap);
                        ctx.first_parent_input_weight_mut(ix).unwrap().result =
                            NodeResult::Consumed;
                    }
                    _ => {
                        panic!{"Previous node not ready"}
                    }
                }
            }
            f
        }),
        ..Default::default()
    }
}

fn white_balance_srgb_def() -> NodeDefinition {
    NodeDefinition {
        fqn: "imazen.whitebal_srgb_area",
        name: "Auto white balance",
        description: "Auto white balance",
        fn_estimate: Some(NodeDefHelpers::copy_frame_est_from_first_input),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex) {
                let mut new_nodes = Vec::with_capacity(2);
                if ctx.has_other_children(ctx.first_parent_input(ix).unwrap(), ix) {
                    new_nodes.push(Node::new(&CLONE, NodeParams::None));
                }
                new_nodes.push(Node::new(&WHITE_BALANCE_SRGB_MUTATE,
                                         NodeParams::Json(ctx.get_json_params(ix).unwrap())));
                ctx.replace_node(ix, new_nodes);
            }
            f
        }),
        ..Default::default()
    }
}

lazy_static! {
    pub static ref WHITE_BALANCE_SRGB: NodeDefinition = white_balance_srgb_def();
    pub static ref WHITE_BALANCE_SRGB_MUTATE: NodeDefinition = white_balance_srgb_mutate_def();
}
