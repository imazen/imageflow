//! Bridge between imageflow's `BitmapWindowMut` and zenbitmaps flat buffers.
//!
//! Provides stride-stripping conversion to contiguous pixel data with
//! known `PixelLayout` for checksum computation and local analysis.

use imageflow_core::graphics::bitmaps::BitmapWindowMut;
use imageflow_types::PixelLayout;
use zenbitmaps::PixelLayout as ZenLayout;

/// Extract contiguous pixel bytes from a strided `BitmapWindowMut`.
///
/// Returns `(flat_bytes, zenbitmaps_layout)` for checksum and analysis.
/// Strips per-row stride padding so the output is width * bpp * height bytes.
pub fn to_flat_pixels(window: &mut BitmapWindowMut<u8>) -> (Vec<u8>, ZenLayout) {
    let layout = match window.info().pixel_layout() {
        PixelLayout::BGRA => ZenLayout::Bgra8,
        PixelLayout::BGR => ZenLayout::Bgr8,
        PixelLayout::Gray => ZenLayout::Gray8,
    };

    let w = window.w() as usize;
    let h = window.h() as usize;
    let bpp = layout.bytes_per_pixel();
    let row_bytes = w * bpp;
    let stride = window.info().t_stride() as usize;
    let slice = window.get_slice();

    let mut flat = Vec::with_capacity(row_bytes * h);
    for y in 0..h {
        let row_start = y * stride;
        flat.extend_from_slice(&slice[row_start..row_start + row_bytes]);
    }
    (flat, layout)
}

/// Encode flat bitmap bytes as PAM for lossless local storage.
#[allow(dead_code)]
pub fn encode_as_pam(
    flat: &[u8],
    w: u32,
    h: u32,
    layout: ZenLayout,
) -> Result<Vec<u8>, zenbitmaps::BitmapError> {
    zenbitmaps::encode_pam(flat, w, h, layout, zenbitmaps::Unstoppable)
}

/// Decode a PAM/BMP/farbfeld file to flat bytes + dimensions + layout.
#[allow(dead_code)]
pub fn decode_bitmap(
    data: &[u8],
) -> Result<(Vec<u8>, u32, u32, ZenLayout), zenbitmaps::BitmapError> {
    let output = zenbitmaps::decode(data, zenbitmaps::Unstoppable)?;
    Ok((
        output.pixels().to_vec(),
        output.width,
        output.height,
        output.layout,
    ))
}
