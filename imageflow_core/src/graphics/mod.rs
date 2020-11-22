pub mod whitespace;
pub(crate) mod copy_rect;
pub mod bitmaps;
pub(crate) mod  aligned_buffer;
pub(crate) mod math;
pub mod color;
pub mod weights;
pub mod scaling;
pub mod flip;
pub mod transpose;
pub(crate) mod histogram;
pub(crate) mod color_matrix;
pub(crate) mod luv;
pub(crate) mod convolve;
pub(crate) mod fill;
pub(crate) mod blend;

//pub mod faces;

#[doc(hidden)]
mod prelude{
    pub(crate) use crate::ffi::{BitmapFloat,BitmapBgra,BitmapCompositingMode};
    pub(crate) use crate::FlowError;
    pub(crate) use crate::errors::ErrorKind;

    #[cfg(target_arch = "x86")]
    pub(crate) use std::arch::x86::{
        __m128, _mm_add_ps, _mm_loadu_ps, _mm_movehl_ps, _mm_movelh_ps, _mm_mul_ps, _mm_set1_ps,
        _mm_setr_ps, _mm_setzero_ps, _mm_storeu_ps, _mm_unpackhi_ps, _mm_unpacklo_ps,
    };
    #[cfg(target_arch = "x86_64")]
    pub(crate) use std::arch::x86_64::{
        __m128, _mm_add_ps, _mm_loadu_ps, _mm_movehl_ps, _mm_movelh_ps, _mm_mul_ps, _mm_set1_ps,
        _mm_setr_ps, _mm_setzero_ps, _mm_storeu_ps, _mm_unpackhi_ps, _mm_unpacklo_ps,
    };
    pub(crate) use crate::graphics::bitmaps::{BitmapWindowMut, PixelLayout, Bitmap, ColorSpace};
    pub(crate) use crate::graphics::color::{WorkingFloatspace, ColorContext, flow_colorcontext_floatspace_to_srgb, uchar_clamp_ff};
    pub(crate) use crate::graphics::weights::{PixelRowWeights, PixelWeightIndexes};
    pub(crate) use crate::graphics::aligned_buffer::AlignedBuffer;

    pub(crate) use crate::ffi::BitmapFloat as flow_bitmap_float;
    pub(crate) use crate::ffi::BitmapBgra as flow_bitmap_bgra;
    pub(crate) use imageflow_types::PixelFormat;

    pub(crate) fn flow_pixel_format_bytes_per_pixel(format: crate::ffi::PixelFormat) -> u32
    {
        format.bytes() as u32
    }
    pub(crate) fn flow_pixel_format_channels(format: crate::ffi::PixelFormat) -> u32{
        match format{
            PixelFormat::Bgra32 => 4,
            PixelFormat::Bgr32 => 3,
            PixelFormat::Bgr24 => 3,
            PixelFormat::Gray8 => 1
        }
    }
}