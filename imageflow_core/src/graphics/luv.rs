use crate::graphics::prelude::*;
use crate::graphics::color::{linear_to_luv, luv_to_linear};


pub unsafe fn flow_bitmap_float_linear_to_luv_rows(
    bit: *mut flow_bitmap_float,
    start_row: u32,
    row_count: u32,
) -> Result<(), FlowError> {
    if !(start_row.wrapping_add(row_count) <= (*bit).h) {
        return Err(nerror!(ErrorKind::InvalidState));
    }
    if (*bit).w.wrapping_mul((*bit).channels) != (*bit).float_stride {
        return Err(nerror!(ErrorKind::InvalidState));
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
    Ok(())
}
pub unsafe fn flow_bitmap_float_luv_to_linear_rows(
    bit: *mut flow_bitmap_float,
    start_row: u32,
    row_count: u32,
) -> Result<(), FlowError> {
    if !(start_row.wrapping_add(row_count) <= (*bit).h) {
        return Err(nerror!(ErrorKind::InvalidState));
    }
    if (*bit).w.wrapping_mul((*bit).channels) != (*bit).float_stride {
        return Err(nerror!(ErrorKind::InvalidState));
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
    Ok(())
}
