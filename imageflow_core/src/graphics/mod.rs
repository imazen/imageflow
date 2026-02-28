pub(crate) mod aligned_buffer;
pub mod bitmaps;
pub(crate) mod blend;
pub mod color;
pub(crate) mod color_matrix;
pub(crate) mod copy_rect;
pub mod flip;
pub(crate) mod histogram;
pub mod lut;
pub(crate) mod math;
pub(crate) mod rounded_corners;
pub mod scaling;
pub(crate) mod swizzle;
pub mod transpose;
pub mod weights;
pub mod whitespace;

#[doc(hidden)]
mod prelude {
    pub(crate) use crate::errors::ErrorKind;
    pub(crate) use crate::ffi::BitmapCompositingMode;
    pub(crate) use crate::FlowError;

    pub(crate) use crate::graphics::aligned_buffer::AlignedBuffer;
    pub(crate) use crate::graphics::bitmaps::{Bitmap, BitmapWindowMut, ColorSpace, PixelLayout};
    pub(crate) use crate::graphics::color::{
        flow_colorcontext_floatspace_to_srgb, uchar_clamp_ff, ColorContext, WorkingFloatspace,
    };
    pub(crate) use crate::graphics::weights::{PixelRowWeights, PixelWeightIndexes};

    pub(crate) use imageflow_types::PixelFormat;

    pub(crate) fn flow_pixel_format_bytes_per_pixel(format: crate::ffi::PixelFormat) -> u32 {
        format.bytes() as u32
    }
    pub(crate) fn flow_pixel_format_channels(format: crate::ffi::PixelFormat) -> u32 {
        match format {
            PixelFormat::Bgra32 => 4,
            PixelFormat::Bgr32 => 3,
            PixelFormat::Bgr24 => 3,
            PixelFormat::Gray8 => 1,
        }
    }
}
