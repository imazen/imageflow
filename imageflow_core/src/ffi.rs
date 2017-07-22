//! # Do not use
//! Do not use functions from this module outside of `imageflow_core`
//!
//! **Use the `imageflow_abi` crate when creating bindings**
//!
//! These aren't to be exposed, but rather to connect to `imageflow_c`/`c_components` internals.
//! Overlaps in naming are artifacts from restructuring
//!

pub use imageflow_types::EdgeKind;

pub use imageflow_types::Filter;
pub use imageflow_types::IoDirection;
pub use imageflow_types::PixelFormat;
use ::internal_prelude::works_everywhere::*;


// These are reused in the external ABI, but only as opaque pointers
///
/// `ImaeflowJsonResponse` contains a buffer and buffer length (in bytes), as well as a status code
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
    pub buffer_utf8_no_nulls: *const libc::uint8_t,
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


#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct ImageflowContext {
    pub error: ErrorInfo,
    pub underlying_heap: Heap,
    pub log: ProfilingLog,
    pub colorspace: ColorspaceInfo,
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
#[derive(Copy,Clone, Debug)]
pub enum Floatspace {
    Srgb = 0,
    Linear = 1, // gamma = 2,
}

// #[repr(C)]
// #[derive(Copy,Clone, Debug)]
// pub enum BitmapCompositingMode {
// replace_with_self = 0,
// blend_with_self = 1,
// blend_with_matte = 2,
// }
//


#[repr(C)]
#[derive(Copy,Clone,Debug, PartialEq)]
pub enum FlowStatusCode {
    NoError = 0,
    OutOfMemory = 10,
    IOError = 20,
    InvalidInternalState = 30,
    NotImplemented = 40,
    InvalidArgument = 50,
    NullArgument = 51,
    InvalidDimensions = 52,
    UnsupportedPixelFormat = 53,
    ItemDoesNotExist = 54,

    ImageDecodingFailed = 60,
    ImageEncodingFailed = 61,
    GraphInvalid = 70,
    GraphIsCyclic = 71,
    InvalidInputsToNode = 72,
    MaximumGraphPassesExceeded = 73,
    OtherError = 1024,
    // FIXME: FirstUserDefinedError is 1025 in C but it conflicts with __LastLibraryError
    // ___LastLibraryError,
    FirstUserDefinedError = 1025,
    LastUserDefinedError = 2147483647,
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
    // FIXME: replace with a vec or slice
    /// pointer to pixel 0,0; should be of length > h * stride
    pub pixels: *mut u8,
    /// If true, we don't dispose of *pixels when we dispose the struct
    pub borrowed_pixels: bool,
    /// If false, we can even ignore the alpha channel on 4bpp
    pub alpha_meaningful: bool,
    /// If false, we can edit pixels without affecting the stride
    pub pixels_readonly: bool,
    /// If false, we can change the stride of the image
    pub stride_readonly: bool,
    /// If true, we can reuse the allocated memory for other purposes
    pub can_reuse_space: bool,
    pub fmt: PixelFormat,
    /// When using compositing mode blend_with_matte, this color will be used. We should probably define this as
    /// always being sRGBA, 4 bytes.
    pub matte_color: [u8; 4],

    pub compositing_mode: BitmapCompositingMode,
}


impl BitmapBgra{
    pub unsafe fn pixels_slice_mut(&mut self) -> Option<&mut [u8]>{
        if self.pixels.is_null() {
            None
        }else{
            Some(::std::slice::from_raw_parts_mut(self.pixels, (self.stride * self.h) as usize))
        }
    }
}
// imageflow_core::ffi::FlowBitmapBgra{
// alpha_meaningful: false,
// can_reuse_space: false,
// compositing_mode: ffi::BitmapCompositingMode::blend_with_self,
// matte_color: [0,0,0,0],
// pixels_readonly: false,
// stride_readonly: false,
// pixels: ptr::null_mut(),
// stride: 0,
// w: 0,
// h: 0,
// borrowed_pixels: false,
// fmt: ffi::PixelFormat::bgra32
// };



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
    w: uint32_t,
    /// buffer height in pixels
    h: uint32_t,
    /// The number of floats per pixel
    channels: uint32_t,
    /// The pixel data
    pixels: *mut c_float,
    /// If true, don't dispose the buffer with the struct
    pixels_borrowed: bool,
    /// The number of floats in the buffer
    float_count: uint32_t,
    /// The number of floats betwen (0,0) and (0,1)
    float_stride: uint32_t,

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

// #ifdef EXPOSE_SIGMOID
// flow context: Colorspace
// struct flow_SigmoidInfo {
// float constant;
// float x_coeff;
// float x_offset;
// float y_offset;
// float y_coeff;
// };
// #endif
//

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct ColorspaceInfo {
    placeholder: u8, /* FIXME: replace
                      * float byte_to_float[256]; // Converts 0..255 -> 0..1, but knowing that 0.255 has sRGB gamma.
                      * flow_working_floatspace floatspace;
                      * bool apply_srgb;
                      * bool apply_gamma;
                      * float gamma;
                      * float gamma_inverse;
                      * #ifdef EXPOSE_SIGMOID
                      * struct flow_SigmoidInfo sigmoid;
                      * bool apply_sigmoid;
                      * #endif
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
    pub magic_byte_sets: *const CodecMagicBytes,
    pub magic_bytes_sets_count: size_t,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct CodecMagicBytes {
    pub byte_count: size_t,
    pub bytes: *const u8
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
    pub downscale_if_wider_than: int64_t,
    pub or_if_taller_than: int64_t,
    pub downscaled_min_width: int64_t,
    pub downscaled_min_height: int64_t,
    pub scale_luma_spatially: bool,
    pub gamma_correct_for_srgb_during_spatial_luma_scaling: bool,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct EncoderHints {
    pub jpeg_encode_quality: int32_t,
    pub disable_png_alpha: bool,
}



#[repr(C)]
#[derive(Clone,Debug,Copy)]
pub struct Scale2dRenderToCanvas1d {
    // There will need to be consistency checks against the createcanvas node
    //
    // struct flow_interpolation_details * interpolationDetails;
    pub scale_to_width: i32,
    pub scale_to_height: i32,
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
    pub transpose_on_write: bool, // Other fields skipped, not acessed.
}

#[repr(C)]
#[derive(Clone,Debug,Copy)]
pub struct Rect {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32
}



// struct flow_nodeinfo_render_to_canvas_1d {
//    // There will need to be consistency checks against the createcanvas node
//
//    flow_interpolation_filter interpolation_filter;
//    // struct flow_interpolation_details * interpolationDetails;
//    int32_t scale_to_width;
//    bool transpose_on_write;
//    flow_working_floatspace scale_in_colorspace;
//
//    float sharpen_percent_goal;
//
//    flow_compositing_mode compositing_mode;
//    // When using compositing mode blend_with_matte, this color will be used. We should probably define this as always
//    // being sRGBA, 4 bytes.
//    uint8_t matte_color[4];
//
//    struct flow_scanlines_filter * filter_list;
// };

mod must_replace{
    use super::*;
    use ::libc;
    extern "C" {

    }

}

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



        pub fn flow_bitmap_bgra_flip_vertical(c: *mut ImageflowContext, bitmap: *mut BitmapBgra);
        pub fn flow_bitmap_bgra_flip_horizontal(c: *mut ImageflowContext, bitmap: *mut BitmapBgra);

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
        pub fn flow_node_execute_render_to_canvas_1d(c: *mut ImageflowContext,
                                                     input: *mut BitmapBgra,
                                                     canvas: *mut BitmapBgra,
                                                     info: *const RenderToCanvas1d)
                                                     -> bool;

        pub fn flow_bitmap_bgra_fill_rect(c: *mut ImageflowContext,
                                          input: *mut BitmapBgra,
                                          x1: u32,
                                          y1: u32,
                                          x2: u32,
                                          y2: u32,
                                          color_srgb_argb: u32)
                                          -> bool;
    }
}

mod mid_term {
    use super::*;
    use ::libc;

    extern "C" {
        pub fn flow_context_set_floatspace(ctx: *mut ImageflowContext,
                                           space: Floatspace,
                                           a: f32,
                                           b: f32,
                                           c: f32);



    pub fn flow_bitmap_bgra_load_png(c: *mut ImageflowContext,
    b_ref: *mut *const BitmapBgra,
    path: *const libc::c_char)
    -> bool;

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

        pub fn flow_context_set_error_get_message_buffer(context: *mut ImageflowContext,
                                                         code: i32, // FlowStatusCode
                                                         file: *const libc::c_char,
                                                         line: i32,
                                                         function_name: *const libc::c_char)
                                                         -> *const libc::c_char;

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
                                             instance: *mut CodecInstance)
                                             -> *mut BitmapBgra;

        pub fn flow_codec_select_from_seekable_io(context: *mut ImageflowContext, io: *mut ImageflowJobIo) -> i64;

        pub fn flow_codec_decoder_get_info(c: *mut ImageflowContext,
                                           codec_state: *mut libc::c_void, codec_id: i64, info: *mut DecoderInfo) -> bool;

        pub fn flow_codec_decoder_set_downscale_hints(c: *mut ImageflowContext,
                                                      instance: *mut CodecInstance, hints: *const DecoderDownscaleHints, crash_if_not_implemented: bool) -> bool;


        pub fn detect_content(c: *mut ImageflowContext, input: *mut BitmapBgra, threshold: u32 ) -> Rect;
    }
}

pub use self::must_replace::*;
pub use self::long_term::*;
pub use self::mid_term::*;



// https://github.com/rust-lang/rust/issues/17417


#[test]
fn flow_context_create_destroy_works() {
    unsafe {
        let c = flow_context_create();
        assert!(!c.is_null());

        flow_context_destroy(c);
    }
}
