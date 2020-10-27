use crate::graphics::prelude::*;

pub unsafe fn flow_bitmap_bgra_fill_rect(
    b: &mut crate::ffi::BitmapBgra,
    x1: u32,
    y1: u32,
    x2: u32,
    y2: u32,
    color_srgb_argb: u32
) -> Result<(), FlowError> {
    if x1 >= x2 || y1 >= y2 || y2 > b.h || x2 > b.w {
        // Either out of bounds or has a width or height of zero.
        return Err(nerror!(ErrorKind::InvalidArgument));
    }
    let step = b.fmt.bytes();
    if step == 1 {
        return Err(nerror!(ErrorKind::InvalidArgument));
    }

    let topleft : *mut u8 = b.pixels.offset((b.stride * y1 + step as u32 * x1) as isize);

    let rect_width_bytes = step as usize * (x2 -x1) as usize;
    let color = color_srgb_argb;

    for byte_offset  in (0..rect_width_bytes).step_by(step) {
        //TODO: probably faster to use assignment than memcpy
        topleft.offset(byte_offset as isize).copy_from_nonoverlapping(&color as *const u32 as *const u8, step);
    }
    // Copy downwards
    for y in 1..(y2 - y1){
        topleft.offset(b.stride as isize * y as isize).copy_from_nonoverlapping(topleft, rect_width_bytes);
    }
    Ok(())
}
