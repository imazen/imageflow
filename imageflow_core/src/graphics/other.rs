

pub unsafe fn flow_bitmap_float_linear_to_luv_rows(
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
pub unsafe fn flow_bitmap_float_luv_to_linear_rows(
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
