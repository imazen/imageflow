#![forbid(unsafe_code)]

/// Pixel format for frame metadata.
///
/// Describes channel depth for dimension/memory estimation.
/// Actual pixel format negotiation happens at execution time via
/// the codec layer (zenpixels `PixelDescriptor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// 4 channels × 1 byte (BGRA/RGBA).
    Rgba8,
    /// 3 channels × 1 byte (BGR/RGB).
    Rgb8,
    /// 1 channel × 1 byte (grayscale).
    Gray8,
    /// 4 channels × 4 bytes (linear f32 premultiplied).
    Rgbaf32,
    /// 4 channels × 2 bytes (f16).
    Rgbaf16,
}

impl PixelFormat {
    /// Bytes per pixel.
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Rgba8 => 4,
            PixelFormat::Rgb8 => 3,
            PixelFormat::Gray8 => 1,
            PixelFormat::Rgbaf32 => 16,
            PixelFormat::Rgbaf16 => 8,
        }
    }

    /// Number of channels.
    pub fn channels(self) -> usize {
        match self {
            PixelFormat::Rgba8 | PixelFormat::Rgbaf32 | PixelFormat::Rgbaf16 => 4,
            PixelFormat::Rgb8 => 3,
            PixelFormat::Gray8 => 1,
        }
    }
}

/// Frame metadata — dimensions, format, and orientation.
///
/// Tracks stored (on-disk) pixel dimensions. Visual dimensions may differ
/// when EXIF orientation swaps width/height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameInfo {
    /// Width in pixels (stored, not visual).
    pub width: u32,
    /// Height in pixels (stored, not visual).
    pub height: u32,
    /// Pixel format.
    pub format: PixelFormat,
    /// EXIF orientation (1–8). 1 = no transform.
    pub orientation: u8,
}

impl FrameInfo {
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        Self { width, height, format, orientation: 1 }
    }

    pub fn with_orientation(width: u32, height: u32, format: PixelFormat, orientation: u8) -> Self {
        Self { width, height, format, orientation }
    }

    /// Total size in bytes (width × height × bpp).
    pub fn size_bytes(&self) -> usize {
        (self.width as usize) * (self.height as usize) * self.format.bytes_per_pixel()
    }

    /// Visual dimensions (after orientation applied).
    pub fn visual_dimensions(&self) -> (u32, u32) {
        if self.orientation >= 5 && self.orientation <= 8 {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        }
    }
}

/// Frame dimension estimate for lazy computation.
///
/// During graph estimation, dimensions propagate from sources to sinks.
/// Nodes that can't estimate without decoder info return `Impossible`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FrameEstimate {
    /// Not yet estimated.
    #[default]
    None,
    /// Exact dimensions known.
    Some(FrameInfo),
    /// Upper bound (actual may be smaller after content-dependent ops).
    UpperBound(FrameInfo),
    /// Cannot estimate without runtime info (needs decoder).
    Impossible,
    /// Graph must be re-estimated after this node expands.
    InvalidateGraph,
}

impl FrameEstimate {
    /// Get exact frame info.
    pub fn as_some(&self) -> Option<FrameInfo> {
        match self {
            FrameEstimate::Some(info) => Some(*info),
            _ => None,
        }
    }

    /// Get frame info (exact or upper bound).
    pub fn as_info(&self) -> Option<FrameInfo> {
        match self {
            FrameEstimate::Some(info) | FrameEstimate::UpperBound(info) => Some(*info),
            _ => None,
        }
    }

    /// Has estimation completed?
    pub fn is_complete(&self) -> bool {
        matches!(self, FrameEstimate::Some(_) | FrameEstimate::UpperBound(_))
    }
}
