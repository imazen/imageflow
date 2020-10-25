

#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_linear_to_luv_rows(
    context: *mut flow_c,
    bit: *mut flow_bitmap_float,
    start_row: u32,
    row_count: u32,
) -> bool {
    if !(start_row.wrapping_add(row_count) <= (*bit).h) {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1751 as i32,
            (*::std::mem::transmute::<&[u8; 37], &[libc::c_char; 37]>(
                b"flow_bitmap_float_linear_to_luv_rows\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    if (*bit).w.wrapping_mul((*bit).channels) != (*bit).float_stride {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1755 as i32,
            (*::std::mem::transmute::<&[u8; 37], &[libc::c_char; 37]>(
                b"flow_bitmap_float_linear_to_luv_rows\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    let start_at: *mut f32 = (*bit)
        .pixels
        .offset((*bit).float_stride.wrapping_mul(start_row) as isize);
    let end_at: *const f32 = (*bit).pixels.offset(
        (*bit)
            .float_stride
            .wrapping_mul(start_row.wrapping_add(row_count)) as isize,
    );
    let mut pix: *mut f32 = start_at;
    while pix < end_at as *mut f32 {
        linear_to_luv(pix);
        pix = pix.offset(1)
    }
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_luv_to_linear_rows(
    context: *mut flow_c,
    bit: *mut flow_bitmap_float,
    start_row: u32,
    row_count: u32,
) -> bool {
    if !(start_row.wrapping_add(row_count) <= (*bit).h) {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1772 as i32,
            (*::std::mem::transmute::<&[u8; 37], &[libc::c_char; 37]>(
                b"flow_bitmap_float_luv_to_linear_rows\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    if (*bit).w.wrapping_mul((*bit).channels) != (*bit).float_stride {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1776 as i32,
            (*::std::mem::transmute::<&[u8; 37], &[libc::c_char; 37]>(
                b"flow_bitmap_float_luv_to_linear_rows\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    let start_at: *mut f32 = (*bit)
        .pixels
        .offset((*bit).float_stride.wrapping_mul(start_row) as isize);
    let end_at: *const f32 = (*bit).pixels.offset(
        (*bit)
            .float_stride
            .wrapping_mul(start_row.wrapping_add(row_count)) as isize,
    );
    let mut pix: *mut f32 = start_at;
    while pix < end_at as *mut f32 {
        luv_to_linear(pix);
        pix = pix.offset(1)
    }
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_bgra_apply_color_matrix(
    context: *mut flow_c,
    bmp: *mut flow_bitmap_bgra,
    row: u32,
    count: u32,
    m: *const *mut f32,
) -> bool {
    let stride: u32 = (*bmp).stride;
    let ch: u32 = flow_pixel_format_bytes_per_pixel((*bmp).fmt);
    let w: u32 = (*bmp).w;
    let h: u32 = row.wrapping_add(count).min((*bmp).h);
    let m40: f32 = *(*m.offset(4 as i32 as isize)).offset(0) * 255.0f32;
    let m41: f32 = *(*m.offset(4 as i32 as isize)).offset(1) * 255.0f32;
    let m42: f32 = *(*m.offset(4 as i32 as isize)).offset(2) * 255.0f32;
    let m43: f32 = *(*m.offset(4 as i32 as isize)).offset(3 as i32 as isize) * 255.0f32;
    if ch == 4 as i32 as u32 {
        let mut y: u32 = row;
        while y < h {
            let mut x: u32 = 0 as i32 as u32;
            while x < w {
                let data: *mut u8 = (*bmp)
                    .pixels
                    .offset(stride.wrapping_mul(y) as isize)
                    .offset(x.wrapping_mul(ch) as isize);
                let r: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(0) * *data.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(0) * *data.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(0) * *data.offset(0) as i32 as f32
                        + *(*m.offset(3 as i32 as isize)).offset(0)
                        * *data.offset(3 as i32 as isize) as i32 as f32
                        + m40,
                );
                let g: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(1) * *data.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(1) * *data.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(1) * *data.offset(0) as i32 as f32
                        + *(*m.offset(3 as i32 as isize)).offset(1)
                        * *data.offset(3 as i32 as isize) as i32 as f32
                        + m41,
                );
                let b: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(2) * *data.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(2) * *data.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(2) * *data.offset(0) as i32 as f32
                        + *(*m.offset(3 as i32 as isize)).offset(2)
                        * *data.offset(3 as i32 as isize) as i32 as f32
                        + m42,
                );
                let a: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(3 as i32 as isize) * *data.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(3 as i32 as isize) * *data.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(3 as i32 as isize) * *data.offset(0) as i32 as f32
                        + *(*m.offset(3 as i32 as isize)).offset(3 as i32 as isize)
                        * *data.offset(3 as i32 as isize) as i32 as f32
                        + m43,
                );
                let newdata: *mut u8 = (*bmp)
                    .pixels
                    .offset(stride.wrapping_mul(y) as isize)
                    .offset(x.wrapping_mul(ch) as isize);
                *newdata.offset(0) = b;
                *newdata.offset(1) = g;
                *newdata.offset(2) = r;
                *newdata.offset(3 as i32 as isize) = a;
                x = x.wrapping_add(1)
            }
            y = y.wrapping_add(1)
        }
    } else if ch == 3 as i32 as u32 {
        let mut y_0: u32 = row;
        while y_0 < h {
            let mut x_0: u32 = 0 as i32 as u32;
            while x_0 < w {
                let data_0: *mut libc::c_uchar = (*bmp)
                    .pixels
                    .offset(stride.wrapping_mul(y_0) as isize)
                    .offset(x_0.wrapping_mul(ch) as isize);
                let r_0: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(0) * *data_0.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(0) * *data_0.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(0) * *data_0.offset(0) as i32 as f32
                        + m40,
                );
                let g_0: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(1) * *data_0.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(1) * *data_0.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(1) * *data_0.offset(0) as i32 as f32
                        + m41,
                );
                let b_0: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(2) * *data_0.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(2) * *data_0.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(2) * *data_0.offset(0) as i32 as f32
                        + m42,
                );
                let newdata_0: *mut u8 = (*bmp)
                    .pixels
                    .offset(stride.wrapping_mul(y_0) as isize)
                    .offset(x_0.wrapping_mul(ch) as isize);
                *newdata_0.offset(0) = b_0;
                *newdata_0.offset(1) = g_0;
                *newdata_0.offset(2) = r_0;
                x_0 = x_0.wrapping_add(1)
            }
            y_0 = y_0.wrapping_add(1)
        }
    } else {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Unsupported_pixel_format,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1838 as i32,
            (*::std::mem::transmute::<&[u8; 36], &[libc::c_char; 36]>(
                b"flow_bitmap_bgra_apply_color_matrix\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    return true;
}
// note: this file isn't exercised by test suite
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_apply_color_matrix(
    context: *mut flow_c,
    bmp: *mut flow_bitmap_float,
    row: u32,
    count: u32,
    m: *mut *mut f32,
) -> bool {
    let stride: u32 = (*bmp).float_stride;
    let ch: u32 = (*bmp).channels;
    let w: u32 = (*bmp).w;
    let h: u32 = row.wrapping_add(count).min((*bmp).h);
    match ch {
        4 => {
            let mut y: u32 = row;
            while y < h {
                let mut x: u32 = 0 as i32 as u32;
                while x < w {
                    let data: *mut f32 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y) as isize)
                        .offset(x.wrapping_mul(ch) as isize);
                    let r: f32 = *(*m.offset(0)).offset(0) * *data.offset(2)
                        + *(*m.offset(1)).offset(0) * *data.offset(1)
                        + *(*m.offset(2)).offset(0) * *data.offset(0)
                        + *(*m.offset(3 as i32 as isize)).offset(0)
                        * *data.offset(3 as i32 as isize)
                        + *(*m.offset(4 as i32 as isize)).offset(0);
                    let g: f32 = *(*m.offset(0)).offset(1) * *data.offset(2)
                        + *(*m.offset(1)).offset(1) * *data.offset(1)
                        + *(*m.offset(2)).offset(1) * *data.offset(0)
                        + *(*m.offset(3 as i32 as isize)).offset(1)
                        * *data.offset(3 as i32 as isize)
                        + *(*m.offset(4 as i32 as isize)).offset(1);
                    let b: f32 = *(*m.offset(0)).offset(2) * *data.offset(2)
                        + *(*m.offset(1)).offset(2) * *data.offset(1)
                        + *(*m.offset(2)).offset(2) * *data.offset(0)
                        + *(*m.offset(3 as i32 as isize)).offset(2)
                        * *data.offset(3 as i32 as isize)
                        + *(*m.offset(4 as i32 as isize)).offset(2);
                    let a: f32 = *(*m.offset(0)).offset(3 as i32 as isize) * *data.offset(2)
                        + *(*m.offset(1)).offset(3 as i32 as isize) * *data.offset(1)
                        + *(*m.offset(2)).offset(3 as i32 as isize) * *data.offset(0)
                        + *(*m.offset(3 as i32 as isize)).offset(3 as i32 as isize)
                        * *data.offset(3 as i32 as isize)
                        + *(*m.offset(4 as i32 as isize)).offset(3 as i32 as isize);
                    let newdata: *mut f32 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y) as isize)
                        .offset(x.wrapping_mul(ch) as isize);
                    *newdata.offset(0) = b;
                    *newdata.offset(1) = g;
                    *newdata.offset(2) = r;
                    *newdata.offset(3 as i32 as isize) = a;
                    x = x.wrapping_add(1)
                }
                y = y.wrapping_add(1)
            }
            return true;
        }
        3 => {
            let mut y_0: u32 = row;
            while y_0 < h {
                let mut x_0: u32 = 0 as i32 as u32;
                while x_0 < w {
                    let data_0: *mut f32 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y_0) as isize)
                        .offset(x_0.wrapping_mul(ch) as isize);
                    let r_0: f32 = *(*m.offset(0)).offset(0) * *data_0.offset(2)
                        + *(*m.offset(1)).offset(0) * *data_0.offset(1)
                        + *(*m.offset(2)).offset(0) * *data_0.offset(0)
                        + *(*m.offset(4 as i32 as isize)).offset(0);
                    let g_0: f32 = *(*m.offset(0)).offset(1) * *data_0.offset(2)
                        + *(*m.offset(1)).offset(1) * *data_0.offset(1)
                        + *(*m.offset(2)).offset(1) * *data_0.offset(0)
                        + *(*m.offset(4 as i32 as isize)).offset(1);
                    let b_0: f32 = *(*m.offset(0)).offset(2) * *data_0.offset(2)
                        + *(*m.offset(1)).offset(2) * *data_0.offset(1)
                        + *(*m.offset(2)).offset(2) * *data_0.offset(0)
                        + *(*m.offset(4 as i32 as isize)).offset(2);
                    let newdata_0: *mut f32 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y_0) as isize)
                        .offset(x_0.wrapping_mul(ch) as isize);
                    *newdata_0.offset(0) = b_0;
                    *newdata_0.offset(1) = g_0;
                    *newdata_0.offset(2) = r_0;
                    x_0 = x_0.wrapping_add(1)
                }
                y_0 = y_0.wrapping_add(1)
            }
            return true;
        }
        _ => {
            flow_context_set_error_get_message_buffer(
                context,
                flow_status_code::Unsupported_pixel_format,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1893 as i32,
                (*::std::mem::transmute::<&[u8; 37], &[libc::c_char; 37]>(
                    b"flow_bitmap_float_apply_color_matrix\x00",
                ))
                    .as_ptr(),
            );
            return false;
        }
    };
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_bgra_populate_histogram(
    context: *mut flow_c,
    bmp: *const flow_bitmap_bgra,
    histograms: *mut u64,
    histogram_size_per_channel: u32,
    histogram_count: u32,
    pixels_sampled: *mut u64,
) -> bool {
    let row: u32 = 0;
    let count: u32 = (*bmp).h;
    let stride: u32 = (*bmp).stride;
    let ch: u32 = flow_pixel_format_bytes_per_pixel((*bmp).fmt);
    let w: u32 = (*bmp).w;
    let h: u32 = (row.wrapping_add(count)).min((*bmp).h);
    if histogram_size_per_channel != 256 {
        // We're restricting it to this for speed
        FLOW_error(
            context,
            flow_status_code::Invalid_argument,
            "flow_bitmap_bgra_populate_histogram",
        );
        return false;
    }
    let shift = 0; // 8 - intlog2(histogram_size_per_channel);
    if ch == 4 || ch == 3 {
        if histogram_count == 1 {
            let mut y: u32 = row;
            while y < h {
                let mut x: u32 = 0;
                while x < w {
                    let data: *const u8 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y) as isize)
                        .offset(x.wrapping_mul(ch) as isize);
                    let ref mut fresh9 = *histograms.offset(
                        (306 as i32 * *data.offset(2) as i32
                            + 601 as i32 * *data.offset(1) as i32
                            + 117 as i32 * *data.offset(0) as i32
                            >> shift) as isize,
                    );
                    *fresh9 = (*fresh9).wrapping_add(1);
                    x = x.wrapping_add(1)
                }
                y = y.wrapping_add(1)
            }
        } else if histogram_count == 3 {
            let mut y: u32 = row;
            while y < h {
                let mut x: u32 = 0;
                while x < w {
                    let data: *const u8 = (*bmp)
                        .pixels
                        .offset((stride * y) as isize)
                        .offset((x * ch) as isize);
                    let ref mut fresh10 =
                        *histograms.offset((*data.offset(2) as i32 >> shift) as isize);
                    *fresh10 = (*fresh10).wrapping_add(1);
                    let ref mut fresh11 = *histograms.offset(
                        ((*data.offset(1) as i32 >> shift) as u32)
                            .wrapping_add(histogram_size_per_channel)
                            as isize,
                    );
                    *fresh11 = (*fresh11).wrapping_add(1);
                    let ref mut fresh12 = *histograms.offset(
                        ((*data.offset(0) as i32 >> shift) as u32)
                            .wrapping_add((2u32).wrapping_mul(histogram_size_per_channel))
                            as isize,
                    );
                    *fresh12 = (*fresh12).wrapping_add(1);
                    x = x.wrapping_add(1)
                }
                y = y.wrapping_add(1)
            }
        } else if histogram_count == 2 {
            let mut y_1: u32 = row;
            while y_1 < h {
                let mut x_1: u32 = 0 as i32 as u32;
                while x_1 < w {
                    let data_1: *const u8 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y_1) as isize)
                        .offset(x_1.wrapping_mul(ch) as isize);
                    // Calculate luminosity and saturation
                    let ref mut fresh13 = *histograms.offset(
                        (306 as i32 * *data_1.offset(2) as i32
                            + 601 as i32 * *data_1.offset(1) as i32
                            + 117 as i32 * *data_1.offset(0) as i32
                            >> shift) as isize,
                    );
                    *fresh13 = (*fresh13).wrapping_add(1);
                    let ref mut fresh14 =
                        *histograms.offset(histogram_size_per_channel.wrapping_add(
                            (int_max(
                                255 as i32,
                                int_max(
                                    (*data_1.offset(2) as i32 - *data_1.offset(1) as i32).abs(),
                                    (*data_1.offset(1) as i32 - *data_1.offset(0) as i32).abs(),
                                ),
                            ) >> shift) as u32,
                        ) as isize);
                    *fresh14 = (*fresh14).wrapping_add(1);
                    x_1 = x_1.wrapping_add(1)
                }
                y_1 = y_1.wrapping_add(1)
            }
        } else {
            flow_context_set_error_get_message_buffer(
                context,
                flow_status_code::Invalid_internal_state,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1950 as i32,
                (*::std::mem::transmute::<&[u8; 36], &[libc::c_char; 36]>(
                    b"flow_bitmap_bgra_populate_histogram\x00",
                ))
                    .as_ptr(),
            );
            return false;
        }
        *pixels_sampled = h.wrapping_sub(row).wrapping_mul(w) as u64
    } else {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Unsupported_pixel_format,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1956 as i32,
            (*::std::mem::transmute::<&[u8; 36], &[libc::c_char; 36]>(
                b"flow_bitmap_bgra_populate_histogram\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    return true;
}