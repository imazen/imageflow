// ---------------------------------------------------------------------------
// Row-level pixel swizzle operations — thin wrappers around the `garb` crate.
//
// All callers pass correctly-sized buffers by construction, so we unwrap
// garb's Result returns. The debug_asserts document the size contracts.
// ---------------------------------------------------------------------------

/// Swap B and R channels in-place for a row of BGRA/RGBA pixels.
pub(crate) fn swap_br_inplace(row: &mut [u8]) {
    debug_assert!(row.len() % 4 == 0, "BGRA row length must be a multiple of 4");
    garb::bytes::rgba_to_bgra_inplace(row).unwrap();
}

/// Copy a pixel row, swapping B↔R channels (BGRA↔RGBA). Symmetric operation.
pub(crate) fn copy_swap_br(src: &[u8], dst: &mut [u8]) {
    debug_assert!(src.len() % 4 == 0, "src length must be a multiple of 4");
    debug_assert_eq!(src.len(), dst.len(), "src and dst must have equal length");
    garb::bytes::rgba_to_bgra(src, dst).unwrap();
}

/// Set the alpha channel of every BGRA pixel to 255. 4 bytes/pixel, in-place.
pub(crate) fn set_alpha_to_255(row: &mut [u8]) {
    debug_assert!(row.len() % 4 == 0, "BGRA row length must be a multiple of 4");
    garb::bytes::fill_alpha_bgra(row).unwrap();
}

/// RGB24 → BGRA. 3 src bytes → 4 dst bytes per pixel. Alpha = 255.
pub(crate) fn rgb_to_bgra(src: &[u8], dst: &mut [u8]) {
    debug_assert!(src.len() % 3 == 0, "RGB src length must be a multiple of 3");
    debug_assert_eq!(dst.len(), src.len() / 3 * 4, "dst must hold 4 bytes per RGB pixel");
    garb::bytes::rgb_to_bgra(src, dst).unwrap();
}

/// L8 → BGRA. 1 src byte → 4 dst bytes per pixel. R=G=B=gray, A=255.
pub(crate) fn gray_to_bgra(src: &[u8], dst: &mut [u8]) {
    debug_assert_eq!(dst.len(), src.len() * 4, "dst must hold 4 bytes per gray pixel");
    garb::bytes::gray_to_bgra(src, dst).unwrap();
}

/// LA → BGRA. 2 src bytes → 4 dst bytes per pixel. R=G=B=gray, A=alpha.
pub(crate) fn gray_alpha_to_bgra(src: &[u8], dst: &mut [u8]) {
    debug_assert!(src.len() % 2 == 0, "gray-alpha src length must be a multiple of 2");
    debug_assert_eq!(dst.len(), src.len() / 2 * 4, "dst must hold 4 bytes per gray-alpha pixel");
    garb::bytes::gray_alpha_to_bgra(src, dst).unwrap();
}
