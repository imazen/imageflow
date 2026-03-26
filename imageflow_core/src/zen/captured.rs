//! Captured bitmap data from CaptureBitmapKey nodes in the zen pipeline.

/// Pixel data captured by materializing the pipeline at a CaptureBitmapKey node.
///
/// Stores raw pixel bytes with format metadata. This is the zen pipeline's
/// equivalent of v2's BitmapKey → BitmapWindowMut chain.
pub struct CapturedBitmap {
    pub width: u32,
    pub height: u32,
    /// Raw pixel bytes, contiguous rows.
    pub pixels: Vec<u8>,
    /// Pixel format descriptor (from zenpixels).
    pub format: zenpipe::PixelFormat,
}

impl CapturedBitmap {
    pub fn bytes_per_pixel(&self) -> usize {
        self.format.bytes_per_pixel()
    }

    pub fn stride(&self) -> usize {
        self.width as usize * self.bytes_per_pixel()
    }
}
