#![allow(non_camel_case_types)]
//! # Do not use
//! Do not use functions from this module outside of `imageflow_core`
//!
//! **Use the `imageflow_abi` crate when creating bindings**
//!
//! These aren't to be exposed, but rather to connect to `imageflow_c`/`c_components` internals.
//! Overlaps in naming are artifacts from restructuring
//!
use std::slice;
use imgref::ImgRef;

pub use imageflow_types::EdgeKind;

pub use imageflow_types::Filter;
pub use imageflow_types::IoDirection;
pub use imageflow_types::PixelFormat;
use imageflow_types::PixelBuffer;
use crate::internal_prelude::works_everywhere::*;
use mozjpeg_sys::{c_void, c_long};

// These are reused in the external ABI, but only as opaque pointers
///
/// `ImageflowJsonResponse` contains a buffer and buffer length (in bytes), as well as a status code
/// The status code can be used to avoid actual parsing of the response in some cases.
/// For example, you may not care about parsing an error message if you're hacking around -
/// Or, you may not care about success details if you were sending a command that doesn't imply
/// a result.
///
/// The contents of the buffer MAY NOT include any null characters.
/// The contents of the buffer MUST be a valid UTF-8 byte sequence.
/// The contents of the buffer MUST be valid JSON per RFC 7159.
///
/// The schema of the JSON response is not globally defined; consult the API methods in use.
///
/// Use `imageflow_json_response_destroy` to free (it will otherwise remain on the heap and
/// tracking list until the context is destroyed).
///
/// Use `imageflow_context_read_response` to access
#[repr(C)]
pub struct ImageflowJsonResponse {
    pub status_code: i64,
    pub buffer_utf8_no_nulls: *const u8,
    pub buffer_size: libc::size_t,
}


/// Not for external use
#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct BitmapBgra {
    /// bitmap width in pixels
    pub w: u32,
    /// bitmap height in pixels
    pub h: u32,
    /// byte length of each row (may include any amount of padding)
    pub stride: u32,
    /// pointer to pixel 0,0; should be of length > h * stride
    pub pixels: *mut u8,

    pub fmt: PixelFormat,
    /// When using compositing mode blend_with_matte, this color will be used. We should probably define this as
    /// always being sRGBA, 4 bytes.
    pub matte_color: [u8; 4],

    pub compositing_mode: BitmapCompositingMode,
}


impl BitmapBgra {
    #[inline]
    pub fn width(&self) -> usize {
        self.w as usize
    }

    #[inline]
    pub fn stride(&self) -> usize {
        self.stride as usize
    }

    #[inline]
    pub fn height(&self) -> usize {
        self.h as usize
    }

    #[inline]
    pub unsafe fn pixels_slice(&self) -> Option<&[u8]> {
        if self.pixels.is_null() {
            None
        } else {
            let stride = self.stride();
            let width_bytes = self.width() * self.fmt.bytes();
            // Subimages in bottom right corner may not have pixels left for full stride
            Some(slice::from_raw_parts(self.pixels, stride * self.height() + width_bytes - stride))
        }
    }

    /// Unsafe, because it depends on the raw pixels pointer being alive
    pub unsafe fn pixels_buffer(&self) -> Option<PixelBuffer> {
        if self.pixels.is_null() {
            return None;
        }
        let stride_px = self.stride() / self.fmt.bytes();
        let buffer_size_px = stride_px * self.height() + self.width() - stride_px;
        Some(match self.fmt {
            PixelFormat::Bgra32 => {
                let buf = slice::from_raw_parts(self.pixels as *const _, buffer_size_px);
                PixelBuffer::Bgra32(ImgRef::new_stride(buf, self.width(), self.height(), stride_px))
            },
            PixelFormat::Bgr32 => {
                let buf = slice::from_raw_parts(self.pixels as *const _, buffer_size_px);
                PixelBuffer::Bgr32(ImgRef::new_stride(buf, self.width(), self.height(), stride_px))
            },
            PixelFormat::Bgr24 => {
                let buf = slice::from_raw_parts(self.pixels as *const _, buffer_size_px);
                PixelBuffer::Bgr24(ImgRef::new_stride(buf, self.width(), self.height(), stride_px))
            },
            PixelFormat::Gray8 => {
                let buf = slice::from_raw_parts(self.pixels as *const _, buffer_size_px);
                PixelBuffer::Gray8(ImgRef::new_stride(buf, self.width(), self.height(), stride_px))
            },
        })
    }

    pub unsafe fn pixels_slice_mut(&mut self) -> Option<&mut [u8]>{
        if self.pixels.is_null() {
            None
        }else{
            Some(slice::from_raw_parts_mut(self.pixels, (self.stride * self.h) as usize))
        }
    }

    pub fn frame_info(&self) -> crate::flow::definitions::FrameInfo {
        crate::flow::definitions::FrameInfo {
            w: self.w as i32,
            h: self.h as i32,
            fmt: self.fmt
        }
    }
    /// If the format is Bgr32, set each alpha byte to 0xff
    pub fn normalize_alpha(&mut self) -> Result<()>{
        if self.fmt == PixelFormat::Bgr32 {
            let width_bytes = self.w as usize * self.fmt.bytes();
            for h in 0isize..self.h as isize{
                let s = unsafe { slice::from_raw_parts_mut(self.pixels.offset(h * self.stride as isize), width_bytes) };
                for pix in s.chunks_mut(4) {
                    pix[3] = 0xff;
                }
            }
        }
        Ok(())
    }

    pub fn is_pointer_null(&self) -> bool{
        self.pixels.is_null()
    }

    /// Call normalize_alpha first; this function does not skip unused alpha bytes, only unused whole pixels.
    /// Otherwise Bgr32 may be non-deterministic
    pub unsafe fn short_hash_pixels(&self) -> u64{
        use std::hash::Hasher;

        if self.is_pointer_null(){
            panic!("BitmapBgra::short_hash_pixels called on invalid pointer");
        }

        let width_bytes = self.w as usize *  self.fmt.bytes();

        let mut hash = ::twox_hash::XxHash64::with_seed(0x8ed1_2ad9_483d_28a0);
        for h in 0isize..(self.h as isize){
            let row_slice = unsafe{ slice::from_raw_parts(self.pixels.offset(h * self.stride as isize), width_bytes) };
            hash.write(row_slice)
        }
        hash.finish()
    }


    pub fn fill_rect(&mut self, x1: u32, y1: u32, x2: u32, y2: u32, color: &s::Color) -> Result<()> {
        let color_srgb_argb = color.clone().to_u32_bgra().unwrap();
        unsafe {
            crate::graphics::fill::flow_bitmap_bgra_fill_rect(self, x1,y1,x2,y2, color_srgb_argb)
                .map_err(|e| e.at(here!()))?;

        }
        Ok(())
    }

    pub fn get_row_pointers(&self) -> Result<Vec<*mut u8>>{
        let mut vec = Vec::with_capacity(self.h as usize);
        for y in 0..self.h{
            vec.push(unsafe{ self.pixels.offset(self.stride as isize * y as isize) } )
        }
        Ok(vec)
    }

}


#[repr(C)]
#[derive(Copy,Clone,Debug,PartialEq)]
pub enum BitmapCompositingMode {
    ReplaceSelf = 0,
    BlendWithSelf = 1,
    BlendWithMatte = 2,
}


/// floating-point bitmap, typically linear RGBA, premultiplied
#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct BitmapFloat {
    /// buffer width in pixels
    pub w: u32,
    /// buffer height in pixels
    pub h: u32,
    /// The number of floats per pixel
    pub channels: u32,
    /// The pixel data
    pub pixels: *mut c_float,
    /// If true, don't dispose the buffer with the struct
    pub pixels_borrowed: bool,
    /// The number of floats in the buffer
    pub float_count: u32,
    /// The number of floats between (0,0) and (0,1)
    pub float_stride: u32,

    /// If true, alpha has been premultiplied
    pub alpha_premultiplied: bool,
    /// If true, the alpha channel holds meaningful data
    pub alpha_meaningful: bool,
}





#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct DecoderDownscaleHints {
    pub downscale_if_wider_than: i64,
    pub or_if_taller_than: i64,
    pub downscaled_min_width: i64,
    pub downscaled_min_height: i64,
    pub scale_luma_spatially: bool,
    pub gamma_correct_for_srgb_during_spatial_luma_scaling: bool,
}


#[repr(C)]
#[derive(Clone,Debug,Copy,  PartialEq)]
pub enum ColorProfileSource {
    Null = 0,
    ICCP = 1,
    ICCP_GRAY = 2,
    GAMA_CHRM = 3,
    sRGB = 4,

}

#[repr(C)]
#[derive(Clone,Debug,Copy, PartialEq)]
pub struct DecoderColorInfo {
    pub source: ColorProfileSource,
    pub profile_buffer: *const u8,
    pub buffer_length: usize,
    pub white_point: ::lcms2::CIExyY,
    pub primaries: ::lcms2::CIExyYTRIPLE,
    pub gamma: f64
}



type WrapJpegErrorHandler = extern fn(*mut c_void, *mut mozjpeg_sys::jpeg_common_struct, *mut mozjpeg_sys::jpeg_error_mgr, i32, *const u8, i32) -> bool;

type WrapJpegSourceManagerFunc = extern fn(&mut mozjpeg_sys::jpeg_decompress_struct, *mut c_void) -> bool;
type WrapJpegSourceManagerFillBufferFunc = extern fn(&mut mozjpeg_sys::jpeg_decompress_struct, *mut c_void, &mut bool) -> bool;
type WrapJpegSourceManagerSkipBytesFunc = extern fn(&mut mozjpeg_sys::jpeg_decompress_struct, *mut c_void, c_long) -> bool;


// typedef  bool (*wrap_png_custom_read_function) (png_structp png_ptr, void * custom_state, uint8_t * buffer, size_t bytes_requested, size_t * out_bytes_read);
type WrapPngCustomReadFunction = extern fn(*mut c_void, *mut c_void, *mut u8, usize, &mut usize) -> bool;
//typedef void (*wrap_png_error_handler) (png_structp png_ptr, void * custom_state, char * error_message);
type WrapPngErrorHandler = extern fn(*mut c_void, *mut c_void, *const c_char);

//typedef  bool (*wrap_png_custom_write_function) (png_structp png_ptr, void * custom_state, uint8_t * buffer, size_t buffer_length);
type WrapPngCustomWriteFunction = extern fn(*mut c_void, *mut c_void, *mut u8, usize) -> bool;

#[repr(C)]
pub struct WrapJpegSourceManager {
    pub shared_mgr: mozjpeg_sys::jpeg_source_mgr,
    pub init_source_fn: Option<WrapJpegSourceManagerFunc>,
    pub term_source_fn: Option<WrapJpegSourceManagerFunc>,
    pub fill_input_buffer_fn: Option<WrapJpegSourceManagerFillBufferFunc>,
    pub skip_input_data_fn: Option<WrapJpegSourceManagerSkipBytesFunc>,
    pub custom_state: *mut c_void,
}

#[repr(C)]
pub enum JpegMarker{
    App0 = 0xE0,
    ICC = 0xE2,
    EXIF = 0xE1
}


mod long_term{
    use super::*;
    use ::libc;
    extern "C" {

        pub fn wrap_jpeg_error_state_bytes() -> usize;

        pub fn wrap_jpeg_setup_error_handler(codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct,
                                             error_state: *mut c_void,
                                             custom_state: *mut c_void,
                                             error_handler: WrapJpegErrorHandler);

        pub fn wrap_jpeg_set_downscale_type(codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct,
                                            scale_luma_spatially: bool,
                                            gamma_correct_for_srgb_during_spatial_luma_scaling: bool);
        pub fn wrap_jpeg_set_idct_method_selector(codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct);

        pub fn wrap_jpeg_get_custom_state(codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct) -> *mut c_void;
        pub fn wrap_jpeg_create_decompress(codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct) -> bool;

        pub fn wrap_jpeg_setup_source_manager(source_manager: &mut WrapJpegSourceManager);

        pub fn wrap_jpeg_read_header(codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct) -> bool;

        pub fn wrap_jpeg_start_decompress(codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct) -> bool;

        pub fn wrap_jpeg_finish_decompress(codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct) -> bool;

        pub fn wrap_jpeg_read_scan_lines(codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct,
                                         scan_lines: *const *mut u8, max_scan_lines: u32,
                                         scan_lines_read: *mut u32) -> bool;

        pub fn wrap_jpeg_save_markers(codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct, marker_code: i32, length_limit: u32) -> bool;

        pub fn wrap_png_decoder_state_bytes() -> usize;

        pub fn wrap_png_decoder_state_init(state: *mut c_void, custom_state: *mut c_void,
                                           error_handler: WrapPngErrorHandler, read_function: WrapPngCustomReadFunction) -> bool;

        pub fn wrap_png_decode_image_info(state: *mut c_void) -> bool;

        pub fn wrap_png_decode_finish(state: *mut c_void, row_pointers: *mut *mut u8, row_count: usize, row_bytes: usize) -> bool;

        pub fn wrap_png_decoder_get_png_ptr(state: *mut c_void) -> *mut c_void;

        pub fn wrap_png_decoder_get_info_ptr(state: *mut c_void) -> *mut c_void;

        pub fn wrap_png_decoder_get_color_info(state: *mut c_void) -> *const DecoderColorInfo;

        pub fn wrap_png_decoder_destroy(state: *mut c_void) -> bool;

        pub fn wrap_png_decoder_get_info(state: *mut c_void, w: &mut u32, h: &mut u32, uses_alpha: &mut bool) -> bool;

        pub fn wrap_png_encoder_write_png(custom_state: *mut c_void,
                                          error_handler: WrapPngErrorHandler,
                                          write_function: WrapPngCustomWriteFunction,
                                          row_pointers: *const *mut u8,
                                          w: usize,
                                          h: usize,
                                          disable_png_alpha: bool,
                                          zlib_compression_level: i32,
                                          pixel_format: PixelFormat) -> bool;
    }
}

pub use self::long_term::*;
use std::os::raw::c_char;
use crate::graphics::bitmaps::{PixelLayout, BitmapCompositing};


// https://github.com/rust-lang/rust/issues/17417
