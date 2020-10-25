use crate::graphics::prelude::*;

#[inline]
unsafe fn int_max(a: i32, b: i32) -> i32 {
    return if a >= b { a } else { b };
}

pub unsafe fn flow_bitmap_bgra_populate_histogram(
    bmp: *const flow_bitmap_bgra,
    histograms: *mut u64,
    histogram_size_per_channel: u32,
    histogram_count: u32,
    pixels_sampled: *mut u64,
) -> Result<(), FlowError> {
    let row: u32 = 0;
    let count: u32 = (*bmp).h;
    let stride: u32 = (*bmp).stride;
    let ch: u32 = flow_pixel_format_bytes_per_pixel((*bmp).fmt);
    let w: u32 = (*bmp).w;
    let h: u32 = (row.wrapping_add(count)).min((*bmp).h);
    if histogram_size_per_channel != 256 {
        // We're restricting it to this for speed
        return Err(nerror!(ErrorKind::InvalidArgument));
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
            return Err(nerror!(ErrorKind::InvalidState));
        }
        *pixels_sampled = h.wrapping_sub(row).wrapping_mul(w) as u64
    } else {
        // Unsupported pixel kind
        return Err(nerror!(ErrorKind::InvalidState));
    }
    Ok(())
}