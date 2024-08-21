use crate::graphics::prelude::*;
use crate::graphics::weights::*;
use itertools::max;
use multiversion::multiversion;
use rgb::alt::BGRA8;

#[derive(Copy, Clone)]
pub struct ScaleAndRenderParams {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub sharpen_percent_goal: f32,
    pub interpolation_filter: crate::graphics::weights::Filter,
    pub scale_in_colorspace: WorkingFloatspace,
}


pub unsafe fn flow_node_execute_scale2d_render1d(
    mut input: BitmapWindowMut<u8>,
    mut canvas_without_crop: BitmapWindowMut<u8>,
    info: &ScaleAndRenderParams,
    safe_path:bool
) -> Result<(),FlowError> {
    if info.h + info.y > canvas_without_crop.h()
        || info.w + info.x > canvas_without_crop.w()
    {
        return Err(nerror!(ErrorKind::InvalidArgument,
                            "Destination rectangle for scale2d is out of bounds"));
    }
    let mut cropped_canvas;
    if info.x == 0
        && info.y == 0
        && info.w == canvas_without_crop.w()
        && info.h == canvas_without_crop.h()
    {
        cropped_canvas = canvas_without_crop;
    } else {
        cropped_canvas = canvas_without_crop.window(info.x, info.y, info.x + info.w, info.y + info.h)
            .ok_or_else(|| nerror!(ErrorKind::InvalidArgument, "Crop window out of bounds"))?
    };

    if input.info().pixel_layout() != PixelLayout::BGRA {
        return Err(nerror!(ErrorKind::MethodNotImplemented));
    }
    if cropped_canvas.info().pixel_layout() != PixelLayout::BGRA {
        return Err(nerror!(ErrorKind::MethodNotImplemented));
    }

    let colorcontext = ColorContext::new(info.scale_in_colorspace, 0f32);

    let mut details = InterpolationDetails::create(info.interpolation_filter);
    details.set_sharpen_percent_goal(info.sharpen_percent_goal);

    let contrib_v = PixelRowWeights::create_for(&details, info.h, input.info().height())
        .map_err(|e| nerror!(ErrorKind::InvalidState, "Weights error: {:#?}", e))?;

    let contrib_h = PixelRowWeights::create_for(&details, info.w, input.info().width())
        .map_err(|e| nerror!(ErrorKind::InvalidState, "Weights error: {:#?}", e))?;

    if safe_path {
        render_safe(
            &colorcontext,
            &mut input,
            &contrib_h,
            &contrib_v,
            &mut cropped_canvas,
            info,
        ).map_err(|e| e.at(here!()))
    } else {
        render_unsafe(
            &colorcontext,
            &mut input,
            &contrib_h,
            &contrib_v,
            &mut cropped_canvas,
            info,
        ).map_err(|e| e.at(here!()))
    }

}

fn render_safe(cc: &ColorContext, from: &mut BitmapWindowMut<u8>, weights_x: &PixelRowWeights, weights_y: &PixelRowWeights, canvas_window: &mut BitmapWindowMut<u8>, params: &ScaleAndRenderParams) -> Result<(), FlowError> {

    let buffer_color_space = if params.scale_in_colorspace == WorkingFloatspace::LinearRGB {
        ColorSpace::LinearRGB
    } else {
        ColorSpace::StandardRGB
    };

    // Allocate buffer for summing the multiplied rows
    let mut summation_buf = Bitmap::create_float(
        from.w(), 1,PixelLayout::BGRA, true, from.info().alpha_meaningful(),
        buffer_color_space).map_err(|e| e.at(here!()))?;
    let mut summation_buf_window = summation_buf.get_window_f32().unwrap();

    // Allocate target buffer for the horizontally scaled pixels
    let mut h_scaled_buf = Bitmap::create_float(
        canvas_window.w(), 1,PixelLayout::BGRA, true, from.info().alpha_meaningful(),
        buffer_color_space).map_err(|e| e.at(here!()))?;
    let mut h_scaled_buf_window = h_scaled_buf.get_window_f32().unwrap();

    let mut float_buf = Bitmap::create_float(
        from.w(), 1,PixelLayout::BGRA, true, from.info().alpha_meaningful(),
        buffer_color_space).map_err(|e| e.at(here!()))?;
    let mut float_buf_window = float_buf.get_window_f32().unwrap();

    for out_row_ix in 0..canvas_window.h() as usize {
        let contrib = &weights_y.contrib_row()[out_row_ix];
        let contrib_weights = &weights_y.weights()
            [contrib.left_weight as usize..=contrib.right_weight as usize];

        // Clear output row
        summation_buf_window.clear_slice();

        // if out_row_ix == 0 || out_row_ix == 20{
        //     // print the contrib weights for this output row
        //     println!("Contrib weights for row {}: pulling from row {}..={} using weights {:?}", out_row_ix, contrib.left_pixel, contrib.right_pixel, contrib_weights);
        // }

        for input_row_ix in contrib.left_pixel..=contrib.right_pixel {
            bitmap_window_srgba32_to_f32x4(cc,
                &from.row_window(input_row_ix).unwrap(),
                    &mut float_buf_window);



            let weight: f32 = contrib_weights[input_row_ix as usize - contrib.left_pixel as usize];
            if (weight as f64).abs() > 0.00000002f64 {
                multiply_row_safe(
                    float_buf_window.slice_mut(),
                    weight,
                );
                // Add row
                add_row_safe(
                    summation_buf_window.slice_mut(),
                    float_buf_window.slice_mut()
                );
            }
        }
        scale_row_bgra_f32(
            summation_buf_window.get_slice(),
            from.w() as usize,
            h_scaled_buf_window.slice_mut(),
            params.w as usize,
            &weights_x,
            out_row_ix as u32,
        );
        composite_linear_over_srgb(
            cc,
            &mut h_scaled_buf_window,
            &mut canvas_window.row_window(out_row_ix as u32).unwrap()).map_err(|e| e.at(here!()))?



    }
    Ok(())
}
unsafe fn render_unsafe(colorcontext: &ColorContext, input: &mut BitmapWindowMut<u8>, contrib_h: &PixelRowWeights, contrib_v: &PixelRowWeights, cropped_canvas: &mut BitmapWindowMut<u8>, params: &ScaleAndRenderParams) -> Result<(), FlowError> {

    // Determine how many rows we need to buffer
    let max_input_rows = contrib_v.contrib_row()
        .iter()
        .map(|r| r.right_pixel - r.left_pixel + 1)
        .max()
        .ok_or_else(||nerror!(ErrorKind::InvalidState))?;

    // Allocate space
    let row_floats: usize = 4usize * input.w() as usize;

    // Allocate reusable buffer of rows for multiplying by weights
    let mut mult_buf_bitmap = Bitmap::create_float(
        input.w(), max_input_rows,PixelLayout::BGRA, true, input.info().alpha_meaningful(),
        ColorSpace::LinearRGB).map_err(|e| e.at(here!()))?;
    let mut mult_buf_window = mult_buf_bitmap.get_window_f32().unwrap();

    // Allocate coefficients and mappings to real pixel rows
    let mut mult_row_coefficients = Vec::<f32>::with_capacity(max_input_rows as usize);
    let mut mult_row_indexes =  Vec::<i32>::with_capacity(max_input_rows as usize);

    // Initialize them
    for ix in 0..max_input_rows{
        mult_row_coefficients.push(1f32);
        mult_row_indexes.push(-1i32);
    }


    // Allocate buffer for summing the multiplied rows
    let mut summation_buf = Bitmap::create_float(
        input.w(), 1,PixelLayout::BGRA, true, input.info().alpha_meaningful(),
        ColorSpace::LinearRGB).map_err(|e| e.at(here!()))?;
    let mut summation_buf_window = summation_buf.get_window_f32().unwrap();

    // Allocate target buffer for the horizontally scaled pixels
    let mut h_scaled_buf = Bitmap::create_float(
        params.w, 1,PixelLayout::BGRA, true, input.info().alpha_meaningful(),
        ColorSpace::LinearRGB).map_err(|e| e.at(here!()))?;
    let mut h_scaled_buf_window = h_scaled_buf.get_window_f32().unwrap();
    let mut h_scaled_buf_window_ffi = h_scaled_buf_window.to_bitmap_float().map_err(|e| e.at(here!()))?;

    let reuse_loaded_rows = true;

    let mut cropped_canvas_ffi = cropped_canvas.to_bitmap_bgra().map_err(|e| e.at(here!()))?;

    for out_row_ix in 0..cropped_canvas.h() as usize {
        let contrib = &contrib_v.contrib_row()[out_row_ix];
        let contrib_weights = &contrib_v.weights()
            [contrib.left_weight as usize..=contrib.right_weight as usize];

        // Clear output row
        summation_buf_window.clear_slice();

        // DOESN'T HELP
        //h_scaled_buf_window.clear_slice();

        for input_row_ix in contrib.left_pixel..=contrib.right_pixel{
            // Try to find row in buffer if already loaded
            let already_loaded_index= mult_row_indexes
                .iter()
                .position(|v| *v == input_row_ix as i32);

            // Not loaded? Look for a buffer row that we're no longer using
            let reusable_index = already_loaded_index
                .or_else(|| mult_row_indexes
                    .iter()
                    .position(|v| *v < contrib.left_pixel as i32))
                .ok_or_else(|| nerror!(ErrorKind::InvalidState))?;

            if !already_loaded_index.is_some() || reuse_loaded_rows {
                let buffer_window  =&mut mult_buf_window.row_window(reusable_index as u32).unwrap();
                // Load row
                flow_bitmap_float_convert_srgb_to_linear(
                    &colorcontext,
                    &mut input.to_bitmap_bgra()
                        .map_err(|e| e.at(here!()))?,
                    input_row_ix,
                    &mut buffer_window.to_bitmap_float().unwrap(),
                    0u32,
                    1u32,
                ).map_err(|e| e.at(here!()))?;

                //check 2nd from right pixel alpha
                // if get_pixel(&buffer_window, -2, 0)[3] < 0.01f32 {
                //     panic!("Alpha is zero at y={}", input_row_ix);
                // }
                // let px_tr = get_pixel(&buffer_window, -1, 0);
                // let px_tr_brightness = get_brightness(&px_tr);
                // if px_tr_brightness < 0.01f32 {
                //     let s = summarize_corners(&buffer_window);
                //     return Err(nerror!(ErrorKind::InvalidState));
                // }

                mult_row_coefficients[reusable_index] = 1f32;
                mult_row_indexes[reusable_index] = input_row_ix as i32;
            }
            let active_buf_ix = reusable_index;


            let weight: f32 = contrib_weights[input_row_ix as usize - contrib.left_pixel as usize];
            if (weight as f64).abs() > 0.00000002f64 {
                // Apply coefficient, update tracking
                let delta_coefficient: f32 =
                    weight / mult_row_coefficients[active_buf_ix as usize];
                multiply_row(
                    mult_buf_window.row_window(active_buf_ix as u32).unwrap().slice_ptr(),
                    row_floats,
                    delta_coefficient,
                );
                mult_row_coefficients[active_buf_ix as usize] = weight;
                // Add row
                add_row(
                    summation_buf_window.slice_ptr(),
                    mult_buf_window.row_window(active_buf_ix as u32).unwrap().slice_ptr(),
                    row_floats,
                );
            }
        }

        //check 2nd from right pixel alpha
        // if get_pixel(&summation_buf_window, -2, 0)[3] < 0.01f32 {
        //     panic!("Alpha is zero in y-scaled (not yet x-scaled) buffer at y={}", out_row_ix);
        // }
        // Now scale horizontally using scale_rows_bgra_f32

        scale_row_bgra_f32(
            summation_buf_window.get_slice(),
            input.w() as usize,
            h_scaled_buf_window.slice_mut(),
            params.w as usize,
            &contrib_h,
            out_row_ix as u32,
        );


        // flow_bitmap_float_scale_rows(
        //     &summation_buf_window.to_bitmap_float()
        //         .map_err(|e| e.at(here!()))?,
        //     0 as i32 as u32,
        //     &mut h_scaled_buf_window_ffi,
        //     0u32,
        //     1u32,
        //     &contrib_h,
        // ).map_err(|e| e.at(here!()))?;

        // let px_top_left = get_pixel(&summation_buf_window, 0, 0);
        // let px_tr_brightness = get_brightness(&px_top_left);
        // if px_tr_brightness < 0.01f32 {
        //     let s = summarize_corners(&summation_buf_window);
        //     return Err(nerror!(ErrorKind::InvalidState));
        // }
        // if get_pixel(&h_scaled_buf_window, -2, 0)[3] < 0.01f32 {
        //     panic!("Alpha is zero in buffer for canvas compositing at y={}", out_row_ix);
        // }

        flow_bitmap_float_composite_linear_over_srgb(
            &colorcontext,
            &mut h_scaled_buf_window_ffi,
            0u32,
            &mut cropped_canvas_ffi,
            out_row_ix as u32,
            1u32,
            false,
        ).map_err(|e| e.at(here!()))?;
    }
    Ok(())
}

fn get_pixel(b: &BitmapWindowMut<f32>, x:i32, y:i32) -> [f32;4]{
    // wrap negative values from right and bottom
    let x = if x < 0 { b.w() as i32 + x } else { x };
    let y = if y < 0 { b.h() as i32 + y } else { y };
    // clamp to bounds
    let x = x.max(0).min(b.w() as i32 - 1);
    let y = y.max(0).min(b.h() as i32 - 1);

    if b.info().channels() != 4{
        panic!("get_pixel called on non-4 channel bitmap")
    }



    let y_offset = (y) as usize * b.info().item_stride() as usize;
    let x_start = (x) as usize * b.info().channels() as usize + y_offset;
    let pixel = b.get_slice()[x_start..x_start + b.info().channels() as usize].as_ref();

    [pixel[0], pixel[1], pixel[2], pixel[3]]
}
fn get_brightness(pixel: &[f32;4]) -> f32{
    (pixel[0] + pixel[1] + pixel[2]) / 3.0 * pixel[3].max(0.0).min(1.0)
}
fn summarize_corners(b: &BitmapWindowMut<f32>) -> String {

    // check if entire window is zeros
    let mut all_zeros = true;
    for y in 0..b.h(){
        for x in 0..b.w(){
            let pixel = get_pixel(b, x as i32, y as i32);
            if pixel[0] != 0.0 || pixel[1] != 0.0 || pixel[2] != 0.0 || pixel[3] != 0.0{
                all_zeros = false;
                break;
            }
        }
    }
    if all_zeros{
        return "All zeros".to_string();
    }

    let bottom_right = get_pixel(b, -1,-1);
    let bottom_right2 = get_pixel(b, -2,-1);
    let top_right = get_pixel(b, -1,0);
    let top_left = get_pixel(b, 0,0);

    format!("BL: {:?},{:?}, TR: {:?}, TL: {:?}", bottom_right2, bottom_right, top_right, top_left)
}

pub fn scale_row_bgra_f32(
    source: &[f32],
    source_width: usize,
    target: &mut [f32],
    target_width: usize,
    weights: &PixelRowWeights,
    y_fyi: u32
) {
    if source.len() != source_width * 4 || target.len() != target_width * 4 {
        panic!("Mismatched source or target slice lengths: source.len={}, source_width={}, target.len={}, target_width={}", source.len(),source_width, target.len(),target_width);
    }
    let source_pixels = bytemuck::cast_slice::<f32, rgb::Bgra<f32,f32>>(source);
    let target_pixels = bytemuck::cast_slice_mut::<f32, rgb::Bgra<f32,f32>>(target);

    //check weights correspond
    if weights.weights().len() as u32 != weights.contrib_row().iter().map(|r| r.right_weight - r.left_weight + 1).sum::<u32>(){
        panic!("Mismatched weights and contrib_row lengths: weights.len={}, contrib_row.len={}", weights.weights().len(), weights.contrib_row().len());
    }
    //check target width and weights correspond
    if target_width != weights.contrib_row().len(){
        panic!("Mismatched target width and contrib_row lengths: target_width={}, contrib_row.len={}", target_width, weights.contrib_row().len());
    }

    for (dst_x, contrib) in weights.contrib_row().iter().enumerate() {
        let mut sum =  rgb::Bgra::<f32> { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

        // if (dst_x == source_width -1 || dst_x ==  source_width -2) && (y_fyi == 0 || y_fyi == 20){
        //     // print the contrib weights for this output row
        //     println!("y={} Contrib weights for column {}: pulling from column {}..={} using weights {:?}", y_fyi, dst_x, contrib.left_pixel, contrib.right_pixel, &weights.weights()[contrib.left_weight as usize..=contrib.right_weight as usize]);
        // }
        for (src_x, &weight) in (contrib.left_pixel as usize..=contrib.right_pixel as usize)
            .zip(&weights.weights()[contrib.left_weight as usize..=contrib.right_weight as usize])
        {
            let pixel: &rgb::Bgra<f32,f32> = &source_pixels[src_x];

            // if pixel[0] < 0.01 && pixel[1] < 0.01 && pixel[2] < 0.01 && dst_x > source_width - 10{
            //      println!("Dark pixel in buffer prior to horizontal scaling at y={}, x={}, x_target={}, pixel={:?}", y_fyi, src_x, dst_x, &pixel);
            // }
            sum.r += pixel.r * weight;
            sum.g += pixel.g * weight;
            sum.b += pixel.b * weight;
            sum.a += pixel.a * weight;
        }

        target_pixels[dst_x] = sum;

        // if dst_x > target_width - 4 && get_brightness(&sum) < 0.1 {
        //     println!("Dark pixel in buffer after horizontal scaling at y={}, x={}, value={:?}", y_fyi, dst_x, &sum);
        // }
    }

}

// use std::simd::{f32x4, SimdFloat};

// pub fn scale_rows_bgra_f32(
//     source: &[f32],
//     source_width: usize,
//     target: &mut [f32],
//     target_width: usize,
//     weights: &PixelRowWeights,
// ) -> Result<(), FlowError> {
//     if source.len() % 4 != 0 || target.len() % 4 != 0 {
//         return Err(nerror!(ErrorKind::InvalidState, "Source or target slice is not a multiple of 4"));
//     }

//     let source = bytemuck::cast_slice::<f32, f32x4>(source);
//     let target = bytemuck::cast_slice_mut::<f32, f32x4>(target);

//     for (dst_x, contrib) in weights.contrib_row().iter().enumerate() {
//         let mut sum = f32x4::splat(0.0);

//         for (src_x, &weight) in (contrib.left_pixel..=contrib.right_pixel)
//             .zip(&weights.weights()[contrib.left_weight as usize..=contrib.right_weight as usize])
//         {
//             sum += source[src_x as usize] * f32x4::splat(weight);
//         }

//         target[dst_x] = sum;
//     }

//     Ok(())
// }

#[cfg(target_arch = "x86_64")]
pub unsafe fn flow_bitmap_float_scale_rows(
    from: &BitmapFloat,
    from_row: u32,
    to: &mut BitmapFloat,
    to_row: u32,
    row_count: u32,
    weights: &PixelRowWeights
) -> Result<(), FlowError> {
    let from_step: u32 = from.channels;
    let to_step: u32 = to.channels;
    let dest_buffer_count: u32 = to.w;
    let min_channels: u32 = from_step.min(to_step);
    let mut ndx;
    if min_channels > 4 as i32 as u32 {
        return Err(nerror!(ErrorKind::InvalidState));
    }
    let weight_indexes = weights.contrib_row();
    let weight_values = weights.weights();
    let mut avg: [f32; 4] = [0.; 4];
    // if both have alpha, process it
    if from_step == 4 && to_step == 4 {
        let mut row: u32 = 0;
        while row < row_count {
            let source_offset = ((from_row + row) * (*from).float_stride) as isize;
            let source_buffer: *const __m128 =
                (*from).pixels.offset(source_offset) as *const __m128;
            let dest_offset = ((to_row + row) * (*to).float_stride) as isize;
            let dest_buffer: *mut __m128 = (*to).pixels.offset(dest_offset) as *mut __m128;
            let dest_buffer: &mut [__m128] =
                std::slice::from_raw_parts_mut(dest_buffer, dest_buffer_count as usize);
            ndx = 0;
            while ndx < dest_buffer_count {
                let mut sums: __m128 = _mm_set1_ps(0.0);
                let weight_index_set = weight_indexes.get_unchecked(ndx as usize);
                let left: i32 = weight_index_set.left_pixel as i32;
                let right: i32 = weight_index_set.right_pixel as i32;
                let weight_array: *const f32 = weight_values.as_ptr().offset(weight_index_set.left_weight as isize);
                let source_buffer: &[__m128] =
                    std::slice::from_raw_parts(source_buffer, (right + 1) as usize);
                /* Accumulate each channel */
                let mut i = left;
                while i <= right {
                    let factor: __m128 = _mm_set1_ps(*weight_array.offset((i - left) as isize));
                    // sums += factor * *source_buffer[i as usize];
                    let mid = _mm_mul_ps(factor, source_buffer[i as usize]);
                    sums = _mm_add_ps(sums, mid);
                    i += 1
                }
                dest_buffer[ndx as usize] = sums;
                ndx += 1
            }
            row += 1
        }
    } else if from_step == 3 as i32 as u32 && to_step == 3 as i32 as u32 {
        let mut row_0: u32 = 0 as i32 as u32;
        while row_0 < row_count {
            let source_buffer_0: *const f32 = (*from).pixels.offset(
                from_row
                    .wrapping_add(row_0)
                    .wrapping_mul((*from).float_stride) as isize,
            );
            let dest_buffer_0: *mut f32 = (*to)
                .pixels
                .offset(to_row.wrapping_add(row_0).wrapping_mul((*to).float_stride) as isize);
            ndx = 0 as i32 as u32;
            while ndx < dest_buffer_count {
                let mut bgr: [f32; 3] = [0.0f32, 0.0f32, 0.0f32];
                let weight_index_set = weight_indexes.get_unchecked(ndx as usize);
                let left: i32 = weight_index_set.left_pixel as i32;
                let right: i32 = weight_index_set.right_pixel as i32;
                let weight_array: *const f32 = weight_values.as_ptr().offset(weight_index_set.left_weight as isize);
                let mut i_0;
                /* Accumulate each channel */
                i_0 = left;
                while i_0 <= right {
                    let weight: f32 = *weight_array.offset((i_0 - left) as isize);
                    bgr[0] += weight
                        * *source_buffer_0.offset((i_0 as u32).wrapping_mul(from_step) as isize);
                    bgr[1] += weight
                        * *source_buffer_0.offset(
                        (i_0 as u32).wrapping_mul(from_step).wrapping_add(1u32) as isize,
                    );
                    bgr[2] += weight
                        * *source_buffer_0.offset(
                        (i_0 as u32).wrapping_mul(from_step).wrapping_add(2u32) as isize,
                    );
                    i_0 += 1
                }
                *dest_buffer_0.offset(ndx.wrapping_mul(to_step) as isize) = bgr[0];
                *dest_buffer_0.offset(ndx.wrapping_mul(to_step).wrapping_add(1u32) as isize) =
                    bgr[1];
                *dest_buffer_0.offset(ndx.wrapping_mul(to_step).wrapping_add(2u32) as isize) =
                    bgr[2];
                ndx = ndx.wrapping_add(1)
            }
            row_0 = row_0.wrapping_add(1)
        }
    } else {
        let mut row_1: u32 = 0 as i32 as u32;
        while row_1 < row_count {
            let source_buffer_1: *const f32 = (*from).pixels.offset(
                from_row
                    .wrapping_add(row_1)
                    .wrapping_mul((*from).float_stride) as isize,
            );
            let dest_buffer_1: *mut f32 = (*to)
                .pixels
                .offset(to_row.wrapping_add(row_1).wrapping_mul((*to).float_stride) as isize);
            ndx = 0 as i32 as u32;
            while ndx < dest_buffer_count {
                avg[0] = 0 as i32 as f32;
                avg[1] = 0 as i32 as f32;
                avg[2] = 0 as i32 as f32;
                avg[3 as i32 as usize] = 0 as i32 as f32;
                let weight_index_set = weight_indexes.get_unchecked(ndx as usize);
                let left: i32 = weight_index_set.left_pixel as i32;
                let right: i32 = weight_index_set.right_pixel as i32;
                let weight_array: *const f32 = weight_values.as_ptr().offset(weight_index_set.left_weight as isize);
                /* Accumulate each channel */
                let mut i_1: i32 = left;
                while i_1 <= right {
                    let weight_0: f32 = *weight_array.offset((i_1 - left) as isize);
                    let mut j: u32 = 0 as i32 as u32;
                    while j < min_channels {
                        avg[j as usize] += weight_0
                            * *source_buffer_1.offset(
                            (i_1 as u32).wrapping_mul(from_step).wrapping_add(j) as isize,
                        );
                        j = j.wrapping_add(1)
                    }
                    i_1 += 1
                }
                let mut j_0: u32 = 0 as i32 as u32;
                while j_0 < min_channels {
                    *dest_buffer_1.offset(ndx.wrapping_mul(to_step).wrapping_add(j_0) as isize) =
                        avg[j_0 as usize];
                    j_0 = j_0.wrapping_add(1)
                }
                ndx = ndx.wrapping_add(1)
            }
            row_1 = row_1.wrapping_add(1)
        }
    }
    Ok(())
}
unsafe fn multiply_row(row: *mut f32, length: usize, coefficient: f32) {
    let mut i: usize = 0 as i32 as usize;
    while i < length {
        *row.offset(i as isize) *= coefficient;
        i = i.wrapping_add(1)
    }
}
#[multiversion(targets("x86_64+avx"))]
fn multiply_row_safe(row: &mut [f32], coefficient: f32) {
    for v in row {
        *v *= coefficient;
    }
}
#[multiversion(targets("x86_64+avx"))]
fn add_row_safe(mutate_row: &mut [f32], input_row: &[f32]) {

    if mutate_row.len() != input_row.len(){
        panic!("Mismatched row lengths: mutate_row.len={}, input_row.len={}", mutate_row.len(), input_row.len());
    }
    // for i in 0..mutate_row.len() {
    //     mutate_row[i] += input_row[i];
    // }
    // maximum speed and simd compatibility
    for v in mutate_row.iter_mut().zip(input_row.iter()){
        *v.0 += *v.1;
    }
}

unsafe fn add_row(mutate_row: *mut f32, input_row: *mut f32, length: usize) {
    let mut i: usize = 0 as i32 as usize;
    while i < length {
        *mutate_row.offset(i as isize) += *input_row.offset(i as isize);
        i = i.wrapping_add(1)
    }
}

fn bitmap_window_srgba32_to_f32x4(colorcontext: &ColorContext, from: &BitmapWindowMut<u8>, to: &mut BitmapWindowMut<f32>){
    //Ensure the widths and heights match, and that both source and dest are 4 channels
    if from.w() != to.w() || from.h() != to.h() || from.info().channels() != 4 || to.info().channels() != 4{
        panic!("Mismatched source and dest window dimensions or channel counts");
    }

    for row_ix in 0..from.h(){
        let from_row = from.row(row_ix).unwrap();
        let to_row = to.row_mut(row_ix).unwrap();

        for x in 0..from.w() as usize{
            let pixel = &from_row[x * 4..(x + 1) * 4];
            let alpha = if !from.info().alpha_meaningful() {
              1.0
            } else{
                pixel[3] as f32 / 255.0
            };
            to_row[x * 4] = alpha * colorcontext.srgb_to_floatspace(pixel[0]);
            to_row[x * 4 + 1] = alpha * colorcontext.srgb_to_floatspace(pixel[1]);
            to_row[x * 4 + 2] = alpha * colorcontext.srgb_to_floatspace(pixel[2]);
            to_row[x * 4 + 3] = alpha; // Copy alpha directly
        }

        // // cast to BGRA8 and BGRA32 respectively
        // let from_pixels = bytemuck::cast_slice::<u8, rgb::Bgra<u8>>(from_row);
        // let to_pixels = bytemuck::cast_slice_mut::<f32, rgb::Bgra<f32>>(to_row);
        //
        // for (from_pixel, to_pixel) in from_pixels.iter().zip(to_pixels.iter_mut()){
        //     let alpha = if !from.info().alpha_meaningful() {
        //         1.0
        //     } else{
        //         from_pixel.a as f32 / 255.0
        //     };
        //     to_pixel.b = alpha * colorcontext.srgb_to_floatspace(from_pixel.b);
        //     to_pixel.g = alpha * colorcontext.srgb_to_floatspace(from_pixel.g);
        //     to_pixel.r = alpha * colorcontext.srgb_to_floatspace(from_pixel.r);
        //     to_pixel.a = alpha; // Copy alpha directly
        // }
    }
}

pub unsafe  fn flow_bitmap_float_convert_srgb_to_linear(
    colorcontext: &ColorContext,
    src: *mut flow_bitmap_bgra,
    from_row: u32,
    dest: *mut flow_bitmap_float,
    dest_row: u32,
    row_count: u32,
) -> Result<(),FlowError> {
    if ((*src).w != (*dest).w) as i32 as libc::c_long != 0 {
        return Err(nerror!(ErrorKind::InvalidState));
    }
    if !(from_row.wrapping_add(row_count) <= (*src).h
        && dest_row.wrapping_add(row_count) <= (*dest).h) as i32 as libc::c_long
        != 0
    {
        return Err(nerror!(ErrorKind::InvalidState));
    }
    let w = (*src).w;
    let units: u32 = w * flow_pixel_format_bytes_per_pixel((*src).fmt);
    let from_step: u32 = flow_pixel_format_bytes_per_pixel((*src).fmt);
    let from_copy: u32 = flow_pixel_format_channels((*src).fmt);
    let to_step: u32 = (*dest).channels;
    let copy_step: u32 = from_copy.min(to_step);
    if copy_step != 3 && copy_step != 4 {
        return Err(nerror!(ErrorKind::InvalidArgument, "copy_step={:?}", copy_step));
    }
    if copy_step == 4 && from_step != 4 && to_step != 4{
        return Err(nerror!(ErrorKind::InvalidArgument, "copy_step={:?}, from_step={:?}, to_step={:?}", copy_step, from_step, to_step));
    }
    if copy_step == 4 {
        let mut row: u32 = 0 as i32 as u32;
        while row < row_count {
            let src_start: *mut u8 = (*src)
                .pixels
                .offset(from_row.wrapping_add(row).wrapping_mul((*src).stride) as isize);
            let buf: *mut f32 = (*dest).pixels.offset(
                (*dest)
                    .float_stride
                    .wrapping_mul(row.wrapping_add(dest_row)) as isize,
            );
            let mut to_x: u32 = 0 as i32 as u32;
            let mut bix: u32 = 0 as i32 as u32;
            while bix < units {
                let alpha: f32 =
                    *src_start.offset(bix.wrapping_add(3u32) as isize) as f32 / 255.0f32;
                *buf.offset(to_x as isize) = alpha
                    * colorcontext.srgb_to_floatspace(
                    *src_start.offset(bix as isize),
                );
                *buf.offset(to_x.wrapping_add(1u32) as isize) = alpha
                    * colorcontext.srgb_to_floatspace(
                    *src_start.offset(bix.wrapping_add(1u32) as isize),
                );
                *buf.offset(to_x.wrapping_add(2u32) as isize) = alpha
                    * colorcontext.srgb_to_floatspace(
                    *src_start.offset(bix.wrapping_add(2u32) as isize),
                );
                *buf.offset(to_x.wrapping_add(3u32) as isize) = alpha;
                to_x = (to_x as u32).wrapping_add(4u32) as u32 as u32;
                bix = (bix as u32).wrapping_add(4u32) as u32 as u32
            }
            row = row.wrapping_add(1)
        }
    } else if from_step == 3 && to_step == 3 {
        let mut row: u32 = 0 as i32 as u32;
        while row < row_count {
            let src_start_0: *mut u8 = (*src)
                .pixels
                .offset(from_row.wrapping_add(row).wrapping_mul((*src).stride) as isize);
            let buf: *mut f32 = (*dest).pixels.offset(
                (*dest)
                    .float_stride
                    .wrapping_mul(row.wrapping_add(dest_row)) as isize,
            );
            let mut to_x: u32 = 0 as i32 as u32;
            let mut bix: u32 = 0 as i32 as u32;
            while bix < units {
                *buf.offset(to_x as isize) = colorcontext.srgb_to_floatspace(
                    *src_start_0.offset(bix as isize),
                );
                *buf.offset(to_x.wrapping_add(1u32) as isize) =
                    colorcontext.srgb_to_floatspace(
                        *src_start_0.offset(bix.wrapping_add(1u32) as isize),
                    );
                *buf.offset(to_x.wrapping_add(2u32) as isize) =
                    colorcontext.srgb_to_floatspace(
                        *src_start_0.offset(bix.wrapping_add(2u32) as isize),
                    );
                to_x = (to_x as u32).wrapping_add(3u32) as u32 as u32;
                bix = (bix as u32).wrapping_add(3u32) as u32 as u32
            }
            row += 1
        }
    } else if from_step == 4 && to_step == 3 {
        let mut row: u32 = 0 as i32 as u32;
        while row < row_count {
            let src_start: *mut u8 = (*src)
                .pixels
                .offset(from_row.wrapping_add(row).wrapping_mul((*src).stride) as isize);
            let buf: *mut f32 = (*dest).pixels.offset(
                (*dest)
                    .float_stride
                    .wrapping_mul(row.wrapping_add(dest_row)) as isize,
            );
            let mut to_x: u32 = 0 as i32 as u32;
            let mut bix: u32 = 0 as i32 as u32;
            while bix < units {
                *buf.offset(to_x as isize) = colorcontext.srgb_to_floatspace(
                    *src_start.offset(bix as isize),
                );
                *buf.offset(to_x.wrapping_add(1u32) as isize) =
                    colorcontext.srgb_to_floatspace(
                        *src_start.offset(bix.wrapping_add(1u32) as isize),
                    );
                *buf.offset(to_x.wrapping_add(2u32) as isize) =
                    colorcontext.srgb_to_floatspace(
                        *src_start.offset(bix.wrapping_add(2u32) as isize),
                    );
                to_x = (to_x as u32).wrapping_add(3u32) as u32 as u32;
                bix = (bix as u32).wrapping_add(4u32) as u32 as u32
            }
            row += 1
        }
    } else if from_step == 3 && to_step == 4 {
        let mut row: u32 = 0 as i32 as u32;
        while row < row_count {
            let src_start: *mut u8 = (*src)
                .pixels
                .offset(from_row.wrapping_add(row).wrapping_mul((*src).stride) as isize);
            let buf: *mut f32 = (*dest).pixels.offset(
                (*dest)
                    .float_stride
                    .wrapping_mul(row.wrapping_add(dest_row)) as isize,
            );
            let mut to_x: u32 = 0 as i32 as u32;
            let mut bix: u32 = 0 as i32 as u32;
            while bix < units {
                *buf.offset(to_x as isize) = colorcontext.srgb_to_floatspace(
                    *src_start.offset(bix as isize),
                );
                *buf.offset(to_x.wrapping_add(1u32) as isize) =
                    colorcontext.srgb_to_floatspace(
                        *src_start.offset(bix.wrapping_add(1u32) as isize),
                    );
                *buf.offset(to_x.wrapping_add(2u32) as isize) =
                    colorcontext.srgb_to_floatspace(
                        *src_start.offset(bix.wrapping_add(2u32) as isize),
                    );
                to_x = (to_x as u32).wrapping_add(4u32) as u32 as u32;
                bix = (bix as u32).wrapping_add(3u32) as u32 as u32
            }
            row += 1
        }
    } else if from_step == 4 && to_step == 4 {
        let mut row: u32 = 0 as i32 as u32;
        while row < row_count {
            let src_start: *mut u8 = (*src)
                .pixels
                .offset(from_row.wrapping_add(row).wrapping_mul((*src).stride) as isize);
            let buf: *mut f32 = (*dest).pixels.offset(
                (*dest)
                    .float_stride
                    .wrapping_mul(row.wrapping_add(dest_row)) as isize,
            );
            let mut to_x: u32 = 0 as i32 as u32;
            let mut bix: u32 = 0 as i32 as u32;
            while bix < units {
                *buf.offset(to_x as isize) = colorcontext.srgb_to_floatspace(
                    *src_start.offset(bix as isize),
                );
                *buf.offset(to_x.wrapping_add(1u32) as isize) =
                    colorcontext.srgb_to_floatspace(
                        *src_start.offset(bix.wrapping_add(1u32) as isize),
                    );
                *buf.offset(to_x.wrapping_add(2u32) as isize) =
                    colorcontext.srgb_to_floatspace(
                        *src_start.offset(bix.wrapping_add(2u32) as isize),
                    );
                to_x = (to_x as u32).wrapping_add(4u32) as u32 as u32;
                bix = (bix as u32).wrapping_add(4u32) as u32 as u32
            }
            row += 1
        }
    } else {
        return Err(nerror!(ErrorKind::InvalidArgument, "copy_step={:?}, from_step={:?}, to_step={:?}", copy_step, from_step, to_step));
    }
    Ok(())
}

pub unsafe  fn flow_bitmap_float_composite_linear_over_srgb(
    colorcontext: &ColorContext,
    src_mut: *mut flow_bitmap_float,
    from_row: u32,
    dest: *mut flow_bitmap_bgra,
    dest_row: u32,
    row_count: u32,
    transpose: bool,
) -> Result<(),FlowError> {
    if if transpose as i32 != 0 {
        ((*src_mut).w != (*dest).h) as i32
    } else {
        ((*src_mut).w != (*dest).w) as i32
    } != 0
    {
        return Err(nerror!(ErrorKind::InvalidState));
        // TODO: Add more bounds checks
    }
    if (*dest).compositing_mode == BitmapCompositingMode::BlendWithSelf
        && (*src_mut).alpha_meaningful
        && (*src_mut).channels == 4
    {
        if !(*src_mut).alpha_premultiplied {
            // Something went wrong. We should always have alpha premultiplied.
            return Err(nerror!(ErrorKind::InvalidState));
        }
        // Compose
        bitmap_float_compose_linear_over_srgb(
            colorcontext,
            src_mut,
            from_row,
            dest,
            dest_row,
            row_count,
            0u32,
            (*src_mut).w,
            transpose,
        ).map_err(|e| e.at(here!()))?;
    } else {
        if (*src_mut).channels == 4 as i32 as u32 && (*src_mut).alpha_meaningful as i32 != 0 {
            let mut demultiply: bool = (*src_mut).alpha_premultiplied;
            if (*dest).compositing_mode
                == BitmapCompositingMode::BlendWithMatte
            {
                flow_bitmap_float_blend_matte(
                    colorcontext,
                    src_mut,
                    from_row,
                    row_count,
                    (*dest).matte_color.as_mut_ptr(),
                ).map_err(|e| e.at(here!()))?;
                demultiply = false
            }
            if demultiply {
                // Demultiply before copy
                flow_bitmap_float_demultiply_alpha(src_mut, from_row, row_count)
                    .map_err(|e| e.at(here!()))?;
            }
        }
        // Copy/overwrite
        flow_bitmap_float_copy_linear_over_srgb(
            colorcontext,
            src_mut,
            from_row,
            dest,
            dest_row,
            row_count,
            0u32,
            (*src_mut).w,
            transpose,
        ).map_err(|e| e.at(here!()))?;
    } // This algorithm can't handle padding, if present
    Ok(())
}

fn composite_linear_over_srgb(
    cc: &ColorContext,
    src: &mut BitmapWindowMut<f32>,
    canvas: &mut BitmapWindowMut<u8>,
) -> Result<(), FlowError> {
    if src.info().channels() != 4 || !canvas.info().channels() == 4 {
        return Err(nerror!(ErrorKind::InvalidState));
    }

    if canvas.info().compose() == &crate::graphics::bitmaps::BitmapCompositing::BlendWithSelf
        && src.info().alpha_meaningful()
    {
        if !src.info().alpha_premultiplied() {
            return Err(nerror!(ErrorKind::InvalidState));
        }
        compose_linear_over_srgb(cc, src, canvas);
    } else {
        if src.info().alpha_meaningful() {
            if let &crate::graphics::bitmaps::BitmapCompositing::BlendWithMatte(ref color) = canvas.info().compose(){
                let matte = color.to_bgra8().map(
                    |bgra| [bgra.b, bgra.g, bgra.r, bgra.a]
                ).unwrap_or([0,0,0,0]);
                blend_matte(&cc,src, matte).map_err(|e| e.at(here!()))?;
            }
            if src.info().alpha_premultiplied() {
                demultiply_alpha(src).map_err(|e| e.at(here!()))?;
            }
        }
        copy_linear_over_srgb(cc, src, canvas).map_err(|e| e.at(here!()))?;
    }
    Ok(())
}

unsafe fn flow_bitmap_float_blend_matte(
    colorcontext: &ColorContext,
    src: *mut flow_bitmap_float,
    from_row: u32,
    row_count: u32,
    matte: *const u8,
) -> Result<(), FlowError> {
    // We assume that matte is BGRA, regardless.
    let matte_a: f32 = *matte.offset(3 as i32 as isize) as f32 / 255.0f32;
    let b: f32 = colorcontext.srgb_to_floatspace(*matte.offset(0));
    let g: f32 = colorcontext.srgb_to_floatspace(*matte.offset(1));
    let r: f32 = colorcontext.srgb_to_floatspace(*matte.offset(2));
    let mut row: u32 = from_row;
    while row < from_row.wrapping_add(row_count) {
        let start_ix: u32 = row.wrapping_mul((*src).float_stride);
        let end_ix: u32 = start_ix.wrapping_add((*src).w.wrapping_mul((*src).channels));
        let mut ix: u32 = start_ix;
        while ix < end_ix {
            let src_a: f32 = *(*src).pixels.offset(ix.wrapping_add(3u32) as isize);
            let a: f32 = (1.0f32 - src_a) * matte_a;
            let final_alpha: f32 = src_a + a;
            *(*src).pixels.offset(ix as isize) =
                (*(*src).pixels.offset(ix as isize) + b * a) / final_alpha;
            *(*src).pixels.offset(ix.wrapping_add(1u32) as isize) =
                (*(*src).pixels.offset(ix.wrapping_add(1u32) as isize) + g * a) / final_alpha;
            *(*src).pixels.offset(ix.wrapping_add(2u32) as isize) =
                (*(*src).pixels.offset(ix.wrapping_add(2u32) as isize) + r * a) / final_alpha;
            *(*src).pixels.offset(ix.wrapping_add(3u32) as isize) = final_alpha;
            ix = (ix as u32).wrapping_add(4u32) as u32 as u32
        }
        row = row.wrapping_add(1)
    }
    // Ensure alpha is demultiplied
    Ok(())
}
fn blend_matte(cc: &ColorContext, bitmap: &mut BitmapWindowMut<f32>, matte: [u8;4]) -> Result<(), FlowError> {
    let matte_a: f32 = matte[3] as f32 / 255.0f32;
    let b: f32 = cc.srgb_to_floatspace(matte[0]);
    let g: f32 = cc.srgb_to_floatspace(matte[1]);
    let r: f32 = cc.srgb_to_floatspace(matte[2]);
    let h = bitmap.h();
    let w = bitmap.w();
    for row in 0..h as usize {
        let slice = bitmap.row_mut(row as u32).unwrap();
        for col in 0..w as usize {
            let alpha = slice[col * 4 + 3];
            let a: f32 = (1.0f32 - alpha) * matte_a;
            let final_alpha: f32 = alpha + a;
            if alpha > 0 as i32 as f32 {
                slice[col * 4 + 0] = (slice[col * 4 + 0] + b * a) / final_alpha;
                slice[col * 4 + 1] = (slice[col * 4 + 1] + g * a) / final_alpha;
                slice[col * 4 + 2] = (slice[col * 4 + 2] + r * a) / final_alpha;
            }
        }
    }
    Ok(())
}

unsafe fn flow_bitmap_float_demultiply_alpha(
    src: *mut flow_bitmap_float,
    from_row: u32,
    row_count: u32,
) -> Result<(), FlowError> {
    let mut row: u32 = from_row;
    while row < from_row.wrapping_add(row_count) {
        let start_ix: u32 = row.wrapping_mul((*src).float_stride);
        let end_ix: u32 = start_ix.wrapping_add((*src).w.wrapping_mul((*src).channels));
        let mut ix: u32 = start_ix;
        while ix < end_ix {
            let alpha: f32 = *(*src).pixels.offset(ix.wrapping_add(3u32) as isize);
            if alpha > 0 as i32 as f32 {
                *(*src).pixels.offset(ix as isize) /= alpha;
                *(*src).pixels.offset(ix.wrapping_add(1u32) as isize) /= alpha;
                *(*src).pixels.offset(ix.wrapping_add(2u32) as isize) /= alpha
            }
            ix = (ix as u32).wrapping_add(4u32) as u32 as u32
        }
        row = row.wrapping_add(1)
    }
    Ok(())
}


fn demultiply_alpha(bitmap: &mut BitmapWindowMut<f32>) -> Result<(), FlowError> {
    // verify channels == 4
    if bitmap.info().channels() != 4 || !bitmap.info().alpha_meaningful() || !bitmap.info().alpha_premultiplied() {
        return Err(nerror!(ErrorKind::InvalidState));
    }
    let (w,h) = (bitmap.w() as usize, bitmap.h() as usize);
    for row in 0..h {

        let slice = bitmap.row_mut(row as u32).unwrap();


        for col in 0..w as usize{
            let alpha = slice[col * 4 + 3];
            if alpha > 0 as i32 as f32 {
                slice[col * 4 + 0] /= alpha;
                slice[col * 4 + 1] /= alpha;
                slice[col * 4 + 2] /= alpha;
            }
        }
        // // SIMD friendly
        // let pixel_row = bytemuck::cast_slice_mut::<f32, rgb::Bgra<f32>>(slice);
        // for pixel in pixel_row.iter_mut() {
        //     if pixel.a > 0 as i32 as f32 {
        //         pixel.b /= pixel.a;
        //         pixel.g /= pixel.a;
        //         pixel.r /= pixel.a;
        //     }
        // }
    }
    Ok(())
}


unsafe fn bitmap_float_compose_linear_over_srgb(
    colorcontext: &ColorContext,
    src: *mut flow_bitmap_float,
    from_row: u32,
    dest: *mut flow_bitmap_bgra,
    dest_row: u32,
    row_count: u32,
    from_col: u32,
    col_count: u32,
    transpose: bool,
) -> Result<(),FlowError> {
    let dest_bytes_pp: u32 = flow_pixel_format_bytes_per_pixel((*dest).fmt);
    let dest_row_stride: u32 = if transpose as i32 != 0 {
        dest_bytes_pp
    } else {
        (*dest).stride
    };
    let dest_pixel_stride: u32 = if transpose as i32 != 0 {
        (*dest).stride
    } else {
        dest_bytes_pp
    };
    let srcitems: u32 = from_col
        .wrapping_add(col_count)
        .min((*src).w)
        .wrapping_mul((*src).channels);
    let ch: u32 = (*src).channels;
    let dest_effective_format: PixelFormat = (*dest).fmt;
    let dest_alpha: bool = dest_effective_format == PixelFormat::Bgra32;
    let dest_alpha_index: u8 = if dest_alpha as i32 != 0 {
        3 as i32
    } else {
        0 as i32
    } as u8;
    let dest_alpha_to_float_coeff: f32 = if dest_alpha as i32 != 0 {
        (1.0f32) / 255.0f32
    } else {
        0.0f32
    };
    let dest_alpha_to_float_offset: f32 = if dest_alpha as i32 != 0 {
        0.0f32
    } else {
        1.0f32
    };
    let mut row: u32 = 0 as i32 as u32;
    while row < row_count {
        // const float * const __restrict src_row = src->pixels + (row + from_row) * src->float_stride;
        let src_row: *mut f32 = (*src)
            .pixels
            .offset(row.wrapping_add(from_row).wrapping_mul((*src).float_stride) as isize);
        let mut dest_row_bytes: *mut u8 = (*dest)
            .pixels
            .offset(dest_row.wrapping_add(row).wrapping_mul(dest_row_stride) as isize)
            .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
        let mut ix: u32 = from_col.wrapping_mul(ch);
        while ix < srcitems {
            let dest_b: u8 = *dest_row_bytes.offset(0);
            let dest_g: u8 = *dest_row_bytes.offset(1);
            let dest_r: u8 = *dest_row_bytes.offset(2);
            let dest_a: u8 = *dest_row_bytes.offset(dest_alpha_index as isize);
            let src_b: f32 = *src_row.offset(ix.wrapping_add(0u32) as isize);
            let src_g: f32 = *src_row.offset(ix.wrapping_add(1u32) as isize);
            let src_r: f32 = *src_row.offset(ix.wrapping_add(2u32) as isize);
            let src_a: f32 = *src_row.offset(ix.wrapping_add(3u32) as isize);
            let a: f32 = (1.0f32 - src_a)
                * (dest_alpha_to_float_coeff * dest_a as i32 as f32 + dest_alpha_to_float_offset);
            let b: f32 = colorcontext.srgb_to_floatspace(dest_b) * a + src_b;
            let g: f32 = colorcontext.srgb_to_floatspace(dest_g) * a + src_g;
            let r: f32 = colorcontext.srgb_to_floatspace(dest_r) * a + src_r;
            let final_alpha: f32 = src_a + a;
            *dest_row_bytes.offset(0) =
                colorcontext.floatspace_to_srgb(b / final_alpha);
            *dest_row_bytes.offset(1) =
                colorcontext.floatspace_to_srgb(g / final_alpha);
            *dest_row_bytes.offset(2) =
                colorcontext.floatspace_to_srgb(r / final_alpha);
            if dest_alpha {
                *dest_row_bytes.offset(3i32 as isize) =
                    uchar_clamp_ff(final_alpha * 255 as i32 as f32)
            }
            // TODO: split out 4 and 3 so compiler can vectorize maybe?
            dest_row_bytes = dest_row_bytes.offset(dest_pixel_stride as isize);
            ix = (ix as u32).wrapping_add(ch) as u32 as u32
        }
        row = row.wrapping_add(1)
    }
    Ok(())
}

fn compose_linear_over_srgb(
    cc: &ColorContext,
    src:   &BitmapWindowMut<f32>,
    canvas: &mut BitmapWindowMut<u8>){

    let dest_alpha_coeff = if canvas.info().alpha_meaningful() { 1.0f32 / 255.0f32 } else { 0.0f32 };
    let dest_alpha_offset = if canvas.info().alpha_meaningful() { 0.0f32 } else { 1.0f32 };

    for row in 0..src.h() as usize  {
        let src_slice = src.row(row as u32).unwrap();
        let canvas_slice = canvas.row_mut(row as u32).unwrap();


        //
        for col in 0..src.w() as usize{
            let src_a = src_slice[col * 4 + 3];
            if src_a == 1.0f32 || !src.info().alpha_meaningful() {
                canvas_slice[col * 4 + 0] = cc.floatspace_to_srgb(src_slice[col * 4 + 0]);
                canvas_slice[col * 4 + 1] = cc.floatspace_to_srgb(src_slice[col * 4 + 1]);
                canvas_slice[col * 4 + 2] = cc.floatspace_to_srgb(src_slice[col * 4 + 2]);
                canvas_slice[col * 4 + 3] = 255;
            } else {
                let dest_a = canvas_slice[col * 4 + 3];

                let dest_coeff = (1.0f32 - src_a) * (dest_alpha_coeff * dest_a as i32 as f32 + dest_alpha_offset);
                let final_alpha = src_a + dest_coeff;
                canvas_slice[col * 4 + 0] = cc.floatspace_to_srgb((src_slice[col * 4 + 0] + dest_coeff * cc.srgb_to_floatspace(canvas_slice[col * 4 + 0])) / final_alpha);
                canvas_slice[col * 4 + 1] = cc.floatspace_to_srgb((src_slice[col * 4 + 1] + dest_coeff * cc.srgb_to_floatspace(canvas_slice[col * 4 + 1])) / final_alpha);
                canvas_slice[col * 4 + 2] = cc.floatspace_to_srgb((src_slice[col * 4 + 2] + dest_coeff * cc.srgb_to_floatspace(canvas_slice[col * 4 + 2])) / final_alpha);
                canvas_slice[col * 4 + 3] = uchar_clamp_ff(final_alpha * 255 as i32 as f32);
            }
        }
        // let src_pixels = bytemuck::cast_slice::<f32, rgb::Bgra<f32>>(src_slice);
        // let canvas_pixels = bytemuck::cast_slice_mut::<u8, rgb::Bgra<u8>>(canvas_slice);
        // for (src_pixel, canvas_pixel) in
        //         src_pixels.iter().zip(canvas_pixels.iter_mut()) {
        //     let src_a = src_pixel.a;
        //     if src_a == 1.0 || !src.info().alpha_meaningful() {
        //         canvas_pixel.b = cc.floatspace_to_srgb(src_pixel.b);
        //         canvas_pixel.g = cc.floatspace_to_srgb(src_pixel.g);
        //         canvas_pixel.r = cc.floatspace_to_srgb(src_pixel.r);
        //         canvas_pixel.a = 255;
        //     } else {
        //         let dest_a = canvas_pixel.a;
        //         let dest_coeff = (1.0 - src_a) * (dest_alpha_coeff * dest_a as f32 + dest_alpha_offset);
        //         let final_alpha = src_a + dest_coeff;
        //         canvas_pixel.b = cc.floatspace_to_srgb((src_pixel.b + dest_coeff * cc.srgb_to_floatspace(canvas_pixel.b)) / final_alpha);
        //         canvas_pixel.g = cc.floatspace_to_srgb((src_pixel.g + dest_coeff * cc.srgb_to_floatspace(canvas_pixel.g)) / final_alpha);
        //         canvas_pixel.r = cc.floatspace_to_srgb((src_pixel.r + dest_coeff * cc.srgb_to_floatspace(canvas_pixel.r)) / final_alpha);
        //         canvas_pixel.a = uchar_clamp_ff(final_alpha * 255.0);
        //     }
        // }


    }

}

fn copy_linear_over_srgb(
    cc: &ColorContext,
    from: &BitmapWindowMut<f32>,
    canvas: &mut BitmapWindowMut<u8>,
) -> Result<(), FlowError> {
    // w,h, channels must match
    if from.w() != canvas.w() || from.h() != canvas.h() || from.info().channels() != canvas.info().channels() {
        return Err(nerror!(ErrorKind::InvalidState));
    }
    let clear_alpha: bool = !from.info().alpha_meaningful() && canvas.info().alpha_meaningful();
    let copy_alpha: bool = from.info().alpha_meaningful() && canvas.info().alpha_meaningful();
    for row in 0..from.h() as usize{
        let input_slice = from.row(row as u32).unwrap();
        let canvas_slice = canvas.row_mut(row as u32).unwrap();

        // let input_pixels = bytemuck::cast_slice::<f32, rgb::Bgra<f32>>(input_slice);
        // let canvas_pixels = bytemuck::cast_slice_mut::<u8, rgb::Bgra<u8>>(canvas_slice);
        // for (input_pixel, canvas_pixel) in input_pixels.iter().zip(canvas_pixels.iter_mut()) {
        //     canvas_pixel.b = cc.floatspace_to_srgb(input_pixel.b);
        //     canvas_pixel.g = cc.floatspace_to_srgb(input_pixel.g);
        //     canvas_pixel.r = cc.floatspace_to_srgb(input_pixel.r);
        //     canvas_pixel.a = uchar_clamp_ff(input_pixel.a * 255.0);
        //     if clear_alpha {
        //         canvas_pixel.a = 255;
        //     }
        //     if copy_alpha {
        //         canvas_pixel.a = uchar_clamp_ff(input_pixel.a * 255.0);
        //     }
        // }
        for col in 0..from.w() as usize{
            canvas_slice[col * 4 + 0] = cc.floatspace_to_srgb(input_slice[col * 4 + 0]);
            canvas_slice[col * 4 + 1] = cc.floatspace_to_srgb(input_slice[col * 4 + 1]);
            canvas_slice[col * 4 + 2] = cc.floatspace_to_srgb(input_slice[col * 4 + 2]);
            if copy_alpha {
                canvas_slice[col * 4 + 3] = uchar_clamp_ff(input_slice[col * 4 + 3] * 255.0f32);
            }
            if clear_alpha {
                canvas_slice[col * 4 + 3] = 255;
            }
        }
    }

    Ok(())
}


unsafe fn flow_bitmap_float_copy_linear_over_srgb(
    colorcontext: &ColorContext,
    src: *mut flow_bitmap_float,
    from_row: u32,
    dest: *mut flow_bitmap_bgra,
    dest_row: u32,
    row_count: u32,
    from_col: u32,
    col_count: u32,
    transpose: bool,
) -> Result<(), FlowError> {
    let dest_bytes_pp: u32 = flow_pixel_format_bytes_per_pixel((*dest).fmt);
    let srcitems: u32 = from_col
        .wrapping_add(col_count)
        .min((*src).w)
        .wrapping_mul((*src).channels);
    let dest_fmt = (*dest).fmt;
    let ch: u32 = (*src).channels;
    let copy_alpha: bool = dest_fmt == PixelFormat::Bgra32
        && ch == 4 as i32 as u32
        && (*src).alpha_meaningful as i32 != 0;
    let clean_alpha: bool = !copy_alpha && dest_fmt == PixelFormat::Bgra32;
    let dest_row_stride: u32 = if transpose as i32 != 0 {
        dest_bytes_pp
    } else {
        (*dest).stride
    };
    let dest_pixel_stride: u32 = if transpose as i32 != 0 {
        (*dest).stride
    } else {
        dest_bytes_pp
    };
    if dest_pixel_stride == 4 as i32 as u32 {
        if ch == 3 as i32 as u32 {
            if copy_alpha && !clean_alpha {
                let mut row: u32 = 0 as i32 as u32;
                while row < row_count {
                    let src_row: *mut f32 =
                        (*src)
                            .pixels
                            .offset(row.wrapping_add(from_row).wrapping_mul((*src).float_stride)
                                as isize);
                    let mut dest_row_bytes: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix: u32 = from_col.wrapping_mul(3u32);
                    while ix < srcitems {
                        *dest_row_bytes.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row.offset(ix as isize),
                        );
                        *dest_row_bytes.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row.offset(ix.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row.offset(ix.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes.offset(3 as i32 as isize) = uchar_clamp_ff(
                            *src_row.offset(ix.wrapping_add(3u32) as isize) * 255.0f32,
                        );
                        dest_row_bytes = dest_row_bytes.offset(4 as i32 as isize);
                        ix = (ix as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row = row.wrapping_add(1)
                }
            }
            if !copy_alpha && !clean_alpha {
                let mut row_0: u32 = 0 as i32 as u32;
                while row_0 < row_count {
                    let src_row_0: *mut f32 = (*src).pixels.offset(
                        row_0
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_0: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_0).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix_0: u32 = from_col.wrapping_mul(3u32);
                    while ix_0 < srcitems {
                        *dest_row_bytes_0.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_0.offset(ix_0 as isize),
                        );
                        *dest_row_bytes_0.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_0.offset(ix_0.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_0.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_0.offset(ix_0.wrapping_add(2u32) as isize),
                        );
                        dest_row_bytes_0 = dest_row_bytes_0.offset(4 as i32 as isize);
                        ix_0 = (ix_0 as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row_0 = row_0.wrapping_add(1)
                }
            }
            if !copy_alpha && clean_alpha {
                let mut row_1: u32 = 0 as i32 as u32;
                while row_1 < row_count {
                    let src_row_1: *mut f32 = (*src).pixels.offset(
                        row_1
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_1: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_1).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix_1: u32 = from_col.wrapping_mul(3u32);
                    while ix_1 < srcitems {
                        *dest_row_bytes_1.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_1.offset(ix_1 as isize),
                        );
                        *dest_row_bytes_1.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_1.offset(ix_1.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_1.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_1.offset(ix_1.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_1.offset(3 as i32 as isize) = 0xff as i32 as u8;
                        dest_row_bytes_1 = dest_row_bytes_1.offset(4 as i32 as isize);
                        ix_1 = (ix_1 as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row_1 = row_1.wrapping_add(1)
                }
            }
        }
        if ch == 4 as i32 as u32 {
            if copy_alpha && !clean_alpha {
                let mut row_2: u32 = 0 as i32 as u32;
                while row_2 < row_count {
                    let src_row_2: *mut f32 = (*src).pixels.offset(
                        row_2
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_2: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_2).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix_2: u32 = from_col.wrapping_mul(4u32);
                    while ix_2 < srcitems {
                        *dest_row_bytes_2.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_2.offset(ix_2 as isize),
                        );
                        *dest_row_bytes_2.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_2.offset(ix_2.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_2.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_2.offset(ix_2.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_2.offset(3 as i32 as isize) = uchar_clamp_ff(
                            *src_row_2.offset(ix_2.wrapping_add(3u32) as isize) * 255.0f32,
                        );
                        dest_row_bytes_2 = dest_row_bytes_2.offset(4 as i32 as isize);
                        ix_2 = (ix_2 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_2 = row_2.wrapping_add(1)
                }
            }
            if !copy_alpha && !clean_alpha {
                let mut row_3: u32 = 0 as i32 as u32;
                while row_3 < row_count {
                    let src_row_3: *mut f32 = (*src).pixels.offset(
                        row_3
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_3: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_3).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix_3: u32 = from_col.wrapping_mul(4u32);
                    while ix_3 < srcitems {
                        *dest_row_bytes_3.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_3.offset(ix_3 as isize),
                        );
                        *dest_row_bytes_3.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_3.offset(ix_3.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_3.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_3.offset(ix_3.wrapping_add(2u32) as isize),
                        );
                        dest_row_bytes_3 = dest_row_bytes_3.offset(4 as i32 as isize);
                        ix_3 = (ix_3 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_3 = row_3.wrapping_add(1)
                }
            }
            if !copy_alpha && clean_alpha {
                let mut row_4: u32 = 0 as i32 as u32;
                while row_4 < row_count {
                    let src_row_4: *mut f32 = (*src).pixels.offset(
                        row_4
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_4: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_4).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix_4: u32 = from_col.wrapping_mul(4u32);
                    while ix_4 < srcitems {
                        *dest_row_bytes_4.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_4.offset(ix_4 as isize),
                        );
                        *dest_row_bytes_4.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_4.offset(ix_4.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_4.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_4.offset(ix_4.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_4.offset(3 as i32 as isize) = 0xff as i32 as u8;
                        dest_row_bytes_4 = dest_row_bytes_4.offset(4 as i32 as isize);
                        ix_4 = (ix_4 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_4 = row_4.wrapping_add(1)
                }
            }
        }
    } else {
        if ch == 3 as i32 as u32 {
            if copy_alpha && !clean_alpha {
                let mut row_5: u32 = 0 as i32 as u32;
                while row_5 < row_count {
                    let src_row_5: *mut f32 = (*src).pixels.offset(
                        row_5
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_5: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_5).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_5: u32 = from_col.wrapping_mul(3u32);
                    while ix_5 < srcitems {
                        *dest_row_bytes_5.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_5.offset(ix_5 as isize),
                        );
                        *dest_row_bytes_5.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_5.offset(ix_5.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_5.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_5.offset(ix_5.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_5.offset(3 as i32 as isize) = uchar_clamp_ff(
                            *src_row_5.offset(ix_5.wrapping_add(3u32) as isize) * 255.0f32,
                        );
                        dest_row_bytes_5 = dest_row_bytes_5.offset(dest_pixel_stride as isize);
                        ix_5 = (ix_5 as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row_5 = row_5.wrapping_add(1)
                }
            }
            if !copy_alpha && !clean_alpha {
                let mut row_6: u32 = 0 as i32 as u32;
                while row_6 < row_count {
                    let src_row_6: *mut f32 = (*src).pixels.offset(
                        row_6
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_6: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_6).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_6: u32 = from_col.wrapping_mul(3u32);
                    while ix_6 < srcitems {
                        *dest_row_bytes_6.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_6.offset(ix_6 as isize),
                        );
                        *dest_row_bytes_6.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_6.offset(ix_6.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_6.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_6.offset(ix_6.wrapping_add(2u32) as isize),
                        );
                        dest_row_bytes_6 = dest_row_bytes_6.offset(dest_pixel_stride as isize);
                        ix_6 = (ix_6 as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row_6 = row_6.wrapping_add(1)
                }
            }
            if !copy_alpha && clean_alpha {
                let mut row_7: u32 = 0 as i32 as u32;
                while row_7 < row_count {
                    let src_row_7: *mut f32 = (*src).pixels.offset(
                        row_7
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_7: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_7).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_7: u32 = from_col.wrapping_mul(3u32);
                    while ix_7 < srcitems {
                        *dest_row_bytes_7.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_7.offset(ix_7 as isize),
                        );
                        *dest_row_bytes_7.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_7.offset(ix_7.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_7.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_7.offset(ix_7.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_7.offset(3 as i32 as isize) = 0xff as i32 as u8;
                        dest_row_bytes_7 = dest_row_bytes_7.offset(dest_pixel_stride as isize);
                        ix_7 = (ix_7 as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row_7 = row_7.wrapping_add(1)
                }
            }
        }
        if ch == 4 as i32 as u32 {
            if copy_alpha && !clean_alpha {
                let mut row_8: u32 = 0 as i32 as u32;
                while row_8 < row_count {
                    let src_row_8: *mut f32 = (*src).pixels.offset(
                        row_8
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_8: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_8).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_8: u32 = from_col.wrapping_mul(4u32);
                    while ix_8 < srcitems {
                        *dest_row_bytes_8.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_8.offset(ix_8 as isize),
                        );
                        *dest_row_bytes_8.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_8.offset(ix_8.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_8.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_8.offset(ix_8.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_8.offset(3 as i32 as isize) = uchar_clamp_ff(
                            *src_row_8.offset(ix_8.wrapping_add(3u32) as isize) * 255.0f32,
                        );
                        dest_row_bytes_8 = dest_row_bytes_8.offset(dest_pixel_stride as isize);
                        ix_8 = (ix_8 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_8 = row_8.wrapping_add(1)
                }
            }
            if !copy_alpha && !clean_alpha {
                let mut row_9: u32 = 0 as i32 as u32;
                while row_9 < row_count {
                    let src_row_9: *mut f32 = (*src).pixels.offset(
                        row_9
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_9: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_9).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_9: u32 = from_col.wrapping_mul(4u32);
                    while ix_9 < srcitems {
                        *dest_row_bytes_9.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_9.offset(ix_9 as isize),
                        );
                        *dest_row_bytes_9.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_9.offset(ix_9.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_9.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_9.offset(ix_9.wrapping_add(2u32) as isize),
                        );
                        dest_row_bytes_9 = dest_row_bytes_9.offset(dest_pixel_stride as isize);
                        ix_9 = (ix_9 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_9 = row_9.wrapping_add(1)
                }
            }
            if !copy_alpha && clean_alpha {
                let mut row_10: u32 = 0 as i32 as u32;
                while row_10 < row_count {
                    let src_row_10: *mut f32 = (*src).pixels.offset(
                        row_10
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_10: *mut u8 =
                        (*dest)
                            .pixels
                            .offset(dest_row.wrapping_add(row_10).wrapping_mul(dest_row_stride)
                                as isize)
                            .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_10: u32 = from_col.wrapping_mul(4u32);
                    while ix_10 < srcitems {
                        *dest_row_bytes_10.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_10.offset(ix_10 as isize),
                        );
                        *dest_row_bytes_10.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_10.offset(ix_10.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_10.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_10.offset(ix_10.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_10.offset(3 as i32 as isize) = 0xff as i32 as u8;
                        dest_row_bytes_10 = dest_row_bytes_10.offset(dest_pixel_stride as isize);
                        ix_10 = (ix_10 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_10 = row_10.wrapping_add(1)
                }
            }
        }
    }
    Ok(())
}
