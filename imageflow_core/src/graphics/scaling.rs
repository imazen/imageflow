use bytemuck::Zeroable;
use crate::graphics::prelude::*;
use crate::graphics::weights::*;
use itertools::max;
use multiversion::multiversion;
use rgb::alt::BGRA8;
#[cfg(feature = "nightly")]
use std::simd::{Simd};

#[cfg(feature = "nightly")]
use  std::simd::prelude::SimdUint;

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

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
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct Bgra32 {
    /// Blue Component
    pub b: u8,
    /// Green Component
    pub g: u8,
    /// Red Component
    pub r: u8,
    /// Alpha Component
    pub a: u8,
}
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, PartialOrd)]
struct Bgra128 {
    /// Blue Component
    pub b: f32,
    /// Green Component
    pub g: f32,
    /// Red Component
    pub r: f32,
    /// Alpha Component
    pub a: f32,
}

unsafe impl bytemuck::Pod for Bgra32 {}
unsafe impl Zeroable for Bgra32 {}
unsafe impl bytemuck::Pod for Bgra128 {}
unsafe impl Zeroable for Bgra128 {}


pub fn scale_and_render(
    mut input: BitmapWindowMut<u8>,
    mut canvas_without_crop: BitmapWindowMut<u8>,
    info: &ScaleAndRenderParams,
) -> Result<(), FlowError> {
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

    render_safe(
        &colorcontext,
        &mut input,
        &contrib_h,
        &contrib_v,
        &mut cropped_canvas,
        info,
    ).map_err(|e| e.at(here!()))
}

fn render_safe(cc: &ColorContext, from: &mut BitmapWindowMut<u8>, weights_x: &PixelRowWeights, weights_y: &PixelRowWeights, canvas_window: &mut BitmapWindowMut<u8>, params: &ScaleAndRenderParams) -> Result<(), FlowError> {

    // for benchmarking
    // $env:RUSTFLAGS='-C target-cpu=native'; cargo bench --features c_rendering

    let buffer_color_space = if params.scale_in_colorspace == WorkingFloatspace::LinearRGB {
        ColorSpace::LinearRGB
    } else {
        ColorSpace::StandardRGB
    };

    // Determine how many rows we need to buffer
    let max_input_rows = weights_y.contrib_row()
        .iter()
        .map(|r| r.right_pixel - r.left_pixel + 1)
        .max()
        .ok_or_else(|| nerror!(ErrorKind::InvalidState))?;

    // Allocate reusable buffer of rows for multiplying by weights
    let mut mult_buf_bitmap = Bitmap::create_float(
        from.w(), max_input_rows, PixelLayout::BGRA, true, from.info().alpha_meaningful(),
        buffer_color_space).map_err(|e| e.at(here!()))?;
    let mut mult_buf_window = mult_buf_bitmap.get_window_f32().unwrap();

    // Allocate coefficients and mappings to real pixel rows
    let mut mult_row_coefficients = vec![1f32; max_input_rows as usize];
    let mut mult_row_indexes = vec![-1i32; max_input_rows as usize];

    // Allocate buffer for summing the multiplied rows
    let mut summation_buf = Bitmap::create_float(
        from.w(), 1, PixelLayout::BGRA, true, from.info().alpha_meaningful(),
        buffer_color_space).map_err(|e| e.at(here!()))?;
    let mut summation_buf_window = summation_buf.get_window_f32().unwrap();

    // Allocate target buffer for the horizontally scaled pixels
    let mut h_scaled_buf = Bitmap::create_float(
        canvas_window.w(), 1, PixelLayout::BGRA, true, from.info().alpha_meaningful(),
        buffer_color_space).map_err(|e| e.at(here!()))?;
    let mut h_scaled_buf_window = h_scaled_buf.get_window_f32().unwrap();

    for out_row_ix in 0..canvas_window.h() as usize {
        let contrib = &weights_y.contrib_row()[out_row_ix];
        let contrib_weights = &weights_y.weights()
            [contrib.left_weight as usize..=contrib.right_weight as usize];

        // Clear output row
        summation_buf_window.slice_mut().fill(0f32);

        // if out_row_ix == 0 || out_row_ix == 20{
        //     // print the contrib weights for this output row
        //     println!("Contrib weights for row {}: pulling from row {}..={} using weights {:?}", out_row_ix, contrib.left_pixel, contrib.right_pixel, contrib_weights);
        // }

        for input_row_ix in contrib.left_pixel..=contrib.right_pixel {
            // Try to find row in buffer if already loaded
            let already_loaded_index = mult_row_indexes
                .iter()
                .position(|&v| v == input_row_ix as i32);

            // Not loaded? Look for a buffer row that we're no longer using
            let reusable_index = already_loaded_index
                .or_else(|| mult_row_indexes
                    .iter()
                    .position(|&v| v < contrib.left_pixel as i32))
                .ok_or_else(|| nerror!(ErrorKind::InvalidState))?;

            if already_loaded_index.is_none() {
                let buffer_window = &mut mult_buf_window.row_window(reusable_index as u32).unwrap();
                // Load row
                bitmap_window_srgba32_to_f32x4(
                    cc,
                    &from.row_window(input_row_ix).unwrap(),
                    buffer_window,
                );

                mult_row_coefficients[reusable_index] = 1f32;
                mult_row_indexes[reusable_index] = input_row_ix as i32;
            }
            let active_buf_ix = reusable_index;

            let weight: f32 = contrib_weights[input_row_ix as usize - contrib.left_pixel as usize];
            if (weight as f64).abs() > 0.00000002f64 {
                //    // Apply coefficient, update tracking
                //    let delta_coefficient: f32 = weight / mult_row_coefficients[active_buf_ix];
                //    multiply_row_safe(
                //        mult_buf_window.row_window(active_buf_ix as u32).unwrap().slice_mut(),
                //        delta_coefficient,
                //    );
                //    mult_row_coefficients[active_buf_ix] = weight;
                //    // Add row
                //    add_row_safe(
                //        summation_buf_window.slice_mut(),
                //        mult_buf_window.row_window(active_buf_ix as u32).unwrap().get_slice(),
                //    );
                multiply_and_add_row_simple(
                    summation_buf_window.slice_mut(),
                    mult_buf_window.row_window(active_buf_ix as u32).unwrap().get_slice(),
                    weight,
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
            &mut canvas_window.row_window(out_row_ix as u32).unwrap()).map_err(|e| e.at(here!()))?;
    }
    Ok(())
}
fn get_pixel(b: &BitmapWindowMut<f32>, x: i32, y: i32) -> [f32; 4] {
    // wrap negative values from right and bottom
    let x = if x < 0 { b.w() as i32 + x } else { x };
    let y = if y < 0 { b.h() as i32 + y } else { y };
    // clamp to bounds
    let x = x.max(0).min(b.w() as i32 - 1);
    let y = y.max(0).min(b.h() as i32 - 1);

    if b.info().channels() != 4 {
        panic!("get_pixel called on non-4 channel bitmap")
    }


    let y_offset = (y) as usize * b.info().item_stride() as usize;
    let x_start = (x) as usize * b.info().channels() as usize + y_offset;
    let pixel = b.get_slice()[x_start..x_start + b.info().channels() as usize].as_ref();

    [pixel[0], pixel[1], pixel[2], pixel[3]]
}
fn get_brightness(pixel: &[f32; 4]) -> f32 {
    (pixel[0] + pixel[1] + pixel[2]) / 3.0 * pixel[3].max(0.0).min(1.0)
}
fn summarize_corners(b: &BitmapWindowMut<f32>) -> String {

    // check if entire window is zeros
    let mut all_zeros = true;
    for y in 0..b.h() {
        for x in 0..b.w() {
            let pixel = get_pixel(b, x as i32, y as i32);
            if pixel[0] != 0.0 || pixel[1] != 0.0 || pixel[2] != 0.0 || pixel[3] != 0.0 {
                all_zeros = false;
                break;
            }
        }
    }
    if all_zeros {
        return "All zeros".to_string();
    }

    let bottom_right = get_pixel(b, -1, -1);
    let bottom_right2 = get_pixel(b, -2, -1);
    let top_right = get_pixel(b, -1, 0);
    let top_left = get_pixel(b, 0, 0);

    format!("BL: {:?},{:?}, TR: {:?}, TL: {:?}", bottom_right2, bottom_right, top_right, top_left)
}

pub fn scale_row_bgra_f32(
    source: &[f32],
    source_width: usize,
    target: &mut [f32],
    target_width: usize,
    weights: &PixelRowWeights,
    y_fyi: u32,
) {
    if source.len() != source_width * 4 || target.len() != target_width * 4 {
        panic!("Mismatched source or target slice lengths: source.len={}, source_width={}, target.len={}, target_width={}", source.len(), source_width, target.len(), target_width);
    }
    let source_pixels = bytemuck::cast_slice::<f32, Bgra128>(source);
    let target_pixels = bytemuck::cast_slice_mut::<f32, Bgra128>(target);

    //check weights correspond
    if weights.weights().len() as u32 != weights.contrib_row().iter().map(|r| r.right_weight - r.left_weight + 1).sum::<u32>() {
        panic!("Mismatched weights and contrib_row lengths: weights.len={}, contrib_row.len={}", weights.weights().len(), weights.contrib_row().len());
    }
    //check target width and weights correspond
    if target_width != weights.contrib_row().len() {
        panic!("Mismatched target width and contrib_row lengths: target_width={}, contrib_row.len={}", target_width, weights.contrib_row().len());
    }

    for (dst_x, contrib) in weights.contrib_row().iter().enumerate() {
        let mut sum = Bgra128 { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

        // if (dst_x == source_width -1 || dst_x ==  source_width -2) && (y_fyi == 0 || y_fyi == 20){
        //     // print the contrib weights for this output row
        //     println!("y={} Contrib weights for column {}: pulling from column {}..={} using weights {:?}", y_fyi, dst_x, contrib.left_pixel, contrib.right_pixel, &weights.weights()[contrib.left_weight as usize..=contrib.right_weight as usize]);
        // }
        for (src_x, &weight) in (contrib.left_pixel as usize..=contrib.right_pixel as usize)
            .zip(&weights.weights()[contrib.left_weight as usize..=contrib.right_weight as usize])
        {
            let pixel: &Bgra128 = &source_pixels[src_x];

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
#[multiversion(targets("x86_64+avx2", "aarch64+neon", "x86_64+sse4.1"))]
fn multiply_and_add_row_simple(mutate_row: &mut [f32], input_row: &[f32], coefficient: f32) {
    assert_eq!(mutate_row.len(), input_row.len(), "Mismatched row lengths");

    for (v, &input) in mutate_row.iter_mut().zip(input_row.iter()) {
        *v += input * coefficient;
    }
}

fn bitmap_window_srgba32_to_f32x4(colorcontext: &ColorContext, from: &BitmapWindowMut<u8>, to: &mut BitmapWindowMut<f32>) {
    //Ensure the widths and heights match, and that both source and dest are 4 channels
    let (w, h) = from.size_usize();
    if from.size() != to.size() || from.info().channels() != 4 || to.info().channels() != 4 {
        panic!("Mismatched source and dest window dimensions or channel counts");
    }

    for row_ix in 0..h {
        let from_row = from.row(row_ix).unwrap();
        let to_row = to.row_mut(row_ix).unwrap();

        for x in 0..w {
            let pixel = &from_row[x * 4..(x + 1) * 4];
            let alpha = if !from.info().alpha_meaningful() {
                1.0
            } else {
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
            if let &crate::graphics::bitmaps::BitmapCompositing::BlendWithMatte(ref color) = canvas.info().compose() {
                let matte = color.to_bgra8().map(
                    |bgra| [bgra.b, bgra.g, bgra.r, bgra.a]
                ).unwrap_or([0, 0, 0, 0]);
                blend_matte(&cc, src, matte).map_err(|e| e.at(here!()))?;
            }
            if src.info().alpha_premultiplied() {
                demultiply_alpha(src).map_err(|e| e.at(here!()))?;
            }
        }
        copy_linear_over_srgb(cc, src, canvas).map_err(|e| e.at(here!()))?;
    }
    Ok(())
}

fn blend_matte(cc: &ColorContext, bitmap: &mut BitmapWindowMut<f32>, matte: [u8; 4]) -> Result<(), FlowError> {
    let matte_a: f32 = matte[3] as f32 / 255.0f32;
    let b: f32 = cc.srgb_to_floatspace(matte[0]);
    let g: f32 = cc.srgb_to_floatspace(matte[1]);
    let r: f32 = cc.srgb_to_floatspace(matte[2]);
    let h = bitmap.h();
    let w = bitmap.w();
    for row in 0..h as usize {
        let slice = bitmap.row_mut(row).unwrap();
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


fn demultiply_alpha(bitmap: &mut BitmapWindowMut<f32>) -> Result<(), FlowError> {
    // verify channels == 4
    if bitmap.info().channels() != 4 || !bitmap.info().alpha_meaningful() || !bitmap.info().alpha_premultiplied() {
        return Err(nerror!(ErrorKind::InvalidState));
    }
    let (w, h) = (bitmap.w() as usize, bitmap.h() as usize);
    for row in 0..h {
        let slice = bitmap.row_mut(row).unwrap();


        for col in 0..w as usize {
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

fn compose_linear_over_srgb(
    cc: &ColorContext,
    src: &BitmapWindowMut<f32>,
    canvas: &mut BitmapWindowMut<u8>) {
    let dest_alpha_coeff = if canvas.info().alpha_meaningful() { 1.0f32 / 255.0f32 } else { 0.0f32 };
    let dest_alpha_offset = if canvas.info().alpha_meaningful() { 0.0f32 } else { 1.0f32 };

    for row in 0..src.h() as usize {
        let src_slice = src.row(row).unwrap();
        let canvas_slice = canvas.row_mut(row).unwrap();


        //
        for col in 0..src.w() as usize {
            let src_a = src_slice[col * 4 + 3];
            if src_a > 0.994f32 || !src.info().alpha_meaningful() {
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
    for row in 0..from.h() as usize {
        let input_slice = from.row(row).unwrap();
        let canvas_slice = canvas.row_mut(row).unwrap();

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
        for col in 0..from.w() as usize {
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

