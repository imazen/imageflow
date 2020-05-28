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

#[repr(C)]
pub struct ImageflowJobIo {
    context: *mut ImageflowContext,
    mode: IoMode,// Call nothing, dereference nothing, if this is 0
    pub read_fn: Option<IoReadFn>,// Optional for write modes
    pub write_fn: Option<IoWriteFn>,// Optional for read modes
    position_fn: Option<IoPositionFn>, // Optional for sequential modes
    pub seek_fn: Option<IoSeekFn>, // Optional for sequential modes
    dispose_fn: Option<DestructorFn>,// Optional
    user_data: *mut c_void,
    /// Whoever sets up this structure can populate this value - or set it to -1 - as they
    /// wish. useful for resource estimation.
    optional_file_length: i64
}

#[repr(C)]
#[derive(Debug,Copy,Clone, PartialEq)]
pub enum IoMode {
    None = 0,
    ReadSequential = 1,
    WriteSequential = 2,
    ReadSeekable = 5, // 1 | 4,
    WriteSeekable = 6, // 2 | 4,
    ReadWriteSeekable = 15, // 1 | 2 | 4 | 8
}
impl IoMode{
    pub fn can_read(self) -> bool{
        (self as i32 & IoMode::ReadSequential as i32) > 0
    }
    pub fn can_write(self) -> bool{
        (self as i32 & IoMode::WriteSequential as i32) > 0
    }
}
#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct ImageflowContext {
    pub error: ErrorInfo,
    pub underlying_heap: Heap,
    pub log: ProfilingLog,
    pub object_tracking: ObjTrackingInfo,
    pub codec_set: *mut ContextCodecSet,
}

// end reuse

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct CodecInstance {
    pub io_id: i32,
    pub codec_id: i64,
    pub codec_state: *mut c_void,
    pub io: *mut ImageflowJobIo,
    pub direction: IoDirection,
}






#[repr(C)]
#[derive(Copy,Clone, Debug,  PartialEq)]
pub enum Floatspace {
    Srgb = 0,
    Linear = 1, // gamma = 2,
}

impl From<::imageflow_types::ScalingFloatspace> for Floatspace{
    fn from(s: ::imageflow_types::ScalingFloatspace) -> Self {
        match s {
            s::ScalingFloatspace::Srgb => Floatspace::Srgb,
            s::ScalingFloatspace::Linear => Floatspace::Linear,
        }
    }
}


pub const TESTED_FILTER_OPTIONS: &'static [&'static str] = &["",
                                                             "robidoux",
                                                             "robidouxsharp",
                                                             "ginseng",
                                                             "lanczos",
                                                             "lanczos2",
                                                             "catmullrom",
                                                             "catrom",
                                                             "mitchell",
                                                             "cubicbspline",
                                                             "bspline",
                                                             "cubic_0_1",
                                                             "hermite",
                                                             "triangle",
                                                             "ncubic",
                                                             "ncubicsharp"];

pub const FILTER_OPTIONS: &'static [&'static str] = &["robidouxfast",
                                                      "robidoux",
                                                      "robidouxsharp",
                                                      "ginseng",
                                                      "ginsengsharp",
                                                      "lanczos",
                                                      "lanczossharp",
                                                      "lanczos2",
                                                      "lanczos2sharp",
                                                      "cubicfast",
                                                      "cubic_0_1",
                                                      "cubicsharp",
                                                      "catmullrom",
                                                      "catrom",
                                                      "mitchell",
                                                      "cubicbspline",
                                                      "bspline",
                                                      "hermite",
                                                      "jinc",
                                                      "rawlanczos3",
                                                      "rawlanczos3sharp",
                                                      "rawlanczos2",
                                                      "rawlanczos2sharp",
                                                      "triangle",
                                                      "linear",
                                                      "box",
                                                      "catmullromfast",
                                                      "catmullromfastsharp",
                                                      "fastest",
                                                      "mitchellfast",
                                                      "ncubic",
                                                      "ncubicsharp"];



impl Default for DecoderInfo {
    fn default() -> DecoderInfo {
        DecoderInfo {
            codec_id: -1,
            preferred_mime_type: ptr::null(),
            preferred_extension: ptr::null(),
            frame_count: 0,
            current_frame_index: 0,
            image_width: 0,
            image_height: 0,
            frame_decodes_into: PixelFormat::Bgra32,
        }
    }
}

#[repr(C)]
pub struct DecoderInfo {
    pub codec_id: i64,
    pub preferred_mime_type: *const i8,
    pub preferred_extension: *const i8,
    pub frame_count: usize,
    pub current_frame_index: i64,
    pub image_width: i32,
    pub image_height: i32,
    pub frame_decodes_into: PixelFormat,
}



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

    /// Call normalize_alpha first; this function does not skip unused alpha bytes, only unused whole pixels.
    /// Otherwise Bgr32 may be non-deterministic
    pub fn short_hash_pixels(&self) -> u64{
        use std::hash::Hasher;

        let width_bytes = self.w as usize *  self.fmt.bytes();

        let mut hash = ::twox_hash::XxHash::with_seed(0x8ed1_2ad9_483d_28a0);
        for h in 0isize..(self.h as isize){
            let row_slice = unsafe{ slice::from_raw_parts(self.pixels.offset(h * self.stride as isize), width_bytes) };
            hash.write(row_slice)
        }
        hash.finish()
    }


    pub unsafe fn destroy(bitmap: *mut Self, c: &crate::Context) {
        flow_destroy(c.flow_c(), bitmap as *const libc::c_void, std::ptr::null(), 0);
    }


    pub fn fill_rect(&mut self, c: &crate::Context, x1: u32, y1: u32, x2: u32, y2: u32, color: &s::Color) -> Result<()> {
        let color_srgb_argb = color.clone().to_u32_bgra().unwrap();
        unsafe {
            if !flow_bitmap_bgra_fill_rect(c.flow_c(),
                                           self as *mut BitmapBgra,
                                           x1,
                                           y1,
                                           x2,
                                           y2,
                                           color_srgb_argb) {
                return Err(cerror!(c, "Failed to fill rectangle"))
            }
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


    pub fn create(c: &crate::Context, w: u32, h: u32, format: PixelFormat, color: s::Color) -> Result<*mut BitmapBgra> {
        let flow_pointer = c.flow_c();

        unsafe {
            let ptr =
                crate::ffi::flow_bitmap_bgra_create(flow_pointer, w as i32, h as i32, true, format);
            if ptr.is_null() {
                return Err(cerror!(c, "Failed to allocate {}x{}x{} bitmap ({} bytes). Reduce dimensions or increase RAM.", w, h, format.bytes(), w as usize * h as usize * format.bytes()))
            }
            let color_val = color.clone();
            let color_srgb_argb = color_val.clone().to_u32_bgra().unwrap();
            (*ptr).compositing_mode = crate::ffi::BitmapCompositingMode::ReplaceSelf;
            if color_val != s::Color::Transparent {
                (&mut *ptr).fill_rect(c,
                                      0,
                                      0,
                                      w as u32,
                                      h as u32,
                                      &color)?;
                (*ptr).compositing_mode = crate::ffi::BitmapCompositingMode::BlendWithMatte;
            }

            (*ptr).matte_color = mem::transmute(color_srgb_argb);

            Ok(ptr)
        }
    }

    //bgr24_to_bgra32 -> Set alpha as 0xff
    //bgr24_to_bgrx32 -> skip alpha
    //bgrx32_to_bgr24
    //bgrx32_to_bgra32 -> set alpha as 0xff


    //bgra32_to_bgr24 -> prevent
    //bgra32_to_bgrx32 -> prevent - lossy
//
//    pub fn copy_rect_to(&self, from_x1: u32, from_y1: u32, width: u32, height: u32, canvas: &mut BitmapBgra, x1: u32, y1: u32) -> NResult<()>{
//
//        if canvas.fmt == PixelFormat::Bgr32 && input.fmt == PixelFormat::Bgra32{
//
//        } else if canvas.fmt = PixelFormat.Bgra32 && input.fmt == PixelFormat::Bgr32{
//
//        }
//        if input.fmt != canvas.fmt {
//            return Err(nerror!(::ErrorKind::InvalidNodeConnections, "Canvas pixel format {:?} differs from Input pixel format {:?}.", input.fmt, canvas.fmt));
//        }
//        if input == canvas {
//            return Err(nerror!(::ErrorKind::InvalidNodeConnections, "Canvas and Input are the same bitmap!"));
//        }
//
//        if input.w <= from_x || input.h <= from_y ||
//            input.w < from_x + width ||
//            input.h < from_y + height ||
//            canvas.w < x + width ||
//            canvas.h < y + height {
//            return Err(nerror!(::ErrorKind::InvalidNodeParams, "Invalid coordinates. Canvas is {}x{}, Input is {}x{}, Params provided: {:?}",
//                         canvas.w,
//                         canvas.h,
//                         input.w,
//                         input.h,
//                         p));
//        }
//
//        let bytes_pp = input.fmt.bytes() as u32;
//        if from_x == 0 && x == 0 && width == input.w && width == canvas.w &&
//            input.stride == canvas.stride {
//            //This optimization has the side effect of copying irrelevant data, so we don't want to do it if windowed, only
//            // if padded or permanently cropped.
//            unsafe {
//                let from_offset = input.stride * from_y;
//                let from_ptr = input.pixels.offset(from_offset as isize);
//                let to_offset = canvas.stride * y;
//                let to_ptr = canvas.pixels.offset(to_offset as isize);
//                ptr::copy_nonoverlapping(from_ptr, to_ptr, (input.stride * height) as usize);
//            }
//        } else {
//            for row in 0..height {
//                unsafe {
//                    let from_offset = input.stride * (from_y + row) + bytes_pp * from_x;
//                    let from_ptr = input.pixels.offset(from_offset as isize);
//                    let to_offset = canvas.stride * (y + row) + bytes_pp * x;
//                    let to_ptr = canvas.pixels.offset(to_offset as isize);
//
//                    ptr::copy_nonoverlapping(from_ptr, to_ptr, (width * bytes_pp) as usize);
//                }
//            }
//        }
//    }

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
    w: u32,
    /// buffer height in pixels
    h: u32,
    /// The number of floats per pixel
    channels: u32,
    /// The pixel data
    pixels: *mut c_float,
    /// If true, don't dispose the buffer with the struct
    pixels_borrowed: bool,
    /// The number of floats in the buffer
    float_count: u32,
    /// The number of floats between (0,0) and (0,1)
    float_stride: u32,

    /// If true, alpha has been premultiplied
    alpha_premultiplied: bool,
    /// If true, the alpha channel holds meaningful data
    alpha_meaningful: bool,
}

/** flow context: Heap Manager **/
#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct Heap {
    placeholder: u8, /* FIXME: fill in the rest
                      * flow_heap_calloc_function _calloc;
                      * flow_heap_malloc_function _malloc;
                      * flow_heap_realloc_function _realloc;
                      * flow_heap_free_function _free;
                      * flow_heap_terminate_function _context_terminate;
                      * void * _private_state;
                      * */
}

// struct flow_objtracking_info;
// void flow_context_objtracking_initialize(struct flow_objtracking_info * heap_tracking);
// void flow_context_objtracking_terminate(flow_c * c);

/** flow context: struct `flow_error_info` **/
// struct flow_error_callstack_line {
// const char * file;
// int line;
// const char * function_name;
// };
//
#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct ErrorInfo {
    placeholder: u8, /* FIXME: replace
                      * flow_status_code reason;
                      * struct flow_error_callstack_line callstack[14];
                      * int callstack_count;
                      * int callstack_capacity;
                      * bool locked;
                      * char message[FLOW_ERROR_MESSAGE_SIZE + 1];
                      * */
}


#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct HeapObjectRecord {
    placeholder: u8, /* FIXME: fill in the rest
                      * void * ptr;
                      * size_t bytes;
                      * void * owner;
                      * flow_destructor_function destructor;
                      * bool destructor_called;
                      * const char * allocated_by;
                      * int allocated_by_line;
                      * bool is_owner;
                      * */
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct ObjTrackingInfo {
    pub allocs: HeapObjectRecord,
    pub next_free_slot: size_t,
    pub total_slots: size_t,
    pub bytes_allocated_net: size_t,
    pub bytes_allocated_gross: size_t,
    pub allocations_net: size_t,
    pub allocations_gross: size_t,
    pub bytes_free: size_t,
    pub allocations_net_peak: size_t,
    pub bytes_allocations_net_peak: size_t,
}


#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct DecoderFrameInfo{
    pub w: i32,
    pub h: i32,
    pub format: PixelFormat
}

// TODO: find a way to distinguish between (rust/c) context and IO types here


type DestructorFn = extern fn(*mut ImageflowContext, *mut c_void) -> bool;


/// Returns the number of read into the buffer. Failure to read 'count' bytes could mean EOF or failure. Check context
/// status. Pass NULL to buffer if you want to skip 'count' many bytes, seeking ahead.
type IoReadFn = extern fn(*mut ImageflowContext, *mut ImageflowJobIo, *mut u8, size_t) -> i64;

/// Returns the number of bytes written. If it doesn't equal 'count', there was an error. Check context status
type IoWriteFn = extern fn(*mut ImageflowContext, *mut ImageflowJobIo, *const u8, size_t) -> i64;


/// Returns negative on failure - check context for more detail. Returns the current position in the stream when
/// successful
type IoPositionFn = extern fn(*mut ImageflowContext, *mut ImageflowJobIo) -> i64;

/// Returns true if seek was successful.
type IoSeekFn = extern fn(*mut ImageflowContext, *mut ImageflowJobIo, i64) -> bool;




type CodecInitializeFn = extern fn(*mut ImageflowContext, *mut CodecInstance) -> bool;

type CodecGetInfoFn = extern fn(*mut ImageflowContext, codec_state: *mut c_void, info_out: *mut DecoderInfo) -> bool;

type CodecSwitchFrameFn = extern fn(*mut ImageflowContext, codec_state: *mut c_void, frame_index: size_t) -> bool;

type CodecGetFrameInfoFn = extern fn(*mut ImageflowContext, codec_state: *mut c_void, info_out: *mut DecoderFrameInfo) -> bool;

type CodecSetDownscaleHintsFn = extern fn(*mut ImageflowContext, *mut CodecInstance, *const DecoderDownscaleHints ) -> bool;


type CodecReadFrameFn = extern fn(*mut ImageflowContext,  codec_state: *mut c_void, *mut BitmapBgra) -> bool;


type CodecWriteFrameFn = extern fn(*mut ImageflowContext,
                                   codec_state: *mut libc::c_void,
                                   *mut BitmapBgra,
                                   *const EncoderHints)
                                   -> bool;


type CodecStringifyFn = extern fn(*mut ImageflowContext,
                                   codec_state: *mut libc::c_void,
                                   buffer: *mut libc::c_char, buffer_size: size_t)
                                   -> bool;



#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct CodecDefinition {
    pub codec_id: i64,
    pub initialize: Option<CodecInitializeFn>,
    pub get_info: Option<CodecGetInfoFn>,
    pub get_frame_info: Option<CodecGetFrameInfoFn>,
    pub set_downscale_hints: Option<CodecSetDownscaleHintsFn>,
    pub switch_frame: Option<CodecSwitchFrameFn>,
    pub read_frame: Option<CodecWriteFrameFn>,
    pub write_frame: Option<CodecWriteFrameFn>,

    pub stringify: Option<CodecStringifyFn>,
    pub name: *const u8,
    pub preferred_mime_type: *const u8,
    pub preferred_extension: *const u8,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
struct CodecDefinitionSet {
    pub codecs: *const CodecDefinition,
    pub count: size_t,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct ContextCodecSet {
    codecs: *mut CodecDefinition,
    codecs_count: size_t,
}


#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct ProfilingLog {
    placeholder: u8, // FIXME: replace
}



#[repr(C)]
#[derive(Copy,Clone,Debug,PartialEq)]
pub enum CodecType {
    Null = 0,
    DecodePng = 1,
    EncodePng = 2,
    DecodeJpeg = 3,
    EncodeJpeg = 4,
    DecodeGif = 5,
}


#[repr(C)]
#[derive(Copy,Clone,Debug,PartialEq)]
enum InterpolationFilter {
    RobidouxFast = 1,
    Robidoux = 2,
    RobidouxSharp = 3,
    Ginseng = 4,
    GinsengSharp = 5,
    Lanczos = 6,
    LanczosSharp = 7,
    Lanczos2 = 8,
    Lanczos2Sharp = 9,
    CubicFast = 10,
    Cubic = 11,
    CubicSharp = 12,
    CatmullRom = 13,
    Mitchell = 14,

    CubicBSpline = 15,
    Hermite = 16,
    Jinc = 17,
    RawLanczos3 = 18,
    RawLanczos3Sharp = 19,
    RawLanczos2 = 20,
    RawLanczos2Sharp = 21,
    Triangle = 22,
    Linear = 23,
    Box = 24,
    CatmullRomFast = 25,
    CatmullRomFastSharp = 26,

    Fastest = 27,

    MitchellFast = 28,

    NCubic = 29,

    NCubicSharp = 30,
}


#[repr(C)]
#[derive(Copy,Clone,Debug,PartialEq)]
enum ScaleFlags {
    None = 0,
    UseScale2d = 1,
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
#[derive(Clone,Debug,PartialEq)]
pub struct EncoderHints {
    pub disable_png_alpha: bool,
    pub zlib_compression_level: i32
}



#[repr(C)]
#[derive(Clone,Debug,Copy)]
pub struct Scale2dRenderToCanvas1d {
    // There will need to be consistency checks against the createcanvas node
    //
    // struct flow_interpolation_details * interpolationDetails;
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub sharpen_percent_goal: f32,
    pub interpolation_filter: Filter,
    pub scale_in_colorspace: Floatspace,
}
#[repr(C)]
#[derive(Clone,Debug,Copy)]
pub struct RenderToCanvas1d {
    // There will need to be consistency checks against the createcanvas node
    pub interpolation_filter: Filter,
    pub scale_to_width: i32,
    pub transpose_on_write: bool,
    pub scale_in_colorspace: Floatspace,
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
    pub profile_buffer: *mut u8,
    pub buffer_length: usize,
    pub white_point: ::lcms2::CIExyY,
    pub primaries: ::lcms2::CIExyYTRIPLE,
    pub gamma: f64
}




#[repr(C)]
#[derive(Clone,Debug,Copy, Eq, PartialEq)]
pub struct Rect {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32
}

impl Rect{
    pub fn failure() -> Rect{
        Rect{ x1: -1, y1: -1, x2: -1, y2: -1}
    }
}


type WrapJpegErrorHandler = extern fn(*mut c_void, *mut mozjpeg_sys::jpeg_common_struct, i32, *const u8, i32) -> bool;

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

mod must_replace{}

mod long_term{
    use super::*;
    use ::libc;
    extern "C" {
        pub fn flow_context_create() -> *mut ImageflowContext;
        pub fn flow_context_begin_terminate(context: *mut ImageflowContext) -> bool;
        pub fn flow_context_destroy(context: *mut ImageflowContext);
        pub fn flow_destroy(context: *mut ImageflowContext,
                            pointer: *const libc::c_void,
                            file: *const libc::c_char,
                            line: i32)
                            -> bool;

        pub fn flow_bitmap_bgra_flip_vertical(c: *mut ImageflowContext, bitmap: *mut BitmapBgra) -> bool;
        pub fn flow_bitmap_bgra_flip_horizontal(c: *mut ImageflowContext, bitmap: *mut BitmapBgra) -> bool;

        pub fn flow_bitmap_bgra_create(c: *mut ImageflowContext,
                                       sx: i32,
                                       sy: i32,
                                       zeroed: bool,
                                       format: PixelFormat)
                                       -> *mut BitmapBgra;

        pub fn flow_node_execute_scale2d_render1d(c: *mut ImageflowContext,
                                                  input: *mut BitmapBgra,
                                                  canvas: *mut BitmapBgra,
                                                  info: *const Scale2dRenderToCanvas1d)
                                                  -> bool;

        pub fn flow_bitmap_bgra_fill_rect(c: *mut ImageflowContext,
                                          input: *mut BitmapBgra,
                                          x1: u32,
                                          y1: u32,
                                          x2: u32,
                                          y2: u32,
                                          color_srgb_argb: u32)
                                          -> bool;

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

mod mid_term {
    use super::*;
    use ::libc;

    extern "C" {


        pub fn flow_bitmap_bgra_save_png(c: *mut ImageflowContext,
                                         input: *const BitmapBgra,
                                         path: *const libc::c_char)
                                         -> bool;

        pub fn flow_codecs_jpg_decoder_get_exif(context: *mut ImageflowContext,
                                                codec_instance: *mut CodecInstance)
                                                -> i32;

        pub fn flow_context_has_error(context: *mut ImageflowContext) -> bool;
        pub fn flow_context_clear_error(context: *mut ImageflowContext);
        pub fn flow_context_error_and_stacktrace(context: *mut ImageflowContext,
                                                 buffer: *mut u8,
                                                 buffer_length: libc::size_t,
                                                 full_file_path: bool)
                                                 -> i64;


        pub fn flow_context_print_and_exit_if_err(context: *mut ImageflowContext) -> bool;

        pub fn flow_context_error_reason(context: *mut ImageflowContext) -> i32;

        pub fn flow_context_error_status_included_in_message(context: *mut ImageflowContext) -> bool;

        pub fn flow_context_set_error_get_message_buffer_info(context: *mut ImageflowContext,
                                                     code: i32,
                                                              status_included_in_buffer: bool,
                                                     buffer_out: *mut *mut u8,
                                                     buffer_size_out: *mut libc::size_t)
                                                     -> bool;


        pub fn flow_context_raise_error(context: *mut ImageflowContext,
                                        error_code: i32,
                                        message: *const libc::c_char,
                                        file: *const libc::c_char,
                                        line: i32,
                                        function_name: *const libc::c_char)
                                        -> bool;


        pub fn flow_context_add_to_callstack(context: *mut ImageflowContext,
                                             file: *const libc::c_char,
                                             line: i32,
                                             function_name: *const libc::c_char)
                                             -> bool;




        pub fn flow_context_calloc(context: *mut ImageflowContext,
                                   instance_count: usize,
                                   instance_size: usize,
                                   destructor: *const libc::c_void,
                                   owner: *const libc::c_void,
                                   file: *const libc::c_char,
                                   line: i32)
                                   -> *mut libc::c_void;


        pub fn flow_io_create_for_file(context: *mut ImageflowContext,
                                       mode: IoMode,
                                       filename: *const libc::c_char,
                                       owner: *const libc::c_void)
                                       -> *mut ImageflowJobIo;

        pub fn flow_io_create_from_memory(context: *mut ImageflowContext,
                                          mode: IoMode,
                                          memory: *const u8,
                                          length: libc::size_t,
                                          owner: *const libc::c_void,
                                          destructor_function: *const libc::c_void)
                                          -> *mut ImageflowJobIo;

        pub fn flow_io_create_for_output_buffer(context: *mut ImageflowContext,
                                                owner: *const libc::c_void)
                                                -> *mut ImageflowJobIo;


        // Returns false if the flow_io struct is disposed or not an output buffer type (or for any other error)
        //
        pub fn flow_io_get_output_buffer(context: *mut ImageflowContext,
                                         io: *mut ImageflowJobIo,
                                         result_buffer: *mut *const u8,
                                         result_buffer_length: *mut libc::size_t)
                                         -> bool;



        pub fn flow_codec_initialize(c: *mut ImageflowContext, instance: *mut CodecInstance) -> bool;

        pub fn flow_codec_get_definition(c: *mut ImageflowContext,
                                         codec_id: i64)
                                         -> *mut CodecDefinition;

        pub fn flow_codec_execute_read_frame(c: *mut ImageflowContext,
                                             instance: *mut CodecInstance, color_info: *mut DecoderColorInfo)
                                             -> *mut BitmapBgra;


        pub fn flow_codec_decoder_get_info(c: *mut ImageflowContext,
                                           codec_state: *mut libc::c_void, codec_id: i64, info: *mut DecoderInfo) -> bool;

        pub fn flow_codec_decoder_set_downscale_hints(c: *mut ImageflowContext,
                                                      instance: *mut CodecInstance, hints: *const DecoderDownscaleHints, crash_if_not_implemented: bool) -> bool;


        pub fn flow_bitmap_bgra_populate_histogram(c: *mut ImageflowContext, input: *mut BitmapBgra, histograms: *mut u64, histogram_size_per_channel: u32, histogram_count: u32, pixels_sampled: *mut u64) -> bool;
        pub fn flow_bitmap_bgra_apply_color_matrix(c: *mut ImageflowContext, input: *mut BitmapBgra, row: u32, count: u32, matrix: *const *const f32) -> bool;

        pub fn flow_bitmap_bgra_transpose(c: *mut ImageflowContext, input: *mut BitmapBgra, output: *mut BitmapBgra) -> bool;


    pub fn flow_bitmap_bgra_write_png_with_hints(c: *mut ImageflowContext, input: *mut BitmapBgra, io: *mut ImageflowJobIo, hints: *const EncoderHints) -> bool;

}
}

pub use self::must_replace::*;
pub use self::long_term::*;
pub use self::mid_term::*;
use std::os::raw::c_char;


// https://github.com/rust-lang/rust/issues/17417


#[test]
fn flow_context_create_destroy_works() {
    unsafe {
        let c = flow_context_create();
        assert!(!c.is_null());

        flow_context_destroy(c);
    }
}
