use crate::graphics::prelude::*;
use archmage::incant;
use archmage::prelude::*;

// We reinterpret u32 pixels as f32 for SIMD shuffles (no arithmetic, just lane permutation).
// bytemuck::cast_slice handles the u8→u32 and u32→f32 conversions safely.

const BLOCK_SIZE: usize = 128;

/// Transpose a matrix of u32 values from `from` into `to`.
/// The source has `width` columns, `height` rows, and `from_stride` elements per row.
/// The destination has `height` columns, `width` rows, and `to_stride` elements per row.
pub fn transpose_u32_slices(
    from: &[u32],
    to: &mut [u32],
    from_stride: usize,
    to_stride: usize,
    width: usize,
    height: usize,
) -> Result<(), FlowError> {
    if to_stride < height {
        return Err(nerror!(
            ErrorKind::InvalidArgument,
            "to_stride({}) < height({})",
            to_stride,
            height
        ));
    }
    if from_stride * (height - 1) + width > from.len() {
        return Err(nerror!(ErrorKind::InvalidArgument,
            "Slice bounds exceeded: from_stride({}) * (height ({}) - 1) + width ({}) > from.len({})", from_stride, height, width, from.len()));
    }
    if from_stride < width {
        return Err(nerror!(
            ErrorKind::InvalidArgument,
            "from_stride({}) < width({})",
            from_stride,
            width
        ));
    }
    if to_stride * (width - 1) + height > to.len() {
        return Err(nerror!(
            ErrorKind::InvalidArgument,
            "Slice bounds exceeded: to_stride({}) * (width ({}) - 1) + height ({}) > to.len({})",
            to_stride,
            width,
            height,
            to.len()
        ));
    }

    // Reinterpret u32 slices as f32 for SIMD shuffle operations.
    // No arithmetic is performed — only lane permutation.
    let src: &[f32] = bytemuck::cast_slice(from);
    let dst: &mut [f32] = bytemuck::cast_slice_mut(to);

    incant!(
        transpose_tiled(src, dst, from_stride, to_stride, width, height, BLOCK_SIZE),
        [v3, arm_v2, wasm128, scalar]
    );

    Ok(())
}

/// Transpose a BitmapWindowMut from one orientation to another.
pub fn bitmap_window_transpose(
    from: &mut BitmapWindowMut<u8>,
    to: &mut BitmapWindowMut<u8>,
) -> Result<(), FlowError> {
    if from.w() != to.h()
        || from.h() != to.w()
        || from.info().pixel_layout() != to.info().pixel_layout()
    {
        return Err(nerror!(ErrorKind::InvalidArgument, "For transposition, canvas and input formats must be the same and dimensions must be swapped"));
    }

    if from.info().pixel_layout() != PixelLayout::BGRA {
        return Err(nerror!(ErrorKind::InvalidArgument, "Only BGRA layout is supported"));
    }

    let from_stride = from.info().t_stride() as usize / 4;
    let to_stride = to.info().t_stride() as usize / 4;
    let width = from.w() as usize;
    let height = from.h() as usize;

    let from_slice: &[u32] = bytemuck::cast_slice(from.get_slice());
    let to_slice: &mut [u32] = bytemuck::cast_slice_mut(to.slice_mut());

    transpose_u32_slices(from_slice, to_slice, from_stride, to_stride, width, height)
        .map_err(|e| e.at(here!()))
}

// ---------------------------------------------------------------------------
// Scalar fallback — always available, plain function (ScalarToken has no CPU features)
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn transpose_tiled_scalar(
    _token: ScalarToken,
    src: &[f32],
    dst: &mut [f32],
    src_stride: usize,
    dst_stride: usize,
    width: usize,
    height: usize,
    block_size: usize,
) {
    let cropped_h = (height / block_size) * block_size;
    let cropped_w = (width / block_size) * block_size;

    for y_block in (0..cropped_h).step_by(block_size) {
        for x_block in (0..cropped_w).step_by(block_size) {
            let max_y = y_block + block_size;
            let max_x = x_block + block_size;
            for y in (y_block..max_y).step_by(8) {
                for x in (x_block..max_x).step_by(8) {
                    scalar_transpose_8x8(src, dst, src_stride, dst_stride, x, y);
                }
            }
        }
    }

    transpose_edges_scalar(src, dst, cropped_h, cropped_w, src_stride, dst_stride, width, height);
}

#[inline(always)]
fn scalar_transpose_8x8(
    src: &[f32],
    dst: &mut [f32],
    src_stride: usize,
    dst_stride: usize,
    x: usize,
    y: usize,
) {
    for i in 0..8 {
        for j in 0..8 {
            dst[(x + j) * dst_stride + (y + i)] = src[(y + i) * src_stride + (x + j)];
        }
    }
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn transpose_edges_scalar(
    src: &[f32],
    dst: &mut [f32],
    cropped_height: usize,
    cropped_width: usize,
    src_stride: usize,
    dst_stride: usize,
    width: usize,
    height: usize,
) {
    // Right edge
    for x in cropped_width..width {
        for y in 0..cropped_height {
            dst[x * dst_stride + y] = src[y * src_stride + x];
        }
    }
    // Bottom edge (full width)
    for y in cropped_height..height {
        for x in 0..width {
            dst[x * dst_stride + y] = src[y * src_stride + x];
        }
    }
}

// ---------------------------------------------------------------------------
// x86-64 AVX2 (V3 tier)
// ---------------------------------------------------------------------------

#[cfg(target_arch = "x86_64")]
#[allow(clippy::too_many_arguments)]
#[arcane]
fn transpose_tiled_v3(
    token: X64V3Token,
    src: &[f32],
    dst: &mut [f32],
    src_stride: usize,
    dst_stride: usize,
    width: usize,
    height: usize,
    block_size: usize,
) {
    let cropped_h = (height / block_size) * block_size;
    let cropped_w = (width / block_size) * block_size;

    for y_block in (0..cropped_h).step_by(block_size) {
        for x_block in (0..cropped_w).step_by(block_size) {
            let max_y = y_block + block_size;
            let max_x = x_block + block_size;
            for y in (y_block..max_y).step_by(8) {
                for x in (x_block..max_x).step_by(8) {
                    avx2_transpose_8x8(token, src, dst, src_stride, dst_stride, x, y);
                }
            }
        }
    }

    transpose_edges_scalar(src, dst, cropped_h, cropped_w, src_stride, dst_stride, width, height);
}

/// AVX2 8x8 transpose using f32 shuffle intrinsics (Highway-style 3-stage).
/// Works on u32 data reinterpreted as f32 — no arithmetic, only lane permutation.
#[cfg(target_arch = "x86_64")]
#[rite]
fn avx2_transpose_8x8(
    _token: X64V3Token,
    src: &[f32],
    dst: &mut [f32],
    src_stride: usize,
    dst_stride: usize,
    x: usize,
    y: usize,
) {
    // safe_unaligned_simd takes &[f32; 8] — bounds checked before the unsafe block.
    // unsafe is only for the #[target_feature] calling convention, not memory safety.
    let load_row = |row: usize| -> __m256 {
        let off = row * src_stride + x;
        let arr: &[f32; 8] = src[off..off + 8].try_into().unwrap();
        unsafe { safe_unaligned_simd::x86_64::_mm256_loadu_ps(arr) }
    };

    // Load 8 rows of 8 floats each
    let r0 = load_row(y);
    let r1 = load_row(y + 1);
    let r2 = load_row(y + 2);
    let r3 = load_row(y + 3);
    let r4 = load_row(y + 4);
    let r5 = load_row(y + 5);
    let r6 = load_row(y + 6);
    let r7 = load_row(y + 7);

    // Stage 1: interleave pairs within 128-bit lanes
    let t0 = _mm256_unpacklo_ps(r0, r1);
    let t1 = _mm256_unpackhi_ps(r0, r1);
    let t2 = _mm256_unpacklo_ps(r2, r3);
    let t3 = _mm256_unpackhi_ps(r2, r3);
    let t4 = _mm256_unpacklo_ps(r4, r5);
    let t5 = _mm256_unpackhi_ps(r4, r5);
    let t6 = _mm256_unpacklo_ps(r6, r7);
    let t7 = _mm256_unpackhi_ps(r6, r7);

    // Stage 2: shuffle within 128-bit lanes
    let s0 = _mm256_shuffle_ps::<0x44>(t0, t2);
    let s1 = _mm256_shuffle_ps::<0xEE>(t0, t2);
    let s2 = _mm256_shuffle_ps::<0x44>(t1, t3);
    let s3 = _mm256_shuffle_ps::<0xEE>(t1, t3);
    let s4 = _mm256_shuffle_ps::<0x44>(t4, t6);
    let s5 = _mm256_shuffle_ps::<0xEE>(t4, t6);
    let s6 = _mm256_shuffle_ps::<0x44>(t5, t7);
    let s7 = _mm256_shuffle_ps::<0xEE>(t5, t7);

    // Stage 3: exchange 128-bit halves
    let o0 = _mm256_permute2f128_ps::<0x20>(s0, s4);
    let o1 = _mm256_permute2f128_ps::<0x20>(s1, s5);
    let o2 = _mm256_permute2f128_ps::<0x20>(s2, s6);
    let o3 = _mm256_permute2f128_ps::<0x20>(s3, s7);
    let o4 = _mm256_permute2f128_ps::<0x31>(s0, s4);
    let o5 = _mm256_permute2f128_ps::<0x31>(s1, s5);
    let o6 = _mm256_permute2f128_ps::<0x31>(s2, s6);
    let o7 = _mm256_permute2f128_ps::<0x31>(s3, s7);

    // Store transposed rows
    let mut store_row = |row: usize, v: __m256| {
        let off = row * dst_stride + y;
        let arr: &mut [f32; 8] = (&mut dst[off..off + 8]).try_into().unwrap();
        unsafe { safe_unaligned_simd::x86_64::_mm256_storeu_ps(arr, v) };
    };
    store_row(x, o0);
    store_row(x + 1, o1);
    store_row(x + 2, o2);
    store_row(x + 3, o3);
    store_row(x + 4, o4);
    store_row(x + 5, o5);
    store_row(x + 6, o6);
    store_row(x + 7, o7);
}

// ---------------------------------------------------------------------------
// ARM NEON (arm_v2 tier) — 4x4 composed into 8x8
// ---------------------------------------------------------------------------

#[cfg(target_arch = "aarch64")]
#[allow(clippy::too_many_arguments)]
#[arcane]
fn transpose_tiled_arm_v2(
    token: Arm64V2Token,
    src: &[f32],
    dst: &mut [f32],
    src_stride: usize,
    dst_stride: usize,
    width: usize,
    height: usize,
    block_size: usize,
) {
    let cropped_h = (height / block_size) * block_size;
    let cropped_w = (width / block_size) * block_size;

    for y_block in (0..cropped_h).step_by(block_size) {
        for x_block in (0..cropped_w).step_by(block_size) {
            let max_y = y_block + block_size;
            let max_x = x_block + block_size;
            for y in (y_block..max_y).step_by(8) {
                for x in (x_block..max_x).step_by(8) {
                    neon_transpose_8x8(token, src, dst, src_stride, dst_stride, x, y);
                }
            }
        }
    }

    transpose_edges_scalar(src, dst, cropped_h, cropped_w, src_stride, dst_stride, width, height);
}

/// NEON 4x4 transpose, composed 4 times for 8x8.
#[cfg(target_arch = "aarch64")]
#[rite]
fn neon_transpose_8x8(
    token: Arm64V2Token,
    src: &[f32],
    dst: &mut [f32],
    src_stride: usize,
    dst_stride: usize,
    x: usize,
    y: usize,
) {
    neon_transpose_4x4(token, src, dst, src_stride, dst_stride, x, y);
    neon_transpose_4x4(token, src, dst, src_stride, dst_stride, x + 4, y);
    neon_transpose_4x4(token, src, dst, src_stride, dst_stride, x, y + 4);
    neon_transpose_4x4(token, src, dst, src_stride, dst_stride, x + 4, y + 4);
}

#[cfg(target_arch = "aarch64")]
#[rite]
fn neon_transpose_4x4(
    _token: Arm64V2Token,
    src: &[f32],
    dst: &mut [f32],
    src_stride: usize,
    dst_stride: usize,
    x: usize,
    y: usize,
) {
    use std::arch::aarch64::{float32x4_t, vld1q_f32, vst1q_f32, vzip1q_f32, vzip2q_f32};

    let load_row =
        |row: usize| -> float32x4_t { vld1q_f32(src[(row * src_stride + x)..].as_ptr()) };

    let r0 = load_row(y);
    let r1 = load_row(y + 1);
    let r2 = load_row(y + 2);
    let r3 = load_row(y + 3);

    let c0 = vzip1q_f32(r0, r2);
    let c1 = vzip2q_f32(r0, r2);
    let c2 = vzip1q_f32(r1, r3);
    let c3 = vzip2q_f32(r1, r3);

    let t0 = vzip1q_f32(c0, c2);
    let t1 = vzip2q_f32(c0, c2);
    let t2 = vzip1q_f32(c1, c3);
    let t3 = vzip2q_f32(c1, c3);

    let mut store_row = |row: usize, v: float32x4_t| {
        vst1q_f32(dst[(row * dst_stride + y)..].as_mut_ptr(), v);
    };
    store_row(x, t0);
    store_row(x + 1, t1);
    store_row(x + 2, t2);
    store_row(x + 3, t3);
}

// ---------------------------------------------------------------------------
// WASM SIMD128 — 4x4 composed into 8x8
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
#[allow(clippy::too_many_arguments)]
#[arcane]
fn transpose_tiled_wasm128(
    token: Wasm128Token,
    src: &[f32],
    dst: &mut [f32],
    src_stride: usize,
    dst_stride: usize,
    width: usize,
    height: usize,
    block_size: usize,
) {
    let cropped_h = (height / block_size) * block_size;
    let cropped_w = (width / block_size) * block_size;

    for y_block in (0..cropped_h).step_by(block_size) {
        for x_block in (0..cropped_w).step_by(block_size) {
            let max_y = y_block + block_size;
            let max_x = x_block + block_size;
            for y in (y_block..max_y).step_by(8) {
                for x in (x_block..max_x).step_by(8) {
                    wasm_transpose_8x8(token, src, dst, src_stride, dst_stride, x, y);
                }
            }
        }
    }

    transpose_edges_scalar(src, dst, cropped_h, cropped_w, src_stride, dst_stride, width, height);
}

#[cfg(target_arch = "wasm32")]
#[rite]
fn wasm_transpose_8x8(
    token: Wasm128Token,
    src: &[f32],
    dst: &mut [f32],
    src_stride: usize,
    dst_stride: usize,
    x: usize,
    y: usize,
) {
    wasm_transpose_4x4(token, src, dst, src_stride, dst_stride, x, y);
    wasm_transpose_4x4(token, src, dst, src_stride, dst_stride, x + 4, y);
    wasm_transpose_4x4(token, src, dst, src_stride, dst_stride, x, y + 4);
    wasm_transpose_4x4(token, src, dst, src_stride, dst_stride, x + 4, y + 4);
}

#[cfg(target_arch = "wasm32")]
#[rite]
fn wasm_transpose_4x4(
    _token: Wasm128Token,
    src: &[f32],
    dst: &mut [f32],
    src_stride: usize,
    dst_stride: usize,
    x: usize,
    y: usize,
) {
    use std::arch::wasm32::{i32x4_shuffle, v128, v128_load, v128_store};

    let load_row =
        |row: usize| -> v128 { v128_load(src[(row * src_stride + x)..].as_ptr() as *const v128) };

    let r0 = load_row(y);
    let r1 = load_row(y + 1);
    let r2 = load_row(y + 2);
    let r3 = load_row(y + 3);

    // First round: interleave
    let s0 = i32x4_shuffle::<0, 4, 1, 5>(r0, r1);
    let s1 = i32x4_shuffle::<2, 6, 3, 7>(r0, r1);
    let s2 = i32x4_shuffle::<0, 4, 1, 5>(r2, r3);
    let s3 = i32x4_shuffle::<2, 6, 3, 7>(r2, r3);

    // Second round: final transpose
    let t0 = i32x4_shuffle::<0, 1, 4, 5>(s0, s2);
    let t1 = i32x4_shuffle::<2, 3, 6, 7>(s0, s2);
    let t2 = i32x4_shuffle::<0, 1, 4, 5>(s1, s3);
    let t3 = i32x4_shuffle::<2, 3, 6, 7>(s1, s3);

    let mut store_row = |row: usize, v: v128| {
        v128_store(dst[(row * dst_stride + y)..].as_mut_ptr() as *mut v128, v);
    };
    store_row(x, t0);
    store_row(x + 1, t1);
    store_row(x + 2, t2);
    store_row(x + 3, t3);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
        let from: Vec<u32> = (0..size * size).map(|_| rand::random::<u32>()).collect();
        let mut to = vec![0; size * size];

        transpose_u32_slices(&from, &mut to, size, size, size, size).unwrap();

        for i in 0..size {
            for j in 0..size {
                assert_eq!(from[i * size + j], to[j * size + i]);
            }
        }
    }

    #[test]
    fn test_transpose_large_rectangular_matrix() {
        let width = 256;
        let height = 128;
        let from: Vec<u32> = (0..width * height).map(|_| rand::random::<u32>()).collect();
        let mut to = vec![0; width * height];

        transpose_u32_slices(&from, &mut to, width, height, width, height).unwrap();

        for i in 0..height {
            for j in 0..width {
                assert_eq!(from[i * width + j], to[j * height + i]);
            }
        }
    }

    #[test]
    fn test_transpose_with_padding() {
        let width = 130;
        let height = 100;
        let src_stride = 132;
        let dst_stride = 104;
        let from: Vec<u32> = (0..src_stride * height).map(|_| rand::random::<u32>()).collect();
        let mut to = vec![0; dst_stride * width];

        transpose_u32_slices(&from, &mut to, src_stride, dst_stride, width, height).unwrap();

        for i in 0..height {
            for j in 0..width {
                assert_eq!(from[i * src_stride + j], to[j * dst_stride + i]);
            }
        }
    }

    #[test]
    fn test_transpose_small_matrices() {
        let sizes = vec![(4, 4), (8, 8), (16, 16), (32, 32), (64, 64)];
        for (width, height) in sizes {
            let from: Vec<u32> = (0..width * height).map(|_| rand::random::<u32>()).collect();
            let mut to = vec![0; width * height];

            transpose_u32_slices(&from, &mut to, width, height, width, height).unwrap();

            for i in 0..height {
                for j in 0..width {
                    assert_eq!(from[i * width + j], to[j * height + i]);
                }
            }
        }
    }

    #[test]
    fn test_transpose_edge_cases() {
        let from = vec![42];
        let mut to = vec![0];
        transpose_u32_slices(&from, &mut to, 1, 1, 1, 1).unwrap();
        assert_eq!(to[0], 42);

        let from = vec![1, 2, 3, 4, 5];
        let mut to = vec![0; 5];
        transpose_u32_slices(&from, &mut to, 5, 1, 5, 1).unwrap();
        assert_eq!(to, from);

        let from = vec![1, 2, 3, 4, 5];
        let mut to = vec![0; 5];
        transpose_u32_slices(&from, &mut to, 1, 5, 1, 5).unwrap();
        assert_eq!(to, from);
    }

    #[test]
    fn test_transpose_error_cases() {
        let from = vec![1, 2, 3, 4];
        let mut to = vec![0; 4];

        assert!(transpose_u32_slices(&from, &mut to, 1, 2, 2, 2).is_err());
        assert!(transpose_u32_slices(&from, &mut to, 2, 1, 2, 2).is_err());
        assert!(transpose_u32_slices(&from, &mut to, 3, 2, 3, 2).is_err());
        assert!(transpose_u32_slices(&from, &mut to, 2, 3, 2, 3).is_err());
    }

    #[test]
    fn test_transpose_all_dimensions() {
        const TEST_DIMS: &[(usize, usize)] = &[
            (1, 1),
            (1, 2),
            (2, 1),
            (2, 2),
            (3, 3),
            (3, 5),
            (5, 3),
            (4, 4),
            (7, 7),
            (8, 8),
            (16, 16),
            (128, 128),
            (7, 9),
            (9, 7),
            (15, 17),
            (17, 15),
            (127, 129),
            (129, 127),
            (13, 17),
            (17, 13),
            (127, 131),
            (1, 128),
            (128, 1),
            (1, 255),
            (255, 1),
            (4, 100),
            (100, 4),
            (255, 255),
            (256, 256),
            (257, 257),
        ];

        for &(w, h) in TEST_DIMS {
            let from: Vec<u32> = (0..(w * h) as u32).collect();
            let stride_src = w;
            let stride_dst = h;
            let mut to = vec![0u32; stride_dst * w];

            transpose_u32_slices(&from, &mut to, stride_src, stride_dst, w, h)
                .unwrap_or_else(|e| panic!("Failed for ({w}, {h}): {e}"));

            for y in 0..h {
                for x in 0..w {
                    assert_eq!(
                        from[y * stride_src + x],
                        to[x * stride_dst + y],
                        "Mismatch at src[{y}][{x}] for dims ({w}, {h})"
                    );
                }
            }
        }
    }

    #[test]
    fn test_transpose_with_stride_padding() {
        let dims: &[(usize, usize, usize, usize)] = &[
            (8, 8, 10, 12),
            (16, 16, 20, 24),
            (13, 17, 15, 20),
            (128, 128, 130, 132),
            (129, 127, 132, 130),
        ];

        for &(w, h, src_stride, dst_stride) in dims {
            let from: Vec<u32> = (0..(src_stride * h) as u32).collect();
            let mut to = vec![0u32; dst_stride * w];

            transpose_u32_slices(&from, &mut to, src_stride, dst_stride, w, h).unwrap_or_else(
                |e| panic!("Failed for ({w}, {h}, strides {src_stride},{dst_stride}): {e}"),
            );

            for y in 0..h {
                for x in 0..w {
                    assert_eq!(
                        from[y * src_stride + x],
                        to[x * dst_stride + y],
                        "Mismatch at src[{y}][{x}] for dims ({w}, {h}), strides ({src_stride}, {dst_stride})"
                    );
                }
            }
        }
    }

    #[test]
    fn test_transpose_roundtrip() {
        let dims: &[(usize, usize)] =
            &[(1, 1), (7, 13), (8, 8), (16, 16), (128, 128), (255, 255), (129, 127)];

        for &(w, h) in dims {
            let original: Vec<u32> = (0..(w * h) as u32).collect();
            let mut transposed = vec![0u32; w * h];
            let mut roundtripped = vec![0u32; w * h];

            transpose_u32_slices(&original, &mut transposed, w, h, w, h).unwrap();
            transpose_u32_slices(&transposed, &mut roundtripped, h, w, h, w).unwrap();

            assert_eq!(original, roundtripped, "Roundtrip failed for dims ({w}, {h})");
        }
    }

    #[test]
    fn test_transpose_preserves_values() {
        let w = 131;
        let h = 97;
        let from: Vec<u32> = (0..(w * h) as u32).collect();
        let mut to = vec![0u32; w * h];

        transpose_u32_slices(&from, &mut to, w, h, w, h).unwrap();

        for y in 0..h {
            for x in 0..w {
                let expected_val = (y * w + x) as u32;
                assert_eq!(from[y * w + x], expected_val);
                assert_eq!(
                    to[x * h + y],
                    expected_val,
                    "Value {expected_val} not at expected position dst[{x}][{y}]"
                );
            }
        }
    }
}
