// ---------------------------------------------------------------------------
// Row-level pixel swizzle operations — B↔R swap, format expansion, alpha fill
// ---------------------------------------------------------------------------

use archmage::incant;
use archmage::prelude::*;

// ===========================================================================
// Public API — all functions are pub(crate) with SIMD dispatch
// ===========================================================================

/// Swap bytes 0 and 2 of a u32 — B↔R channel swap for BGRA/RGBA pixels.
#[inline(always)]
fn swap_br_u32(v: u32) -> u32 {
    (v & 0xFF00_FF00) | (v.rotate_left(16) & 0x00FF_00FF)
}

/// Swap B and R channels in-place for a row of BGRA/RGBA pixels.
pub(crate) fn swap_br_inplace(row: &mut [u8]) {
    incant!(swap_br_impl(row), [v3, arm_v2, wasm128, scalar]);
}

/// Copy a pixel row, swapping B↔R channels (BGRA↔RGBA). Symmetric operation.
pub(crate) fn copy_swap_br(src: &[u8], dst: &mut [u8]) {
    incant!(copy_swap_br_impl(src, dst), [v3, arm_v2, wasm128, scalar]);
}

/// Set the alpha channel of every BGRA pixel to 255. 4 bytes/pixel, in-place.
pub(crate) fn set_alpha_to_255(row: &mut [u8]) {
    incant!(set_alpha_impl(row), [v3, arm_v2, wasm128, scalar]);
}

/// RGB24 → BGRA. 3 src bytes → 4 dst bytes per pixel. Alpha = 255.
pub(crate) fn rgb_to_bgra(src: &[u8], dst: &mut [u8]) {
    incant!(rgb_to_bgra_impl(src, dst), [v3, arm_v2, wasm128, scalar]);
}

/// L8 → BGRA. 1 src byte → 4 dst bytes per pixel. R=G=B=gray, A=255.
pub(crate) fn gray_to_bgra(src: &[u8], dst: &mut [u8]) {
    incant!(gray_to_bgra_impl(src, dst), [v3, arm_v2, wasm128, scalar]);
}

/// LA → BGRA. 2 src bytes → 4 dst bytes per pixel. R=G=B=gray, A=alpha.
pub(crate) fn gray_alpha_to_bgra(src: &[u8], dst: &mut [u8]) {
    incant!(gray_alpha_to_bgra_impl(src, dst), [v3, arm_v2, wasm128, scalar]);
}

// ===========================================================================
// Benchmark entry points — #[doc(hidden)] pub for bench harness
// ===========================================================================

#[doc(hidden)]
pub fn bench_swap_br_inplace(row: &mut [u8]) {
    swap_br_inplace(row);
}
#[doc(hidden)]
pub fn bench_copy_swap_br(src: &[u8], dst: &mut [u8]) {
    copy_swap_br(src, dst);
}
#[doc(hidden)]
pub fn bench_swap_br_inplace_scalar(row: &mut [u8]) {
    swap_br_impl_scalar(ScalarToken, row);
}
#[doc(hidden)]
pub fn bench_copy_swap_br_scalar(src: &[u8], dst: &mut [u8]) {
    copy_swap_br_impl_scalar(ScalarToken, src, dst);
}
#[doc(hidden)]
pub fn bench_set_alpha_to_255(row: &mut [u8]) {
    set_alpha_to_255(row);
}
#[doc(hidden)]
pub fn bench_set_alpha_to_255_scalar(row: &mut [u8]) {
    set_alpha_impl_scalar(ScalarToken, row);
}
#[doc(hidden)]
pub fn bench_rgb_to_bgra(src: &[u8], dst: &mut [u8]) {
    rgb_to_bgra(src, dst);
}
#[doc(hidden)]
pub fn bench_rgb_to_bgra_scalar(src: &[u8], dst: &mut [u8]) {
    rgb_to_bgra_impl_scalar(ScalarToken, src, dst);
}
#[doc(hidden)]
pub fn bench_gray_to_bgra(src: &[u8], dst: &mut [u8]) {
    gray_to_bgra(src, dst);
}
#[doc(hidden)]
pub fn bench_gray_to_bgra_scalar(src: &[u8], dst: &mut [u8]) {
    gray_to_bgra_impl_scalar(ScalarToken, src, dst);
}
#[doc(hidden)]
pub fn bench_gray_alpha_to_bgra(src: &[u8], dst: &mut [u8]) {
    gray_alpha_to_bgra(src, dst);
}
#[doc(hidden)]
pub fn bench_gray_alpha_to_bgra_scalar(src: &[u8], dst: &mut [u8]) {
    gray_alpha_to_bgra_impl_scalar(ScalarToken, src, dst);
}

// ===========================================================================
// Scalar fallback implementations
// ===========================================================================

fn swap_br_impl_scalar(_token: ScalarToken, row: &mut [u8]) {
    for px in row.chunks_exact_mut(4) {
        let v = u32::from_ne_bytes([px[0], px[1], px[2], px[3]]);
        let s = swap_br_u32(v);
        px.copy_from_slice(&s.to_ne_bytes());
    }
}

fn copy_swap_br_impl_scalar(_token: ScalarToken, src: &[u8], dst: &mut [u8]) {
    for (s_px, d_px) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        let v = u32::from_ne_bytes([s_px[0], s_px[1], s_px[2], s_px[3]]);
        let s = swap_br_u32(v);
        d_px.copy_from_slice(&s.to_ne_bytes());
    }
}

fn set_alpha_impl_scalar(_token: ScalarToken, row: &mut [u8]) {
    for px in row.chunks_exact_mut(4) {
        px[3] = 0xFF;
    }
}

/// Scalar RGB→BGRA: reverse RGB to BGR, set A=0xFF.
fn rgb_to_bgra_impl_scalar(_token: ScalarToken, src: &[u8], dst: &mut [u8]) {
    for (s, d_px) in src.chunks_exact(3).zip(dst.chunks_exact_mut(4)) {
        d_px[0] = s[2];
        d_px[1] = s[1];
        d_px[2] = s[0];
        d_px[3] = 0xFF;
    }
}

/// Scalar Gray→BGRA: broadcast gray byte to B,G,R, set A=0xFF.
fn gray_to_bgra_impl_scalar(_token: ScalarToken, src: &[u8], dst: &mut [u8]) {
    for (&v, d_px) in src.iter().zip(dst.chunks_exact_mut(4)) {
        d_px[0] = v;
        d_px[1] = v;
        d_px[2] = v;
        d_px[3] = 0xFF;
    }
}

/// Scalar GrayAlpha→BGRA: broadcast gray to B,G,R, copy alpha.
fn gray_alpha_to_bgra_impl_scalar(_token: ScalarToken, src: &[u8], dst: &mut [u8]) {
    for (ga, d_px) in src.chunks_exact(2).zip(dst.chunks_exact_mut(4)) {
        d_px[0] = ga[0];
        d_px[1] = ga[0];
        d_px[2] = ga[0];
        d_px[3] = ga[1];
    }
}

// ===========================================================================
// x86-64 AVX2 (V3 tier) — 8 pixels / 32 bytes per iteration
// ===========================================================================

/// Byte shuffle mask: swap bytes 0↔2 within each 4-byte pixel.
#[cfg(target_arch = "x86_64")]
const BR_SHUF_MASK_AVX: [i8; 32] = [
    2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15, // lower lane
    2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15, // upper lane
];

/// Alpha mask: 0xFF at byte 3 of each pixel (BGRA alpha position).
/// -1i8 = 0xFF as u8.
#[cfg(target_arch = "x86_64")]
const ALPHA_FF_MASK_AVX: [i8; 32] = [
    0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0, -1, // lower lane
    0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0, -1, // upper lane
];

/// Gray expand: replicate each of 8 bytes to B,G,R positions, zero alpha.
/// Input: 8 gray bytes broadcast to all four 64-bit slots via set1_epi64x.
/// Low lane processes grays 0-3, high lane processes grays 4-7.
#[cfg(target_arch = "x86_64")]
const GRAY_EXPAND_MASK_AVX: [i8; 32] = [
    0, 0, 0, -128, 1, 1, 1, -128, 2, 2, 2, -128, 3, 3, 3, -128, // low lane
    4, 4, 4, -128, 5, 5, 5, -128, 6, 6, 6, -128, 7, 7, 7, -128, // high lane
];

/// GrayAlpha expand: replicate gray to B,G,R, keep alpha byte.
/// Input: 8 GA pairs (16 bytes) with the same 16 bytes in both lanes.
#[cfg(target_arch = "x86_64")]
const GA_EXPAND_MASK_AVX: [i8; 32] = [
    0, 0, 0, 1, 2, 2, 2, 3, 4, 4, 4, 5, 6, 6, 6, 7, // low lane: GA 0-3
    8, 8, 8, 9, 10, 10, 10, 11, 12, 12, 12, 13, 14, 14, 14, 15, // high lane: GA 4-7
];

/// RGB→BGRA shuffle: reverse RGB to BGR within each 3-byte pixel, zero alpha.
/// Applied after vpermd aligns 24 input bytes across both lanes.
#[cfg(target_arch = "x86_64")]
const RGB_TO_BGRA_SHUF_AVX: [i8; 32] = [
    2, 1, 0, -128, 5, 4, 3, -128, 8, 7, 6, -128, 11, 10, 9, -128, // low lane
    2, 1, 0, -128, 5, 4, 3, -128, 8, 7, 6, -128, 11, 10, 9, -128, // high lane
];

/// Permute for RGB→BGRA: align 24 bytes of RGB data so each lane has 12
/// valid bytes at positions 0-11.
/// Input dwords: [0..5] contain 24 bytes of RGB (8 pixels).
/// Output: low lane gets dwords [0,1,2,3], high lane gets [3,4,5,6].
/// Only low 3 bits of each index matter.
#[cfg(target_arch = "x86_64")]
const RGB_ALIGN_PERM_AVX: [i8; 32] = [
    0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, // low lane: dwords 0-3
    3, 0, 0, 0, 4, 0, 0, 0, 5, 0, 0, 0, 6, 0, 0, 0, // high lane: dwords 3-6
];

#[cfg(target_arch = "x86_64")]
#[arcane]
fn swap_br_impl_v3(_token: X64V3Token, row: &mut [u8]) {
    let mask = safe_unaligned_simd::x86_64::_mm256_loadu_si256(&BR_SHUF_MASK_AVX);
    let n = row.len();
    let mut i = 0;
    while i + 32 <= n {
        let arr: &[u8; 32] = row[i..i + 32].try_into().unwrap();
        let v = safe_unaligned_simd::x86_64::_mm256_loadu_si256(arr);
        let shuffled = _mm256_shuffle_epi8(v, mask);
        let out: &mut [u8; 32] = (&mut row[i..i + 32]).try_into().unwrap();
        safe_unaligned_simd::x86_64::_mm256_storeu_si256(out, shuffled);
        i += 32;
    }
    for v in bytemuck::cast_slice_mut::<u8, u32>(&mut row[i..]) {
        *v = swap_br_u32(*v);
    }
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn copy_swap_br_impl_v3(_token: X64V3Token, src: &[u8], dst: &mut [u8]) {
    let mask = safe_unaligned_simd::x86_64::_mm256_loadu_si256(&BR_SHUF_MASK_AVX);
    let n = src.len().min(dst.len());
    let mut i = 0;
    while i + 32 <= n {
        let s: &[u8; 32] = src[i..i + 32].try_into().unwrap();
        let v = safe_unaligned_simd::x86_64::_mm256_loadu_si256(s);
        let shuffled = _mm256_shuffle_epi8(v, mask);
        let d: &mut [u8; 32] = (&mut dst[i..i + 32]).try_into().unwrap();
        safe_unaligned_simd::x86_64::_mm256_storeu_si256(d, shuffled);
        i += 32;
    }
    for (s, d) in bytemuck::cast_slice::<u8, u32>(&src[i..])
        .iter()
        .zip(bytemuck::cast_slice_mut::<u8, u32>(&mut dst[i..]))
    {
        *d = swap_br_u32(*s);
    }
}

/// Set alpha to 0xFF: vpor with broadcast alpha mask. 8 pixels per iteration.
#[cfg(target_arch = "x86_64")]
#[arcane]
fn set_alpha_impl_v3(_token: X64V3Token, row: &mut [u8]) {
    let alpha = safe_unaligned_simd::x86_64::_mm256_loadu_si256(&ALPHA_FF_MASK_AVX);
    let n = row.len();
    let mut i = 0;
    while i + 32 <= n {
        let arr: &[u8; 32] = row[i..i + 32].try_into().unwrap();
        let v = safe_unaligned_simd::x86_64::_mm256_loadu_si256(arr);
        let result = _mm256_or_si256(v, alpha);
        let out: &mut [u8; 32] = (&mut row[i..i + 32]).try_into().unwrap();
        safe_unaligned_simd::x86_64::_mm256_storeu_si256(out, result);
        i += 32;
    }
    for v in bytemuck::cast_slice_mut::<u8, u32>(&mut row[i..]) {
        *v |= 0xFF00_0000;
    }
}

/// RGB→BGRA: vpermd to align 24→32 bytes, vpshufb to reverse RGB→BGR, vpor alpha.
/// 8 pixels per iteration (24 bytes in → 32 bytes out).
#[cfg(target_arch = "x86_64")]
#[arcane]
fn rgb_to_bgra_impl_v3(_token: X64V3Token, src: &[u8], dst: &mut [u8]) {
    let perm = safe_unaligned_simd::x86_64::_mm256_loadu_si256(&RGB_ALIGN_PERM_AVX);
    let shuf = safe_unaligned_simd::x86_64::_mm256_loadu_si256(&RGB_TO_BGRA_SHUF_AVX);
    let alpha = safe_unaligned_simd::x86_64::_mm256_loadu_si256(&ALPHA_FF_MASK_AVX);
    let src_len = src.len();
    let dst_len = dst.len();
    let mut i_src = 0;
    let mut i_dst = 0;
    // Load 32 bytes (only 24 consumed), need 32 readable bytes in src
    while i_src + 32 <= src_len && i_dst + 32 <= dst_len {
        let s: &[u8; 32] = src[i_src..i_src + 32].try_into().unwrap();
        let rgb = safe_unaligned_simd::x86_64::_mm256_loadu_si256(s);
        // Align: low lane gets bytes 0-15, high lane gets bytes 12-27
        let aligned = _mm256_permutevar8x32_epi32(rgb, perm);
        // Shuffle: reverse RGB→BGR within each pixel, zero alpha position
        let bgr0 = _mm256_shuffle_epi8(aligned, shuf);
        // Set alpha to 0xFF
        let bgra = _mm256_or_si256(bgr0, alpha);
        let d: &mut [u8; 32] = (&mut dst[i_dst..i_dst + 32]).try_into().unwrap();
        safe_unaligned_simd::x86_64::_mm256_storeu_si256(d, bgra);
        i_src += 24;
        i_dst += 32;
    }
    // Scalar remainder
    let dst32 = bytemuck::cast_slice_mut::<u8, u32>(&mut dst[i_dst..]);
    for (s, d) in src[i_src..].chunks_exact(3).zip(dst32.iter_mut()) {
        *d = s[2] as u32 | ((s[1] as u32) << 8) | ((s[0] as u32) << 16) | 0xFF00_0000;
    }
}

/// Gray→BGRA: broadcast 8 gray bytes via set1_epi64x, vpshufb expand, vpor alpha.
/// 8 pixels per iteration (8 bytes in → 32 bytes out).
#[cfg(target_arch = "x86_64")]
#[arcane]
fn gray_to_bgra_impl_v3(_token: X64V3Token, src: &[u8], dst: &mut [u8]) {
    let expand = safe_unaligned_simd::x86_64::_mm256_loadu_si256(&GRAY_EXPAND_MASK_AVX);
    let alpha = safe_unaligned_simd::x86_64::_mm256_loadu_si256(&ALPHA_FF_MASK_AVX);
    let src_len = src.len();
    let dst_len = dst.len();
    let mut i_src = 0;
    let mut i_dst = 0;
    while i_src + 8 <= src_len && i_dst + 32 <= dst_len {
        let gray8 = u64::from_ne_bytes(src[i_src..i_src + 8].try_into().unwrap());
        // Broadcast 8 gray bytes to all four 64-bit slots
        let grays = _mm256_set1_epi64x(gray8 as i64);
        // Expand: replicate each byte to B,G,R positions
        let expanded = _mm256_shuffle_epi8(grays, expand);
        // Set alpha to 0xFF
        let bgra = _mm256_or_si256(expanded, alpha);
        let d: &mut [u8; 32] = (&mut dst[i_dst..i_dst + 32]).try_into().unwrap();
        safe_unaligned_simd::x86_64::_mm256_storeu_si256(d, bgra);
        i_src += 8;
        i_dst += 32;
    }
    // Scalar remainder
    let dst32 = bytemuck::cast_slice_mut::<u8, u32>(&mut dst[i_dst..]);
    for (&v, d) in src[i_src..].iter().zip(dst32.iter_mut()) {
        let g = v as u32;
        *d = g | (g << 8) | (g << 16) | 0xFF00_0000;
    }
}

/// GrayAlpha→BGRA: load 16 GA bytes into both lanes, vpshufb expand.
/// 8 pixels per iteration (16 bytes in → 32 bytes out).
#[cfg(target_arch = "x86_64")]
#[arcane]
fn gray_alpha_to_bgra_impl_v3(_token: X64V3Token, src: &[u8], dst: &mut [u8]) {
    let expand = safe_unaligned_simd::x86_64::_mm256_loadu_si256(&GA_EXPAND_MASK_AVX);
    let src_len = src.len();
    let dst_len = dst.len();
    let mut i_src = 0;
    let mut i_dst = 0;
    while i_src + 16 <= src_len && i_dst + 32 <= dst_len {
        // Load 8 GA pairs (16 bytes) and put the same data in both lanes
        let lo = u64::from_ne_bytes(src[i_src..i_src + 8].try_into().unwrap());
        let hi = u64::from_ne_bytes(src[i_src + 8..i_src + 16].try_into().unwrap());
        let gas = _mm256_set_epi64x(hi as i64, lo as i64, hi as i64, lo as i64);
        // Expand: gray→B,G,R, alpha stays
        let bgra = _mm256_shuffle_epi8(gas, expand);
        let d: &mut [u8; 32] = (&mut dst[i_dst..i_dst + 32]).try_into().unwrap();
        safe_unaligned_simd::x86_64::_mm256_storeu_si256(d, bgra);
        i_src += 16;
        i_dst += 32;
    }
    // Scalar remainder
    let dst32 = bytemuck::cast_slice_mut::<u8, u32>(&mut dst[i_dst..]);
    for (ga, d) in src[i_src..].chunks_exact(2).zip(dst32.iter_mut()) {
        let g = ga[0] as u32;
        *d = g | (g << 8) | (g << 16) | ((ga[1] as u32) << 24);
    }
}

// ===========================================================================
// ARM NEON (arm_v2 tier) — vqtbl1q_u8 + vorrq_u8
// ===========================================================================

#[cfg(target_arch = "aarch64")]
#[arcane]
fn swap_br_impl_arm_v2(_token: Arm64V2Token, row: &mut [u8]) {
    use std::arch::aarch64::vqtbl1q_u8;

    let mask_bytes: [u8; 16] = [2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15];
    let mask = safe_unaligned_simd::aarch64::vld1q_u8(&mask_bytes);
    let n = row.len();
    let mut i = 0;
    while i + 16 <= n {
        let arr: &[u8; 16] = row[i..i + 16].try_into().unwrap();
        let v = safe_unaligned_simd::aarch64::vld1q_u8(arr);
        let shuffled = vqtbl1q_u8(v, mask);
        let out: &mut [u8; 16] = (&mut row[i..i + 16]).try_into().unwrap();
        safe_unaligned_simd::aarch64::vst1q_u8(out, shuffled);
        i += 16;
    }
    for v in bytemuck::cast_slice_mut::<u8, u32>(&mut row[i..]) {
        *v = swap_br_u32(*v);
    }
}

#[cfg(target_arch = "aarch64")]
#[arcane]
fn copy_swap_br_impl_arm_v2(_token: Arm64V2Token, src: &[u8], dst: &mut [u8]) {
    use std::arch::aarch64::vqtbl1q_u8;

    let mask_bytes: [u8; 16] = [2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15];
    let mask = safe_unaligned_simd::aarch64::vld1q_u8(&mask_bytes);
    let n = src.len().min(dst.len());
    let mut i = 0;
    while i + 16 <= n {
        let s: &[u8; 16] = src[i..i + 16].try_into().unwrap();
        let v = safe_unaligned_simd::aarch64::vld1q_u8(s);
        let shuffled = vqtbl1q_u8(v, mask);
        let d: &mut [u8; 16] = (&mut dst[i..i + 16]).try_into().unwrap();
        safe_unaligned_simd::aarch64::vst1q_u8(d, shuffled);
        i += 16;
    }
    for (s, d) in bytemuck::cast_slice::<u8, u32>(&src[i..])
        .iter()
        .zip(bytemuck::cast_slice_mut::<u8, u32>(&mut dst[i..]))
    {
        *d = swap_br_u32(*s);
    }
}

#[cfg(target_arch = "aarch64")]
#[arcane]
fn set_alpha_impl_arm_v2(_token: Arm64V2Token, row: &mut [u8]) {
    use std::arch::aarch64::vorrq_u8;

    let alpha_bytes: [u8; 16] = [0, 0, 0, 0xFF, 0, 0, 0, 0xFF, 0, 0, 0, 0xFF, 0, 0, 0, 0xFF];
    let alpha = safe_unaligned_simd::aarch64::vld1q_u8(&alpha_bytes);
    let n = row.len();
    let mut i = 0;
    while i + 16 <= n {
        let arr: &[u8; 16] = row[i..i + 16].try_into().unwrap();
        let v = safe_unaligned_simd::aarch64::vld1q_u8(arr);
        let result = vorrq_u8(v, alpha);
        let out: &mut [u8; 16] = (&mut row[i..i + 16]).try_into().unwrap();
        safe_unaligned_simd::aarch64::vst1q_u8(out, result);
        i += 16;
    }
    for v in bytemuck::cast_slice_mut::<u8, u32>(&mut row[i..]) {
        *v |= 0xFF00_0000;
    }
}

/// RGB→BGRA NEON: 4 pixels per iteration (12 bytes in → 16 bytes out).
/// Loads 16 bytes (12 valid + 4 over-read), shuffles, ORs alpha.
#[cfg(target_arch = "aarch64")]
#[arcane]
fn rgb_to_bgra_impl_arm_v2(_token: Arm64V2Token, src: &[u8], dst: &mut [u8]) {
    use std::arch::aarch64::{vorrq_u8, vqtbl1q_u8};

    let shuf_bytes: [u8; 16] = [2, 1, 0, 0x80, 5, 4, 3, 0x80, 8, 7, 6, 0x80, 11, 10, 9, 0x80];
    let shuf = safe_unaligned_simd::aarch64::vld1q_u8(&shuf_bytes);
    let alpha_bytes: [u8; 16] = [0, 0, 0, 0xFF, 0, 0, 0, 0xFF, 0, 0, 0, 0xFF, 0, 0, 0, 0xFF];
    let alpha = safe_unaligned_simd::aarch64::vld1q_u8(&alpha_bytes);
    let src_len = src.len();
    let dst_len = dst.len();
    let mut i_src = 0;
    let mut i_dst = 0;
    // Need 16 readable bytes from src for the load (12 consumed + 4 over-read)
    while i_src + 16 <= src_len && i_dst + 16 <= dst_len {
        let s: &[u8; 16] = src[i_src..i_src + 16].try_into().unwrap();
        let v = safe_unaligned_simd::aarch64::vld1q_u8(s);
        let bgr0 = vqtbl1q_u8(v, shuf);
        let bgra = vorrq_u8(bgr0, alpha);
        let d: &mut [u8; 16] = (&mut dst[i_dst..i_dst + 16]).try_into().unwrap();
        safe_unaligned_simd::aarch64::vst1q_u8(d, bgra);
        i_src += 12;
        i_dst += 16;
    }
    let dst32 = bytemuck::cast_slice_mut::<u8, u32>(&mut dst[i_dst..]);
    for (s, d) in src[i_src..].chunks_exact(3).zip(dst32.iter_mut()) {
        *d = s[2] as u32 | ((s[1] as u32) << 8) | ((s[0] as u32) << 16) | 0xFF00_0000;
    }
}

/// Gray→BGRA NEON: 16 pixels per iteration (16 bytes in → 64 bytes out).
/// Single 16-byte load, four shuffle+OR+store passes.
#[cfg(target_arch = "aarch64")]
#[arcane]
fn gray_to_bgra_impl_arm_v2(_token: Arm64V2Token, src: &[u8], dst: &mut [u8]) {
    use std::arch::aarch64::{vorrq_u8, vqtbl1q_u8};

    let masks: [[u8; 16]; 4] = [
        [0, 0, 0, 0x80, 1, 1, 1, 0x80, 2, 2, 2, 0x80, 3, 3, 3, 0x80],
        [4, 4, 4, 0x80, 5, 5, 5, 0x80, 6, 6, 6, 0x80, 7, 7, 7, 0x80],
        [8, 8, 8, 0x80, 9, 9, 9, 0x80, 10, 10, 10, 0x80, 11, 11, 11, 0x80],
        [12, 12, 12, 0x80, 13, 13, 13, 0x80, 14, 14, 14, 0x80, 15, 15, 15, 0x80],
    ];
    let m0 = safe_unaligned_simd::aarch64::vld1q_u8(&masks[0]);
    let m1 = safe_unaligned_simd::aarch64::vld1q_u8(&masks[1]);
    let m2 = safe_unaligned_simd::aarch64::vld1q_u8(&masks[2]);
    let m3 = safe_unaligned_simd::aarch64::vld1q_u8(&masks[3]);
    let alpha_bytes: [u8; 16] = [0, 0, 0, 0xFF, 0, 0, 0, 0xFF, 0, 0, 0, 0xFF, 0, 0, 0, 0xFF];
    let alpha = safe_unaligned_simd::aarch64::vld1q_u8(&alpha_bytes);
    let src_len = src.len();
    let dst_len = dst.len();
    let mut i_src = 0;
    let mut i_dst = 0;
    while i_src + 16 <= src_len && i_dst + 64 <= dst_len {
        let s: &[u8; 16] = src[i_src..i_src + 16].try_into().unwrap();
        let grays = safe_unaligned_simd::aarch64::vld1q_u8(s);
        for (j, m) in [m0, m1, m2, m3].iter().enumerate() {
            let expanded = vqtbl1q_u8(grays, *m);
            let bgra = vorrq_u8(expanded, alpha);
            let d: &mut [u8; 16] =
                (&mut dst[i_dst + j * 16..i_dst + (j + 1) * 16]).try_into().unwrap();
            safe_unaligned_simd::aarch64::vst1q_u8(d, bgra);
        }
        i_src += 16;
        i_dst += 64;
    }
    let dst32 = bytemuck::cast_slice_mut::<u8, u32>(&mut dst[i_dst..]);
    for (&v, d) in src[i_src..].iter().zip(dst32.iter_mut()) {
        let g = v as u32;
        *d = g | (g << 8) | (g << 16) | 0xFF00_0000;
    }
}

/// GrayAlpha→BGRA NEON: 8 pixels per iteration (16 bytes in → 32 bytes out).
#[cfg(target_arch = "aarch64")]
#[arcane]
fn gray_alpha_to_bgra_impl_arm_v2(_token: Arm64V2Token, src: &[u8], dst: &mut [u8]) {
    use std::arch::aarch64::vqtbl1q_u8;

    let masks: [[u8; 16]; 2] = [
        [0, 0, 0, 1, 2, 2, 2, 3, 4, 4, 4, 5, 6, 6, 6, 7],
        [8, 8, 8, 9, 10, 10, 10, 11, 12, 12, 12, 13, 14, 14, 14, 15],
    ];
    let m0 = safe_unaligned_simd::aarch64::vld1q_u8(&masks[0]);
    let m1 = safe_unaligned_simd::aarch64::vld1q_u8(&masks[1]);
    let src_len = src.len();
    let dst_len = dst.len();
    let mut i_src = 0;
    let mut i_dst = 0;
    while i_src + 16 <= src_len && i_dst + 32 <= dst_len {
        let s: &[u8; 16] = src[i_src..i_src + 16].try_into().unwrap();
        let gas = safe_unaligned_simd::aarch64::vld1q_u8(s);
        let r0 = vqtbl1q_u8(gas, m0);
        let d0: &mut [u8; 16] = (&mut dst[i_dst..i_dst + 16]).try_into().unwrap();
        safe_unaligned_simd::aarch64::vst1q_u8(d0, r0);
        let r1 = vqtbl1q_u8(gas, m1);
        let d1: &mut [u8; 16] = (&mut dst[i_dst + 16..i_dst + 32]).try_into().unwrap();
        safe_unaligned_simd::aarch64::vst1q_u8(d1, r1);
        i_src += 16;
        i_dst += 32;
    }
    let dst32 = bytemuck::cast_slice_mut::<u8, u32>(&mut dst[i_dst..]);
    for (ga, d) in src[i_src..].chunks_exact(2).zip(dst32.iter_mut()) {
        let g = ga[0] as u32;
        *d = g | (g << 8) | (g << 16) | ((ga[1] as u32) << 24);
    }
}

// ===========================================================================
// WASM SIMD128 — i8x16_swizzle + v128_or
// ===========================================================================

#[cfg(target_arch = "wasm32")]
#[arcane]
fn swap_br_impl_wasm128(_token: Wasm128Token, row: &mut [u8]) {
    use std::arch::wasm32::{i8x16, i8x16_swizzle};

    let mask = i8x16(2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15);
    let n = row.len();
    let mut i = 0;
    while i + 16 <= n {
        let arr: &[u8; 16] = row[i..i + 16].try_into().unwrap();
        let v = safe_unaligned_simd::wasm32::v128_load(arr);
        let shuffled = i8x16_swizzle(v, mask);
        let out: &mut [u8; 16] = (&mut row[i..i + 16]).try_into().unwrap();
        safe_unaligned_simd::wasm32::v128_store(out, shuffled);
        i += 16;
    }
    for v in bytemuck::cast_slice_mut::<u8, u32>(&mut row[i..]) {
        *v = swap_br_u32(*v);
    }
}

#[cfg(target_arch = "wasm32")]
#[arcane]
fn copy_swap_br_impl_wasm128(_token: Wasm128Token, src: &[u8], dst: &mut [u8]) {
    use std::arch::wasm32::{i8x16, i8x16_swizzle};

    let mask = i8x16(2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15);
    let n = src.len().min(dst.len());
    let mut i = 0;
    while i + 16 <= n {
        let s: &[u8; 16] = src[i..i + 16].try_into().unwrap();
        let v = safe_unaligned_simd::wasm32::v128_load(s);
        let shuffled = i8x16_swizzle(v, mask);
        let d: &mut [u8; 16] = (&mut dst[i..i + 16]).try_into().unwrap();
        safe_unaligned_simd::wasm32::v128_store(d, shuffled);
        i += 16;
    }
    for (s, d) in bytemuck::cast_slice::<u8, u32>(&src[i..])
        .iter()
        .zip(bytemuck::cast_slice_mut::<u8, u32>(&mut dst[i..]))
    {
        *d = swap_br_u32(*s);
    }
}

#[cfg(target_arch = "wasm32")]
#[arcane]
fn set_alpha_impl_wasm128(_token: Wasm128Token, row: &mut [u8]) {
    use std::arch::wasm32::{u32x4_splat, v128_or};

    let alpha = u32x4_splat(0xFF000000);
    let n = row.len();
    let mut i = 0;
    while i + 16 <= n {
        let arr: &[u8; 16] = row[i..i + 16].try_into().unwrap();
        let v = safe_unaligned_simd::wasm32::v128_load(arr);
        let result = v128_or(v, alpha);
        let out: &mut [u8; 16] = (&mut row[i..i + 16]).try_into().unwrap();
        safe_unaligned_simd::wasm32::v128_store(out, result);
        i += 16;
    }
    for v in bytemuck::cast_slice_mut::<u8, u32>(&mut row[i..]) {
        *v |= 0xFF00_0000;
    }
}

#[cfg(target_arch = "wasm32")]
#[arcane]
fn rgb_to_bgra_impl_wasm128(_token: Wasm128Token, src: &[u8], dst: &mut [u8]) {
    use std::arch::wasm32::{i8x16, i8x16_swizzle, u32x4_splat, v128_or};

    let shuf = i8x16(2, 1, 0, -128, 5, 4, 3, -128, 8, 7, 6, -128, 11, 10, 9, -128);
    let alpha = u32x4_splat(0xFF000000);
    let src_len = src.len();
    let dst_len = dst.len();
    let mut i_src = 0;
    let mut i_dst = 0;
    while i_src + 16 <= src_len && i_dst + 16 <= dst_len {
        let s: &[u8; 16] = src[i_src..i_src + 16].try_into().unwrap();
        let v = safe_unaligned_simd::wasm32::v128_load(s);
        let bgr0 = i8x16_swizzle(v, shuf);
        let bgra = v128_or(bgr0, alpha);
        let d: &mut [u8; 16] = (&mut dst[i_dst..i_dst + 16]).try_into().unwrap();
        safe_unaligned_simd::wasm32::v128_store(d, bgra);
        i_src += 12;
        i_dst += 16;
    }
    let dst32 = bytemuck::cast_slice_mut::<u8, u32>(&mut dst[i_dst..]);
    for (s, d) in src[i_src..].chunks_exact(3).zip(dst32.iter_mut()) {
        *d = s[2] as u32 | ((s[1] as u32) << 8) | ((s[0] as u32) << 16) | 0xFF00_0000;
    }
}

#[cfg(target_arch = "wasm32")]
#[arcane]
fn gray_to_bgra_impl_wasm128(_token: Wasm128Token, src: &[u8], dst: &mut [u8]) {
    use std::arch::wasm32::{i8x16, i8x16_swizzle, u32x4_splat, v128_or};

    let m0 = i8x16(0, 0, 0, -128, 1, 1, 1, -128, 2, 2, 2, -128, 3, 3, 3, -128);
    let m1 = i8x16(4, 4, 4, -128, 5, 5, 5, -128, 6, 6, 6, -128, 7, 7, 7, -128);
    let m2 = i8x16(8, 8, 8, -128, 9, 9, 9, -128, 10, 10, 10, -128, 11, 11, 11, -128);
    let m3 = i8x16(12, 12, 12, -128, 13, 13, 13, -128, 14, 14, 14, -128, 15, 15, 15, -128);
    let alpha = u32x4_splat(0xFF000000);
    let src_len = src.len();
    let dst_len = dst.len();
    let mut i_src = 0;
    let mut i_dst = 0;
    while i_src + 16 <= src_len && i_dst + 64 <= dst_len {
        let s: &[u8; 16] = src[i_src..i_src + 16].try_into().unwrap();
        let grays = safe_unaligned_simd::wasm32::v128_load(s);
        for (j, m) in [m0, m1, m2, m3].iter().enumerate() {
            let expanded = i8x16_swizzle(grays, *m);
            let bgra = v128_or(expanded, alpha);
            let d: &mut [u8; 16] =
                (&mut dst[i_dst + j * 16..i_dst + (j + 1) * 16]).try_into().unwrap();
            safe_unaligned_simd::wasm32::v128_store(d, bgra);
        }
        i_src += 16;
        i_dst += 64;
    }
    let dst32 = bytemuck::cast_slice_mut::<u8, u32>(&mut dst[i_dst..]);
    for (&v, d) in src[i_src..].iter().zip(dst32.iter_mut()) {
        let g = v as u32;
        *d = g | (g << 8) | (g << 16) | 0xFF00_0000;
    }
}

#[cfg(target_arch = "wasm32")]
#[arcane]
fn gray_alpha_to_bgra_impl_wasm128(_token: Wasm128Token, src: &[u8], dst: &mut [u8]) {
    use std::arch::wasm32::{i8x16, i8x16_swizzle};

    let m0 = i8x16(0, 0, 0, 1, 2, 2, 2, 3, 4, 4, 4, 5, 6, 6, 6, 7);
    let m1 = i8x16(8, 8, 8, 9, 10, 10, 10, 11, 12, 12, 12, 13, 14, 14, 14, 15);
    let src_len = src.len();
    let dst_len = dst.len();
    let mut i_src = 0;
    let mut i_dst = 0;
    while i_src + 16 <= src_len && i_dst + 32 <= dst_len {
        let s: &[u8; 16] = src[i_src..i_src + 16].try_into().unwrap();
        let gas = safe_unaligned_simd::wasm32::v128_load(s);
        let r0 = i8x16_swizzle(gas, m0);
        let d0: &mut [u8; 16] = (&mut dst[i_dst..i_dst + 16]).try_into().unwrap();
        safe_unaligned_simd::wasm32::v128_store(d0, r0);
        let r1 = i8x16_swizzle(gas, m1);
        let d1: &mut [u8; 16] = (&mut dst[i_dst + 16..i_dst + 32]).try_into().unwrap();
        safe_unaligned_simd::wasm32::v128_store(d1, r1);
        i_src += 16;
        i_dst += 32;
    }
    let dst32 = bytemuck::cast_slice_mut::<u8, u32>(&mut dst[i_dst..]);
    for (ga, d) in src[i_src..].chunks_exact(2).zip(dst32.iter_mut()) {
        let g = ga[0] as u32;
        *d = g | (g << 8) | (g << 16) | ((ga[1] as u32) << 24);
    }
}
