// ---------------------------------------------------------------------------
// Row-level pixel swizzle operations — thin wrappers around the `garb` crate.
//
// All callers pass correctly-sized buffers by construction, so we unwrap
// garb's Result returns. The debug_asserts document the size contracts.
// ---------------------------------------------------------------------------

/// Swap B and R channels in-place for a row of BGRA/RGBA pixels.
pub(crate) fn swap_br_inplace(row: &mut [u8]) {
    debug_assert!(row.len() % 4 == 0, "BGRA row length must be a multiple of 4");
    garb::rgba_to_bgra_inplace(row).unwrap();
}

/// Copy a pixel row, swapping B↔R channels (BGRA↔RGBA). Symmetric operation.
pub(crate) fn copy_swap_br(src: &[u8], dst: &mut [u8]) {
    debug_assert!(src.len() % 4 == 0, "BGRA row length must be a multiple of 4");
    garb::rgba_to_bgra(src, dst).unwrap();
}

/// Set the alpha channel of every BGRA pixel to 255. 4 bytes/pixel, in-place.
pub(crate) fn set_alpha_to_255(row: &mut [u8]) {
    debug_assert!(row.len() % 4 == 0, "BGRA row length must be a multiple of 4");
    garb::fill_alpha(row).unwrap();
}

/// RGB24 → BGRA. 3 src bytes → 4 dst bytes per pixel. Alpha = 255.
pub(crate) fn rgb_to_bgra(src: &[u8], dst: &mut [u8]) {
    garb::rgb_to_bgra(src, dst).unwrap();
}

/// L8 → BGRA. 1 src byte → 4 dst bytes per pixel. R=G=B=gray, A=255.
pub(crate) fn gray_to_bgra(src: &[u8], dst: &mut [u8]) {
    garb::gray_to_bgra(src, dst).unwrap();
}

/// LA → BGRA. 2 src bytes → 4 dst bytes per pixel. R=G=B=gray, A=alpha.
pub(crate) fn gray_alpha_to_bgra(src: &[u8], dst: &mut [u8]) {
    garb::gray_alpha_to_bgra(src, dst).unwrap();
}
