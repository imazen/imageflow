use crate::graphics::prelude::*;
use smallvec::CollectionAllocErr::CapacityOverflow;
use crate::graphics::math::ir_gaussian;
use num::traits::real::Real;


#[derive(Clone)]
pub struct ConvolutionKernel {
    ///TODO: may require aligned allocation
    pub kernel: Vec<f32>,
    pub width: u32,
    pub radius: u32,
    pub threshold_min_change: f32,
    pub threshold_max_change: f32,
    pub buffer: Vec<f32>,
}
impl ConvolutionKernel{
    pub fn create(radius: u32) -> Result<ConvolutionKernel, FlowError> {
        Ok(ConvolutionKernel{
            kernel: vec![0f32;2 * radius as usize + 1],
            width: radius * 2 + 1,
            radius,
            threshold_min_change: 0.0,
            threshold_max_change: 0.0,
            buffer: vec![0f32; (radius as usize + 2) * 4]
        })
    }

}


pub unsafe fn flow_convolution_kernel_create_gaussian(
    std_dev: f64,
    radius: u32,
) -> Result<ConvolutionKernel, FlowError> {
    let mut k = ConvolutionKernel::create(radius)
        .map_err(|e| e.at(here!()))?;

    for i in 0..k.width {
        k.kernel[i as usize] =
            ir_gaussian((radius as i32 - i as i32).abs() as f64, std_dev) as f32;
    }

    Ok(k)
}

pub fn flow_convolution_kernel_sum(kernel: &ConvolutionKernel) -> f64 {
    let mut sum: f64 = 0 as i32 as f64;
    for i in 0..kernel.width {
        sum += kernel.kernel[i as usize] as f64;
    }
    return sum;
}

pub unsafe fn flow_convolution_kernel_normalize(
    kernel: &mut ConvolutionKernel,
    desired_sum: f32,
) {
    let sum: f64 = flow_convolution_kernel_sum(kernel);
    if sum == 0 as i32 as f64 {
        return;
    }
    let factor: f32 = (desired_sum as f64 / sum) as f32;
    let mut i: u32 = 0 as i32 as u32;
    while i < kernel.width {
        kernel.kernel[i as usize] *= factor;
        i = i.wrapping_add(1)
    }
}

pub unsafe fn flow_convolution_kernel_create_gaussian_normalized(
    std_dev: f64,
    radius: u32,
) -> Result<ConvolutionKernel, FlowError> {
    let mut kernel =
        flow_convolution_kernel_create_gaussian(std_dev, radius)
            .map_err(|e| e.at(here!()))?;
    flow_convolution_kernel_normalize(&mut kernel, 1 as i32 as f32);

    Ok(kernel)
}

pub unsafe fn flow_convolution_kernel_create_gaussian_sharpen(
    std_dev: f64,
    radius: u32,
) -> Result<ConvolutionKernel,FlowError> {
    let mut kernel=
        flow_convolution_kernel_create_gaussian( std_dev, radius)
            .map_err(|e| e.at(here!()))?;

        let sum: f64 = flow_convolution_kernel_sum(&kernel);
        let mut i: u32 = 0 as i32 as u32;
        while i < kernel.width {
            if i == radius {
                kernel.kernel[i as usize] =
                    (2 as i32 as f64 * sum - kernel.kernel[i as usize] as f64) as f32
            } else {
                kernel.kernel[i as usize] *= -(1 as i32) as f32
            }
            i = i.wrapping_add(1)
        }
        flow_convolution_kernel_normalize(&mut kernel, 1 as i32 as f32);
    Ok(kernel)
}

pub unsafe fn flow_bitmap_float_convolve_rows(
    buf: *mut flow_bitmap_float,
    kernel: &mut ConvolutionKernel,
    convolve_channels: u32,
    from_row: u32,
    row_count: i32,
) -> Result<(), FlowError> {
    let radius: u32 = kernel.radius;
    let threshold_min: f32 = kernel.threshold_min_change;
    let threshold_max: f32 = kernel.threshold_max_change;
    // Do nothing unless the image is at least half as wide as the kernel.
    if (*buf).w < radius.wrapping_add(1u32) {
        return Ok(());
    }
    let buffer_count: u32 = radius.wrapping_add(1u32);
    let w: u32 = (*buf).w;
    let int_w: i32 = (*buf).w as i32;
    let step: u32 = (*buf).channels;
    let until_row: u32 = if row_count < 0 as i32 {
        (*buf).h
    } else {
        from_row.wrapping_add(row_count as u32)
    };
    let ch_used: u32 = convolve_channels;
    let buffer: *mut f32 = kernel.buffer.as_mut_ptr();
    let avg: *mut f32 = kernel.buffer
        .as_mut_ptr()
        .offset(buffer_count.wrapping_mul(ch_used) as isize) as *mut f32;
    let kern: *const f32 = kernel.kernel.as_mut_ptr();
    let wrap_mode: i32 = 0 as i32;
    let mut row: u32 = from_row;
    while row < until_row {
        let source_buffer: *mut f32 = &mut *(*buf)
            .pixels
            .offset(row.wrapping_mul((*buf).float_stride) as isize)
            as *mut f32;
        let mut circular_idx: i32 = 0 as i32;
        let mut ndx: u32 = 0 as i32 as u32;
        while ndx < w.wrapping_add(buffer_count) {
            // Flush old value
            if ndx >= buffer_count {
                libc::memcpy(
                    &mut *source_buffer
                        .offset(ndx.wrapping_sub(buffer_count).wrapping_mul(step) as isize)
                        as *mut f32 as *mut libc::c_void,
                    &mut *buffer.offset((circular_idx as u32).wrapping_mul(ch_used) as isize)
                        as *mut f32 as *const libc::c_void,
                    (ch_used as usize).wrapping_mul(::std::mem::size_of::<f32>() as usize),
                );
            }
            // Calculate and enqueue new value
            if ndx < w {
                let left: i32 = ndx.wrapping_sub(radius) as i32;
                let right: i32 = ndx.wrapping_add(radius) as i32;
                let mut i;
                libc::memset(
                    avg as *mut libc::c_void,
                    0 as i32,
                    (::std::mem::size_of::<f32>() as usize).wrapping_mul(ch_used as usize),
                );
                if left < 0 as i32 || right >= w as i32 {
                    if wrap_mode == 0 as i32 {
                        // Only sample what's present, and fix the average later.
                        let mut total_weight: f32 = 0 as i32 as f32;
                        /* Accumulate each channel */
                        i = left;
                        while i <= right {
                            if i > 0 as i32 && i < int_w {
                                let weight: f32 = *kern.offset((i - left) as isize);
                                total_weight += weight;
                                let mut j: u32 = 0 as i32 as u32;
                                while j < ch_used {
                                    *avg.offset(j as isize) += weight
                                        * *source_buffer
                                        .offset((i as u32).wrapping_mul(step).wrapping_add(j)
                                            as isize);
                                    j = j.wrapping_add(1)
                                }
                            }
                            i += 1
                        }
                        let mut j_0: u32 = 0 as i32 as u32;
                        while j_0 < ch_used {
                            *avg.offset(j_0 as isize) = *avg.offset(j_0 as isize) / total_weight;
                            j_0 = j_0.wrapping_add(1)
                        }
                    } else if wrap_mode == 1 as i32 {
                        // Extend last pixel to be used for all missing inputs
                        /* Accumulate each channel */
                        i = left;
                        while i <= right {
                            let weight_0: f32 = *kern.offset((i - left) as isize);
                            let ix: u32 = if i > int_w - 1 as i32 {
                                (int_w) - 1 as i32
                            } else if i < 0 as i32 {
                                0 as i32
                            } else {
                                i
                            } as u32;
                            let mut j_1: u32 = 0 as i32 as u32;
                            while j_1 < ch_used {
                                *avg.offset(j_1 as isize) += weight_0
                                    * *source_buffer
                                    .offset(ix.wrapping_mul(step).wrapping_add(j_1) as isize);
                                j_1 = j_1.wrapping_add(1)
                            }
                            i += 1
                        }
                    }
                } else {
                    /* Accumulate each channel */
                    i = left;
                    while i <= right {
                        let weight_1: f32 = *kern.offset((i - left) as isize);
                        let mut j_2: u32 = 0 as i32 as u32;
                        while j_2 < ch_used {
                            *avg.offset(j_2 as isize) += weight_1
                                * *source_buffer.offset(
                                (i as u32).wrapping_mul(step).wrapping_add(j_2) as isize,
                            );
                            j_2 = j_2.wrapping_add(1)
                        }
                        i += 1
                    }
                }
                // Enqueue difference
                libc::memcpy(
                    &mut *buffer.offset((circular_idx as u32).wrapping_mul(ch_used) as isize)
                        as *mut f32 as *mut libc::c_void,
                    avg as *const libc::c_void,
                    (ch_used as usize).wrapping_mul(::std::mem::size_of::<f32>() as usize),
                );
                if threshold_min > 0 as i32 as f32 || threshold_max > 0 as i32 as f32 {
                    let mut change: f32 = 0 as i32 as f32;
                    let mut j_3: u32 = 0 as i32 as u32;
                    while j_3 < ch_used {
                        change += (
                            *source_buffer
                                .offset(ndx.wrapping_mul(step).wrapping_add(j_3) as isize)
                                - *avg.offset(j_3 as isize)
                        ).abs();
                        j_3 = j_3.wrapping_add(1)
                    }
                    if change < threshold_min || change > threshold_max {
                        libc::memcpy(
                            &mut *buffer
                                .offset((circular_idx as u32).wrapping_mul(ch_used) as isize)
                                as *mut f32 as *mut libc::c_void,
                            &mut *source_buffer.offset(ndx.wrapping_mul(step) as isize) as *mut f32
                                as *const libc::c_void,
                            (ch_used as usize).wrapping_mul(::std::mem::size_of::<f32>() as usize),
                        );
                    }
                }
            }
            circular_idx = ((circular_idx + 1 as i32) as u32).wrapping_rem(buffer_count) as i32;
            ndx = ndx.wrapping_add(1)
        }
        row = row.wrapping_add(1)
    }
    Ok(())
}
unsafe fn bitmap_float_boxblur_rows(
    image: *mut flow_bitmap_float,
    radius: u32,
    passes: u32,
    convolve_channels: u32,
    work_buffer: *mut f32,
    from_row: u32,
    row_count: i32,
) -> Result<(), FlowError> {
    let buffer_count: u32 = radius.wrapping_add(1u32);
    let w: u32 = (*image).w;
    let step: u32 = (*image).channels;
    let until_row: u32 = if row_count < 0 as i32 {
        (*image).h
    } else {
        from_row.wrapping_add(row_count as u32)
    };
    let ch_used: u32 = (*image).channels;
    let buffer: *mut f32 = work_buffer;
    let std_count: u32 = radius.wrapping_mul(2u32).wrapping_add(1u32);
    let std_factor: f32 = 1.0f32 / std_count as f32;
    let mut row: u32 = from_row;
    while row < until_row {
        let source_buffer: *mut f32 = &mut *(*image)
            .pixels
            .offset(row.wrapping_mul((*image).float_stride) as isize)
            as *mut f32;
        let mut pass_index: u32 = 0 as i32 as u32;
        while pass_index < passes {
            let mut circular_idx: i32 = 0 as i32;
            let mut sum: [f32; 4] = [
                0 as i32 as f32,
                0 as i32 as f32,
                0 as i32 as f32,
                0 as i32 as f32,
            ];
            let mut count: u32 = 0 as i32 as u32;
            let mut ndx: u32 = 0 as i32 as u32;
            while ndx < radius {
                let mut ch: u32 = 0 as i32 as u32;
                while ch < convolve_channels {
                    sum[ch as usize] +=
                        *source_buffer.offset(ndx.wrapping_mul(step).wrapping_add(ch) as isize);
                    ch = ch.wrapping_add(1)
                }
                count = count.wrapping_add(1);
                ndx = ndx.wrapping_add(1)
            }
            let mut ndx_0: u32 = 0 as i32 as u32;
            while ndx_0 < w.wrapping_add(buffer_count) {
                // Pixels
                if ndx_0 >= buffer_count {
                    // same as ndx > radius
                    // Remove trailing item from average
                    let mut ch_0: u32 = 0 as i32 as u32;
                    while ch_0 < convolve_channels {
                        sum[ch_0 as usize] -= *source_buffer.offset(
                            ndx_0
                                .wrapping_sub(radius)
                                .wrapping_sub(1u32)
                                .wrapping_mul(step)
                                .wrapping_add(ch_0) as isize,
                        );
                        ch_0 = ch_0.wrapping_add(1)
                    }
                    count = count.wrapping_sub(1);
                    // Flush old value
                    libc::memcpy(
                        &mut *source_buffer
                            .offset(ndx_0.wrapping_sub(buffer_count).wrapping_mul(step) as isize)
                            as *mut f32 as *mut libc::c_void,
                        &mut *buffer.offset((circular_idx as u32).wrapping_mul(ch_used) as isize)
                            as *mut f32 as *const libc::c_void,
                        (ch_used as usize).wrapping_mul(::std::mem::size_of::<f32>() as usize),
                    );
                }
                // Calculate and enqueue new value
                if ndx_0 < w {
                    if ndx_0 < w.wrapping_sub(radius) {
                        let mut ch_1: u32 = 0 as i32 as u32;
                        while ch_1 < convolve_channels {
                            sum[ch_1 as usize] += *source_buffer.offset(
                                ndx_0
                                    .wrapping_add(radius)
                                    .wrapping_mul(step)
                                    .wrapping_add(ch_1) as isize,
                            );
                            ch_1 = ch_1.wrapping_add(1)
                        }
                        count = count.wrapping_add(1)
                    }
                    // Enqueue averaged value
                    if count != std_count {
                        let mut ch_2: u32 = 0 as i32 as u32;
                        while ch_2 < convolve_channels {
                            *buffer.offset(
                                (circular_idx as u32)
                                    .wrapping_mul(ch_used)
                                    .wrapping_add(ch_2) as isize,
                            ) = sum[ch_2 as usize] / count as f32;
                            ch_2 = ch_2.wrapping_add(1)
                            // Recompute factor
                        }
                    } else {
                        let mut ch_3: u32 = 0 as i32 as u32;
                        while ch_3 < convolve_channels {
                            *buffer.offset(
                                (circular_idx as u32)
                                    .wrapping_mul(ch_used)
                                    .wrapping_add(ch_3) as isize,
                            ) = sum[ch_3 as usize] * std_factor;
                            ch_3 = ch_3.wrapping_add(1)
                        }
                    }
                }
                circular_idx = ((circular_idx + 1 as i32) as u32).wrapping_rem(buffer_count) as i32;
                ndx_0 = ndx_0.wrapping_add(1)
            }
            pass_index = pass_index.wrapping_add(1)
        }
        row = row.wrapping_add(1)
    }
    Ok(())
}
unsafe fn bitmap_float_boxblur_misaligned_rows(
    image: *mut flow_bitmap_float,
    radius: u32,
    align: i32,
    convolve_channels: u32,
    work_buffer: *mut f32,
    from_row: u32,
    row_count: i32,
) -> Result<(), FlowError> {
    if align != 1 as i32 && align != -(1 as i32) {
        return Err(nerror!(ErrorKind::InvalidState));
    }
    let buffer_count: u32 = radius.wrapping_add(2u32);
    let w: u32 = (*image).w;
    let step: u32 = (*image).channels;
    let until_row: u32 = if row_count < 0 as i32 {
        (*image).h
    } else {
        from_row.wrapping_add(row_count as u32)
    };
    let ch_used: u32 = (*image).channels;
    let buffer: *mut f32 = work_buffer;
    let write_offset: u32 = if align == -(1 as i32) {
        0 as i32
    } else {
        1 as i32
    } as u32;
    let mut row: u32 = from_row;
    while row < until_row {
        let source_buffer: *mut f32 = &mut *(*image)
            .pixels
            .offset(row.wrapping_mul((*image).float_stride) as isize)
            as *mut f32;
        let mut circular_idx: i32 = 0 as i32;
        let mut sum: [f32; 4] = [
            0 as i32 as f32,
            0 as i32 as f32,
            0 as i32 as f32,
            0 as i32 as f32,
        ];
        let mut count: f32 = 0 as i32 as f32;
        let mut ndx: u32 = 0 as i32 as u32;
        while ndx < radius {
            let factor: f32 = if ndx == radius.wrapping_sub(1u32) {
                0.5f32
            } else {
                1 as i32 as f32
            };
            let mut ch: u32 = 0 as i32 as u32;
            while ch < convolve_channels {
                sum[ch as usize] += *source_buffer
                    .offset(ndx.wrapping_mul(step).wrapping_add(ch) as isize)
                    * factor;
                ch = ch.wrapping_add(1)
            }
            count += factor;
            ndx = ndx.wrapping_add(1)
        }
        let mut ndx_0: u32 = 0 as i32 as u32;
        while ndx_0 < w.wrapping_add(buffer_count).wrapping_sub(write_offset) {
            // Pixels
            // Calculate new value
            if ndx_0 < w {
                if ndx_0 < w.wrapping_sub(radius) {
                    let mut ch_0: u32 = 0 as i32 as u32;
                    while ch_0 < convolve_channels {
                        sum[ch_0 as usize] += *source_buffer.offset(
                            ndx_0
                                .wrapping_add(radius)
                                .wrapping_mul(step)
                                .wrapping_add(ch_0) as isize,
                        ) * 0.5f32;
                        ch_0 = ch_0.wrapping_add(1)
                    }
                    count += 0.5f32
                }
                if ndx_0 < w.wrapping_sub(radius).wrapping_add(1u32) {
                    let mut ch_1: u32 = 0 as i32 as u32;
                    while ch_1 < convolve_channels {
                        sum[ch_1 as usize] += *source_buffer.offset(
                            ndx_0
                                .wrapping_sub(1u32)
                                .wrapping_add(radius)
                                .wrapping_mul(step)
                                .wrapping_add(ch_1) as isize,
                        ) * 0.5f32;
                        ch_1 = ch_1.wrapping_add(1)
                    }
                    count += 0.5f32
                }
                // Remove trailing items from average
                if ndx_0 >= radius {
                    let mut ch_2: u32 = 0 as i32 as u32;
                    while ch_2 < convolve_channels {
                        sum[ch_2 as usize] -= *source_buffer.offset(
                            ndx_0
                                .wrapping_sub(radius)
                                .wrapping_mul(step)
                                .wrapping_add(ch_2) as isize,
                        ) * 0.5f32;
                        ch_2 = ch_2.wrapping_add(1)
                    }
                    count -= 0.5f32
                }
                if ndx_0 >= radius.wrapping_add(1u32) {
                    let mut ch_3: u32 = 0 as i32 as u32;
                    while ch_3 < convolve_channels {
                        sum[ch_3 as usize] -= *source_buffer.offset(
                            ndx_0
                                .wrapping_sub(1u32)
                                .wrapping_sub(radius)
                                .wrapping_mul(step)
                                .wrapping_add(ch_3) as isize,
                        ) * 0.5f32;
                        ch_3 = ch_3.wrapping_add(1)
                    }
                    count -= 0.5f32
                }
            }
            // Flush old value
            if ndx_0 >= buffer_count.wrapping_sub(write_offset) {
                libc::memcpy(
                    &mut *source_buffer.offset(
                        ndx_0
                            .wrapping_add(write_offset)
                            .wrapping_sub(buffer_count)
                            .wrapping_mul(step) as isize,
                    ) as *mut f32 as *mut libc::c_void,
                    &mut *buffer.offset((circular_idx as u32).wrapping_mul(ch_used) as isize)
                        as *mut f32 as *const libc::c_void,
                    (ch_used as usize).wrapping_mul(::std::mem::size_of::<f32>() as usize),
                );
            }
            // enqueue new value
            if ndx_0 < w {
                let mut ch_4: u32 = 0 as i32 as u32; // Never exceed half the size of the buffer.
                while ch_4 < convolve_channels {
                    *buffer.offset(
                        (circular_idx as u32)
                            .wrapping_mul(ch_used)
                            .wrapping_add(ch_4) as isize,
                    ) = sum[ch_4 as usize] / count;
                    ch_4 = ch_4.wrapping_add(1)
                }
            }
            circular_idx = ((circular_idx + 1 as i32) as u32).wrapping_rem(buffer_count) as i32;
            ndx_0 = ndx_0.wrapping_add(1)
        }
        row = row.wrapping_add(1)
    }
    Ok(())
}

pub unsafe fn flow_bitmap_float_approx_gaussian_calculate_d(
    sigma: f32,
    bitmap_width: u32,
) -> u32 {
    let mut d: u32 = (1.8799712059732503768118239636082839397552400554574537f32 * sigma + 0.5f32)
        .floor() as i32 as u32;
    d = d.min(bitmap_width.wrapping_sub(1u32).wrapping_div(2u32));
    return d;
}

pub unsafe fn flow_bitmap_float_approx_gaussian_buffer_element_count_required(
    sigma: f32,
    bitmap_width: u32,
) -> u32 {
    return flow_bitmap_float_approx_gaussian_calculate_d(sigma, bitmap_width)
        .wrapping_mul(2u32)
        .wrapping_add(12 as i32 as u32);
    // * sizeof(float);
}

pub unsafe fn flow_bitmap_float_approx_gaussian_blur_rows(
    image: *mut flow_bitmap_float,
    sigma: f32,
    buffer: *mut f32,
    buffer_element_count: usize,
    from_row: u32,
    row_count: i32,
) -> Result<(), FlowError> {
    // Ensure sigma is large enough for approximation to be accurate.
    if sigma < 2 as i32 as f32 {
        return Err(nerror!(ErrorKind::InvalidState));
    }
    // Ensure the buffer is large enough
    if flow_bitmap_float_approx_gaussian_buffer_element_count_required(sigma, (*image).w) as usize
        > buffer_element_count
    {
        return Err(nerror!(ErrorKind::InvalidState));
    }
    // http://www.w3.org/TR/SVG11/filters.html#feGaussianBlur
    // For larger values of 's' (s >= 2.0), an approximation can be used :
    // Three successive box - blurs build a piece - wise quadratic convolution kernel, which approximates the Gaussian
    // kernel to within roughly 3 % .
    let d: u32 = flow_bitmap_float_approx_gaussian_calculate_d(sigma, (*image).w);
    //... if d is odd, use three box - blurs of size 'd', centered on the output pixel.
    if d.wrapping_rem(2u32) > 0 as i32 as u32 {
        bitmap_float_boxblur_rows(
            image,
            d.wrapping_div(2u32),
            3 as i32 as u32,
            (*image).channels,
            buffer,
            from_row,
            row_count,
        ).map_err(|e| e.at(here!()))?;

    } else {
        // ... if d is even, two box - blurs of size 'd'
        // (the first one centered on the pixel boundary between the output pixel and the one to the left,
        //  the second one centered on the pixel boundary between the output pixel and the one to the right)
        // and one box blur of size 'd+1' centered on the output pixel.
        bitmap_float_boxblur_misaligned_rows(
            image,
            d.wrapping_div(2u32),
            -(1 as i32),
            (*image).channels,
            buffer,
            from_row,
            row_count,
        ).map_err(|e| e.at(here!()))?;

        bitmap_float_boxblur_misaligned_rows(
            image,
            d.wrapping_div(2u32),
            1 as i32,
            (*image).channels,
            buffer,
            from_row,
            row_count,
        ).map_err(|e| e.at(here!()))?;
        bitmap_float_boxblur_rows(
            image,
            d.wrapping_div(2u32).wrapping_add(1u32),
            1 as i32 as u32,
            (*image).channels,
            buffer,
            from_row,
            row_count,
        ).map_err(|e| e.at(here!()))?;
    }
    Ok(())
}