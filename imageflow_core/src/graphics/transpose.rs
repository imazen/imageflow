#![allow(non_snake_case)]
// Consider using mit-licensed https://github.com/ejmahler/transpose/blob/master/src/out_of_place.rs
// for recursive approach?
use crate::graphics::prelude::*;
use multiversion::multiversion;

#[cfg(feature = "nightly")]
use std::simd::{Simd};

#[cfg(feature = "nightly")]
use  std::simd::prelude::SimdUint;

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[inline]
#[allow(unused_assignments)]
#[cfg(target_arch = "x86_64")]
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
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn transpose4x4_sse2(A: *const u32, B: *mut u32, stride_a: usize, stride_b: usize) {
    let row1: __m128i = _mm_loadu_si128(A as *const __m128i);
    let row2: __m128i = _mm_loadu_si128(A.add(stride_a) as *const __m128i);
    let row3: __m128i = _mm_loadu_si128(A.add(stride_a * 2) as *const __m128i);
    let row4: __m128i = _mm_loadu_si128(A.add(stride_a * 3) as *const __m128i);

    let tmp0: __m128i = _mm_unpacklo_epi32(row1, row2);
    let tmp1: __m128i = _mm_unpackhi_epi32(row1, row2);
    let tmp2: __m128i = _mm_unpacklo_epi32(row3, row4);
    let tmp3: __m128i = _mm_unpackhi_epi32(row3, row4);

    let row1 = _mm_unpacklo_epi64(tmp0, tmp2);
    let row2 = _mm_unpackhi_epi64(tmp0, tmp2);
    let row3 = _mm_unpacklo_epi64(tmp1, tmp3);
    let row4 = _mm_unpackhi_epi64(tmp1, tmp3);

    _mm_storeu_si128(B as *mut __m128i, row1);
    _mm_storeu_si128(B.add(stride_b) as *mut __m128i, row2);
    _mm_storeu_si128(B.add(stride_b * 2) as *mut __m128i, row3);
    _mm_storeu_si128(B.add(stride_b * 3) as *mut __m128i, row4);
}

#[target_feature(enable = "avx2")]
#[cfg(target_arch = "x86_64")]
unsafe fn transpose_8x8_avx2(src: *const u32, dst: *mut u32, src_stride: usize, dst_stride: usize) {
    // Load 8 rows of 8 32-bit integers each
    let row0 = _mm256_loadu_si256(src as *const __m256i);
    let row1 = _mm256_loadu_si256(src.add(src_stride) as *const __m256i);
    let row2 = _mm256_loadu_si256(src.add(src_stride * 2) as *const __m256i);
    let row3 = _mm256_loadu_si256(src.add(src_stride * 3) as *const __m256i);
    let row4 = _mm256_loadu_si256(src.add(src_stride * 4) as *const __m256i);
    let row5 = _mm256_loadu_si256(src.add(src_stride * 5) as *const __m256i);
    let row6 = _mm256_loadu_si256(src.add(src_stride * 6) as *const __m256i);
    let row7 = _mm256_loadu_si256(src.add(src_stride * 7) as *const __m256i);

    // Transpose 8x8 matrix
    let tmp0 = _mm256_unpacklo_epi32(row0, row1);
    let tmp1 = _mm256_unpackhi_epi32(row0, row1);
    let tmp2 = _mm256_unpacklo_epi32(row2, row3);
    let tmp3 = _mm256_unpackhi_epi32(row2, row3);
    let tmp4 = _mm256_unpacklo_epi32(row4, row5);
    let tmp5 = _mm256_unpackhi_epi32(row4, row5);
    let tmp6 = _mm256_unpacklo_epi32(row6, row7);
    let tmp7 = _mm256_unpackhi_epi32(row6, row7);

    let tmp8 = _mm256_unpacklo_epi64(tmp0, tmp2);
    let tmp9 = _mm256_unpackhi_epi64(tmp0, tmp2);
    let tmp10 = _mm256_unpacklo_epi64(tmp1, tmp3);
    let tmp11 = _mm256_unpackhi_epi64(tmp1, tmp3);
    let tmp12 = _mm256_unpacklo_epi64(tmp4, tmp6);
    let tmp13 = _mm256_unpackhi_epi64(tmp4, tmp6);
    let tmp14 = _mm256_unpacklo_epi64(tmp5, tmp7);
    let tmp15 = _mm256_unpackhi_epi64(tmp5, tmp7);

    let row0 = _mm256_permute2x128_si256(tmp8, tmp12, 0x20);
    let row1 = _mm256_permute2x128_si256(tmp9, tmp13, 0x20);
    let row2 = _mm256_permute2x128_si256(tmp10, tmp14, 0x20);
    let row3 = _mm256_permute2x128_si256(tmp11, tmp15, 0x20);
    let row4 = _mm256_permute2x128_si256(tmp8, tmp12, 0x31);
    let row5 = _mm256_permute2x128_si256(tmp9, tmp13, 0x31);
    let row6 = _mm256_permute2x128_si256(tmp10, tmp14, 0x31);
    let row7 = _mm256_permute2x128_si256(tmp11, tmp15, 0x31);

    // Store the transposed rows
    _mm256_storeu_si256(dst as *mut __m256i, row0);
    _mm256_storeu_si256(dst.add(dst_stride) as *mut __m256i, row1);
    _mm256_storeu_si256(dst.add(dst_stride * 2) as *mut __m256i, row2);
    _mm256_storeu_si256(dst.add(dst_stride * 3) as *mut __m256i, row3);
    _mm256_storeu_si256(dst.add(dst_stride * 4) as *mut __m256i, row4);
    _mm256_storeu_si256(dst.add(dst_stride * 5) as *mut __m256i, row5);
    _mm256_storeu_si256(dst.add(dst_stride * 6) as *mut __m256i, row6);
    _mm256_storeu_si256(dst.add(dst_stride * 7) as *mut __m256i, row7);
}

#[inline]
#[target_feature(enable = "neon")]
#[cfg(target_arch = "aarch64")]
unsafe fn transpose_4x4_neon(src: *const u32, dst: *mut u32, src_stride: usize, dst_stride: usize) {
    let r0 = vld1q_f32(src as *const f32);
    let r1 = vld1q_f32(src.add(src_stride) as *const f32);
    let r2 = vld1q_f32(src.add(src_stride * 2) as *const f32);
    let r3 = vld1q_f32(src.add(src_stride * 3) as *const f32);

    let c0 = vzip1q_f32(r0, r1);
    let c1 = vzip2q_f32(r0, r1);
    let c2 = vzip1q_f32(r2, r3);
    let c3 = vzip2q_f32(r2, r3);

    let t0 = vcombine_f32(vget_low_f32(c0), vget_low_f32(c2));
    let t1 = vcombine_f32(vget_high_f32(c0), vget_high_f32(c2));
    let t2 = vcombine_f32(vget_low_f32(c1), vget_low_f32(c3));
    let t3 = vcombine_f32(vget_high_f32(c1), vget_high_f32(c3));

    vst1q_f32(dst as *mut f32, t0);
    vst1q_f32(dst.add(dst_stride) as *mut f32, t1);
    vst1q_f32(dst.add(dst_stride * 2) as *mut f32, t2);
    vst1q_f32(dst.add(dst_stride * 3) as *mut f32, t3);
}

#[target_feature(enable = "neon")]
#[cfg(target_arch = "aarch64")]
pub unsafe fn transpose_8x8_neon(src: *const u32, dst: *mut u32, src_stride: usize, dst_stride: usize) {
    // Transpose top-left 4x4 quadrant
    transpose_4x4_neon(src, dst, src_stride, dst_stride);

    // Transpose top-right 4x4 quadrant
    transpose_4x4_neon(src.add(4), dst.add(dst_stride * 4), src_stride, dst_stride);

    // Transpose bottom-left 4x4 quadrant
    transpose_4x4_neon(src.add(src_stride * 4), dst.add(4), src_stride, dst_stride);

    // Transpose bottom-right 4x4 quadrant
    transpose_4x4_neon(src.add(src_stride * 4).add(4), dst.add(dst_stride * 4).add(4), src_stride, dst_stride);
}
#[inline]
unsafe fn transpose4x4_generic(A: *mut f32, B: *mut f32, lda: i32, ldb: i32) {
    for i in 0..4 {
        for j in 0..4 {
            *B.offset((j * ldb + i) as isize) = *A.offset((i * lda + j) as isize);
        }
    }
}

// #[inline]
// unsafe fn transpose_block_4x4(
//     A: *mut f32,
//     B: *mut f32,
//     n: i32,
//     m: i32,
//     lda: i32,
//     ldb: i32,
//     block_size: i32,
// ) {
//     //#pragma omp parallel for collapse(2)
//     let mut i: i32 = 0 as i32;
//     while i < n {
//         let mut j: i32 = 0 as i32;
//         while j < m {
//             let max_i2: i32 = if i + block_size < n {
//                 (i) + block_size
//             } else {
//                 n
//             };
//             let max_j2: i32 = if j + block_size < m {
//                 (j) + block_size
//             } else {
//                 m
//             };
//             let mut i2: i32 = i;
//             while i2 < max_i2 {
//                 let mut j2: i32 = j;
//                 while j2 < max_j2 {
//                     #[cfg(target_arch = "x86_64")]
//                     {
//                         transpose4x4_sse(
//                             &mut *A.offset((i2 * lda + j2) as isize),
//                             &mut *B.offset((j2 * ldb + i2) as isize),
//                             lda,
//                             ldb,
//                         );
//                     }
//                     #[cfg(target_arch = "aarch64")]
//                     {
//                         transpose4x4_neon(
//                             &mut *A.offset((i2 * lda + j2) as isize),
//                             &mut *B.offset((j2 * ldb + i2) as isize),
//                             lda,
//                             ldb,
//                         );
//                     }
//                     #[cfg(all(not(target_arch = "aarch64"), not(target_arch = "x86_64")))]
//                     {
//                         transpose4x4_generic(
//                             &mut *A.offset((i2 * lda + j2) as isize),
//                             &mut *B.offset((j2 * ldb + i2) as isize),
//                             lda,
//                             ldb,
//                         );
//                     }
//                     j2 += 4 as i32
//                 }
//                 i2 += 4 as i32
//             }
//             j += block_size
//         }
//         i += block_size
//     }
// }

// Generic transposition function for [u32] slices
pub fn transpose_u32_slices(
    from: &[u32],
    to: &mut [u32],
    from_stride: usize,
    to_stride: usize,
    width: usize,
    height: usize,
) -> Result<(), FlowError> {
    if to_stride < height {
        return Err(nerror!(ErrorKind::InvalidArgument,
            "to_stride({}) < height({})", to_stride, height));
    }
    // Ensure we don't go out of bounds
    if from_stride * (height -1) + width > from.len() {
        return Err(nerror!(ErrorKind::InvalidArgument,
            "Slice bounds exceeded: from_stride({}) * (height ({}) - 1) + width ({}) > from.len({})", from_stride, height, width, from.len()));
    }
    if from_stride < width {
        return Err(nerror!(ErrorKind::InvalidArgument,
            "from_stride({}) < width({})", from_stride, width));
    }

    if to_stride * (width - 1) + height > to.len() {
        return Err(nerror!(ErrorKind::InvalidArgument,
            "Slice bounds exceeded: to_stride({}) * (width ({}) - 1) + height ({}) > to.len({})", to_stride, width, height, to.len()));
    }

    let block_size = 128;
    let cropped_h = (height / block_size) * block_size;
    let cropped_w = (width / block_size) * block_size;

    // Transpose the main part of the image
    transpose_multiple_of_block_size_rectangle(from, to, from_stride, to_stride, cropped_w, cropped_h, block_size);

    // Handle the remaining edges
    transpose_edges(from, to, cropped_h, cropped_w, from_stride, to_stride, width, height);

    Ok(())
}


// transpose main cropped part of the image
#[multiversion(targets("x86_64+avx2","aarch64+neon","x86_64+sse4.1"))]
fn transpose_multiple_of_block_size_rectangle(
    src: &[u32],
    dst: &mut [u32],
    src_stride: usize,
    dst_stride: usize,
    width: usize,
    height: usize,
    block_size: usize
) {
    #[cfg(target_arch = "x86_64")]
    let use8x8simd = is_x86_feature_detected!("avx2");

    #[cfg(target_arch = "aarch64")]
    let use8x8simd = std::arch::is_aarch64_feature_detected!("neon");

    #[cfg(all(not(target_arch = "aarch64"), not(target_arch = "x86_64")))]
    let use8x8simd = false;

    #[cfg(target_arch = "x86_64")]
    let use4x4simd = true;

    #[cfg(not(target_arch = "x86_64"))]
    let use4x4simd = false;


    for y_block in (0..height).step_by(block_size) {
        for x_block in (0..width).step_by(block_size) {
            let max_y = (y_block + block_size).min(height);
            let max_x = (x_block + block_size).min(width);

            if use8x8simd {
                for y in (y_block..max_y).step_by(8) {
                    for x in (x_block..max_x).step_by(8) {
                        #[cfg(target_arch = "x86_64")]
                        unsafe {
                            transpose_8x8_avx2(
                                src.as_ptr().add(y * src_stride + x),
                                dst.as_mut_ptr().add(x * dst_stride + y),
                                src_stride,
                                dst_stride,
                            );
                        }
                        #[cfg(target_arch = "aarch64")]
                        unsafe {
                            transpose_8x8_neon(
                                src.as_ptr().add(y * src_stride + x),
                                dst.as_mut_ptr().add(x * dst_stride + y),
                                src_stride,
                                dst_stride,
                            );
                        }
                    }
                }
            } else if use4x4simd {
                #[cfg(target_arch = "x86_64")]
                unsafe {
                    for y in (y_block..max_y).step_by(4) {
                        for x in (x_block..max_x).step_by(4) {

                            transpose4x4_sse2(
                                src.as_ptr().add(y * src_stride + x),
                                dst.as_mut_ptr().add(x * dst_stride + y),
                                src_stride,
                                dst_stride,
                            );
                        }
                    }
                }
            }else {
                #[cfg(feature = "nightly")]
                {
                    transpose_in_blocks_of_8x8_simd(
                        src,
                        dst,
                        src_stride,
                        dst_stride,
                        x_block,
                        y_block,
                        max_x,
                        max_y,
                    );
                }
                #[cfg(not(feature = "nightly"))]
                {
                    transpose_in_blocks_of_8x8_scalar(
                        src,
                        dst,
                        src_stride,
                        dst_stride,
                        x_block,
                        y_block,
                        max_x,
                        max_y,
                    );
                }
            }

        }
    }
}


#[multiversion(targets("x86_64+avx2","aarch64+neon","x86_64+sse4.1"))]
#[cfg(feature = "nightly")]
fn transpose_in_blocks_of_8x8_simd(
    src: &[u32],
    dst: &mut [u32],
    src_stride: usize,
    dst_stride: usize,
    x1: usize,
    y1: usize,
    x2: usize,
    y2: usize,
) {
    for y in (y1..y2).step_by(8) {
        let src_row = y * src_stride;
        for x in (x1..x2).step_by(8) {
            let block_x = src_row + x;
            let block_y = x * dst_stride + y;

            // Load 8x8 block from source
            let input: [Simd<u32, 8>; 8] = [
                Simd::from_slice(&src[block_x..]),
                Simd::from_slice(&src[block_x + src_stride..]),
                Simd::from_slice(&src[block_x + 2 * src_stride..]),
                Simd::from_slice(&src[block_x + 3 * src_stride..]),
                Simd::from_slice(&src[block_x + 4 * src_stride..]),
                Simd::from_slice(&src[block_x + 5 * src_stride..]),
                Simd::from_slice(&src[block_x + 6 * src_stride..]),
                Simd::from_slice(&src[block_x + 7 * src_stride..]),
            ];

            // Transpose the block
            let transposed = transpose_8x8_simd(input);

            // Store the transposed block
            for (i, row) in transposed.iter().enumerate() {
                row.copy_to_slice(&mut dst[block_y + i * dst_stride..]);
            }
        }
    }
}

#[cfg(feature = "nightly")]
fn transpose_8x8_simd(input: [Simd<u32, 8>; 8]) -> [Simd<u32, 8>; 8] {
    let [r0, r1, r2, r3, r4, r5, r6, r7] = input;

    // Transpose pairs
    let (r0, r1) = r0.interleave(r1);
    let (r2, r3) = r2.interleave(r3);
    let (r4, r5) = r4.interleave(r5);
    let (r6, r7) = r6.interleave(r7);

    // Transpose quads
    let (r0, r2) = r0.interleave(r2);
    let (r1, r3) = r1.interleave(r3);
    let (r4, r6) = r4.interleave(r6);
    let (r5, r7) = r5.interleave(r7);

    // Final transpose
    let (r0, r4) = r0.interleave(r4);
    let (r1, r5) = r1.interleave(r5);
    let (r2, r6) = r2.interleave(r6);
    let (r3, r7) = r3.interleave(r7);

    [r0, r1, r2, r3, r4, r5, r6, r7]
}

#[cfg(not(feature = "nightly"))]
#[multiversion(targets("x86_64+avx2","aarch64+neon","x86_64+sse4.1"))]
fn transpose_in_blocks_of_8x8_scalar(
    src: &[u32],
    dst: &mut [u32],
    src_stride: usize,
    dst_stride: usize,
    x1: usize,
    y1: usize,
    x2: usize,
    y2: usize,
) {
    for y in (y1..y2).step_by(8) {
        let src_row = y * src_stride;
        for x in (x1..x2).step_by(8) {
            let block_x = src_row + x;
            let block_y = x * dst_stride + y;

            unsafe {
                for i in 0..8 {
                    for j in 0..8 {
                        *dst.get_unchecked_mut(block_y + j * dst_stride + i) =
                            *src.get_unchecked(block_x + i * src_stride + j);
                    }
                }
            }
        }
    }
}

#[multiversion(targets("x86_64+avx2","aarch64+neon","x86_64+sse4.1"))]
fn transpose_in_blocks_of_4x4(
    src: &[u32],
    dst: &mut [u32],
    src_stride: usize,
    dst_stride: usize,
    x1: usize,
    y1: usize,
    x2: usize,
    y2: usize,
) {
    unsafe {
        for y in (y1..y2).step_by(4) {
            let src_row = y*src_stride;
            for x in (x1..x2).step_by(4) {
                let block_x = src_row + x;
                let block_y = x*dst_stride + y;
                for i in 0..4 {
                    for j in 0..4 {
                        *dst.get_unchecked_mut(block_y + j * dst_stride + i) =
                        *src.get_unchecked(block_x + i * src_stride + j);
                    }
                }
            }
        }
    }
}

// #[multiversion(targets("x86_64+avx2","aarch64+neon","x86_64+sse4.1"))]
#[inline(always)]
fn transpose_4x4(
    src: &[u32],
    dst: &mut [u32],
    src_stride: usize,
    dst_stride: usize,
) {
    unsafe {
        for i in 0..4 {
            for j in 0..4 {
                *dst.get_unchecked_mut(j * dst_stride + i) = *src.get_unchecked(i * src_stride + j);
            }
        }
    }
}


// #[multiversion(targets("x86_64+avx2","aarch64+neon","x86_64+sse4.1"))]
#[inline(always)]
fn transpose_8x8(
    src: &[u32],
    dst: &mut [u32],
    src_stride: usize,
    dst_stride: usize,
) {
    unsafe {
        for i in 0..8 {
            for j in 0..8 {
                *dst.get_unchecked_mut(j * dst_stride + i) = *src.get_unchecked(i * src_stride + j);
            }
        }
    }
}



#[inline(always)]
fn transpose_edges(
    src: &[u32],
    dst: &mut [u32],
    cropped_height: usize,
    cropped_width: usize,
    src_stride: usize,
    dst_stride: usize,
    width: usize,
    height: usize,
) {
    unsafe {
        // Transpose the right edge
        for x in cropped_width..width {
            for y in 0..cropped_height {
                *dst.get_unchecked_mut(x * dst_stride + y) = *src.get_unchecked(y * src_stride + x);
            }
        }

        // Transpose the bottom edge
        for y in cropped_height..height {
            for x in 0..width {
                *dst.get_unchecked_mut(x * dst_stride + y) = *src.get_unchecked(y * src_stride + x);
            }
        }
    }
}

// Function for BitmapWindowMut
pub fn bitmap_window_transpose(
    from: &mut BitmapWindowMut<u8>,
    to: &mut BitmapWindowMut<u8>
) -> Result<(), FlowError> {
    if from.w() != to.h() || from.h() != to.w() || from.info().pixel_layout() != to.info().pixel_layout() {
        return Err(nerror!(ErrorKind::InvalidArgument, "For transposition, canvas and input formats must be the same and dimensions must be swapped"));
    }

    if from.info().pixel_layout() != PixelLayout::BGRA {
        return Err(nerror!(ErrorKind::InvalidArgument, "Only BGRA layout is supported"));
    }

    let from_slice = unsafe {
        std::slice::from_raw_parts(from.slice_mut().as_ptr() as *const u32, from.slice_mut().len() / 4)
    };
    let to_slice = unsafe {
        std::slice::from_raw_parts_mut(to.slice_mut().as_mut_ptr() as *mut u32, to.slice_mut().len() / 4)
    };

    let from_stride = from.info().t_stride() as usize / 4;
    let to_stride = to.info().t_stride() as usize / 4;
    let width = from.w() as usize;
    let height = from.h() as usize;

    transpose_u32_slices(from_slice, to_slice, from_stride, to_stride, width, height).map_err(|e| e.at(here!()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn test_transpose_u32_slices_square() {
        let from = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        let mut to = vec![0; 9];
        transpose_u32_slices(&from, &mut to, 3, 3, 3, 3).unwrap();
        assert_eq!(to, vec![1, 4, 7, 2, 5, 8, 3, 6, 9]);
    }

    #[test]
    fn test_transpose_u32_slices_rectangle_wide() {
        let from = vec![1, 2, 3, 4, 5, 6];
        let mut to = vec![0; 6];
        transpose_u32_slices(&from, &mut to, 6, 1, 6, 1).unwrap();
        assert_eq!(to, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_transpose_u32_slices_rectangle_tall() {
        let from = vec![1, 2, 3, 4, 5, 6];
        let mut to = vec![0; 6];
        transpose_u32_slices(&from, &mut to, 1, 6, 1, 6).unwrap();
        assert_eq!(to, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_transpose_u32_slices_small_rectangle() {
        let from = vec![1, 2, 3, 4, 5, 6];
        let mut to = vec![0; 6];
        transpose_u32_slices(&from, &mut to, 3, 2, 3, 2).unwrap();
        assert_eq!(to, vec![1, 4, 2, 5, 3, 6]);
    }

    #[test]
    fn test_transpose_u32_slices_with_stride() {
        let from = vec![1, 2, 3, 0, 4, 5, 6, 0, 7, 8, 9, 0];
        let mut to = vec![0; 9];
        transpose_u32_slices(&from, &mut to, 4, 3, 3, 3).unwrap();
        assert_eq!(to, vec![1, 4, 7, 2, 5, 8, 3, 6, 9]);
    }

    #[test]
    fn test_transpose_u32_slices_partial_fill() {
        let from = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        let mut to = vec![0; 12];
        transpose_u32_slices(&from, &mut to, 3, 4, 3, 3).unwrap();
        assert_eq!(to, vec![1, 4, 7, 0, 2, 5, 8, 0, 3, 6, 9, 0]);
    }

    #[test]
    fn test_transpose_u32_slices_error_dimensions_mismatch() {
        let from = vec![1, 2, 3, 4];
        let mut to = vec![0; 4];
        let result = transpose_u32_slices(&from, &mut to, 2, 2, 3, 2);
        assert!(result.is_err());
    }

    #[test]
    fn test_transpose_large_square_matrix() {
        let size = 256;
        let mut rng = rand::rng();
        let from: Vec<u32> = (0..size*size).map(|_| rng.random()).collect();
        let mut to = vec![0; size*size];

        transpose_u32_slices(&from, &mut to, size, size, size, size).unwrap();

        for i in 0..size {
            for j in 0..size {
                assert_eq!(from[i*size + j], to[j*size + i]);
            }
        }
    }

    #[test]
    fn test_transpose_large_rectangular_matrix() {
        let width = 256;
        let height = 128;
        let mut rng = rand::rng();
        let from: Vec<u32> = (0..width*height).map(|_| rng.random()).collect();
        let mut to = vec![0; width*height];

        transpose_u32_slices(&from, &mut to, width, height, width, height).unwrap();

        for i in 0..height {
            for j in 0..width {
                assert_eq!(from[i*width + j], to[j*height + i]);
            }
        }
    }

    #[test]
    fn test_transpose_with_padding() {
        let width = 130;
        let height = 100;
        let src_stride = 132;
        let dst_stride = 104;
        let mut rng = rand::rng();
        let from: Vec<u32> = (0..src_stride*height).map(|_| rng.random()).collect();
        let mut to = vec![0; dst_stride*width];

        transpose_u32_slices(&from, &mut to, src_stride, dst_stride, width, height).unwrap();

        for i in 0..height {
            for j in 0..width {
                assert_eq!(from[i*src_stride + j], to[j*dst_stride + i]);
            }
        }
    }

    #[test]
    fn test_transpose_small_matrices() {
        let sizes = vec![(4, 4), (8, 8), (16, 16), (32, 32), (64, 64)];

        for (width, height) in sizes {
            let mut rng = rand::rng();
            let from: Vec<u32> = (0..width*height).map(|_| rng.random()).collect();
            let mut to = vec![0; width*height];

            transpose_u32_slices(&from, &mut to, width, height, width, height).unwrap();

            for i in 0..height {
                for j in 0..width {
                    assert_eq!(from[i*width + j], to[j*height + i]);
                }
            }
        }
    }

    #[test]
    fn test_transpose_edge_cases() {
        // Test 1x1 matrix
        let from = vec![42];
        let mut to = vec![0];
        transpose_u32_slices(&from, &mut to, 1, 1, 1, 1).unwrap();
        assert_eq!(to[0], 42);

        // Test 1xN matrix
        let from = vec![1, 2, 3, 4, 5];
        let mut to = vec![0; 5];
        transpose_u32_slices(&from, &mut to, 5, 1, 5, 1).unwrap();
        assert_eq!(to, from);

        // Test Nx1 matrix
        let from = vec![1, 2, 3, 4, 5];
        let mut to = vec![0; 5];
        transpose_u32_slices(&from, &mut to, 1, 5, 1, 5).unwrap();
        assert_eq!(to, from);
    }

    #[test]
    fn test_transpose_error_cases() {
        let from = vec![1, 2, 3, 4];
        let mut to = vec![0; 4];

        // Test invalid from_stride
        assert!(transpose_u32_slices(&from, &mut to, 1, 2, 2, 2).is_err());

        // Test invalid to_stride
        assert!(transpose_u32_slices(&from, &mut to, 2, 1, 2, 2).is_err());

        // Test out of bounds access in from slice
        assert!(transpose_u32_slices(&from, &mut to, 3, 2, 3, 2).is_err());

        // Test out of bounds access in to slice
        assert!(transpose_u32_slices(&from, &mut to, 2, 3, 2, 3).is_err());
    }
    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_transpose_8x8_neon() {
        // Create input matrix with obvious values
        let input: [u32; 64] = [
            0,  1,  2,  3,  4,  5,  6,  7,
            10, 11, 12, 13, 14, 15, 16, 17,
            20, 21, 22, 23, 24, 25, 26, 27,
            30, 31, 32, 33, 34, 35, 36, 37,
            40, 41, 42, 43, 44, 45, 46, 47,
            50, 51, 52, 53, 54, 55, 56, 57,
            60, 61, 62, 63, 64, 65, 66, 67,
            70, 71, 72, 73, 74, 75, 76, 77
        ];

        let mut output = [0u32; 64];

        unsafe {
            transpose_8x8_neon(input.as_ptr(), output.as_mut_ptr(), 8, 8);
        }

        // Expected transposed matrix
        let expected: [u32; 64] = [
            0, 10, 20, 30, 40, 50, 60, 70,
            1, 11, 21, 31, 41, 51, 61, 71,
            2, 12, 22, 32, 42, 52, 62, 72,
            3, 13, 23, 33, 43, 53, 63, 73,
            4, 14, 24, 34, 44, 54, 64, 74,
            5, 15, 25, 35, 45, 55, 65, 75,
            6, 16, 26, 36, 46, 56, 66, 76,
            7, 17, 27, 37, 47, 57, 67, 77
        ];

        assert_eq!(output, expected, "Transposed matrix does not match expected output");
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_transpose_8x8_avx2() {
        if !is_x86_feature_detected!("avx2") {
            println!("AVX2 not supported, skipping test");
            return;
        }

        // Create input matrix with obvious values
        let input: [u32; 64] = [
            0,  1,  2,  3,  4,  5,  6,  7,
            10, 11, 12, 13, 14, 15, 16, 17,
            20, 21, 22, 23, 24, 25, 26, 27,
            30, 31, 32, 33, 34, 35, 36, 37,
            40, 41, 42, 43, 44, 45, 46, 47,
            50, 51, 52, 53, 54, 55, 56, 57,
            60, 61, 62, 63, 64, 65, 66, 67,
            70, 71, 72, 73, 74, 75, 76, 77
        ];

        let mut output = [0u32; 64];

        unsafe {
            transpose_8x8_avx2(input.as_ptr(), output.as_mut_ptr(), 8, 8);
        }

        // Expected transposed matrix
        let expected: [u32; 64] = [
            0, 10, 20, 30, 40, 50, 60, 70,
            1, 11, 21, 31, 41, 51, 61, 71,
            2, 12, 22, 32, 42, 52, 62, 72,
            3, 13, 23, 33, 43, 53, 63, 73,
            4, 14, 24, 34, 44, 54, 64, 74,
            5, 15, 25, 35, 45, 55, 65, 75,
            6, 16, 26, 36, 46, 56, 66, 76,
            7, 17, 27, 37, 47, 57, 67, 77
        ];

        assert_eq!(output, expected, "Transposed matrix does not match expected output");
    }
}
