
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_bgra_flip_vertical(
    context: *mut flow_c,
    b: *mut flow_bitmap_bgra,
) -> bool {
    let swap: *mut libc::c_void = flow_context_malloc(
        context,
        (*b).stride as usize,
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        context as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        1430 as i32,
    );
    if swap.is_null() {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Out_of_memory,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1432 as i32,
            (*::std::mem::transmute::<&[u8; 31], &[libc::c_char; 31]>(
                b"flow_bitmap_bgra_flip_vertical\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    // Dont' copy the full stride (padding), it could be windowed!
    // Todo: try multiple swap rows? 5ms isn't bad, but could be better
    let row_length: u32 = (*b).stride.min(
        (*b).w
            .wrapping_mul(flow_pixel_format_bytes_per_pixel((*b).fmt)),
    );
    let mut i: u32 = 0 as i32 as u32;
    while i < (*b).h.wrapping_div(2u32) {
        let top: *mut libc::c_void =
            (*b).pixels.offset(i.wrapping_mul((*b).stride) as isize) as *mut libc::c_void;
        let bottom: *mut libc::c_void = (*b).pixels.offset(
            (*b).h
                .wrapping_sub(1u32)
                .wrapping_sub(i)
                .wrapping_mul((*b).stride) as isize,
        ) as *mut libc::c_void;
        memcpy(swap, top, row_length as u64);
        memcpy(top, bottom, row_length as u64);
        memcpy(bottom, swap, row_length as u64);
        i = i.wrapping_add(1)
    }
    flow_deprecated_free(
        context,
        swap,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        1445 as i32,
    );
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_bgra_flip_horizontal(
    _context: *mut flow_c,
    b: *mut flow_bitmap_bgra,
) -> bool {
    if (*b).fmt as u32 == flow_bgra32 as i32 as u32 || (*b).fmt as u32 == flow_bgr32 as i32 as u32 {
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
    } else if (*b).fmt as u32 == flow_bgr24 as i32 as u32 {
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
                memcpy(
                    &mut swap_0 as *mut [u32; 4] as *mut libc::c_void,
                    left_0 as *const libc::c_void,
                    3 as i32 as u64,
                );
                memcpy(
                    left_0 as *mut libc::c_void,
                    right_0 as *const libc::c_void,
                    3 as i32 as u64,
                );
                memcpy(
                    right_0 as *mut libc::c_void,
                    &mut swap_0 as *mut [u32; 4] as *const libc::c_void,
                    3 as i32 as u64,
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
                memcpy(
                    &mut swap_1 as *mut [u32; 4] as *mut libc::c_void,
                    left_1 as *const libc::c_void,
                    flow_pixel_format_bytes_per_pixel((*b).fmt) as u64,
                );
                memcpy(
                    left_1 as *mut libc::c_void,
                    right_1 as *const libc::c_void,
                    flow_pixel_format_bytes_per_pixel((*b).fmt) as u64,
                );
                memcpy(
                    right_1 as *mut libc::c_void,
                    &mut swap_1 as *mut [u32; 4] as *const libc::c_void,
                    flow_pixel_format_bytes_per_pixel((*b).fmt) as u64,
                );
                left_1 = left_1.offset(flow_pixel_format_bytes_per_pixel((*b).fmt) as isize);
                right_1 = right_1.offset(-(flow_pixel_format_bytes_per_pixel((*b).fmt) as isize))
            }
            y_1 = y_1.wrapping_add(1)
        }
    }
    return true;
}
