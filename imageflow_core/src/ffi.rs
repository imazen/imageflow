//! # Do not use
//! Do not use functions from this module outside of imageflow_core
//!
//! **Use imageflow_core::abi functions instead when creating bindings**
//!
//! These aren't to be exposed, but rather to connect to C internals

extern crate libc;
use std::ascii::AsciiExt;
use std::ptr;

use std::str::FromStr;

pub enum Context {}

pub enum JobIO {}

pub enum Job {}

pub enum Graph {}

#[repr(C)]
pub enum IoMode {
    None = 0,
    read_sequential = 1,
    write_sequential = 2,
    read_seekable = 5, // 1 | 4,
    write_seekable = 6, // 2 | 4,
    read_write_seekable = 15, // 1 | 2 | 4 | 8
}
#[repr(C)]
#[derive(Copy,Clone)]
pub enum IoDirection {
    Out = 8,
    In = 4,
}


#[repr(C)]
#[derive(Copy,Clone)]
pub enum EdgeKind {
    None = 0,
    Input = 1,
    Canvas = 2,
    Info = 3,
}


#[repr(C)]
#[derive(Copy,Clone, Debug)]
pub enum PixelFormat {
    bgr24 = 3,
    bgra32 = 4,
    gray8 = 1,
}

#[repr(C)]
#[derive(Copy,Clone)]
pub enum Floatspace {
    srgb = 0,
    linear = 1,
    gamma = 2,
}

#[repr(C)]
#[derive(Copy,Clone, Debug)]
pub enum BitmapCompositingMode {
    replace_with_self = 0,
    blend_with_self = 1,
    blend_with_matte = 2,
}



#[repr(C)]
#[derive(Copy,Clone,Debug, PartialEq)]
pub enum Filter {
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
#[derive(Copy,Clone,Debug, PartialEq)]
pub enum FlowStatusCode {
    NoError                    = 0,
    OutOfMemory                = 10,
    IOError                    = 20,
    InvalidInternalState       = 30,
    NotImplemented             = 40,
    InvalidArgument            = 50,
    NullArgument               = 51,
    InvalidDimensions          = 52,
    UnsupportedPixelFormat     = 53,
    ItemDoesNotExist           = 54,

    ImageDecodingFailed        = 60,
    ImageEncodingFailed        = 61,
    GraphInvalid               = 70,
    GraphIsCyclic              = 71,
    InvalidInputsToNode        = 72,
    MaximumGraphPassesExceeded = 73,
    OtherError                 = 1024,
    //FIXME: FirstUserDefinedError is 1025 in C but it conflicts with __LastLibraryError
    //___LastLibraryError,
    FirstUserDefinedError      = 1025,
    LastUserDefinedError       = 2147483647,
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
                                                      "cubic",
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


impl FromStr for Filter {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match &*s.to_ascii_lowercase() {
            "robidouxfast" => Ok(Filter::RobidouxFast),
            "robidoux" => Ok(Filter::Robidoux),
            "robidouxsharp" => Ok(Filter::RobidouxSharp),
            "ginseng" => Ok(Filter::Ginseng),
            "ginsengsharp" => Ok(Filter::GinsengSharp),
            "lanczos" => Ok(Filter::Lanczos),
            "lanczossharp" => Ok(Filter::LanczosSharp),
            "lanczos2" => Ok(Filter::Lanczos2),
            "lanczos2sharp" => Ok(Filter::Lanczos2Sharp),
            "cubicfast" => Ok(Filter::CubicFast),
            "cubic" => Ok(Filter::Cubic),
            "cubicsharp" => Ok(Filter::CubicSharp),
            "catmullrom" => Ok(Filter::CatmullRom),
            "catrom" => Ok(Filter::CatmullRom),
            "mitchell" => Ok(Filter::Mitchell),
            "cubicbspline" => Ok(Filter::CubicBSpline),
            "bspline" => Ok(Filter::CubicBSpline),
            "hermite" => Ok(Filter::Hermite),
            "jinc" => Ok(Filter::Jinc),
            "rawlanczos3" => Ok(Filter::RawLanczos3),
            "rawlanczos3sharp" => Ok(Filter::RawLanczos3Sharp),
            "rawlanczos2" => Ok(Filter::RawLanczos2),
            "rawlanczos2sharp" => Ok(Filter::RawLanczos2Sharp),
            "triangle" => Ok(Filter::Triangle),
            "linear" => Ok(Filter::Linear),
            "box" => Ok(Filter::Box),
            "catmullromfast" => Ok(Filter::CatmullRomFast),
            "catmullromfastsharp" => Ok(Filter::CatmullRomFastSharp),
            "fastest" => Ok(Filter::Fastest),
            "mitchellfast" => Ok(Filter::MitchellFast),
            "ncubic" => Ok(Filter::NCubic),
            "ncubicsharp" => Ok(Filter::NCubicSharp),
            _ => Err("no match"),
        }
    }
}

impl Default for DecoderInfo {
    fn default() -> DecoderInfo {
        DecoderInfo {
            codec_id: -1,
            preferred_mime_type: ptr::null(),
            preferred_extension: ptr::null(),
            frame_count: 0,
            current_frame_index: 0,
            frame0_width: 0,
            frame0_height: 0,
            frame0_post_decode_format: PixelFormat::bgra32,
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
    pub frame0_width: i32,
    pub frame0_height: i32,
    pub frame0_post_decode_format: PixelFormat,
}

#[repr(C)]
pub struct EncoderHints {
    pub jpeg_quality: i32,
    pub disable_png_alpha: bool,
}


#[repr(C)]
#[derive(Debug)]
pub struct FlowBitmapBgra {
    // bitmap width in pixels
    pub w: u32,
    // bitmap height in pixels
    pub h: u32,
    // byte length of each row (may include any amount of padding)
    pub stride: u32,
    // pointer to pixel 0,0; should be of length > h * stride
    pub pixels: *mut u8,
    // If true, we don't dispose of *pixels when we dispose the struct
    pub borrowed_pixels: bool,
    // If false, we can even ignore the alpha channel on 4bpp
    pub alpha_meaningful: bool,
    // If false, we can edit pixels without affecting the stride
    pub pixels_readonly: bool,
    // If false, we can change the stride of the image.
    pub stride_readonly: bool,
    // If true, we can reuse the allocated memory for other purposes.
    pub can_reuse_space: bool,

    pub fmt: PixelFormat,
    // When using compositing mode blend_with_matte, this color will be used. We should probably define this as always
    // being sRGBA, 4 bytes.
    pub matte_color: [u8; 4],
    pub compositing_mode: BitmapCompositingMode,
}
/*imageflow_core::ffi::FlowBitmapBgra{
        alpha_meaningful: false,
        can_reuse_space: false,
        compositing_mode: ffi::BitmapCompositingMode::blend_with_self,
        matte_color: [0,0,0,0],
        pixels_readonly: false,
        stride_readonly: false,
        pixels: ptr::null_mut(),
        stride: 0,
        w: 0,
        h: 0,
        borrowed_pixels: false,
        fmt: ffi::PixelFormat::bgra32
    };*/

extern "C" {
    pub fn flow_context_create() -> *mut Context;
    pub fn flow_context_begin_terminate(context: *mut Context) -> bool;
    pub fn flow_context_destroy(context: *mut Context);
    pub fn flow_context_has_error(context: *mut Context) -> bool;
    pub fn flow_context_clear_error(context: *mut Context);
    pub fn flow_context_error_and_stacktrace(context: *mut Context,
                                             buffer: *mut u8,
                                             buffer_length: libc::size_t,
                                             full_file_path: bool)
                                             -> i64;
    pub fn flow_context_print_and_exit_if_err(context: *mut Context) -> bool;

    pub fn flow_context_error_reason(context: *mut Context) -> i32;

    pub fn flow_context_set_error_get_message_buffer(context: *mut Context, code: i32/*FlowStatusCode*/,
        file: *const libc::c_char, line: i32, function_name: *const libc::c_char) -> *const libc::c_char;

    pub fn flow_context_raise_error(context: *mut Context,
                                    error_code: i32,
                                    message: *const libc::c_char,
                                    file: *const libc::c_char,
                                    line: i32,
                                    function_name: *const libc::c_char)
                                    -> bool;


    pub fn flow_context_add_to_callstack(context: *mut Context,
                                         file: *const libc::c_char,
                                         line: i32,
                                         function_name: *const libc::c_char)
                                         -> bool;



    pub fn flow_context_calloc(context: *mut Context,
                               instance_count: usize,
                               instance_size: usize,
                               destructor: *const libc::c_void,
                               owner: *const libc::c_void,
                               file: *const libc::c_char,
                               line: i32)
                               -> *mut libc::c_void;

    pub fn flow_destroy(context: *mut Context,
                        pointer: *const libc::c_void,
                        file: *const libc::uint8_t,
                        line: i32)
                        -> bool;

    pub fn flow_job_destroy(context: *mut Context, job: *mut Job) -> bool;




    pub fn flow_job_create(context: *mut Context) -> *mut Job;


    pub fn flow_job_configure_recording(context: *mut Context,
                                        job: *mut Job,
                                        record_graph_versions: bool,
                                        record_frame_images: bool,
                                        render_last_graph: bool,
                                        render_graph_versions: bool,
                                        render_animated_graph: bool)
                                        -> bool;





    pub fn flow_io_create_for_file(context: *mut Context,
                                   mode: IoMode,
                                   filename: *const libc::c_char,
                                   owner: *const libc::c_void)
                                   -> *mut JobIO;

    pub fn flow_io_create_from_memory(context: *mut Context,
                                      mode: IoMode,
                                      memory: *const u8,
                                      length: libc::size_t,
                                      owner: *const libc::c_void,
                                      destructor_function: *const libc::c_void)
                                      -> *mut JobIO;

    pub fn flow_io_create_for_output_buffer(context: *mut Context,
                                            owner: *const libc::c_void)
                                            -> *mut JobIO;


    // Returns false if the flow_io struct is disposed or not an output buffer type (or for any other error)
    //
    pub fn flow_io_get_output_buffer(context: *mut Context,
                                     io: *mut JobIO,
                                     result_buffer: *mut *const u8,
                                     result_buffer_length: *mut libc::size_t)
                                     -> bool;

    pub fn flow_job_get_io(context: *mut Context,
                           job: *mut Job,
                           placeholder_id: i32)
                           -> *mut JobIO;



    pub fn flow_job_add_io(context: *mut Context,
                           job: *mut Job,
                           io: *mut JobIO,
                           placeholder_id: i32,
                           direction: IoDirection)
                           -> bool;

    pub fn flow_job_get_decoder_info(c: *mut Context,
                                     job: *mut Job,
                                     by_placeholder_id: i32,
                                     info: *mut DecoderInfo)
                                     -> bool;




    pub fn flow_job_decoder_set_downscale_hints_by_placeholder_id(c: *mut Context,
                                                                  job: *mut Job, placeholder_id:i32,
                                                                  if_wider_than: i64,  or_taller_than: i64,
                                                                  downscaled_min_width: i64,  downscaled_min_height:i64,  scale_luma_spatially:bool,
                                                                  gamma_correct_for_srgb_during_spatial_luma_scaling:bool) -> bool;


    pub fn flow_context_set_floatspace(c: *mut Context,
                                       space: Floatspace,
                                       a: f32,
                                       b: f32,
                                       c: f32);

    pub fn flow_bitmap_bgra_test_compare_to_record(c: *mut Context,
                                                   bitmap: *mut FlowBitmapBgra,
                                                   storage_name: *const libc::c_char,
                                                   store_if_missing: bool,
                                                   off_by_one_byte_differences_permitted: usize,
                                                   caller_filename: *const libc::c_char,
                                                   caller_linenumber: i32,
                                                    storage_relative_to: *const libc::c_char)
                                                   -> bool;


    /// THESE SHOULD BE DELETED AS THEY ARE BEING REWRITTEN IN RUST
    /// Creating and manipulating graphs directly is going away very soon in favor of a JSON string.

    /*
    pub fn flow_job_execute(c: *mut Context, job: *mut Job, g: *mut *mut Graph) -> bool;
*/

    /**/
    pub fn flow_graph_print_to_stdout(c: *mut Context, g: *const Graph) -> bool;

    pub fn flow_graph_create(context: *mut Context,
                             max_edges: u32,
                             max_nodes: u32,
                             max_info_bytes: u32,
                             growth_factor: f32)
                             -> *mut Graph;


    pub fn flow_edge_create(c: *mut Context,
                            g: *mut *mut Graph,
                            from: i32,
                            to: i32,
                            kind: EdgeKind)
                            -> i32;
    pub fn flow_node_create_decoder(c: *mut Context,
                                    g: *mut *mut Graph,
                                    prev_node: i32,
                                    placeholder_id: i32)
                                    -> i32;
    pub fn flow_node_create_canvas(c: *mut Context,
                                   g: *mut *mut Graph,
                                   prev_node: i32,
                                   format: PixelFormat,
                                   width: usize,
                                   height: usize,
                                   bgcolor: u32)
                                   -> i32;

    pub fn flow_node_create_scale(c: *mut Context,
                                  g: *mut *mut Graph,
                                  prev_node: i32,
                                  width: usize,
                                  height: usize,
                                  downscale_filter: i32,
                                  upscale_filter: i32,
                                  flags: usize,
                                  sharpen: f32)
                                  -> i32;

    pub fn flow_node_create_expand_canvas(c: *mut Context,
                                          g: *mut *mut Graph,
                                          prev_node: i32,
                                          left: u32,
                                          top: u32,
                                          right: u32,
                                          bottom: u32,
                                          canvas_color_srgb: u32)
                                          -> i32;

    pub fn flow_node_create_fill_rect(c: *mut Context,
                                      g: *mut *mut Graph,
                                      prev_node: i32,
                                      x1: u32,
                                      y1: u32,
                                      x2: u32,
                                      y2: u32,
                                      color_srgb: u32)
                                      -> i32;

    pub fn flow_node_create_bitmap_bgra_reference(c: *mut Context,
                                        g: *mut *mut Graph,
                                        prev_node: i32, reference: *mut *mut FlowBitmapBgra) -> i32;


    pub fn flow_node_create_rotate_90(c: *mut Context, g: *mut *mut Graph, prev_node: i32) -> i32;
    pub fn flow_node_create_rotate_180(c: *mut Context, g: *mut *mut Graph, prev_node: i32) -> i32;
    pub fn flow_node_create_rotate_270(c: *mut Context, g: *mut *mut Graph, prev_node: i32) -> i32;

    pub fn flow_node_create_transpose(c: *mut Context, g: *mut *mut Graph, prev_node: i32) -> i32;

    pub fn flow_node_create_primitive_copy_rect_to_canvas(c: *mut Context,
                                                          g: *mut *mut Graph,
                                                          prev_node: i32,
                                                          from_x: u32,
                                                          from_y: u32,
                                                          width: u32,
                                                          height: u32,
                                                          x: u32,
                                                          y: u32)
                                                          -> i32;

    pub fn flow_node_create_encoder(c: *mut Context,
                                    g: *mut *mut Graph,
                                    prev_node: i32,
                                    placeholder_id: i32,
                                    desired_encoder_id: i64,
                                    hints: *const EncoderHints)
                                    -> i32;

    pub fn flow_node_create_primitive_flip_vertical(c: *mut Context,
                                                    g: *mut *mut Graph,
                                                    prev_node: i32)
                                                    -> i32;

    pub fn flow_node_create_primitive_flip_horizontal(c: *mut Context,
                                                      g: *mut *mut Graph,
                                                      prev_node: i32)
                                                      -> i32;

    pub fn flow_node_create_primitive_crop(c: *mut Context,
                                           g: *mut *mut Graph,
                                           prev_node: i32,
                                           x1: u32,
                                           y1: u32,
                                           x2: u32,
                                           y2: u32)
                                           -> i32;

    /**/
//  /////////// END HEADERS TO DELETE


}


// https://github.com/rust-lang/rust/issues/17417


#[test]
fn flow_context_create_destroy_works() {
    unsafe {
        let c = flow_context_create();
        assert!(!c.is_null());

        flow_context_destroy(c);
    }
}

#[test]
fn flow_job_creation_works() {
    unsafe {
        let c = flow_context_create();
        assert!(!c.is_null());

        let j = flow_job_create(c);
        assert!(!j.is_null());

        flow_context_destroy(c);
    }
}


#[test]
fn flow_graph_creation_works() {
    unsafe {
        let c = flow_context_create();
        assert!(!c.is_null());

        let mut g = flow_graph_create(c, 10, 10, 10, 2.0);
        assert!(!g.is_null());

        let j = flow_job_create(c);
        assert!(!j.is_null());

        let last = flow_node_create_canvas(c,
                                           (&mut g) as *mut *mut Graph,
                                           -1,
                                           PixelFormat::bgra32,
                                           100,
                                           100,
                                           0);
        assert!(last == 0);

        flow_context_destroy(c);
    }
}
