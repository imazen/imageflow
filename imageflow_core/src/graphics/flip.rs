use crate::graphics::prelude::*;

pub unsafe fn flow_bitmap_bgra_flip_vertical(
    b: *mut flow_bitmap_bgra,
) -> Result<(), FlowError> {
    let mut swap_buf = AlignedBuffer::<u8>::new((*b).stride(), 64)
        .map_err(|_| nerror!(ErrorKind::AllocationFailed))?;
    let swap = swap_buf.as_slice_mut().as_mut_ptr();

    // Dont' copy the full stride (padding), it could be windowed!
    // Todo: try multiple swap rows? 5ms isn't bad, but could be better
    let row_length: u32 = (*b).stride.min(
        (*b).w
            .wrapping_mul(flow_pixel_format_bytes_per_pixel((*b).fmt)),
    );
    let mut i: u32 = 0 as i32 as u32;
    while i < (*b).h.wrapping_div(2u32) {
        let top =
            (*b).pixels.offset(i.wrapping_mul((*b).stride) as isize);
        let bottom = (*b).pixels.offset(
            (*b).h
                .wrapping_sub(1u32)
                .wrapping_sub(i)
                .wrapping_mul((*b).stride) as isize,
        );
        swap.copy_from_nonoverlapping(top, row_length as usize);
        top.copy_from_nonoverlapping(bottom, row_length as usize);
        bottom.copy_from_nonoverlapping(swap, row_length as usize);
        i = i.wrapping_add(1)
    }
    Ok(())
}
pub unsafe fn flow_bitmap_bgra_flip_horizontal(
    b: *mut flow_bitmap_bgra,
) -> Result<(), FlowError> {
    if (*b).fmt == PixelFormat::Bgra32 || (*b).fmt == PixelFormat::Bgr32 {
        // 12ms simple
        let mut y: u32 = 0 as i32 as u32;
        while y < (*b).h {
            let mut left: *mut u32 =
                (*b).pixels.offset(y.wrapping_mul((*b).stride) as isize) as *mut u32;
            let mut right: *mut u32 = (*b)
                .pixels
                .offset(y.wrapping_mul((*b).stride) as isize)
                .offset((4u32).wrapping_mul((*b).w.wrapping_sub(1u32)) as isize)
                as *mut u32;
            while left < right {
                let swap: u32 = *left;
                *left = *right;
                *right = swap;
                left = left.offset(1);
                right = right.offset(-1)
            }
            y = y.wrapping_add(1)
        }
    } else if (*b).fmt == PixelFormat::Bgr24 {
        let mut swap_0: [u32; 4] = [0; 4];
        // Dont' copy the full stride (padding), it could be windowed!
        let mut y_0: u32 = 0 as i32 as u32;
        while y_0 < (*b).h {
            let mut left_0: *mut u8 = (*b).pixels.offset(y_0.wrapping_mul((*b).stride) as isize);
            let mut right_0: *mut u8 = (*b)
                .pixels
                .offset(y_0.wrapping_mul((*b).stride) as isize)
                .offset((3u32).wrapping_mul((*b).w.wrapping_sub(1u32)) as isize);
            while left_0 < right_0 {
                libc::memcpy(
                    &mut swap_0 as *mut [u32; 4] as *mut libc::c_void,
                    left_0 as *const libc::c_void,
                    3 as usize,
                );
                libc::memcpy(
                    left_0 as *mut libc::c_void,
                    right_0 as *const libc::c_void,
                    3 as usize,
                );
                libc::memcpy(
                    right_0 as *mut libc::c_void,
                    &mut swap_0 as *mut [u32; 4] as *const libc::c_void,
                    3 as usize,
                );
                left_0 = left_0.offset(3 as i32 as isize);
                right_0 = right_0.offset(-(3 as i32 as isize))
            }
            y_0 = y_0.wrapping_add(1)
        }
    } else {
        let mut swap_1: [u32; 4] = [0; 4];
        // Dont' copy the full stride (padding), it could be windowed!
        let mut y_1: u32 = 0 as i32 as u32;
        while y_1 < (*b).h {
            let mut left_1: *mut u8 = (*b).pixels.offset(y_1.wrapping_mul((*b).stride) as isize);
            let mut right_1: *mut u8 = (*b)
                .pixels
                .offset(y_1.wrapping_mul((*b).stride) as isize)
                .offset(
                    flow_pixel_format_bytes_per_pixel((*b).fmt)
                        .wrapping_mul((*b).w.wrapping_sub(1u32)) as isize,
                );
            while left_1 < right_1 {
                libc::memcpy(
                    &mut swap_1 as *mut [u32; 4] as *mut libc::c_void,
                    left_1 as *const libc::c_void,
                    flow_pixel_format_bytes_per_pixel((*b).fmt) as usize,
                );
                libc::memcpy(
                    left_1 as *mut libc::c_void,
                    right_1 as *const libc::c_void,
                    flow_pixel_format_bytes_per_pixel((*b).fmt) as usize,
                );
                libc::memcpy(
                    right_1 as *mut libc::c_void,
                    &mut swap_1 as *mut [u32; 4] as *const libc::c_void,
                    flow_pixel_format_bytes_per_pixel((*b).fmt) as usize,
                );
                left_1 = left_1.offset(flow_pixel_format_bytes_per_pixel((*b).fmt) as isize);
                right_1 = right_1.offset(-(flow_pixel_format_bytes_per_pixel((*b).fmt) as isize))
            }
            y_1 = y_1.wrapping_add(1)
        }
    }
    Ok(())
}
