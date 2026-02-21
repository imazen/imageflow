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
    transpose_u32_slices_with_block_size(
        from,
        to,
        from_stride,
        to_stride,
        width,
        height,
        BLOCK_SIZE,
    )
}

/// Same as `transpose_u32_slices` but with a configurable cache tile size.
/// Block size must be a multiple of 8 and at least 8.
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn transpose_u32_slices_with_block_size(
    from: &[u32],
    to: &mut [u32],
    from_stride: usize,
    to_stride: usize,
    width: usize,
    height: usize,
    block_size: usize,
) -> Result<(), FlowError> {
    debug_assert!(
        block_size >= 8 && block_size.is_multiple_of(8),
        "block_size must be a multiple of 8"
    );

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
        transpose_tiled(src, dst, from_stride, to_stride, width, height, block_size),
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
// Shared tiling logic
// ---------------------------------------------------------------------------
// Two-level tiling: BLOCK_SIZE tiles for cache locality, 8×8 sub-blocks for SIMD.
// The transpose_fn callback handles one 8×8 sub-block. After the tiled region,
// remaining rows/columns are also processed in 8×8 blocks where they fit, with
// element-by-element scalar only for the final < 8 remainder.

/// Process a rectangular region in 8×8 blocks, calling `f` for each block.
/// Only full 8×8 blocks are processed — caller handles the remainder.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn process_8x8_blocks(
    x_start: usize,
    x_end: usize,
    y_start: usize,
    y_end: usize,
    mut f: impl FnMut(usize, usize),
) {
    let x_limit = x_start + ((x_end - x_start) / 8) * 8;
    let y_limit = y_start + ((y_end - y_start) / 8) * 8;
    for y in (y_start..y_limit).step_by(8) {
        for x in (x_start..x_limit).step_by(8) {
            f(x, y);
        }
    }
}

/// Scalar element-by-element transpose for regions smaller than 8×8.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn scalar_remainder(
    src: &[f32],
    dst: &mut [f32],
    src_stride: usize,
    dst_stride: usize,
    x_start: usize,
    x_end: usize,
    y_start: usize,
    y_end: usize,
) {
    for y in y_start..y_end {
        for x in x_start..x_end {
            dst[x * dst_stride + y] = src[y * src_stride + x];
        }
    }
}

/// Two-level tiled transpose with a callback for each 8×8 sub-block.
/// Handles all regions: full tiles, edge tiles, and scalar remainders.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn transpose_tiled_impl(
    src: &[f32],
    dst: &mut [f32],
    src_stride: usize,
    dst_stride: usize,
    width: usize,
    height: usize,
    block_size: usize,
    mut transpose_8x8: impl FnMut(&[f32], &mut [f32], usize, usize, usize, usize),
) {
    let tile_h = (height / block_size) * block_size;
    let tile_w = (width / block_size) * block_size;
    let blk_h = (height / 8) * 8;
    let blk_w = (width / 8) * 8;

    // 1. Full tiles: process in BLOCK_SIZE×BLOCK_SIZE tiles for cache locality
    for y_tile in (0..tile_h).step_by(block_size) {
        for x_tile in (0..tile_w).step_by(block_size) {
            for y in (y_tile..y_tile + block_size).step_by(8) {
                for x in (x_tile..x_tile + block_size).step_by(8) {
                    transpose_8x8(src, dst, src_stride, dst_stride, x, y);
                }
            }
        }
    }

    // 2. Right edge strip (columns tile_w..blk_w, rows 0..tile_h) — 8×8 blocks
    if tile_w < blk_w {
        for y_tile in (0..tile_h).step_by(block_size) {
            process_8x8_blocks(tile_w, blk_w, y_tile, y_tile + block_size, |x, y| {
                transpose_8x8(src, dst, src_stride, dst_stride, x, y);
            });
        }
    }

    // 3. Bottom edge strip (rows tile_h..blk_h, columns 0..blk_w) — 8×8 blocks
    if tile_h < blk_h {
        process_8x8_blocks(0, blk_w, tile_h, blk_h, |x, y| {
            transpose_8x8(src, dst, src_stride, dst_stride, x, y);
        });
    }

    // 4. Scalar remainders (< 8 pixels) — right column strip
    if blk_w < width {
        scalar_remainder(src, dst, src_stride, dst_stride, blk_w, width, 0, blk_h);
    }

    // 5. Scalar remainders — bottom row strip (full width, including corner)
    if blk_h < height {
        scalar_remainder(src, dst, src_stride, dst_stride, 0, width, blk_h, height);
    }
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
    transpose_tiled_impl(
        src,
        dst,
        src_stride,
        dst_stride,
        width,
        height,
        block_size,
        scalar_transpose_8x8,
    );
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
    // Sub-slice to reduce bounds checks: one check per block instead of per element
    let src_base = y * src_stride + x;
    let s = &src[src_base..src_base + 7 * src_stride + 8];
    let dst_base = x * dst_stride + y;
    let d = &mut dst[dst_base..dst_base + 7 * dst_stride + 8];
    for i in 0..8 {
        for j in 0..8 {
            d[j * dst_stride + i] = s[i * src_stride + j];
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
    transpose_tiled_impl(
        src,
        dst,
        src_stride,
        dst_stride,
        width,
        height,
        block_size,
        |src, dst, ss, ds, x, y| avx2_transpose_8x8(token, src, dst, ss, ds, x, y),
    );
}

/// AVX2 8x8 transpose using f32 shuffle intrinsics (Highway-style 3-stage).
/// Works on u32 data reinterpreted as f32 — no arithmetic, only lane permutation.
///
/// Uses sub-slicing to reduce bounds checks: one check for all 8 source rows,
/// one check for all 8 destination rows, instead of per-row checks.
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
    // Sub-slice source: one bounds check covers all 8 rows
    let src_base = y * src_stride + x;
    let s = &src[src_base..src_base + 7 * src_stride + 8];
    let load_row = |i: usize| -> __m256 {
        let off = i * src_stride;
        let arr: &[f32; 8] = s[off..off + 8].try_into().unwrap();
        safe_unaligned_simd::x86_64::_mm256_loadu_ps(arr)
    };

    // Load 8 rows of 8 floats each
    let r0 = load_row(0);
    let r1 = load_row(1);
    let r2 = load_row(2);
    let r3 = load_row(3);
    let r4 = load_row(4);
    let r5 = load_row(5);
    let r6 = load_row(6);
    let r7 = load_row(7);

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

    // Sub-slice destination: one bounds check covers all 8 rows
    let dst_base = x * dst_stride + y;
    let d = &mut dst[dst_base..dst_base + 7 * dst_stride + 8];
    let mut store_row = |i: usize, v: __m256| {
        let off = i * dst_stride;
        let arr: &mut [f32; 8] = (&mut d[off..off + 8]).try_into().unwrap();
        safe_unaligned_simd::x86_64::_mm256_storeu_ps(arr, v);
    };
    store_row(0, o0);
    store_row(1, o1);
    store_row(2, o2);
    store_row(3, o3);
    store_row(4, o4);
    store_row(5, o5);
    store_row(6, o6);
    store_row(7, o7);
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
    transpose_tiled_impl(
        src,
        dst,
        src_stride,
        dst_stride,
        width,
        height,
        block_size,
        |src, dst, ss, ds, x, y| neon_transpose_8x8(token, src, dst, ss, ds, x, y),
    );
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
    use std::arch::aarch64::{vzip1q_f32, vzip2q_f32};

    // Sub-slice: one bounds check for all 4 source rows
    let src_base = y * src_stride + x;
    let s = &src[src_base..src_base + 3 * src_stride + 4];
    let load_row = |i: usize| -> float32x4_t {
        let off = i * src_stride;
        let arr: &[f32; 4] = s[off..off + 4].try_into().unwrap();
        safe_unaligned_simd::aarch64::vld1q_f32(arr)
    };

    let r0 = load_row(0);
    let r1 = load_row(1);
    let r2 = load_row(2);
    let r3 = load_row(3);

    let c0 = vzip1q_f32(r0, r2);
    let c1 = vzip2q_f32(r0, r2);
    let c2 = vzip1q_f32(r1, r3);
    let c3 = vzip2q_f32(r1, r3);

    let t0 = vzip1q_f32(c0, c2);
    let t1 = vzip2q_f32(c0, c2);
    let t2 = vzip1q_f32(c1, c3);
    let t3 = vzip2q_f32(c1, c3);

    // Sub-slice: one bounds check for all 4 destination rows
    let dst_base = x * dst_stride + y;
    let d = &mut dst[dst_base..dst_base + 3 * dst_stride + 4];
    let mut store_row = |i: usize, v: float32x4_t| {
        let off = i * dst_stride;
        let arr: &mut [f32; 4] = (&mut d[off..off + 4]).try_into().unwrap();
        safe_unaligned_simd::aarch64::vst1q_f32(arr, v);
    };
    store_row(0, t0);
    store_row(1, t1);
    store_row(2, t2);
    store_row(3, t3);
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
    transpose_tiled_impl(
        src,
        dst,
        src_stride,
        dst_stride,
        width,
        height,
        block_size,
        |src, dst, ss, ds, x, y| wasm_transpose_8x8(token, src, dst, ss, ds, x, y),
    );
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
    use std::arch::wasm32::i32x4_shuffle;

    // Sub-slice: one bounds check for all 4 source rows
    let src_base = y * src_stride + x;
    let s = &src[src_base..src_base + 3 * src_stride + 4];
    let load_row = |i: usize| -> v128 {
        let off = i * src_stride;
        let arr: &[f32; 4] = s[off..off + 4].try_into().unwrap();
        safe_unaligned_simd::wasm32::v128_load(arr)
    };

    let r0 = load_row(0);
    let r1 = load_row(1);
    let r2 = load_row(2);
    let r3 = load_row(3);

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

    // Sub-slice: one bounds check for all 4 destination rows
    let dst_base = x * dst_stride + y;
    let d = &mut dst[dst_base..dst_base + 3 * dst_stride + 4];
    let mut store_row = |i: usize, v: v128| {
        let off = i * dst_stride;
        let arr: &mut [f32; 4] = (&mut d[off..off + 4]).try_into().unwrap();
        safe_unaligned_simd::wasm32::v128_store(arr, v);
    };
    store_row(0, t0);
    store_row(1, t1);
    store_row(2, t2);
    store_row(3, t3);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use archmage::testing::{for_each_token_permutation, CompileTimePolicy};

    const SENTINEL: u32 = 0xDEAD_BEEF;

    /// Verify transpose correctness with sentinel-filled destination.
    /// Uses unique sequential values offset by 1 (so no cell has value 0 or SENTINEL).
    /// Checks every active cell AND verifies stride padding is untouched.
    fn verify_transpose(w: usize, h: usize, src_stride: usize, dst_stride: usize) {
        // Source: sequential values 1..=w*h in active region, SENTINEL in padding
        let mut from = vec![SENTINEL; src_stride * h];
        for y in 0..h {
            for x in 0..w {
                from[y * src_stride + x] = (y * w + x + 1) as u32;
            }
        }

        // Destination: all SENTINEL — any correctly transposed cell overwrites it
        let mut to = vec![SENTINEL; dst_stride * w];

        transpose_u32_slices(&from, &mut to, src_stride, dst_stride, w, h).unwrap_or_else(|e| {
            panic!("transpose failed for {w}x{h} (strides {src_stride},{dst_stride}): {e}")
        });

        // Verify every active cell was written correctly
        for y in 0..h {
            for x in 0..w {
                let expected = (y * w + x + 1) as u32;
                let actual = to[x * dst_stride + y];
                assert_eq!(
                    actual, expected,
                    "wrong value at dst[{x}][{y}] for {w}x{h} (strides {src_stride},{dst_stride}): \
                     got {actual:#X}, expected {expected:#X}"
                );
            }
        }

        // Verify stride padding in destination is untouched
        for dst_row in 0..w {
            for col in h..dst_stride {
                let actual = to[dst_row * dst_stride + col];
                assert_eq!(
                    actual, SENTINEL,
                    "padding corrupted at dst row {dst_row} col {col} for {w}x{h} \
                     (strides {src_stride},{dst_stride}): got {actual:#X}, expected SENTINEL"
                );
            }
        }
    }

    // Dimensions chosen to systematically hit every code path:
    //
    //   BLOCK_SIZE = 128, SIMD block = 8x8
    //
    //   Category                  | What it exercises
    //   --------------------------+------------------------------------------
    //   < 8 in both dims          | pure scalar edges (no SIMD blocks at all)
    //   < 128 multiples of 8      | SIMD blocks inside one tile, no tile edges
    //   < 128 non-multiples of 8  | SIMD blocks + scalar remainder within tile
    //   exact 128                  | one full tile, no tile edges
    //   128 + remainder < 8       | full tile + pure scalar right/bottom edge
    //   128 + remainder = 8k      | full tile + SIMD edge blocks
    //   128 + remainder = 8k+r    | full tile + SIMD edge blocks + scalar edge
    //   256+                       | multiple tiles
    //   asymmetric                 | right edge only, bottom edge only, or both

    const TEST_DIMS: &[(usize, usize)] = &[
        // --- Tiny (pure scalar, no SIMD blocks) ---
        (1, 1),
        (1, 2),
        (2, 1),
        (3, 5),
        (5, 3),
        (7, 7),
        // --- Sub-128, exact multiples of 8 (SIMD blocks, no remainder) ---
        (8, 8),
        (16, 8),
        (8, 16),
        (16, 16),
        (64, 64),
        (120, 120),
        // --- Sub-128, crossing multiple 8×8 sub-blocks with remainder ---
        (9, 9),     // 1 block + 1 remainder in each dim
        (15, 17),   // 1+7 × 2+1
        (17, 15),   // 2+1 × 1+7
        (31, 17),   // 3+7 × 2+1 — many blocks, large remainder
        (17, 31),   // 2+1 × 3+7
        (25, 13),   // 3+1 × 1+5
        (47, 33),   // 5+7 × 4+1
        (33, 65),   // 4+1 × 8+1
        (65, 33),   // 8+1 × 4+1
        (100, 67),  // 12+4 × 8+3
        (127, 127), // 15+7 × 15+7, one pixel short of tile
        // --- Exact block boundary ---
        (128, 128),
        // --- 128 + small remainder (< 8, pure scalar edge tile) ---
        (129, 128),
        (128, 129),
        (129, 129),
        (131, 133),
        // --- 128 + remainder crossing 8×8 sub-blocks in the edge ---
        (136, 128), // 8-wide edge, exact sub-block
        (128, 136), // 8-tall edge, exact sub-block
        (136, 136),
        (137, 128), // 9-wide edge: 1 sub-block + 1 remainder
        (128, 137), // 9-tall edge: 1 sub-block + 1 remainder
        (137, 139),
        (145, 153), // 17-wide × 25-tall edge: 2+1 × 3+1 sub-blocks in edge
        (153, 145),
        // --- Multiple full tiles ---
        (256, 256),
        (257, 257),
        (255, 255),
        // --- Multiple tiles + non-trivial edge ---
        (263, 271), // 256+7 × 256+15: 2 tiles + crossing sub-blocks in edge
        // --- Highly asymmetric (one dim all-edge) ---
        (1, 128),
        (128, 1),
        (1, 257),
        (257, 1),
        (3, 256),
        (256, 3),
        (7, 129),
        (129, 7),
        (31, 256), // many sub-blocks in narrow dim, full tiles in wide dim
        (256, 31),
    ];

    /// Large dimensions for multi-tier permutation testing.
    /// These exercise multiple 128×128 tiles with varied edge conditions.
    const LARGE_DIMS: &[(usize, usize)] = &[
        // Multiple tiles, exact boundaries
        (512, 512),
        (384, 256),
        (256, 384),
        // Multiple tiles + remainder < 8 (scalar edge only)
        (513, 385),
        (385, 513),
        // Multiple tiles + remainder crossing 8×8 sub-blocks
        (519, 391), // 512+7 × 384+7: 4/3 tiles + 7px edge (no SIMD edge blocks)
        (527, 399), // 512+15 × 384+15: SIMD edge blocks + scalar remainder
        (537, 409), // 512+25 × 384+25: 3 SIMD edge blocks + 1px scalar
        // Primes — nothing aligns
        (521, 397),
        (397, 521),
        // Realistic image sizes
        (1000, 1000),
        (1920, 1080),
        (1080, 1920),
        // 4K
        (3840, 2160),
        (2160, 3840),
        // 8K
        (7680, 4320),
        (4320, 7680),
        // 8K with odd remainder (nothing aligns to 128 or 8)
        (7681, 4321),
        // Highly asymmetric, large
        (7, 1024),
        (1024, 7),
        (31, 513),
        (513, 31),
    ];

    #[test]
    fn test_transpose_all_dims_no_padding() {
        for &(w, h) in TEST_DIMS {
            verify_transpose(w, h, w, h);
        }
    }

    #[test]
    fn test_transpose_all_dims_with_stride_padding() {
        for &(w, h) in TEST_DIMS {
            // Add 1..3 elements of padding per row
            verify_transpose(w, h, w + 1, h + 2);
            verify_transpose(w, h, w + 3, h + 1);
        }
    }

    #[test]
    fn test_transpose_roundtrip() {
        for &(w, h) in TEST_DIMS {
            let original: Vec<u32> = (1..=(w * h) as u32).collect();
            let mut transposed = vec![SENTINEL; w * h];
            let mut roundtripped = vec![SENTINEL; w * h];

            transpose_u32_slices(&original, &mut transposed, w, h, w, h).unwrap();
            transpose_u32_slices(&transposed, &mut roundtripped, h, w, h, w).unwrap();

            assert_eq!(original, roundtripped, "roundtrip failed for {w}x{h}");
        }
    }

    #[test]
    fn test_transpose_error_cases() {
        let from = vec![1, 2, 3, 4];
        let mut to = vec![0; 4];

        assert!(transpose_u32_slices(&from, &mut to, 1, 2, 2, 2).is_err()); // from_stride < width
        assert!(transpose_u32_slices(&from, &mut to, 2, 1, 2, 2).is_err()); // to_stride < height
        assert!(transpose_u32_slices(&from, &mut to, 3, 2, 3, 2).is_err()); // from OOB
        assert!(transpose_u32_slices(&from, &mut to, 2, 3, 2, 3).is_err()); // to OOB
    }

    #[test]
    fn test_transpose_known_small() {
        // 3x3 with known expected output
        let from = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        let mut to = vec![SENTINEL; 9];
        transpose_u32_slices(&from, &mut to, 3, 3, 3, 3).unwrap();
        assert_eq!(to, vec![1, 4, 7, 2, 5, 8, 3, 6, 9]);

        // 3x2 → 2x3
        let from = vec![1, 2, 3, 4, 5, 6];
        let mut to = vec![SENTINEL; 6];
        transpose_u32_slices(&from, &mut to, 3, 2, 3, 2).unwrap();
        assert_eq!(to, vec![1, 4, 2, 5, 3, 6]);

        // With source stride padding
        let from = vec![1, 2, 3, 0xFF, 4, 5, 6, 0xFF, 7, 8, 9, 0xFF];
        let mut to = vec![SENTINEL; 9];
        transpose_u32_slices(&from, &mut to, 4, 3, 3, 3).unwrap();
        assert_eq!(to, vec![1, 4, 7, 2, 5, 8, 3, 6, 9]);
    }

    // =========================================================================
    // Multi-tier permutation tests
    // =========================================================================
    //
    // for_each_token_permutation disables SIMD tokens in every combination,
    // so incant! falls through to different tiers (AVX2, scalar, etc.) each
    // iteration. We verify that ALL tiers produce identical correct results
    // using sentinel-filled destinations and sequential source values.
    //
    // Must run with --test-threads=1 (process-wide mutex for token disabling).

    /// Build source data with sequential values and sentinel padding,
    /// transpose, and verify every cell + padding integrity.
    fn verify_transpose_strict(
        w: usize,
        h: usize,
        src_stride: usize,
        dst_stride: usize,
        label: &str,
    ) {
        let mut from = vec![SENTINEL; src_stride * h];
        for y in 0..h {
            for x in 0..w {
                from[y * src_stride + x] = (y * w + x + 1) as u32;
            }
        }

        let mut to = vec![SENTINEL; dst_stride * w];

        transpose_u32_slices(&from, &mut to, src_stride, dst_stride, w, h).unwrap_or_else(|e| {
            panic!(
                "transpose failed for {w}x{h} (strides {src_stride},{dst_stride}) at {label}: {e}"
            )
        });

        // Verify every active cell
        for y in 0..h {
            for x in 0..w {
                let expected = (y * w + x + 1) as u32;
                let actual = to[x * dst_stride + y];
                if actual != expected {
                    panic!(
                        "wrong value at dst[{x}][{y}] for {w}x{h} (strides {src_stride},{dst_stride}) \
                         at {label}: got {actual:#X}, expected {expected:#X}"
                    );
                }
            }
        }

        // Verify stride padding untouched
        for dst_row in 0..w {
            for col in h..dst_stride {
                let actual = to[dst_row * dst_stride + col];
                if actual != SENTINEL {
                    panic!(
                        "padding corrupted at dst row {dst_row} col {col} for {w}x{h} \
                         (strides {src_stride},{dst_stride}) at {label}: got {actual:#X}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_transpose_all_tiers_small() {
        let report = for_each_token_permutation(CompileTimePolicy::WarnStderr, |perm| {
            for &(w, h) in TEST_DIMS {
                verify_transpose_strict(w, h, w, h, &perm.label);
                // With stride padding
                verify_transpose_strict(w, h, w + 3, h + 1, &perm.label);
            }
        });
        eprintln!("all_tiers_small: {report}");
    }

    #[test]
    fn test_transpose_all_tiers_large() {
        let report = for_each_token_permutation(CompileTimePolicy::WarnStderr, |perm| {
            for &(w, h) in LARGE_DIMS {
                verify_transpose_strict(w, h, w, h, &perm.label);
            }
        });
        eprintln!("all_tiers_large: {report}");
    }

    #[test]
    fn test_transpose_all_tiers_large_with_stride() {
        let report = for_each_token_permutation(CompileTimePolicy::WarnStderr, |perm| {
            for &(w, h) in LARGE_DIMS {
                // Stride padding of 5 elements — odd, not aligned to any block size
                verify_transpose_strict(w, h, w + 5, h + 5, &perm.label);
            }
        });
        eprintln!("all_tiers_large_with_stride: {report}");
    }

    #[test]
    fn test_transpose_all_tiers_roundtrip() {
        let report = for_each_token_permutation(CompileTimePolicy::WarnStderr, |perm| {
            // Roundtrip a selection of large and tricky sizes
            let dims = [(521, 397), (1000, 1000), (263, 271), (31, 513), (513, 31)];
            for (w, h) in dims {
                let original: Vec<u32> = (1..=(w * h) as u32).collect();
                let mut transposed = vec![SENTINEL; w * h];
                let mut roundtripped = vec![SENTINEL; w * h];

                transpose_u32_slices(&original, &mut transposed, w, h, w, h).unwrap();
                transpose_u32_slices(&transposed, &mut roundtripped, h, w, h, w).unwrap();

                assert_eq!(
                    original, roundtripped,
                    "roundtrip failed for {w}x{h} at {}",
                    perm.label
                );
            }
        });
        eprintln!("all_tiers_roundtrip: {report}");
    }
}
