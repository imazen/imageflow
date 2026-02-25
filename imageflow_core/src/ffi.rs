#![allow(non_camel_case_types)]
//! # Do not use
//! Do not use functions from this module outside of `imageflow_core`
//!
//! **Use the `imageflow_abi` crate when creating bindings**
//!
//! These aren't to be exposed, but rather to connect to `imageflow_c`/`c_components` internals.
//! Overlaps in naming are artifacts from restructuring
//!
use imgref::ImgRef;
use std::slice;

pub use imageflow_types::EdgeKind;

use crate::internal_prelude::works_everywhere::*;
pub use imageflow_types::Filter;
pub use imageflow_types::IoDirection;
use imageflow_types::PixelBuffer;
pub use imageflow_types::PixelFormat;
use mozjpeg_sys::{c_long, c_void};

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
    pub buffer_size: usize,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BitmapCompositingMode {
    ReplaceSelf = 0,
    BlendWithSelf = 1,
    BlendWithMatte = 2,
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct DecoderDownscaleHints {
    pub downscale_if_wider_than: i64,
    pub or_if_taller_than: i64,
    pub downscaled_min_width: i64,
    pub downscaled_min_height: i64,
    pub scale_luma_spatially: bool,
    pub gamma_correct_for_srgb_during_spatial_luma_scaling: bool,
}

#[repr(C)]
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum ColorProfileSource {
    Null = 0,
    ICCP = 1,
    ICCP_GRAY = 2,
    GAMA_CHRM = 3,
    sRGB = 4,
}

/// CIE xyY color value. Layout-compatible with `lcms2::CIExyY` / `lcms2_sys::CIExyY`.
#[repr(C)]
#[derive(Clone, Debug, Copy, PartialEq)]
#[allow(non_snake_case)]
pub struct CIExyY {
    pub x: f64,
    pub y: f64,
    pub Y: f64,
}
impl Default for CIExyY {
    fn default() -> Self {
        CIExyY { x: 0., y: 0., Y: 1. }
    }
}

/// CIE xyY triple (Red/Green/Blue primaries). Layout-compatible with `lcms2::CIExyYTRIPLE`.
#[repr(C)]
#[derive(Clone, Debug, Copy, PartialEq, Default)]
#[allow(non_snake_case)]
pub struct CIExyYTRIPLE {
    pub Red: CIExyY,
    pub Green: CIExyY,
    pub Blue: CIExyY,
}

impl From<CIExyY> for lcms2::CIExyY {
    fn from(v: CIExyY) -> Self {
        lcms2::CIExyY { x: v.x, y: v.y, Y: v.Y }
    }
}
impl From<CIExyYTRIPLE> for lcms2::CIExyYTRIPLE {
    fn from(v: CIExyYTRIPLE) -> Self {
        lcms2::CIExyYTRIPLE { Red: v.Red.into(), Green: v.Green.into(), Blue: v.Blue.into() }
    }
}

#[repr(C)]
#[derive(Clone, Debug, Copy, PartialEq)]
pub struct DecoderColorInfo {
    pub source: ColorProfileSource,
    pub profile_buffer: *const u8,
    pub buffer_length: usize,
    pub white_point: CIExyY,
    pub primaries: CIExyYTRIPLE,
    pub gamma: f64,
}

type WrapJpegErrorHandler = extern "C" fn(
    *mut c_void,
    *mut mozjpeg_sys::jpeg_common_struct,
    *mut mozjpeg_sys::jpeg_error_mgr,
    i32,
    *const u8,
    i32,
) -> bool;

type WrapJpegSourceManagerFunc =
    extern "C" fn(&mut mozjpeg_sys::jpeg_decompress_struct, *mut c_void) -> bool;
type WrapJpegSourceManagerFillBufferFunc =
    extern "C" fn(&mut mozjpeg_sys::jpeg_decompress_struct, *mut c_void, &mut bool) -> bool;
type WrapJpegSourceManagerSkipBytesFunc =
    extern "C" fn(&mut mozjpeg_sys::jpeg_decompress_struct, *mut c_void, c_long) -> bool;

// typedef  bool (*wrap_png_custom_read_function) (png_structp png_ptr, void * custom_state, uint8_t * buffer, size_t bytes_requested, size_t * out_bytes_read);
type WrapPngCustomReadFunction =
    extern "C" fn(*mut c_void, *mut c_void, *mut u8, usize, &mut usize) -> bool;
//typedef void (*wrap_png_error_handler) (png_structp png_ptr, void * custom_state, char * error_message);
type WrapPngErrorHandler = extern "C" fn(*mut c_void, *mut c_void, *const c_char);

//typedef  bool (*wrap_png_custom_write_function) (png_structp png_ptr, void * custom_state, uint8_t * buffer, size_t buffer_length);
type WrapPngCustomWriteFunction = extern "C" fn(*mut c_void, *mut c_void, *mut u8, usize) -> bool;

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
pub enum JpegMarker {
    App0 = 0xE0,
    ICC = 0xE2,
    EXIF = 0xE1,
}

mod long_term {
    use super::*;
    extern "C" {

        pub fn wrap_jpeg_error_state_bytes() -> usize;

        pub fn wrap_jpeg_setup_error_handler(
            codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct,
            error_state: *mut c_void,
            custom_state: *mut c_void,
            error_handler: WrapJpegErrorHandler,
        );

        pub fn wrap_jpeg_set_downscale_type(
            codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct,
            scale_luma_spatially: bool,
            gamma_correct_for_srgb_during_spatial_luma_scaling: bool,
        );
        pub fn wrap_jpeg_set_idct_method_selector(
            codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct,
        );

        pub fn wrap_jpeg_get_custom_state(
            codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct,
        ) -> *mut c_void;
        pub fn wrap_jpeg_create_decompress(
            codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct,
        ) -> bool;

        pub fn wrap_jpeg_setup_source_manager(source_manager: &mut WrapJpegSourceManager);

        pub fn wrap_jpeg_read_header(
            codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct,
        ) -> bool;

        pub fn wrap_jpeg_start_decompress(
            codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct,
        ) -> bool;

        pub fn wrap_jpeg_finish_decompress(
            codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct,
        ) -> bool;

        pub fn wrap_jpeg_read_scan_lines(
            codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct,
            scan_lines: *const *mut u8,
            max_scan_lines: u32,
            scan_lines_read: *mut u32,
        ) -> bool;

        pub fn wrap_jpeg_save_markers(
            codec_info: *mut ::mozjpeg_sys::jpeg_decompress_struct,
            marker_code: i32,
            length_limit: u32,
        ) -> bool;

        pub fn wrap_png_decoder_state_bytes() -> usize;

        pub fn wrap_png_decoder_state_init(
            state: *mut c_void,
            custom_state: *mut c_void,
            error_handler: WrapPngErrorHandler,
            read_function: WrapPngCustomReadFunction,
        ) -> bool;

        pub fn wrap_png_decode_image_info(state: *mut c_void) -> bool;

        pub fn wrap_png_decode_finish(
            state: *mut c_void,
            row_pointers: *mut *mut u8,
            row_count: usize,
            row_bytes: usize,
        ) -> bool;

        pub fn wrap_png_decoder_get_png_ptr(state: *mut c_void) -> *mut c_void;

        pub fn wrap_png_decoder_get_info_ptr(state: *mut c_void) -> *mut c_void;

        pub fn wrap_png_decoder_get_color_info(state: *mut c_void) -> *const DecoderColorInfo;

        pub fn wrap_png_decoder_destroy(state: *mut c_void) -> bool;

        pub fn wrap_png_decoder_get_info(
            state: *mut c_void,
            w: &mut u32,
            h: &mut u32,
            uses_alpha: &mut bool,
            uses_palette: &mut bool,
        ) -> bool;

        pub fn wrap_png_encoder_write_png(
            custom_state: *mut c_void,
            error_handler: WrapPngErrorHandler,
            write_function: WrapPngCustomWriteFunction,
            row_pointers: *const *mut u8,
            w: usize,
            h: usize,
            disable_png_alpha: bool,
            zlib_compression_level: i32,
            pixel_format: PixelFormat,
        ) -> bool;
    }
}

pub use self::long_term::*;
use crate::graphics::bitmaps::{BitmapCompositing, PixelLayout};
use std::os::raw::c_char;

// https://github.com/rust-lang/rust/issues/17417
