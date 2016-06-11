extern crate libc;
use std::ptr;


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
pub enum IoDirection {
    Out = 8,
    In = 4,
}

#[repr(C)]
pub enum PixelFormat {
    bgr24 = 3,
    bgra32 = 4,
    gray8 = 1,
}

#[repr(C)]
pub enum Floatspace {
    srgb = 0,
    linear = 1,
    gamma = 2,
}


#[repr(C)]
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
    pub jpeg_quality: i32
}

extern "C" {
    pub fn flow_context_create() -> *mut Context;
    pub fn flow_context_begin_terminate(context: *mut Context) -> bool;
    pub fn flow_context_destroy(context: *mut Context);
    pub fn flow_context_has_error(context: *mut Context) -> bool;
    pub fn flow_context_clear_error(context: *mut Context);
    pub fn flow_context_error_and_stacktrace(context: *mut Context,
                                             buffer: *mut u8,
                                             buffer_length: *mut libc::size_t,
                                             full_file_path: bool)
                                             -> i64;
    pub fn flow_context_print_and_exit_if_err(context: *mut Context) -> bool;


    pub fn flow_job_create(context: *mut Context) -> *mut Job;
    pub fn flow_io_create_for_file(context: *mut Context,
                                   mode: IoMode,
                                   filename: *const libc::c_char,
                                   owner: *mut libc::c_void)
                                   -> *mut JobIO;
    pub fn flow_job_add_io(context: *mut Context,
                           job: *mut Job,
                           io: *mut JobIO,
                           placeholder_id: i32,
                           direction: IoDirection)
                           -> bool;

    // Creating and manipulating graphs directly is going away very soon in favor of a JSON string.

    pub fn flow_graph_create(context: *mut Context,
                             max_edges: u32,
                             max_nodes: u32,
                             max_info_bytes: u32,
                             growth_factor: f32)
                             -> *mut Graph;


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
                                  downscale_filter: Filter,
                                  upscale_filter: Filter,
                                  flags: usize)
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

    pub fn flow_node_create_rotate_90(c: *mut Context, g: *mut *mut Graph, prev_node: i32) -> i32;
    pub fn flow_node_create_rotate_180(c: *mut Context, g: *mut *mut Graph, prev_node: i32) -> i32;
    pub fn flow_node_create_rotate_270(c: *mut Context, g: *mut *mut Graph, prev_node: i32) -> i32;

    pub fn flow_node_create_encoder(c: *mut Context,
                                    g: *mut *mut Graph,
                                    prev_node: i32,
                                    placeholder_id: i32,
                                    desired_encoder_id: i64, hints: &EncoderHints)
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

    pub fn flow_job_execute(c: *mut Context, job: *mut Job, g: *mut *mut Graph) -> bool;


    pub fn flow_context_set_floatspace(c: *mut Context,
                                       space: Floatspace,
                                       a: f32,
                                       b: f32,
                                       c: f32);

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
