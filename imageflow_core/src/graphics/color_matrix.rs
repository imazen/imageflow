use crate::graphics::prelude::*;

pub unsafe fn flow_bitmap_bgra_apply_color_matrix(
    bmp: *mut flow_bitmap_bgra,
    row: u32,
    count: u32,
    m: *const *mut f32,
) -> Result<(), FlowError> {
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
        //Unsupported_pixel_format
        return Err(nerror!(ErrorKind::InvalidState));
    }
    Ok(())
}

// note: this file isn't exercised by test suite
pub unsafe fn flow_bitmap_float_apply_color_matrix(
    bmp: *mut flow_bitmap_float,
    row: u32,
    count: u32,
    m: *mut *mut f32,
) -> Result<(), FlowError> {
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
            Ok(())
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
            Ok(())
        }
        _ => {
            //Unsupported_pixel_format
            Err(nerror!(ErrorKind::InvalidState))
        }
    }
}
