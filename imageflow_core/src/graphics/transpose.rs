#![allow(non_snake_case)]

use crate::graphics::prelude::*;

#[inline]
#[allow(unused_assignments)]
unsafe fn transpose4x4_sse(A: *mut f32, B: *mut f32, lda: i32, ldb: i32) {
    let mut row1: __m128 = _mm_loadu_ps(&mut *A.offset((0 as i32 * lda) as isize));
    let mut row2: __m128 = _mm_loadu_ps(&mut *A.offset((1 as i32 * lda) as isize));
    let mut row3: __m128 = _mm_loadu_ps(&mut *A.offset((2 as i32 * lda) as isize));
    let mut row4: __m128 = _mm_loadu_ps(&mut *A.offset((3 as i32 * lda) as isize));
    let mut tmp3: __m128 = _mm_setzero_ps();
    let mut tmp2: __m128 = _mm_setzero_ps();
    let mut tmp1: __m128 = _mm_setzero_ps();
    let mut tmp0: __m128 = _mm_setzero_ps();
    tmp0 = _mm_unpacklo_ps(row1, row2);
    tmp2 = _mm_unpacklo_ps(row3, row4);
    tmp1 = _mm_unpackhi_ps(row1, row2);
    tmp3 = _mm_unpackhi_ps(row3, row4);
    row1 = _mm_movelh_ps(tmp0, tmp2);
    row2 = _mm_movehl_ps(tmp2, tmp0);
    row3 = _mm_movelh_ps(tmp1, tmp3);
    row4 = _mm_movehl_ps(tmp3, tmp1);
    _mm_storeu_ps(&mut *B.offset((0 as i32 * ldb) as isize), row1);
    _mm_storeu_ps(&mut *B.offset((1 as i32 * ldb) as isize), row2);
    _mm_storeu_ps(&mut *B.offset((2 as i32 * ldb) as isize), row3);
    _mm_storeu_ps(&mut *B.offset((3 as i32 * ldb) as isize), row4);
}
#[inline]
unsafe fn transpose_block_SSE4x4(
    A: *mut f32,
    B: *mut f32,
    n: i32,
    m: i32,
    lda: i32,
    ldb: i32,
    block_size: i32,
) {
    //#pragma omp parallel for collapse(2)
    let mut i: i32 = 0 as i32;
    while i < n {
        let mut j: i32 = 0 as i32;
        while j < m {
            let max_i2: i32 = if i + block_size < n {
                (i) + block_size
            } else {
                n
            };
            let max_j2: i32 = if j + block_size < m {
                (j) + block_size
            } else {
                m
            };
            let mut i2: i32 = i;
            while i2 < max_i2 {
                let mut j2: i32 = j;
                while j2 < max_j2 {
                    transpose4x4_sse(
                        &mut *A.offset((i2 * lda + j2) as isize),
                        &mut *B.offset((j2 * ldb + i2) as isize),
                        lda,
                        ldb,
                    );
                    j2 += 4 as i32
                }
                i2 += 4 as i32
            }
            j += block_size
        }
        i += block_size
    }
}

pub unsafe fn flow_bitmap_bgra_transpose(
    from: *mut flow_bitmap_bgra,
    to: *mut flow_bitmap_bgra,
) -> Result<(), FlowError> {
    if (*from).w != (*to).h || (*from).h != (*to).w || (*from).fmt != (*to).fmt  {
        return Err(nerror!(ErrorKind::InvalidArgument, "Canvas and input formats must be the same and dimensions must be swapped"));
    }

    if (*from).fmt != PixelFormat::Bgra32
        && (*from).fmt != PixelFormat::Bgr32
    {
        flow_bitmap_bgra_transpose_slow( from, to)
            .map_err(|e| e.at(here!()))?;

        return Ok(());
    }
    // We require 8 when we only need 4 - in case we ever want to enable avx (like if we make it faster)
    let min_block_size: i32 = 8 as i32;
    // Strides must be multiple of required alignments
    if (*from).stride.wrapping_rem(min_block_size as u32) != 0 as i32 as u32
        || (*to).stride.wrapping_rem(min_block_size as u32) != 0 as i32 as u32
    {
        return Err(nerror!(ErrorKind::InvalidArgument));
    }
    // 256 (1024x1024 bytes) at 18.18ms, 128 at 18.6ms,  64 at 20.4ms, 16 at 25.71ms
    let block_size: i32 = 128 as i32;
    let cropped_h: i32 = (*from)
        .h
        .wrapping_sub((*from).h.wrapping_rem(min_block_size as u32))
        as i32;
    let cropped_w: i32 = (*from)
        .w
        .wrapping_sub((*from).w.wrapping_rem(min_block_size as u32))
        as i32;
    transpose_block_SSE4x4(
        (*from).pixels as *mut f32,
        (*to).pixels as *mut f32,
        cropped_h,
        cropped_w,
        (*from).stride.wrapping_div(4u32) as i32,
        (*to).stride.wrapping_div(4u32) as i32,
        block_size,
    );
    // Copy missing bits
    let mut x: u32 = cropped_h as u32;
    while x < (*to).w {
        let mut y: u32 = 0 as i32 as u32;
        while y < (*to).h {
            *(&mut *(*to).pixels.offset(
                x.wrapping_mul(4u32)
                    .wrapping_add(y.wrapping_mul((*to).stride)) as isize,
            ) as *mut libc::c_uchar as *mut u32) = *(&mut *(*from).pixels.offset(
                x.wrapping_mul((*from).stride)
                    .wrapping_add(y.wrapping_mul(4u32)) as isize,
            ) as *mut libc::c_uchar
                as *mut u32);
            y = y.wrapping_add(1)
        }
        x = x.wrapping_add(1)
    }
    let mut x_0: u32 = 0 as i32 as u32;
    while x_0 < cropped_h as u32 {
        let mut y_0: u32 = cropped_w as u32;
        while y_0 < (*to).h {
            *(&mut *(*to).pixels.offset(
                x_0.wrapping_mul(4u32)
                    .wrapping_add(y_0.wrapping_mul((*to).stride)) as isize,
            ) as *mut libc::c_uchar as *mut u32) = *(&mut *(*from).pixels.offset(
                x_0.wrapping_mul((*from).stride)
                    .wrapping_add(y_0.wrapping_mul(4u32)) as isize,
            ) as *mut libc::c_uchar
                as *mut u32);
            y_0 = y_0.wrapping_add(1)
        }
        x_0 = x_0.wrapping_add(1)
    }
    return Ok(());
}

unsafe fn flow_bitmap_bgra_transpose_slow(
    from: *mut flow_bitmap_bgra,
    to: *mut flow_bitmap_bgra,
) -> Result<(), FlowError> {
    if (*from).w != (*to).h || (*from).h != (*to).w || (*from).fmt as u32 != (*to).fmt as u32 {
        return Err(nerror!(ErrorKind::InvalidArgument));
    }
    if (*from).fmt == PixelFormat::Bgra32
        || (*from).fmt == PixelFormat::Bgr32
    {
        let mut x: u32 = 0 as i32 as u32;
        while x < (*to).w {
            let mut y: u32 = 0 as i32 as u32;
            while y < (*to).h {
                *(&mut *(*to).pixels.offset(
                    x.wrapping_mul(4u32)
                        .wrapping_add(y.wrapping_mul((*to).stride)) as isize,
                ) as *mut libc::c_uchar as *mut u32) = *(&mut *(*from).pixels.offset(
                    x.wrapping_mul((*from).stride)
                        .wrapping_add(y.wrapping_mul(4u32)) as isize,
                ) as *mut libc::c_uchar
                    as *mut u32);
                y = y.wrapping_add(1)
            }
            x = x.wrapping_add(1)
        }
        return Ok(());
    } else if (*from).fmt == PixelFormat::Bgr24 {
        let from_stride: i32 = (*from).stride as i32;
        let to_stride: i32 = (*to).stride as i32;
        let mut x_0: u32 = 0 as i32 as u32;
        let mut x_stride: u32 = 0 as i32 as u32;
        let mut x_3: u32 = 0 as i32 as u32;
        while x_0 < (*to).w {
            let mut y_0: u32 = 0 as i32 as u32;
            let mut y_stride: u32 = 0 as i32 as u32;
            let mut y_3: u32 = 0 as i32 as u32;
            while y_0 < (*to).h {
                *(*to).pixels.offset(x_3.wrapping_add(y_stride) as isize) =
                    *(*from).pixels.offset(x_stride.wrapping_add(y_3) as isize);
                *(*to)
                    .pixels
                    .offset(x_3.wrapping_add(y_stride).wrapping_add(1u32) as isize) = *(*from)
                    .pixels
                    .offset(x_stride.wrapping_add(y_3).wrapping_add(1u32) as isize);
                *(*to)
                    .pixels
                    .offset(x_3.wrapping_add(y_stride).wrapping_add(2u32) as isize) = *(*from)
                    .pixels
                    .offset(x_stride.wrapping_add(y_3).wrapping_add(2u32) as isize);
                y_0 = y_0.wrapping_add(1);
                y_stride = (y_stride as u32).wrapping_add(to_stride as u32) as u32 as u32;
                y_3 = (y_3 as u32).wrapping_add(3u32) as u32 as u32
            }
            x_0 = x_0.wrapping_add(1);
            x_stride = (x_stride as u32).wrapping_add(from_stride as u32) as u32 as u32;
            x_3 = (x_3 as u32).wrapping_add(3u32) as u32 as u32
        }
        return Ok(());
    } else {
        return Err(nerror!(ErrorKind::InvalidArgument));
    };
}
/*
static void unpack24bitRow(u32 width, unsigned char* sourceLine, unsigned char* destArray){
    for (u32 i = 0; i < width; i++){

        memcpy(destArray + i * 4, sourceLine + i * 3, 3);
        destArray[i * 4 + 3] = 255;
    }
}
*/