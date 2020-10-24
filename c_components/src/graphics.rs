#![allow(
    dead_code,
    mutable_transmutes,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#[cfg(target_arch = "x86")]
pub use std::arch::x86::{
    __m128, _mm_add_ps, _mm_loadu_ps, _mm_movehl_ps, _mm_movelh_ps, _mm_mul_ps, _mm_set1_ps,
    _mm_setr_ps, _mm_setzero_ps, _mm_storeu_ps, _mm_unpackhi_ps, _mm_unpacklo_ps,
};
#[cfg(target_arch = "x86_64")]
pub use std::arch::x86_64::{
    __m128, _mm_add_ps, _mm_loadu_ps, _mm_movehl_ps, _mm_movelh_ps, _mm_mul_ps, _mm_set1_ps,
    _mm_setr_ps, _mm_setzero_ps, _mm_storeu_ps, _mm_unpackhi_ps, _mm_unpacklo_ps,
};
use std::f64;
use std::ffi::CString;
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_decoder_frame_info {
    pub w: i32,
    pub h: i32,
    pub format: flow_pixel_format,
}
extern "C" {
    #[no_mangle]
    fn flow_pixel_format_bytes_per_pixel(format: flow_pixel_format) -> u32;
    #[no_mangle]
    fn flow_effective_pixel_format(b: *mut flow_bitmap_bgra) -> flow_pixel_format;
    #[no_mangle]
    fn flow_pixel_format_channels(format: flow_pixel_format) -> u32;
    #[no_mangle]
    fn flow_snprintf(s: *mut libc::c_char, n: usize, fmt: *const libc::c_char, _: ...) -> i32;
    #[no_mangle]
    fn flow_set_owner(c: *mut flow_c, thing: *mut libc::c_void, owner: *mut libc::c_void) -> bool;
    #[no_mangle]
    fn flow_context_calloc(
        c: *mut flow_c,
        instance_count: usize,
        instance_size: usize,
        destructor: flow_destructor_function,
        owner: *mut libc::c_void,
        file: *const libc::c_char,
        line: i32,
    ) -> *mut libc::c_void;
    #[no_mangle]
    fn flow_context_malloc(
        c: *mut flow_c,
        byte_count: usize,
        destructor: flow_destructor_function,
        owner: *mut libc::c_void,
        file: *const libc::c_char,
        line: i32,
    ) -> *mut libc::c_void;
    #[no_mangle]
    fn flow_deprecated_free(
        c: *mut flow_c,
        pointer: *mut libc::c_void,
        file: *const libc::c_char,
        line: i32,
    );
    #[no_mangle]
    fn flow_destroy(
        c: *mut flow_c,
        pointer: *mut libc::c_void,
        file: *const libc::c_char,
        line: i32,
    ) -> bool;
    #[no_mangle]
    fn flow_context_set_error_get_message_buffer(
        c: *mut flow_c,
        code: flow_status_code,
        file: *const libc::c_char,
        line: i32,
        function_name: *const libc::c_char,
    ) -> *mut libc::c_char;
    #[no_mangle]
    fn flow_context_add_to_callstack(
        c: *mut flow_c,
        file: *const libc::c_char,
        line: i32,
        function_name: *const libc::c_char,
    ) -> bool;
    #[no_mangle]
    fn flow_context_profiler_start(
        c: *mut flow_c,
        name: *const libc::c_char,
        allow_recursion: bool,
    );
    #[no_mangle]
    fn flow_context_profiler_stop(
        c: *mut flow_c,
        name: *const libc::c_char,
        assert_started: bool,
        stop_children: bool,
    );
    #[no_mangle]
    fn pow(_: f64, _: f64) -> f64;
    #[no_mangle]
    fn flow_bitmap_bgra_create_header(c: *mut flow_c, sx: i32, sy: i32) -> *mut flow_bitmap_bgra;
    #[no_mangle]
    fn memcpy(_: *mut libc::c_void, _: *const libc::c_void, _: u64) -> *mut libc::c_void;
    #[no_mangle]
    fn fabs(_: f64) -> f64;
    #[no_mangle]
    fn j1(_: f64) -> f64;
    #[no_mangle]
    fn fmin(_: f64, _: f64) -> f64;
    #[no_mangle]
    fn ceil(_: f64) -> f64;
    #[no_mangle]
    fn floor(_: f64) -> f64;
    #[no_mangle]
    fn fmax(_: f64, _: f64) -> f64;
    #[no_mangle]
    fn sqrt(_: f64) -> f64;
    #[no_mangle]
    fn exp(_: f64) -> f64;
    #[no_mangle]
    fn memset(_: *mut libc::c_void, _: i32, _: u64) -> *mut libc::c_void;
    #[no_mangle]
    fn flow_bitmap_float_create(
        c: *mut flow_c,
        sx: i32,
        sy: i32,
        channels: i32,
        zeroed: bool,
    ) -> *mut flow_bitmap_float;
    #[no_mangle]
    fn flow_bitmap_float_create_header(
        c: *mut flow_c,
        sx: i32,
        sy: i32,
        channels: i32,
    ) -> *mut flow_bitmap_float;
}
pub type cmsFloat64Number = f64;
#[derive(Copy, Clone)]
#[repr(C)]
pub struct cmsCIExyY {
    pub x: cmsFloat64Number,
    pub y: cmsFloat64Number,
    pub Y: cmsFloat64Number,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct cmsCIExyYTRIPLE {
    pub Red: cmsCIExyY,
    pub Green: cmsCIExyY,
    pub Blue: cmsCIExyY,
}

pub type FLOW_DIRECTION = u32;
pub const FLOW_INPUT: FLOW_DIRECTION = 4;
pub const FLOW_OUTPUT: FLOW_DIRECTION = 8;

#[repr(u32)]
#[derive(Copy, Clone)]
pub enum flow_status_code {
    No_Error = 0,
    Out_of_memory = 10,
    IO_error = 20,
    Invalid_internal_state = 30,
    Panic = 31,
    Not_implemented = 40,
    Invalid_argument = 50,
    Null_argument = 51,
    Invalid_dimensions = 52,
    Unsupported_pixel_format = 53,
    Item_does_not_exist = 54,

    Image_decoding_failed = 60,
    Image_encoding_failed = 61,
    ErrorReportingInconsistency = 90,
    First_rust_error = 200,

    Other_error = 1024,
    // ___Last_library_error,
    First_user_defined_error = 1025,
    Last_user_defined_error = 2147483647,
}
pub type flow_interpolation_filter = u32;
pub const flow_interpolation_filter_NCubicSharp: flow_interpolation_filter = 30;
pub const flow_interpolation_filter_NCubic: flow_interpolation_filter = 29;
pub const flow_interpolation_filter_MitchellFast: flow_interpolation_filter = 28;
pub const flow_interpolation_filter_Fastest: flow_interpolation_filter = 27;
pub const flow_interpolation_filter_CatmullRomFastSharp: flow_interpolation_filter = 26;
pub const flow_interpolation_filter_CatmullRomFast: flow_interpolation_filter = 25;
pub const flow_interpolation_filter_Box: flow_interpolation_filter = 24;
pub const flow_interpolation_filter_Linear: flow_interpolation_filter = 23;
pub const flow_interpolation_filter_Triangle: flow_interpolation_filter = 22;
pub const flow_interpolation_filter_RawLanczos2Sharp: flow_interpolation_filter = 21;
pub const flow_interpolation_filter_RawLanczos2: flow_interpolation_filter = 20;
pub const flow_interpolation_filter_RawLanczos3Sharp: flow_interpolation_filter = 19;
pub const flow_interpolation_filter_RawLanczos3: flow_interpolation_filter = 18;
pub const flow_interpolation_filter_Jinc: flow_interpolation_filter = 17;
pub const flow_interpolation_filter_Hermite: flow_interpolation_filter = 16;
pub const flow_interpolation_filter_CubicBSpline: flow_interpolation_filter = 15;
pub const flow_interpolation_filter_Mitchell: flow_interpolation_filter = 14;
pub const flow_interpolation_filter_CatmullRom: flow_interpolation_filter = 13;
pub const flow_interpolation_filter_CubicSharp: flow_interpolation_filter = 12;
pub const flow_interpolation_filter_Cubic: flow_interpolation_filter = 11;
pub const flow_interpolation_filter_CubicFast: flow_interpolation_filter = 10;
pub const flow_interpolation_filter_Lanczos2Sharp: flow_interpolation_filter = 9;
pub const flow_interpolation_filter_Lanczos2: flow_interpolation_filter = 8;
pub const flow_interpolation_filter_LanczosSharp: flow_interpolation_filter = 7;
pub const flow_interpolation_filter_Lanczos: flow_interpolation_filter = 6;
pub const flow_interpolation_filter_GinsengSharp: flow_interpolation_filter = 5;
pub const flow_interpolation_filter_Ginseng: flow_interpolation_filter = 4;
pub const flow_interpolation_filter_RobidouxSharp: flow_interpolation_filter = 3;
pub const flow_interpolation_filter_Robidoux: flow_interpolation_filter = 2;
pub const flow_interpolation_filter_RobidouxFast: flow_interpolation_filter = 1;
pub type flow_pixel_format = u32;
pub const flow_gray8: flow_pixel_format = 1;
pub const flow_bgr32: flow_pixel_format = 70;
pub const flow_bgra32: flow_pixel_format = 4;
pub const flow_bgr24: flow_pixel_format = 3;
pub type flow_bitmap_compositing_mode = u32;
pub const flow_bitmap_compositing_blend_with_matte: flow_bitmap_compositing_mode = 2;
pub const flow_bitmap_compositing_blend_with_self: flow_bitmap_compositing_mode = 1;
pub const flow_bitmap_compositing_replace_self: flow_bitmap_compositing_mode = 0;
pub type flow_working_floatspace = u32;
pub const flow_working_floatspace_gamma: flow_working_floatspace = 2;
pub const flow_working_floatspace_linear: flow_working_floatspace = 1;
pub const flow_working_floatspace_as_is: flow_working_floatspace = 0;
pub const flow_working_floatspace_srgb: flow_working_floatspace = 0;
pub type flow_io_mode = u32;
pub const flow_io_mode_read_write_seekable: flow_io_mode = 15;
pub const flow_io_mode_write_seekable: flow_io_mode = 6;
pub const flow_io_mode_read_seekable: flow_io_mode = 5;
pub const flow_io_mode_write_sequential: flow_io_mode = 2;
pub const flow_io_mode_read_sequential: flow_io_mode = 1;
pub const flow_io_mode_null: flow_io_mode = 0;
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_context {
    pub codec_set: *mut flow_context_codec_set,
    pub underlying_heap: flow_heap,
    pub object_tracking: flow_objtracking_info,
    pub log: flow_profiling_log,
    pub error: flow_error_info,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_error_info {
    pub reason: flow_status_code,
    pub callstack: [flow_error_callstack_line; 8],
    pub callstack_count: i32,
    pub callstack_capacity: i32,
    pub locked: bool,
    pub status_included_in_message: bool,
    pub message: [libc::c_char; 1024],
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_error_callstack_line {
    pub file: *const libc::c_char,
    pub line: i32,
    pub function_name: *const libc::c_char,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_profiling_log {
    pub log: *mut flow_profiling_entry,
    pub count: u32,
    pub capacity: u32,
    pub ticks_per_second: i64,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_profiling_entry {
    pub time: i64,
    pub name: *const libc::c_char,
    pub flags: flow_profiling_entry_flags,
}
pub type flow_profiling_entry_flags = u32;
pub const flow_profiling_entry_stop_children: flow_profiling_entry_flags = 56;
pub const flow_profiling_entry_stop_assert_started: flow_profiling_entry_flags = 24;
pub const flow_profiling_entry_stop: flow_profiling_entry_flags = 8;
pub const flow_profiling_entry_start_allow_recursion: flow_profiling_entry_flags = 6;
pub const flow_profiling_entry_start: flow_profiling_entry_flags = 2;
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_objtracking_info {
    pub allocs: *mut flow_heap_object_record,
    pub next_free_slot: usize,
    pub total_slots: usize,
    pub bytes_allocated_net: usize,
    pub bytes_allocated_gross: usize,
    pub allocations_net: usize,
    pub allocations_gross: usize,
    pub bytes_freed: usize,
    pub allocations_net_peak: usize,
    pub bytes_allocated_net_peak: usize,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_heap_object_record {
    pub ptr: *mut libc::c_void,
    pub bytes: usize,
    pub owner: *mut libc::c_void,
    pub destructor: flow_destructor_function,
    pub destructor_called: bool,
    pub allocated_by: *const libc::c_char,
    pub allocated_by_line: i32,
    pub is_owner: bool,
}
pub type flow_destructor_function =
    Option<unsafe extern "C" fn(_: *mut flow_c, _: *mut libc::c_void) -> bool>;
pub type flow_c = flow_context;
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_heap {
    pub _calloc: flow_heap_calloc_function,
    pub _malloc: flow_heap_malloc_function,
    pub _realloc: flow_heap_realloc_function,
    pub _free: flow_heap_free_function,
    pub _context_terminate: flow_heap_terminate_function,
    pub _private_state: *mut libc::c_void,
}
pub type flow_heap_terminate_function =
    Option<unsafe extern "C" fn(_: *mut flow_context, _: *mut flow_heap) -> ()>;
pub type flow_heap_free_function = Option<
    unsafe extern "C" fn(
        _: *mut flow_context,
        _: *mut flow_heap,
        _: *mut libc::c_void,
        _: *const libc::c_char,
        _: i32,
    ) -> (),
>;
pub type flow_heap_realloc_function = Option<
    unsafe extern "C" fn(
        _: *mut flow_context,
        _: *mut flow_heap,
        _: *mut libc::c_void,
        _: usize,
        _: *const libc::c_char,
        _: i32,
    ) -> *mut libc::c_void,
>;
pub type flow_heap_malloc_function = Option<
    unsafe extern "C" fn(
        _: *mut flow_context,
        _: *mut flow_heap,
        _: usize,
        _: *const libc::c_char,
        _: i32,
    ) -> *mut libc::c_void,
>;
pub type flow_heap_calloc_function = Option<
    unsafe extern "C" fn(
        _: *mut flow_context,
        _: *mut flow_heap,
        _: usize,
        _: usize,
        _: *const libc::c_char,
        _: i32,
    ) -> *mut libc::c_void,
>;
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_context_codec_set {
    pub codecs: *mut flow_codec_definition,
    pub codecs_count: usize,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_codec_definition {
    pub codec_id: i64,
    pub initialize: codec_initialize,
    pub get_info: codec_get_info_fn,
    pub get_frame_info: codec_get_frame_info_fn,
    pub set_downscale_hints: codec_set_downscale_hints_fn,
    pub switch_frame: codec_switch_frame_fn,
    pub read_frame: codec_read_frame_fn,
    pub write_frame: codec_write_frame_fn,
    pub stringify: codec_stringify_fn,
    pub name: *const libc::c_char,
    pub preferred_mime_type: *const libc::c_char,
    pub preferred_extension: *const libc::c_char,
}
pub type codec_stringify_fn = Option<
    unsafe extern "C" fn(
        _: *mut flow_c,
        _: *mut libc::c_void,
        _: *mut libc::c_char,
        _: usize,
    ) -> bool,
>;
pub type codec_write_frame_fn = Option<
    unsafe extern "C" fn(
        _: *mut flow_c,
        _: *mut libc::c_void,
        _: *mut flow_bitmap_bgra,
        _: *mut flow_encoder_hints,
    ) -> bool,
>;
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_encoder_hints {
    pub disable_png_alpha: bool,
    pub zlib_compression_level: i32,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_bitmap_bgra {
    pub w: u32,
    pub h: u32,
    pub stride: u32,
    pub pixels: *mut libc::c_uchar,
    pub fmt: flow_pixel_format,
    pub matte_color: [u8; 4],
    pub compositing_mode: flow_bitmap_compositing_mode,
}
pub type codec_read_frame_fn = Option<
    unsafe extern "C" fn(
        _: *mut flow_c,
        _: *mut libc::c_void,
        _: *mut flow_bitmap_bgra,
        _: *mut flow_decoder_color_info,
    ) -> bool,
>;
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_decoder_color_info {
    pub source: flow_codec_color_profile_source,
    pub profile_buf: *mut u8,
    pub buf_length: usize,
    pub white_point: cmsCIExyY,
    pub primaries: cmsCIExyYTRIPLE,
    pub gamma: f64,
}
pub type flow_codec_color_profile_source = u32;
pub const flow_codec_color_profile_source_sRGB: flow_codec_color_profile_source = 4;
pub const flow_codec_color_profile_source_GAMA_CHRM: flow_codec_color_profile_source = 3;
pub const flow_codec_color_profile_source_ICCP_GRAY: flow_codec_color_profile_source = 2;
pub const flow_codec_color_profile_source_ICCP: flow_codec_color_profile_source = 1;
pub const flow_codec_color_profile_source_null: flow_codec_color_profile_source = 0;
pub type codec_switch_frame_fn =
    Option<unsafe extern "C" fn(_: *mut flow_c, _: *mut libc::c_void, _: usize) -> bool>;
pub type codec_set_downscale_hints_fn = Option<
    unsafe extern "C" fn(
        _: *mut flow_c,
        _: *mut flow_codec_instance,
        _: *mut flow_decoder_downscale_hints,
    ) -> bool,
>;
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_decoder_downscale_hints {
    pub downscale_if_wider_than: i64,
    pub or_if_taller_than: i64,
    pub downscaled_min_width: i64,
    pub downscaled_min_height: i64,
    pub scale_luma_spatially: bool,
    pub gamma_correct_for_srgb_during_spatial_luma_scaling: bool,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_codec_instance {
    pub io_id: i32,
    pub codec_id: i64,
    pub codec_state: *mut libc::c_void,
    pub io: *mut flow_io,
    pub direction: FLOW_DIRECTION,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_io {
    pub context: *mut flow_c,
    pub mode: flow_io_mode,
    pub read_func: flow_io_read_function,
    pub write_func: flow_io_write_function,
    pub position_func: flow_io_position_function,
    pub seek_function: flow_io_seek_function,
    pub dispose_func: flow_destructor_function,
    pub user_data: *mut libc::c_void,
    pub optional_file_length: i64,
}
pub type flow_io_seek_function =
    Option<unsafe extern "C" fn(_: *mut flow_c, _: *mut flow_io, _: i64) -> bool>;
pub type flow_io_position_function =
    Option<unsafe extern "C" fn(_: *mut flow_c, _: *mut flow_io) -> i64>;
pub type flow_io_write_function =
    Option<unsafe extern "C" fn(_: *mut flow_c, _: *mut flow_io, _: *const u8, _: usize) -> i64>;
pub type flow_io_read_function =
    Option<unsafe extern "C" fn(_: *mut flow_c, _: *mut flow_io, _: *mut u8, _: usize) -> i64>;
pub type codec_get_frame_info_fn = Option<
    unsafe extern "C" fn(
        _: *mut flow_c,
        _: *mut libc::c_void,
        _: *mut flow_decoder_frame_info,
    ) -> bool,
>;
pub type codec_get_info_fn = Option<
    unsafe extern "C" fn(_: *mut flow_c, _: *mut libc::c_void, _: *mut flow_decoder_info) -> bool,
>;
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_decoder_info {
    pub codec_id: i64,
    pub preferred_mime_type: *const libc::c_char,
    pub preferred_extension: *const libc::c_char,
    pub frame_count: usize,
    pub current_frame_index: i64,
    pub image_width: i32,
    pub image_height: i32,
    pub frame_decodes_into: flow_pixel_format,
}
pub type codec_initialize =
    Option<unsafe extern "C" fn(_: *mut flow_c, _: *mut flow_codec_instance) -> bool>;
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_bitmap_float {
    pub w: u32,
    pub h: u32,
    pub channels: u32,
    pub pixels: *mut f32,
    pub pixels_borrowed: bool,
    pub float_count: u32,
    pub float_stride: u32,
    pub alpha_premultiplied: bool,
    pub alpha_meaningful: bool,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_interpolation_details {
    pub window: f64,
    pub p1: f64,
    pub p2: f64,
    pub p3: f64,
    pub q1: f64,
    pub q2: f64,
    pub q3: f64,
    pub q4: f64,
    pub blur: f64,
    pub filter: flow_detailed_interpolation_method,
    pub sharpen_percent_goal: f32,
}
pub type flow_detailed_interpolation_method =
    Option<unsafe extern "C" fn(_: *const flow_interpolation_details, _: f64) -> f64>;
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_interpolation_pixel_contributions {
    pub Weights: *mut f32,
    pub Left: i32,
    pub Right: i32,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_interpolation_line_contributions {
    pub ContribRow: *mut flow_interpolation_pixel_contributions,
    pub WindowSize: u32,
    pub LineLength: u32,
    pub percent_negative: f64,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_convolution_kernel {
    pub kernel: *mut f32,
    pub width: u32,
    pub radius: u32,
    pub threshold_min_change: f32,
    pub threshold_max_change: f32,
    pub buffer: *mut f32,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_colorcontext_info {
    pub byte_to_float: [f32; 256],
    pub floatspace: flow_working_floatspace,
    pub apply_srgb: bool,
    pub apply_gamma: bool,
    pub gamma: f32,
    pub gamma_inverse: f32,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub union C2RustUnnamed {
    pub i: u32,
    pub f: f32,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub union C2RustUnnamed_0 {
    pub i: u32,
    pub f: f32,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub union C2RustUnnamed_1 {
    pub f: f32,
    pub i: u32,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct flow_nodeinfo_scale2d_render_to_canvas1d {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub sharpen_percent_goal: f32,
    pub interpolation_filter: flow_interpolation_filter,
    pub scale_in_colorspace: flow_working_floatspace,
}
/*
 * Copyright (c) Imazen LLC.
 * No part of this project, including this file, may be copied, modified,
 * propagated, or distributed except as permitted in COPYRIGHT.txt.
 * Licensed under the GNU Affero General Public License, Version 3.0.
 * Commercial licenses available at http://imageresizing.net/
 */
pub const BESSEL_01: unsafe extern "C" fn(_: f64) -> f64 = j1;
#[inline]
unsafe extern "C" fn flow_colorcontext_srgb_to_floatspace_uncached(
    colorcontext: *mut flow_colorcontext_info,
    value: u8,
) -> f32 {
    let mut v: f32 = value as f32 * (1.0f32 / 255.0f32);
    if (*colorcontext).apply_srgb {
        v = srgb_to_linear(v)
    } else if (*colorcontext).apply_gamma {
        v = flow_colorcontext_remove_gamma(colorcontext, v)
    }
    return v;
}
#[inline]
unsafe extern "C" fn flow_colorcontext_remove_gamma(
    colorcontext: *mut flow_colorcontext_info,
    value: f32,
) -> f32 {
    return pow(value as f64, (*colorcontext).gamma as f64) as f32;
}
#[inline]
unsafe extern "C" fn srgb_to_linear(s: f32) -> f32 {
    if s <= 0.04045f32 {
        return s / 12.92f32;
    } else {
        return pow(
            ((s + 0.055f32) / (1 as i32 as f32 + 0.055f32)) as f64,
            2.4f32 as f64,
        ) as f32;
    };
}
pub const NULL: i32 = 0 as i32;
pub const FLOW_ERROR_MESSAGE_SIZE: i32 = 1023 as i32;
pub const IR_PI: f64 = 3.1415926535897932384626433832795f64;
#[inline]
unsafe extern "C" fn int_max(a: i32, b: i32) -> i32 {
    return if a >= b { a } else { b };
}
#[inline]
unsafe extern "C" fn int_min(a: i32, b: i32) -> i32 {
    return if a <= b { a } else { b };
}
#[inline]
unsafe extern "C" fn ir_gaussian(x: f64, stdDev: f64) -> f64 {
    return exp(-x * x / (2 as i32 as f64 * stdDev * stdDev))
        / (sqrt(2 as i32 as f64 * IR_PI) * stdDev);
}
#[inline]
unsafe extern "C" fn uchar_clamp_ff(clr: f32) -> u8 {
    let mut result: u16 = 0;
    result = (clr as f64 + 0.5f64) as i16 as u16;
    if result as i32 > 255 as i32 {
        result = if clr < 0 as i32 as f32 {
            0 as i32
        } else {
            255 as i32
        } as u16
    }
    return result as u8;
}
#[inline]
unsafe extern "C" fn fastpow2(p: f32) -> f32 {
    let offset: f32 = if p < 0 as i32 as f32 { 1.0f32 } else { 0.0f32 };
    let clipp: f32 = if p < -(126 as i32) as f32 {
        -126.0f32
    } else {
        p
    };
    let w: i32 = clipp as i32;
    let z: f32 = clipp - w as f32 + offset;
    let v: C2RustUnnamed = C2RustUnnamed {
        i: (((1 as i32) << 23 as i32) as f32
            * (clipp + 121.2740575f32 + 27.7280233f32 / (4.84252568f32 - z) - 1.49012907f32 * z))
            as u32,
    };
    return v.f;
}
#[inline]
unsafe extern "C" fn fastlog2(x: f32) -> f32 {
    let vx: C2RustUnnamed_1 = C2RustUnnamed_1 { f: x };
    let mx: C2RustUnnamed_0 = C2RustUnnamed_0 {
        i: vx.i & 0x7fffff as i32 as u32 | 0x3f000000 as i32 as u32,
    };
    let mut y: f32 = vx.i as f32;
    y *= 1.1920928955078125e-7f32;
    return y - 124.22551499f32 - 1.498030302f32 * mx.f - 1.72587999f32 / (0.3520887068f32 + mx.f);
}
#[inline]
unsafe extern "C" fn fastpow(x: f32, p: f32) -> f32 {
    return fastpow2(p * fastlog2(x));
}
#[inline]
unsafe extern "C" fn linear_to_srgb(clr: f32) -> f32 {
    if clr <= 0.0031308f32 {
        return 12.92f32 * clr * 255.0f32;
    }
    return 1.055f32 * 255.0f32 * fastpow(clr, 0.41666666f32) - 14.025f32;
}
#[inline]
unsafe extern "C" fn flow_colorcontext_apply_gamma(
    colorcontext: *mut flow_colorcontext_info,
    value: f32,
) -> f32 {
    return pow(value as f64, (*colorcontext).gamma_inverse as f64) as f32;
}
#[inline]
unsafe extern "C" fn flow_colorcontext_srgb_to_floatspace(
    colorcontext: *mut flow_colorcontext_info,
    value: u8,
) -> f32 {
    return (*colorcontext).byte_to_float[value as usize];
}
#[inline]
unsafe extern "C" fn flow_colorcontext_floatspace_to_srgb(
    color: *mut flow_colorcontext_info,
    space_value: f32,
) -> u8 {
    let v: f32 = space_value;
    if (*color).apply_gamma {
        return uchar_clamp_ff(flow_colorcontext_apply_gamma(color, v) * 255.0f32);
    }
    if (*color).apply_srgb {
        return uchar_clamp_ff(linear_to_srgb(v));
    }
    return uchar_clamp_ff(255.0f32 * v);
}
#[inline]
unsafe extern "C" fn linear_to_luv(bgr: *mut f32) {
    let xn: f32 = 0.312713f32;
    let yn: f32 = 0.329016f32;
    let Yn: f32 = 1.0f32;
    let un: f32 =
        4 as i32 as f32 * xn / (-(2 as i32) as f32 * xn + 12 as i32 as f32 * yn + 3 as i32 as f32);
    let vn: f32 =
        9 as i32 as f32 * yn / (-(2 as i32) as f32 * xn + 12 as i32 as f32 * yn + 3 as i32 as f32);
    let y_split: f32 = 0.00885645f32;
    let y_adjust: f32 = 903.3f32;
    let R: f32 = *bgr.offset(2);
    let G: f32 = *bgr.offset(1);
    let B: f32 = *bgr.offset(0);
    if R == 0 as i32 as f32 && G == 0 as i32 as f32 && B == 0 as i32 as f32 {
        *bgr.offset(0) = 0 as i32 as f32;
        let ref mut fresh0 = *bgr.offset(2);
        *fresh0 = 100 as i32 as f32;
        *bgr.offset(1) = *fresh0;
        return;
    }
    let X: f32 = 0.412453f32 * R + 0.35758f32 * G + 0.180423f32 * B;
    let Y: f32 = 0.212671f32 * R + 0.71516f32 * G + 0.072169f32 * B;
    let Z: f32 = 0.019334f32 * R + 0.119193f32 * G + 0.950227f32 * B;
    let Yd: f32 = Y / Yn;
    let u: f32 = 4 as i32 as f32 * X / (X + 15 as i32 as f32 * Y + 3 as i32 as f32 * Z);
    let v: f32 = 9 as i32 as f32 * Y / (X + 15 as i32 as f32 * Y + 3 as i32 as f32 * Z);
    let ref mut fresh1 = *bgr.offset(0);
    *fresh1 = if Yd > y_split {
        (116 as i32 as f32 * pow(Yd as f64, (1.0f32 / 3.0f32) as f64) as f32) - 16 as i32 as f32
    } else {
        (y_adjust) * Yd
    };
    let L: f32 = *fresh1;
    *bgr.offset(1) = 13 as i32 as f32 * L * (u - un) + 100 as i32 as f32;
    *bgr.offset(2) = 13 as i32 as f32 * L * (v - vn) + 100 as i32 as f32;
}
#[inline]
unsafe extern "C" fn luv_to_linear(luv: *mut f32) {
    let L: f32 = *luv.offset(0);
    let U: f32 = *luv.offset(1) - 100.0f32;
    let V: f32 = *luv.offset(2) - 100.0f32;
    if L == 0 as i32 as f32 {
        let ref mut fresh2 = *luv.offset(2);
        *fresh2 = 0 as i32 as f32;
        let ref mut fresh3 = *luv.offset(1);
        *fresh3 = *fresh2;
        *luv.offset(0) = *fresh3;
        return;
    }
    let xn: f32 = 0.312713f32;
    let yn: f32 = 0.329016f32;
    let Yn: f32 = 1.0f32;
    let un: f32 =
        4 as i32 as f32 * xn / (-(2 as i32) as f32 * xn + 12 as i32 as f32 * yn + 3 as i32 as f32);
    let vn: f32 =
        9 as i32 as f32 * yn / (-(2 as i32) as f32 * xn + 12 as i32 as f32 * yn + 3 as i32 as f32);
    let y_adjust_2: f32 = 0.00110705645f32;
    let u: f32 = U / (13 as i32 as f32 * L) + un;
    let v: f32 = V / (13 as i32 as f32 * L) + vn;
    let Y: f32 = if L > 8 as i32 as f32 {
        (Yn) * pow(
            ((L + 16 as i32 as f32) / 116 as i32 as f32) as f64,
            3 as i32 as f64,
        ) as f32
    } else {
        (Yn * L) * y_adjust_2
    };
    let X: f32 = 9 as i32 as f32 / 4.0f32 * Y * u / v;
    let Z: f32 = (9 as i32 as f32 * Y - 15 as i32 as f32 * v * Y - v * X) / (3 as i32 as f32 * v);
    let r: f32 = 3.240479f32 * X - 1.53715f32 * Y - 0.498535f32 * Z;
    let g: f32 = -0.969256f32 * X + 1.875991f32 * Y + 0.041556f32 * Z;
    let b: f32 = 0.055648f32 * X - 0.204043f32 * Y + 1.057311f32 * Z;
    *luv.offset(0) = b;
    *luv.offset(1) = g;
    *luv.offset(2) = r;
}
unsafe extern "C" fn derive_cubic_coefficients(
    B: f64,
    C: f64,
    out: *mut flow_interpolation_details,
) {
    let bx2: f64 = B + B;
    (*out).p1 = 1.0f64 - 1.0f64 / 3.0f64 * B;
    (*out).p2 = -3.0f64 + bx2 + C;
    (*out).p3 = 2.0f64 - 1.5f64 * B - C;
    (*out).q1 = 4.0f64 / 3.0f64 * B + 4.0f64 * C;
    (*out).q2 = -8.0f64 * C - bx2;
    (*out).q3 = B + 5.0f64 * C;
    (*out).q4 = -1.0f64 / 6.0f64 * B - C;
}
unsafe extern "C" fn filter_flex_cubic(d: *const flow_interpolation_details, x: f64) -> f64 {
    let t: f64 = fabs(x) / (*d).blur;
    if t < 1.0f64 {
        return (*d).p1 + t * (t * ((*d).p2 + t * (*d).p3));
    }
    if t < 2.0f64 {
        return (*d).q1 + t * ((*d).q2 + t * ((*d).q3 + t * (*d).q4));
    }
    return 0.0f64;
}
unsafe extern "C" fn filter_bicubic_fast(d: *const flow_interpolation_details, t: f64) -> f64 {
    let abs_t: f64 = fabs(t) / (*d).blur;
    let abs_t_sq: f64 = abs_t * abs_t;
    if abs_t < 1 as i32 as f64 {
        return 1 as i32 as f64 - 2 as i32 as f64 * abs_t_sq + abs_t_sq * abs_t;
    }
    if abs_t < 2 as i32 as f64 {
        return 4 as i32 as f64 - 8 as i32 as f64 * abs_t + 5 as i32 as f64 * abs_t_sq
            - abs_t_sq * abs_t;
    }
    return 0 as i32 as f64;
}
unsafe extern "C" fn filter_sinc(d: *const flow_interpolation_details, t: f64) -> f64 {
    let abs_t: f64 = fabs(t) / (*d).blur;
    if abs_t == 0 as i32 as f64 {
        return 1 as i32 as f64;
        // Avoid division by zero
    }
    if abs_t > (*d).window {
        return 0 as i32 as f64;
    }
    let a = abs_t * IR_PI;
    return a.sin() / a;
}
unsafe extern "C" fn filter_box(d: *const flow_interpolation_details, t: f64) -> f64 {
    let x: f64 = t / (*d).blur;
    return if x >= -(1 as i32) as f64 * (*d).window && x < (*d).window {
        1 as i32
    } else {
        0 as i32
    } as f64;
}
unsafe extern "C" fn filter_triangle(d: *const flow_interpolation_details, t: f64) -> f64 {
    let x: f64 = fabs(t) / (*d).blur;
    if x < 1.0f64 {
        return 1.0f64 - x;
    }
    return 0.0f64;
}
unsafe extern "C" fn filter_sinc_windowed(d: *const flow_interpolation_details, t: f64) -> f64 {
    let x: f64 = t / (*d).blur;
    let abs_t: f64 = fabs(x);
    if abs_t == 0 as i32 as f64 {
        return 1 as i32 as f64;
        // Avoid division by zero
    }
    if abs_t > (*d).window {
        return 0 as i32 as f64;
    }
    return (*d).window * (IR_PI * x / (*d).window).sin() * (x * IR_PI).sin()
        / (IR_PI * IR_PI * x * x);
}
unsafe extern "C" fn filter_jinc(d: *const flow_interpolation_details, t: f64) -> f64 {
    let x: f64 = fabs(t) / (*d).blur;
    if x == 0.0f64 {
        return 0.5f64 * IR_PI;
    }
    return j1(IR_PI * x) / x;
    // //x crossing #1 1.2196698912665045
}
/*

static inline double window_jinc (double x) {
    double x_a = x * 1.2196698912665045;
    if (x == 0.0)
        return 1;
    return (BesselOrderOne (IR_PI*x_a) / (x_a * IR_PI * 0.5));
    // //x crossing #1 1.2196698912665045
}

static double filter_window_jinc (const struct flow_interpolation_details * d, double t) {
    return window_jinc (t / (d->blur * d->window));
}
*/
unsafe extern "C" fn filter_ginseng(d: *const flow_interpolation_details, t: f64) -> f64 {
    // Sinc windowed by jinc
    let abs_t: f64 = fabs(t) / (*d).blur;
    let t_pi: f64 = abs_t * IR_PI;
    if abs_t == 0 as i32 as f64 {
        return 1 as i32 as f64;
        // Avoid division by zero
    }
    if abs_t > 3 as i32 as f64 {
        return 0 as i32 as f64;
    }
    let jinc_input: f64 = 1.2196698912665045f64 * t_pi / (*d).window;
    let jinc_output: f64 = j1(jinc_input) / (jinc_input * 0.5f64);
    return jinc_output * (t_pi).sin() / t_pi;
}
pub const TONY: f64 = 0.00001f64;
#[no_mangle]
pub unsafe extern "C" fn flow_interpolation_details_percent_negative_weight(
    details: *const flow_interpolation_details,
) -> f64 {
    let samples: i32 = 50 as i32;
    let step: f64 = (*details).window / samples as f64;
    let mut last_height: f64 =
        (*details).filter.expect("non-null function pointer")(details, -step);
    let mut positive_area: f64 = 0 as i32 as f64;
    let mut negative_area: f64 = 0 as i32 as f64;
    let mut i: i32 = 0 as i32;
    while i <= samples + 2 as i32 {
        let height: f64 =
            (*details).filter.expect("non-null function pointer")(details, i as f64 * step);
        let area: f64 = (height + last_height) / 2.0f64 * step;
        last_height = height;
        if area > 0 as i32 as f64 {
            positive_area += area
        } else {
            negative_area -= area
        }
        i += 1
    }
    return negative_area / positive_area;
}
#[no_mangle]
pub unsafe extern "C" fn flow_interpolation_details_create(
    context: *mut flow_c,
) -> *mut flow_interpolation_details {
    let mut d: *mut flow_interpolation_details = flow_context_calloc(
        context,
        1 as i32 as usize,
        ::std::mem::size_of::<flow_interpolation_details>(),
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        context as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        189 as i32,
    ) as *mut flow_interpolation_details;
    if d.is_null() {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Out_of_memory,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            191 as i32,
            (*::std::mem::transmute::<&[u8; 34], &[libc::c_char; 34]>(
                b"flow_interpolation_details_create\x00",
            ))
            .as_ptr(),
        );
        return NULL as *mut flow_interpolation_details;
    }
    (*d).blur = 1 as i32 as f64;
    (*d).window = 2 as i32 as f64;
    (*d).q1 = 0 as i32 as f64;
    (*d).p1 = (*d).q1;
    (*d).q4 = 1 as i32 as f64;
    (*d).q3 = (*d).q4;
    (*d).p3 = (*d).q3;
    (*d).q2 = (*d).p3;
    (*d).p2 = (*d).q2;
    (*d).sharpen_percent_goal = 0 as i32 as f32;
    return d;
}
#[no_mangle]
pub unsafe extern "C" fn flow_interpolation_details_create_bicubic_custom(
    context: *mut flow_c,
    window: f64,
    blur: f64,
    B: f64,
    C: f64,
) -> *mut flow_interpolation_details {
    let mut d: *mut flow_interpolation_details = flow_interpolation_details_create(context);
    if !d.is_null() {
        (*d).blur = blur;
        derive_cubic_coefficients(B, C, d);
        (*d).filter = Some(
            filter_flex_cubic
                as unsafe extern "C" fn(_: *const flow_interpolation_details, _: f64) -> f64,
        );
        (*d).window = window
    } else {
        flow_context_add_to_callstack(
            context,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            212 as i32,
            (*::std::mem::transmute::<&[u8; 49], &[libc::c_char; 49]>(
                b"flow_interpolation_details_create_bicubic_custom\x00",
            ))
            .as_ptr(),
        );
    }
    return d;
}
#[no_mangle]
pub unsafe extern "C" fn flow_interpolation_details_create_custom(
    context: *mut flow_c,
    window: f64,
    blur: f64,
    filter: flow_detailed_interpolation_method,
) -> *mut flow_interpolation_details {
    let mut d: *mut flow_interpolation_details = flow_interpolation_details_create(context);
    if !d.is_null() {
        (*d).blur = blur;
        (*d).filter = filter;
        (*d).window = window
    } else {
        flow_context_add_to_callstack(
            context,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            226 as i32,
            (*::std::mem::transmute::<&[u8; 41], &[libc::c_char; 41]>(
                b"flow_interpolation_details_create_custom\x00",
            ))
            .as_ptr(),
        );
    }
    return d;
}
#[no_mangle]
pub unsafe extern "C" fn flow_interpolation_details_destroy(
    context: *mut flow_c,
    details: *mut flow_interpolation_details,
) {
    flow_deprecated_free(
        context,
        details as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        233 as i32,
    );
}
unsafe extern "C" fn InterpolationDetails_create_from_internal(
    context: *mut flow_c,
    filter: flow_interpolation_filter,
    checkExistenceOnly: bool,
) -> *mut flow_interpolation_details {
    let ex: bool = checkExistenceOnly;
    let truePtr: *mut flow_interpolation_details = -(1 as i32) as *mut flow_interpolation_details;
    match filter as u32 {
        23 | 22 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_custom(
                    context,
                    1 as i32 as f64,
                    1 as i32 as f64,
                    Some(
                        filter_triangle
                            as unsafe extern "C" fn(
                                _: *const flow_interpolation_details,
                                _: f64,
                            ) -> f64,
                    ),
                )
            }
        }
        20 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_custom(
                    context,
                    2 as i32 as f64,
                    1 as i32 as f64,
                    Some(
                        filter_sinc
                            as unsafe extern "C" fn(
                                _: *const flow_interpolation_details,
                                _: f64,
                            ) -> f64,
                    ),
                )
            }
        }
        18 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_custom(
                    context,
                    3 as i32 as f64,
                    1 as i32 as f64,
                    Some(
                        filter_sinc
                            as unsafe extern "C" fn(
                                _: *const flow_interpolation_details,
                                _: f64,
                            ) -> f64,
                    ),
                )
            }
        }
        21 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_custom(
                    context,
                    2 as i32 as f64,
                    0.9549963639785485f64,
                    Some(
                        filter_sinc
                            as unsafe extern "C" fn(
                                _: *const flow_interpolation_details,
                                _: f64,
                            ) -> f64,
                    ),
                )
            }
        }
        19 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_custom(
                    context,
                    3 as i32 as f64,
                    0.9812505644269356f64,
                    Some(
                        filter_sinc
                            as unsafe extern "C" fn(
                                _: *const flow_interpolation_details,
                                _: f64,
                            ) -> f64,
                    ),
                )
            }
        }
        15 => {
            // Hermite and BSpline no negative weights
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_bicubic_custom(
                    context,
                    2 as i32 as f64,
                    1 as i32 as f64,
                    1 as i32 as f64,
                    0 as i32 as f64,
                )
            };
        }
        8 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_custom(
                    context,
                    2 as i32 as f64,
                    1 as i32 as f64,
                    Some(
                        filter_sinc_windowed
                            as unsafe extern "C" fn(
                                _: *const flow_interpolation_details,
                                _: f64,
                            ) -> f64,
                    ),
                )
            }
        }
        6 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_custom(
                    context,
                    3 as i32 as f64,
                    1 as i32 as f64,
                    Some(
                        filter_sinc_windowed
                            as unsafe extern "C" fn(
                                _: *const flow_interpolation_details,
                                _: f64,
                            ) -> f64,
                    ),
                )
            }
        }
        9 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_custom(
                    context,
                    2 as i32 as f64,
                    0.9549963639785485f64,
                    Some(
                        filter_sinc_windowed
                            as unsafe extern "C" fn(
                                _: *const flow_interpolation_details,
                                _: f64,
                            ) -> f64,
                    ),
                )
            }
        }
        7 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_custom(
                    context,
                    3 as i32 as f64,
                    0.9812505644269356f64,
                    Some(
                        filter_sinc_windowed
                            as unsafe extern "C" fn(
                                _: *const flow_interpolation_details,
                                _: f64,
                            ) -> f64,
                    ),
                )
            }
        }
        10 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_custom(
                    context,
                    2 as i32 as f64,
                    1 as i32 as f64,
                    Some(
                        filter_bicubic_fast
                            as unsafe extern "C" fn(
                                _: *const flow_interpolation_details,
                                _: f64,
                            ) -> f64,
                    ),
                )
            }
        }
        11 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_bicubic_custom(
                    context,
                    2 as i32 as f64,
                    1 as i32 as f64,
                    0 as i32 as f64,
                    1 as i32 as f64,
                )
            }
        }
        12 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_bicubic_custom(
                    context,
                    2 as i32 as f64,
                    0.9549963639785485f64,
                    0 as i32 as f64,
                    1 as i32 as f64,
                )
            }
        }
        13 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_bicubic_custom(
                    context,
                    2 as i32 as f64,
                    1 as i32 as f64,
                    0 as i32 as f64,
                    0.5f64,
                )
            }
        }
        25 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_bicubic_custom(
                    context,
                    1 as i32 as f64,
                    1 as i32 as f64,
                    0 as i32 as f64,
                    0.5f64,
                )
            }
        }
        26 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_bicubic_custom(
                    context,
                    1 as i32 as f64,
                    13.0f64 / 16.0f64,
                    0 as i32 as f64,
                    0.5f64,
                )
            }
        }
        14 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_bicubic_custom(
                    context,
                    2 as i32 as f64,
                    1 as i32 as f64,
                    1.0f64 / 3.0f64,
                    1.0f64 / 3.0f64,
                )
            }
        }
        28 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_bicubic_custom(
                    context,
                    1 as i32 as f64,
                    1 as i32 as f64,
                    1.0f64 / 3.0f64,
                    1.0f64 / 3.0f64,
                )
            }
        }
        29 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_bicubic_custom(
                    context,
                    2.5f64,
                    1.0f64 / 1.1685777620836932f64,
                    0.37821575509399867f64,
                    0.31089212245300067f64,
                )
            }
        }
        30 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_bicubic_custom(
                    context,
                    2.5f64,
                    1.0f64 / 1.105822933719019f64,
                    0.2620145123990142f64,
                    0.3689927438004929f64,
                )
            }
        }
        2 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_bicubic_custom(
                    context,
                    2 as i32 as f64,
                    1 as i32 as f64,
                    0.37821575509399867f64,
                    0.31089212245300067f64,
                )
            }
        }
        27 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_bicubic_custom(
                    context,
                    0.74f64,
                    0.74f64,
                    0.37821575509399867f64,
                    0.31089212245300067f64,
                )
            }
        }
        1 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_bicubic_custom(
                    context,
                    1.05f64,
                    1 as i32 as f64,
                    0.37821575509399867f64,
                    0.31089212245300067f64,
                )
            }
        }
        3 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_bicubic_custom(
                    context,
                    2 as i32 as f64,
                    1 as i32 as f64,
                    0.2620145123990142f64,
                    0.3689927438004929f64,
                )
            }
        }
        16 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_bicubic_custom(
                    context,
                    1 as i32 as f64,
                    1 as i32 as f64,
                    0 as i32 as f64,
                    0 as i32 as f64,
                )
            }
        }
        24 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_custom(
                    context,
                    0.5f64,
                    1 as i32 as f64,
                    Some(
                        filter_box
                            as unsafe extern "C" fn(
                                _: *const flow_interpolation_details,
                                _: f64,
                            ) -> f64,
                    ),
                )
            }
        }
        4 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_custom(
                    context,
                    3 as i32 as f64,
                    1 as i32 as f64,
                    Some(
                        filter_ginseng
                            as unsafe extern "C" fn(
                                _: *const flow_interpolation_details,
                                _: f64,
                            ) -> f64,
                    ),
                )
            }
        }
        5 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_custom(
                    context,
                    3 as i32 as f64,
                    0.9812505644269356f64,
                    Some(
                        filter_ginseng
                            as unsafe extern "C" fn(
                                _: *const flow_interpolation_details,
                                _: f64,
                            ) -> f64,
                    ),
                )
            }
        }
        17 => {
            return if ex as i32 != 0 {
                truePtr
            } else {
                flow_interpolation_details_create_custom(
                    context,
                    6 as i32 as f64,
                    1.0f64,
                    Some(
                        filter_jinc
                            as unsafe extern "C" fn(
                                _: *const flow_interpolation_details,
                                _: f64,
                            ) -> f64,
                    ),
                )
            }
        }
        _ => {}
    }
    if !checkExistenceOnly {
        flow_snprintf(
            flow_context_set_error_get_message_buffer(
                context,
                flow_status_code::Invalid_argument,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                323 as i32,
                (*::std::mem::transmute::<&[u8; 42], &[libc::c_char; 42]>(
                    b"InterpolationDetails_create_from_internal\x00",
                ))
                .as_ptr(),
            ),
            FLOW_ERROR_MESSAGE_SIZE as usize,
            b"Invalid interpolation filter %d\x00" as *const u8 as *const libc::c_char,
            filter as i32,
        );
    }
    return NULL as *mut flow_interpolation_details;
}
#[no_mangle]
pub unsafe extern "C" fn flow_interpolation_details_create_from(
    context: *mut flow_c,
    filter: flow_interpolation_filter,
) -> *mut flow_interpolation_details {
    return InterpolationDetails_create_from_internal(context, filter, false);
}
#[no_mangle]
pub unsafe extern "C" fn flow_interpolation_filter_exists(
    filter: flow_interpolation_filter,
) -> bool {
    return !InterpolationDetails_create_from_internal(NULL as *mut flow_c, filter, true).is_null();
}
unsafe extern "C" fn LineContributions_alloc(
    context: *mut flow_c,
    line_length: u32,
    windows_size: u32,
) -> *mut flow_interpolation_line_contributions {
    let mut res: *mut flow_interpolation_line_contributions = flow_context_malloc(
        context,
        ::std::mem::size_of::<flow_interpolation_line_contributions>(),
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        context as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        343 as i32,
    )
        as *mut flow_interpolation_line_contributions;
    if res.is_null() {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Out_of_memory,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            345 as i32,
            (*::std::mem::transmute::<&[u8; 24], &[libc::c_char; 24]>(
                b"LineContributions_alloc\x00",
            ))
            .as_ptr(),
        );
        return NULL as *mut flow_interpolation_line_contributions;
    }
    (*res).WindowSize = windows_size;
    (*res).LineLength = line_length;
    (*res).ContribRow = flow_context_malloc(
        context,
        (line_length as usize)
            .wrapping_mul(::std::mem::size_of::<flow_interpolation_pixel_contributions>()),
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        context as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        351 as i32,
    ) as *mut flow_interpolation_pixel_contributions;
    if (*res).ContribRow.is_null() {
        flow_deprecated_free(
            context,
            res as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            353 as i32,
        );
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Out_of_memory,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            354 as i32,
            (*::std::mem::transmute::<&[u8; 24], &[libc::c_char; 24]>(
                b"LineContributions_alloc\x00",
            ))
            .as_ptr(),
        );
        return NULL as *mut flow_interpolation_line_contributions;
    }
    let allWeights: *mut f32 = flow_context_calloc(
        context,
        windows_size.wrapping_mul(line_length) as usize,
        ::std::mem::size_of::<f32>(),
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        context as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        358 as i32,
    ) as *mut f32;
    if allWeights.is_null() {
        flow_deprecated_free(
            context,
            (*res).ContribRow as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            360 as i32,
        );
        flow_deprecated_free(
            context,
            res as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            361 as i32,
        );
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Out_of_memory,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            362 as i32,
            (*::std::mem::transmute::<&[u8; 24], &[libc::c_char; 24]>(
                b"LineContributions_alloc\x00",
            ))
            .as_ptr(),
        );
        return NULL as *mut flow_interpolation_line_contributions;
    }
    let mut i: u32 = 0 as i32 as u32;
    while i < line_length {
        let ref mut fresh4 = (*(*res).ContribRow.offset(i as isize)).Weights;
        *fresh4 = allWeights.offset(i.wrapping_mul(windows_size) as isize);
        i = i.wrapping_add(1)
    }
    return res;
}
#[no_mangle]
pub unsafe extern "C" fn flow_interpolation_line_contributions_destroy(
    context: *mut flow_c,
    p: *mut flow_interpolation_line_contributions,
) {
    if !p.is_null() {
        if !(*p).ContribRow.is_null() {
            flow_deprecated_free(
                context,
                (*(*p).ContribRow.offset(0)).Weights as *mut libc::c_void,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                377 as i32,
            );
        }
        flow_deprecated_free(
            context,
            (*p).ContribRow as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            379 as i32,
        );
    }
    flow_deprecated_free(
        context,
        p as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        381 as i32,
    );
}
#[no_mangle]
pub unsafe extern "C" fn flow_interpolation_line_contributions_create(
    context: *mut flow_c,
    output_line_size: u32,
    input_line_size: u32,
    details: *const flow_interpolation_details,
) -> *mut flow_interpolation_line_contributions {
    let sharpen_ratio: f64 = flow_interpolation_details_percent_negative_weight(details);
    let desired_sharpen_ratio: f64 = fmin(
        0.999999999f32 as f64,
        fmax(
            sharpen_ratio,
            (*details).sharpen_percent_goal as f64 / 100.0f64,
        ),
    );
    let scale_factor: f64 = output_line_size as f64 / input_line_size as f64;
    let downscale_factor: f64 = fmin(1.0f64, scale_factor);
    let half_source_window: f64 = ((*details).window + 0.5f64) / downscale_factor;
    let allocated_window_size: u32 =
        (ceil(2 as i32 as f64 * (half_source_window - TONY)) as i32 + 1 as i32) as u32;
    let mut u: u32 = 0;
    let mut ix: u32 = 0;
    let mut res: *mut flow_interpolation_line_contributions =
        LineContributions_alloc(context, output_line_size, allocated_window_size);
    if res.is_null() {
        flow_context_add_to_callstack(
            context,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            401 as i32,
            (*::std::mem::transmute::<&[u8; 45], &[libc::c_char; 45]>(
                b"flow_interpolation_line_contributions_create\x00",
            ))
            .as_ptr(),
        );
        return NULL as *mut flow_interpolation_line_contributions;
    }
    let mut negative_area: f64 = 0 as i32 as f64;
    let mut positive_area: f64 = 0 as i32 as f64;
    u = 0 as i32 as u32;
    while u < output_line_size {
        let center_src_pixel: f64 = (u as f64 + 0.5f64) / scale_factor - 0.5f64;
        let left_edge: i32 = (floor(center_src_pixel) as i32 as u32)
            .wrapping_sub(allocated_window_size.wrapping_sub(1u32).wrapping_div(2u32))
            as i32;
        let right_edge: i32 = (left_edge as u32)
            .wrapping_add(allocated_window_size)
            .wrapping_sub(1u32) as i32;
        let left_src_pixel: u32 = int_max(0 as i32, left_edge) as u32;
        let right_src_pixel: u32 = int_min(right_edge, input_line_size as i32 - 1 as i32) as u32;
        // Net weight
        let mut total_weight: f64 = 0.0f64;
        // Sum of negative and positive weights
        let mut total_negative_weight: f64 = 0.0f64;
        let mut total_positive_weight: f64 = 0.0f64;
        let source_pixel_count: u32 = right_src_pixel
            .wrapping_sub(left_src_pixel)
            .wrapping_add(1u32);
        if source_pixel_count > allocated_window_size {
            flow_interpolation_line_contributions_destroy(context, res);
            flow_context_set_error_get_message_buffer(
                context,
                flow_status_code::Invalid_internal_state,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                426 as i32,
                (*::std::mem::transmute::<&[u8; 45], &[libc::c_char; 45]>(
                    b"flow_interpolation_line_contributions_create\x00",
                ))
                .as_ptr(),
            );
            return NULL as *mut flow_interpolation_line_contributions;
        }
        (*(*res).ContribRow.offset(u as isize)).Left = left_src_pixel as i32;
        (*(*res).ContribRow.offset(u as isize)).Right = right_src_pixel as i32;
        let mut weights: *mut f32 = (*(*res).ContribRow.offset(u as isize)).Weights;
        ix = left_src_pixel;
        while ix <= right_src_pixel {
            let tx: i32 = ix.wrapping_sub(left_src_pixel) as i32;
            let mut add: f64 = Some((*details).filter.expect("non-null function pointer"))
                .expect("non-null function pointer")(
                details,
                downscale_factor * (ix as f64 - center_src_pixel),
            );
            if fabs(add) <= 0.00000002f64 {
                add = 0.0f64
                // Weights below a certain threshold make consistent x-plat
                // integration test results impossible. pos/neg zero, etc.
                // They should be rounded down to zero at the threshold at which results are consistent.
            }
            *weights.offset(tx as isize) = add as f32;
            total_weight += add;
            total_negative_weight += fmin(0 as i32 as f64, add);
            total_positive_weight += fmax(0 as i32 as f64, add);
            ix = ix.wrapping_add(1)
        }
        let mut neg_factor: f32 = 0.;
        let mut pos_factor: f32 = 0.;
        pos_factor = (1.0f32 as f64 / total_weight) as f32;
        neg_factor = pos_factor;
        //printf("cur= %f cur+= %f cur-= %f desired_sharpen_ratio=%f sharpen_ratio-=%f\n", total_weight, total_positive_weight, total_negative_weight, desired_sharpen_ratio, sharpen_ratio);
        if total_weight <= 0.0f32 as f64 || desired_sharpen_ratio > sharpen_ratio {
            if total_negative_weight < 0.0f32 as f64 {
                if desired_sharpen_ratio < 1.0f32 as f64 {
                    let target_positive_weight: f64 =
                        1.0f32 as f64 / (1.0f32 as f64 - desired_sharpen_ratio);
                    let target_negative_weight: f64 =
                        desired_sharpen_ratio * -target_positive_weight;
                    pos_factor = (target_positive_weight / total_positive_weight) as f32;
                    neg_factor = (target_negative_weight / total_negative_weight) as f32;
                    if total_negative_weight == 0 as i32 as f64 {
                        neg_factor = 1.0f32
                    }
                    //printf("target=%f target-=%f, pos_factor=%f neg_factor=%f\n", total_positive_weight - target_negative_weight,  target_negative_weight, pos_factor, neg_factor);
                }
            } else if total_weight == 0.0 {
                // In this situation we have a problem to report
            }
        }
        //printf("\n");
        ix = 0 as i32 as u32;
        while ix < source_pixel_count {
            if *weights.offset(ix as isize) < 0 as i32 as f32 {
                *weights.offset(ix as isize) *= neg_factor;
                negative_area -= *weights.offset(ix as isize) as f64
            } else {
                *weights.offset(ix as isize) *= pos_factor;
                positive_area += *weights.offset(ix as isize) as f64
            }
            ix = ix.wrapping_add(1)
        }
        // Shrink to improve perf & result consistency
        let mut iix: i32 = 0;
        // Shrink region from the right
        iix = source_pixel_count.wrapping_sub(1u32) as i32;
        while iix >= 0 as i32 {
            if *weights.offset(iix as isize) != 0 as i32 as f32 {
                break;
            }
            let ref mut fresh5 = (*(*res).ContribRow.offset(u as isize)).Right;
            *fresh5 -= 1;
            iix -= 1
        }
        // Shrink region from the left
        iix = 0 as i32;
        while iix < source_pixel_count as i32 {
            if *weights.offset(0) != 0 as i32 as f32 {
                break;
            }
            let ref mut fresh6 = (*(*res).ContribRow.offset(u as isize)).Weights;
            *fresh6 = (*fresh6).offset(1);
            weights = weights.offset(1);
            let ref mut fresh7 = (*(*res).ContribRow.offset(u as isize)).Left;
            *fresh7 += 1;
            iix += 1
        }
        u = u.wrapping_add(1)
    }
    (*res).percent_negative = negative_area / positive_area;
    return res;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_scale_rows(
    context: *mut flow_c,
    from: *mut flow_bitmap_float,
    from_row: u32,
    to: *mut flow_bitmap_float,
    to_row: u32,
    row_count: u32,
    weights: *mut flow_interpolation_pixel_contributions,
) -> bool {
    let from_step: u32 = (*from).channels;
    let to_step: u32 = (*to).channels;
    let dest_buffer_count: u32 = (*to).w;
    let min_channels: u32 = from_step.min(to_step);
    let mut ndx: u32 = 0;
    if min_channels > 4 as i32 as u32 {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            520 as i32,
            (*::std::mem::transmute::<&[u8; 29], &[libc::c_char; 29]>(
                b"flow_bitmap_float_scale_rows\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    let mut avg: [f32; 4] = [0.; 4];
    // if both have alpha, process it
    if from_step == 4 && to_step == 4 {
        let mut row: u32 = 0;
        while row < row_count {
            let source_offset = ((from_row + row) * (*from).float_stride) as isize;
            let source_buffer: *const __m128 =
                (*from).pixels.offset(source_offset) as *const __m128;
            let dest_offset = ((to_row + row) * (*to).float_stride) as isize;
            let dest_buffer: *mut __m128 = (*to).pixels.offset(dest_offset) as *mut __m128;
            let dest_buffer: &mut [__m128] =
                std::slice::from_raw_parts_mut(dest_buffer, dest_buffer_count as usize);
            ndx = 0;
            while ndx < dest_buffer_count {
                let mut sums: __m128 = _mm_set1_ps(0.0);
                let left: i32 = (*weights.offset(ndx as isize)).Left;
                let right: i32 = (*weights.offset(ndx as isize)).Right;
                let weightArray: *const f32 = (*weights.offset(ndx as isize)).Weights;
                let source_buffer: &[__m128] =
                    std::slice::from_raw_parts(source_buffer, (right + 1) as usize);
                /* Accumulate each channel */
                let mut i = left;
                while i <= right {
                    let factor: __m128 = _mm_set1_ps(*weightArray.offset((i - left) as isize));
                    // sums += factor * *source_buffer[i as usize];
                    let mid = _mm_mul_ps(factor, source_buffer[i as usize]);
                    sums = _mm_add_ps(sums, mid);
                    i += 1
                }
                dest_buffer[ndx as usize] = sums;
                ndx += 1
            }
            row += 1
        }
    } else if from_step == 3 as i32 as u32 && to_step == 3 as i32 as u32 {
        let mut row_0: u32 = 0 as i32 as u32;
        while row_0 < row_count {
            let source_buffer_0: *const f32 = (*from).pixels.offset(
                from_row
                    .wrapping_add(row_0)
                    .wrapping_mul((*from).float_stride) as isize,
            );
            let dest_buffer_0: *mut f32 = (*to)
                .pixels
                .offset(to_row.wrapping_add(row_0).wrapping_mul((*to).float_stride) as isize);
            ndx = 0 as i32 as u32;
            while ndx < dest_buffer_count {
                let mut bgr: [f32; 3] = [0.0f32, 0.0f32, 0.0f32];
                let left_0: i32 = (*weights.offset(ndx as isize)).Left;
                let right_0: i32 = (*weights.offset(ndx as isize)).Right;
                let weightArray_0: *const f32 = (*weights.offset(ndx as isize)).Weights;
                let mut i_0: i32 = 0;
                /* Accumulate each channel */
                i_0 = left_0;
                while i_0 <= right_0 {
                    let weight: f32 = *weightArray_0.offset((i_0 - left_0) as isize);
                    bgr[0] += weight
                        * *source_buffer_0.offset((i_0 as u32).wrapping_mul(from_step) as isize);
                    bgr[1] += weight
                        * *source_buffer_0.offset(
                            (i_0 as u32).wrapping_mul(from_step).wrapping_add(1u32) as isize,
                        );
                    bgr[2] += weight
                        * *source_buffer_0.offset(
                            (i_0 as u32).wrapping_mul(from_step).wrapping_add(2u32) as isize,
                        );
                    i_0 += 1
                }
                *dest_buffer_0.offset(ndx.wrapping_mul(to_step) as isize) = bgr[0];
                *dest_buffer_0.offset(ndx.wrapping_mul(to_step).wrapping_add(1u32) as isize) =
                    bgr[1];
                *dest_buffer_0.offset(ndx.wrapping_mul(to_step).wrapping_add(2u32) as isize) =
                    bgr[2];
                ndx = ndx.wrapping_add(1)
            }
            row_0 = row_0.wrapping_add(1)
        }
    } else {
        let mut row_1: u32 = 0 as i32 as u32;
        while row_1 < row_count {
            let source_buffer_1: *const f32 = (*from).pixels.offset(
                from_row
                    .wrapping_add(row_1)
                    .wrapping_mul((*from).float_stride) as isize,
            );
            let dest_buffer_1: *mut f32 = (*to)
                .pixels
                .offset(to_row.wrapping_add(row_1).wrapping_mul((*to).float_stride) as isize);
            ndx = 0 as i32 as u32;
            while ndx < dest_buffer_count {
                avg[0] = 0 as i32 as f32;
                avg[1] = 0 as i32 as f32;
                avg[2] = 0 as i32 as f32;
                avg[3 as i32 as usize] = 0 as i32 as f32;
                let left_1: i32 = (*weights.offset(ndx as isize)).Left;
                let right_1: i32 = (*weights.offset(ndx as isize)).Right;
                let weightArray_1: *const f32 = (*weights.offset(ndx as isize)).Weights;
                /* Accumulate each channel */
                let mut i_1: i32 = left_1;
                while i_1 <= right_1 {
                    let weight_0: f32 = *weightArray_1.offset((i_1 - left_1) as isize);
                    let mut j: u32 = 0 as i32 as u32;
                    while j < min_channels {
                        avg[j as usize] += weight_0
                            * *source_buffer_1.offset(
                                (i_1 as u32).wrapping_mul(from_step).wrapping_add(j) as isize,
                            );
                        j = j.wrapping_add(1)
                    }
                    i_1 += 1
                }
                let mut j_0: u32 = 0 as i32 as u32;
                while j_0 < min_channels {
                    *dest_buffer_1.offset(ndx.wrapping_mul(to_step).wrapping_add(j_0) as isize) =
                        avg[j_0 as usize];
                    j_0 = j_0.wrapping_add(1)
                }
                ndx = ndx.wrapping_add(1)
            }
            row_1 = row_1.wrapping_add(1)
        }
    }
    return true;
}
unsafe extern "C" fn multiply_row(row: *mut f32, length: usize, coefficient: f32) {
    let mut i: usize = 0 as i32 as usize;
    while i < length {
        *row.offset(i as isize) *= coefficient;
        i = i.wrapping_add(1)
    }
}
unsafe extern "C" fn add_row(mutate_row: *mut f32, input_row: *mut f32, length: usize) {
    let mut i: usize = 0 as i32 as usize;
    while i < length {
        *mutate_row.offset(i as isize) += *input_row.offset(i as isize);
        i = i.wrapping_add(1)
    }
}
unsafe extern "C" fn crop(
    c: *mut flow_c,
    b: *mut flow_bitmap_bgra,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> *mut flow_bitmap_bgra {
    if h.wrapping_add(y) > (*b).h || w.wrapping_add(x) > (*b).w {
        flow_context_set_error_get_message_buffer(
            c,
            flow_status_code::Invalid_argument,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            632 as i32,
            (*::std::mem::transmute::<&[u8; 5], &[libc::c_char; 5]>(b"crop\x00")).as_ptr(),
        );
        return NULL as *mut flow_bitmap_bgra;
    }
    let mut cropped_canvas: *mut flow_bitmap_bgra =
        flow_bitmap_bgra_create_header(c, w as i32, h as i32);
    let bpp: u32 = flow_pixel_format_bytes_per_pixel((*b).fmt);
    if cropped_canvas.is_null() {
        flow_context_add_to_callstack(
            c,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            640 as i32,
            (*::std::mem::transmute::<&[u8; 5], &[libc::c_char; 5]>(b"crop\x00")).as_ptr(),
        );
        return NULL as *mut flow_bitmap_bgra;
    }
    (*cropped_canvas).fmt = (*b).fmt;
    memcpy(
        &mut *(*cropped_canvas).matte_color.as_mut_ptr().offset(0) as *mut u8 as *mut libc::c_void,
        &mut *(*b).matte_color.as_mut_ptr().offset(0) as *mut u8 as *const libc::c_void,
        ::std::mem::size_of::<[u8; 4]>() as u64,
    );
    (*cropped_canvas).compositing_mode = (*b).compositing_mode;
    (*cropped_canvas).pixels = (*b)
        .pixels
        .offset(y.wrapping_mul((*b).stride) as isize)
        .offset(x.wrapping_mul(bpp) as isize);
    (*cropped_canvas).stride = (*b).stride;
    return cropped_canvas;
}

/// Note: Rust version of `FLOW_error` takes the name of the caller as its third parameter since
/// there does not seem to be a way to get the name of the current or calling function in Rust.
fn FLOW_error(
    context: *mut flow_context,
    status_code: flow_status_code,
    caller: &str,
) -> *mut libc::c_char {
    let file = CString::new(file!()).unwrap().as_ptr();
    let func = CString::new(caller).unwrap().as_ptr();
    unsafe {
        flow_context_set_error_get_message_buffer(
            context,
            status_code,
            file as *const libc::c_char,
            line!() as i32,
            func as *const libc::c_char, // was __func__ in C macro
        )
    }
}

#[no_mangle]
pub unsafe extern "C" fn flow_node_execute_scale2d_render1d(
    c: *mut flow_c,
    input: *mut flow_bitmap_bgra,
    uncropped_canvas: *mut flow_bitmap_bgra,
    info: *mut flow_nodeinfo_scale2d_render_to_canvas1d,
) -> bool {
    if (*info).h.wrapping_add((*info).y) > (*uncropped_canvas).h
        || (*info).w.wrapping_add((*info).x) > (*uncropped_canvas).w
    {
        FLOW_error(
            c,
            flow_status_code::Invalid_argument,
            "flow_node_execute_scale2d_render1d",
        );
        return false;
    }
    let cropped_canvas: *mut flow_bitmap_bgra = if (*info).x == 0
        && (*info).y == 0
        && (*info).w == (*uncropped_canvas).w
        && (*info).h == (*uncropped_canvas).h
    {
        uncropped_canvas
    } else {
        crop(
            c,
            uncropped_canvas,
            (*info).x,
            (*info).y,
            (*info).w,
            (*info).h,
        )
    };
    if cropped_canvas.is_null() {
        flow_context_add_to_callstack(
            c,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            665 as i32,
            (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                b"flow_node_execute_scale2d_render1d\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    let input_fmt: flow_pixel_format = flow_effective_pixel_format(input);
    let canvas_fmt: flow_pixel_format = flow_effective_pixel_format(cropped_canvas);
    if input_fmt as u32 != flow_bgra32 as i32 as u32 && input_fmt as u32 != flow_bgr32 as i32 as u32
    {
        FLOW_error(
            c,
            flow_status_code::Not_implemented,
            "flow_node_execute_scale2d_render1d",
        );
        return false;
    }
    if canvas_fmt as u32 != flow_bgra32 as i32 as u32
        && canvas_fmt as u32 != flow_bgr32 as i32 as u32
    {
        FLOW_error(
            c,
            flow_status_code::Not_implemented,
            "flow_node_execute_scale2d_render1d",
        );
        return false;
    }
    let mut colorcontext: flow_colorcontext_info = flow_colorcontext_info {
        byte_to_float: [0.; 256],
        floatspace: flow_working_floatspace_srgb,
        apply_srgb: false,
        apply_gamma: false,
        gamma: 0.,
        gamma_inverse: 0.,
    };
    flow_colorcontext_init(
        c,
        &mut colorcontext,
        (*info).scale_in_colorspace,
        0 as i32 as f32,
        0 as i32 as f32,
        0 as i32 as f32,
    );
    // Use details as a parent structure to ensure everything gets freed
    let mut details: *mut flow_interpolation_details =
        flow_interpolation_details_create_from(c, (*info).interpolation_filter);
    if details.is_null() {
        flow_context_add_to_callstack(
            c,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            686 as i32,
            (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                b"flow_node_execute_scale2d_render1d\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    (*details).sharpen_percent_goal = (*info).sharpen_percent_goal;
    let mut contrib_v: *mut flow_interpolation_line_contributions =
        NULL as *mut flow_interpolation_line_contributions;
    let mut contrib_h: *mut flow_interpolation_line_contributions =
        NULL as *mut flow_interpolation_line_contributions;
    flow_context_profiler_start(
        c,
        b"contributions_calc\x00" as *const u8 as *const libc::c_char,
        0 as i32 != 0,
    );
    contrib_v = flow_interpolation_line_contributions_create(c, (*info).h, (*input).h, details);
    if contrib_v.is_null()
        || !flow_set_owner(
            c,
            contrib_v as *mut libc::c_void,
            details as *mut libc::c_void,
        )
    {
        flow_destroy(
            c,
            details as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            697 as i32,
        );
        flow_context_add_to_callstack(
            c,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            698 as i32,
            (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                b"flow_node_execute_scale2d_render1d\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    contrib_h = flow_interpolation_line_contributions_create(c, (*info).w, (*input).w, details);
    if contrib_h.is_null()
        || !flow_set_owner(
            c,
            contrib_h as *mut libc::c_void,
            details as *mut libc::c_void,
        )
    {
        flow_destroy(
            c,
            details as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            702 as i32,
        );
        flow_context_add_to_callstack(
            c,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            703 as i32,
            (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                b"flow_node_execute_scale2d_render1d\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    flow_context_profiler_stop(
        c,
        b"contributions_calc\x00" as *const u8 as *const libc::c_char,
        1 as i32 != 0,
        0 as i32 != 0,
    );
    flow_context_profiler_start(
        c,
        b"create_bitmap_float (buffers)\x00" as *const u8 as *const libc::c_char,
        0 as i32 != 0,
    );
    let mut source_buf: *mut flow_bitmap_float =
        flow_bitmap_float_create_header(c, (*input).w as i32, 1 as i32, 4 as i32);
    if source_buf.is_null()
        || !flow_set_owner(
            c,
            source_buf as *mut libc::c_void,
            details as *mut libc::c_void,
        )
    {
        flow_destroy(
            c,
            details as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            711 as i32,
        );
        flow_context_add_to_callstack(
            c,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            712 as i32,
            (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                b"flow_node_execute_scale2d_render1d\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    let mut dest_buf: *mut flow_bitmap_float =
        flow_bitmap_float_create(c, (*info).w as i32, 1 as i32, 4 as i32, true);
    if dest_buf.is_null()
        || !flow_set_owner(
            c,
            dest_buf as *mut libc::c_void,
            details as *mut libc::c_void,
        )
    {
        flow_destroy(
            c,
            details as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            716 as i32,
        );
        flow_context_add_to_callstack(
            c,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            717 as i32,
            (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                b"flow_node_execute_scale2d_render1d\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    (*source_buf).alpha_meaningful = input_fmt as u32 == flow_bgra32 as i32 as u32;
    (*dest_buf).alpha_meaningful = (*source_buf).alpha_meaningful;
    (*source_buf).alpha_premultiplied = (*source_buf).channels == 4 as i32 as u32;
    (*dest_buf).alpha_premultiplied = (*source_buf).alpha_premultiplied;
    flow_context_profiler_stop(
        c,
        b"create_bitmap_float (buffers)\x00" as *const u8 as *const libc::c_char,
        1 as i32 != 0,
        0 as i32 != 0,
    );
    // Determine how many rows we need to buffer
    let mut max_input_rows: i32 = 0 as i32;
    let mut i: u32 = 0 as i32 as u32;
    while i < (*contrib_v).LineLength {
        let inputs: i32 = (*(*contrib_v).ContribRow.offset(i as isize)).Right
            - (*(*contrib_v).ContribRow.offset(i as isize)).Left
            + 1 as i32;
        if inputs > max_input_rows {
            max_input_rows = inputs
        }
        i = i.wrapping_add(1)
    }
    // Allocate space
    let row_floats: usize = (4u32).wrapping_mul((*input).w) as usize;
    let buf: *mut f32 = flow_context_malloc(
        c,
        ::std::mem::size_of::<f32>()
            .wrapping_mul(row_floats)
            .wrapping_mul((max_input_rows + 1) as usize),
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        details as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        737 as i32,
    ) as *mut f32;
    let rows: *mut *mut f32 = flow_context_malloc(
        c,
        (::std::mem::size_of::<*mut f32>()).wrapping_mul(max_input_rows as usize),
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        details as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        738 as i32,
    ) as *mut *mut f32;
    let row_coefficients: *mut f32 = flow_context_malloc(
        c,
        ::std::mem::size_of::<f32>().wrapping_mul(max_input_rows as usize),
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        details as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        739 as i32,
    ) as *mut f32;
    let row_indexes: *mut i32 = flow_context_malloc(
        c,
        ::std::mem::size_of::<i32>().wrapping_mul(max_input_rows as usize),
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        details as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        740 as i32,
    ) as *mut i32;
    if buf.is_null() || rows.is_null() || row_coefficients.is_null() || row_indexes.is_null() {
        flow_destroy(
            c,
            details as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            742 as i32,
        );
        flow_context_add_to_callstack(
            c,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            743 as i32,
            (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                b"flow_node_execute_scale2d_render1d\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    let output_address: *mut f32 =
        &mut *buf.offset(row_floats.wrapping_mul(max_input_rows as usize) as isize) as *mut f32;
    let mut i_0: i32 = 0 as i32;
    while i_0 < max_input_rows {
        let ref mut fresh8 = *rows.offset(i_0 as isize);
        *fresh8 = &mut *buf
            .offset((4u32).wrapping_mul((*input).w).wrapping_mul(i_0 as u32) as isize)
            as *mut f32;
        *row_coefficients.offset(i_0 as isize) = 1 as i32 as f32;
        *row_indexes.offset(i_0 as isize) = -(1 as i32);
        i_0 += 1
    }
    let mut out_row: u32 = 0 as i32 as u32;
    while out_row < (*cropped_canvas).h {
        let contrib: flow_interpolation_pixel_contributions =
            *(*contrib_v).ContribRow.offset(out_row as isize);

        // Clear output row
        ::libc::memset(
            output_address as *mut libc::c_void,
            0 as i32,
            ::std::mem::size_of::<f32>().wrapping_mul(row_floats),
        );
        let mut input_row: i32 = contrib.Left;
        while input_row <= contrib.Right {
            // Try to find row in buffer if already loaded
            let mut loaded: bool = false;
            let mut active_buf_ix: i32 = -(1 as i32);
            let mut buf_row: i32 = 0 as i32;
            while buf_row < max_input_rows {
                if *row_indexes.offset(buf_row as isize) == input_row {
                    active_buf_ix = buf_row;
                    loaded = true;
                    break;
                } else {
                    buf_row += 1
                }
            }
            // Not loaded?
            if !loaded {
                let mut buf_row_0: i32 = 0 as i32; // Buffer too small!
                while buf_row_0 < max_input_rows {
                    if *row_indexes.offset(buf_row_0 as isize) < contrib.Left {
                        active_buf_ix = buf_row_0;
                        loaded = false;
                        break;
                    } else {
                        buf_row_0 += 1
                    }
                }
            }
            if active_buf_ix < 0 as i32 {
                flow_destroy(
                    c,
                    details as *mut libc::c_void,
                    b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                    779 as i32,
                );
                flow_context_set_error_get_message_buffer(
                    c,
                    flow_status_code::Invalid_internal_state,
                    b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                    780 as i32,
                    (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                        b"flow_node_execute_scale2d_render1d\x00",
                    ))
                    .as_ptr(),
                );
                return false;
            }
            if !loaded {
                // Load row
                (*source_buf).pixels = *rows.offset(active_buf_ix as isize);
                flow_context_profiler_start(
                    c,
                    b"convert_srgb_to_linear\x00" as *const u8 as *const libc::c_char,
                    0 as i32 != 0,
                );
                if !flow_bitmap_float_convert_srgb_to_linear(
                    c,
                    &mut colorcontext,
                    input,
                    input_row as u32,
                    source_buf,
                    0 as i32 as u32,
                    1 as i32 as u32,
                ) {
                    flow_destroy(
                        c,
                        details as *mut libc::c_void,
                        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                        789 as i32,
                    );
                    flow_context_add_to_callstack(
                        c,
                        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                        790 as i32,
                        (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                            b"flow_node_execute_scale2d_render1d\x00",
                        ))
                        .as_ptr(),
                    );
                    return false;
                }
                flow_context_profiler_stop(
                    c,
                    b"convert_srgb_to_linear\x00" as *const u8 as *const libc::c_char,
                    1 as i32 != 0,
                    0 as i32 != 0,
                );
                *row_coefficients.offset(active_buf_ix as isize) = 1 as i32 as f32;
                *row_indexes.offset(active_buf_ix as isize) = input_row;
                loaded = true
            }
            let weight: f32 = *contrib.Weights.offset((input_row - contrib.Left) as isize);
            if fabs(weight as f64) > 0.00000002f64 {
                // Apply coefficient, update tracking
                let delta_coefficient: f32 =
                    weight / *row_coefficients.offset(active_buf_ix as isize);
                multiply_row(
                    *rows.offset(active_buf_ix as isize),
                    row_floats,
                    delta_coefficient,
                );
                *row_coefficients.offset(active_buf_ix as isize) = weight;
                // Add row
                add_row(
                    output_address,
                    *rows.offset(active_buf_ix as isize),
                    row_floats,
                );
            }
            input_row += 1
        }
        // The container now points to the row which has been vertically scaled
        (*source_buf).pixels = output_address;
        // Now scale horizontally!
        flow_context_profiler_start(
            c,
            b"ScaleBgraFloatRows\x00" as *const u8 as *const libc::c_char,
            0 as i32 != 0,
        );
        if !flow_bitmap_float_scale_rows(
            c,
            source_buf,
            0 as i32 as u32,
            dest_buf,
            0 as i32 as u32,
            1 as i32 as u32,
            (*contrib_h).ContribRow,
        ) {
            flow_destroy(
                c,
                details as *mut libc::c_void,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                816 as i32,
            );
            flow_context_add_to_callstack(
                c,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                817 as i32,
                (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                    b"flow_node_execute_scale2d_render1d\x00",
                ))
                .as_ptr(),
            );
            return false;
        }
        flow_context_profiler_stop(
            c,
            b"ScaleBgraFloatRows\x00" as *const u8 as *const libc::c_char,
            1 as i32 != 0,
            0 as i32 != 0,
        );
        if !flow_bitmap_float_composite_linear_over_srgb(
            c,
            &mut colorcontext,
            dest_buf,
            0 as i32 as u32,
            cropped_canvas,
            out_row,
            1 as i32 as u32,
            false,
        ) {
            flow_destroy(
                c,
                details as *mut libc::c_void,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                822 as i32,
            );
            flow_context_add_to_callstack(
                c,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                823 as i32,
                (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                    b"flow_node_execute_scale2d_render1d\x00",
                ))
                .as_ptr(),
            );
            return false;
        }
        out_row = out_row.wrapping_add(1)
    }
    flow_destroy(
        c,
        if cropped_canvas == uncropped_canvas {
            0 as *mut flow_bitmap_bgra
        } else {
            cropped_canvas
        } as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        826 as i32,
    );
    flow_destroy(
        c,
        details as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        827 as i32,
    );
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_convolution_kernel_create(
    context: *mut flow_c,
    radius: u32,
) -> *mut flow_convolution_kernel {
    let mut k: *mut flow_convolution_kernel = flow_context_calloc(
        context,
        1 as i32 as usize,
        ::std::mem::size_of::<flow_convolution_kernel>(),
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        context as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        832 as i32,
    ) as *mut flow_convolution_kernel;
    // For the actual array;
    let a: *mut f32 = flow_context_calloc(
        context,
        radius.wrapping_mul(2u32).wrapping_add(1u32) as usize,
        ::std::mem::size_of::<f32>(),
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        context as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        834 as i32,
    ) as *mut f32;
    // we assume a maximum of 4 channels are going to need buffering during convolution
    let buf: *mut f32 = flow_context_malloc(
        context,
        (radius as usize)
            .wrapping_add(2)
            .wrapping_mul(4)
            .wrapping_mul(::std::mem::size_of::<f32>()),
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        context as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        836 as i32,
    ) as *mut f32; // nothing to do here, zeroes are as normalized as you can get ;)
    if k.is_null() || a.is_null() || buf.is_null() {
        flow_deprecated_free(
            context,
            k as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            839 as i32,
        );
        flow_deprecated_free(
            context,
            a as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            840 as i32,
        );
        flow_deprecated_free(
            context,
            buf as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            841 as i32,
        );
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Out_of_memory,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            842 as i32,
            (*::std::mem::transmute::<&[u8; 31], &[libc::c_char; 31]>(
                b"flow_convolution_kernel_create\x00",
            ))
            .as_ptr(),
        );
        return NULL as *mut flow_convolution_kernel;
    }
    (*k).kernel = a;
    (*k).width = radius.wrapping_mul(2u32).wrapping_add(1u32);
    (*k).buffer = buf;
    (*k).radius = radius;
    return k;
}
#[no_mangle]
pub unsafe extern "C" fn flow_convolution_kernel_destroy(
    context: *mut flow_c,
    mut kernel: *mut flow_convolution_kernel,
) {
    if !kernel.is_null() {
        flow_deprecated_free(
            context,
            (*kernel).kernel as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            854 as i32,
        );
        flow_deprecated_free(
            context,
            (*kernel).buffer as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            855 as i32,
        );
        (*kernel).kernel = NULL as *mut f32;
        (*kernel).buffer = NULL as *mut f32
    }
    flow_deprecated_free(
        context,
        kernel as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        859 as i32,
    );
}
#[no_mangle]
pub unsafe extern "C" fn flow_convolution_kernel_create_gaussian(
    context: *mut flow_c,
    stdDev: f64,
    radius: u32,
) -> *mut flow_convolution_kernel {
    let k: *mut flow_convolution_kernel = flow_convolution_kernel_create(context, radius);
    if !k.is_null() {
        let mut i: u32 = 0 as i32 as u32;
        while i < (*k).width {
            *(*k).kernel.offset(i as isize) =
                ir_gaussian((radius as i32 - i as i32).abs() as f64, stdDev) as f32;
            i = i.wrapping_add(1)
        }
    }
    return k;
}
#[no_mangle]
pub unsafe extern "C" fn flow_convolution_kernel_sum(kernel: *mut flow_convolution_kernel) -> f64 {
    let mut sum: f64 = 0 as i32 as f64;
    let mut i: u32 = 0 as i32 as u32;
    while i < (*kernel).width {
        sum += *(*kernel).kernel.offset(i as isize) as f64;
        i = i.wrapping_add(1)
    }
    return sum;
}
#[no_mangle]
pub unsafe extern "C" fn flow_convolution_kernel_normalize(
    kernel: *mut flow_convolution_kernel,
    desiredSum: f32,
) {
    let sum: f64 = flow_convolution_kernel_sum(kernel);
    if sum == 0 as i32 as f64 {
        return;
    }
    let factor: f32 = (desiredSum as f64 / sum) as f32;
    let mut i: u32 = 0 as i32 as u32;
    while i < (*kernel).width {
        *(*kernel).kernel.offset(i as isize) *= factor;
        i = i.wrapping_add(1)
    }
}
#[no_mangle]
pub unsafe extern "C" fn flow_convolution_kernel_create_gaussian_normalized(
    context: *mut flow_c,
    stdDev: f64,
    radius: u32,
) -> *mut flow_convolution_kernel {
    let kernel: *mut flow_convolution_kernel =
        flow_convolution_kernel_create_gaussian(context, stdDev, radius);
    if !kernel.is_null() {
        flow_convolution_kernel_normalize(kernel, 1 as i32 as f32);
    }
    return kernel;
}
#[no_mangle]
pub unsafe extern "C" fn flow_convolution_kernel_create_gaussian_sharpen(
    context: *mut flow_c,
    stdDev: f64,
    radius: u32,
) -> *mut flow_convolution_kernel {
    let kernel: *mut flow_convolution_kernel =
        flow_convolution_kernel_create_gaussian(context, stdDev, radius);
    if !kernel.is_null() {
        let sum: f64 = flow_convolution_kernel_sum(kernel);
        let mut i: u32 = 0 as i32 as u32;
        while i < (*kernel).width {
            if i == radius {
                *(*kernel).kernel.offset(i as isize) =
                    (2 as i32 as f64 * sum - *(*kernel).kernel.offset(i as isize) as f64) as f32
            } else {
                *(*kernel).kernel.offset(i as isize) *= -(1 as i32) as f32
            }
            i = i.wrapping_add(1)
        }
        flow_convolution_kernel_normalize(kernel, 1 as i32 as f32);
    }
    return kernel;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_convolve_rows(
    _context: *mut flow_c,
    buf: *mut flow_bitmap_float,
    kernel: *mut flow_convolution_kernel,
    convolve_channels: u32,
    from_row: u32,
    row_count: i32,
) -> bool {
    let radius: u32 = (*kernel).radius;
    let threshold_min: f32 = (*kernel).threshold_min_change;
    let threshold_max: f32 = (*kernel).threshold_max_change;
    // Do nothing unless the image is at least half as wide as the kernel.
    if (*buf).w < radius.wrapping_add(1u32) {
        return true;
    }
    let buffer_count: u32 = radius.wrapping_add(1u32);
    let w: u32 = (*buf).w;
    let int_w: i32 = (*buf).w as i32;
    let step: u32 = (*buf).channels;
    let until_row: u32 = if row_count < 0 as i32 {
        (*buf).h
    } else {
        from_row.wrapping_add(row_count as u32)
    };
    let ch_used: u32 = convolve_channels;
    let buffer: *mut f32 = (*kernel).buffer;
    let avg: *mut f32 = &mut *(*kernel)
        .buffer
        .offset(buffer_count.wrapping_mul(ch_used) as isize) as *mut f32;
    let kern: *const f32 = (*kernel).kernel;
    let wrap_mode: i32 = 0 as i32;
    let mut row: u32 = from_row;
    while row < until_row {
        let source_buffer: *mut f32 = &mut *(*buf)
            .pixels
            .offset(row.wrapping_mul((*buf).float_stride) as isize)
            as *mut f32;
        let mut circular_idx: i32 = 0 as i32;
        let mut ndx: u32 = 0 as i32 as u32;
        while ndx < w.wrapping_add(buffer_count) {
            // Flush old value
            if ndx >= buffer_count {
                memcpy(
                    &mut *source_buffer
                        .offset(ndx.wrapping_sub(buffer_count).wrapping_mul(step) as isize)
                        as *mut f32 as *mut libc::c_void,
                    &mut *buffer.offset((circular_idx as u32).wrapping_mul(ch_used) as isize)
                        as *mut f32 as *const libc::c_void,
                    (ch_used as u64).wrapping_mul(::std::mem::size_of::<f32>() as u64),
                );
            }
            // Calculate and enqueue new value
            if ndx < w {
                let left: i32 = ndx.wrapping_sub(radius) as i32;
                let right: i32 = ndx.wrapping_add(radius) as i32;
                let mut i: i32 = 0;
                memset(
                    avg as *mut libc::c_void,
                    0 as i32,
                    (::std::mem::size_of::<f32>() as u64).wrapping_mul(ch_used as u64),
                );
                if left < 0 as i32 || right >= w as i32 {
                    if wrap_mode == 0 as i32 {
                        // Only sample what's present, and fix the average later.
                        let mut total_weight: f32 = 0 as i32 as f32;
                        /* Accumulate each channel */
                        i = left;
                        while i <= right {
                            if i > 0 as i32 && i < int_w {
                                let weight: f32 = *kern.offset((i - left) as isize);
                                total_weight += weight;
                                let mut j: u32 = 0 as i32 as u32;
                                while j < ch_used {
                                    *avg.offset(j as isize) += weight
                                        * *source_buffer
                                            .offset((i as u32).wrapping_mul(step).wrapping_add(j)
                                                as isize);
                                    j = j.wrapping_add(1)
                                }
                            }
                            i += 1
                        }
                        let mut j_0: u32 = 0 as i32 as u32;
                        while j_0 < ch_used {
                            *avg.offset(j_0 as isize) = *avg.offset(j_0 as isize) / total_weight;
                            j_0 = j_0.wrapping_add(1)
                        }
                    } else if wrap_mode == 1 as i32 {
                        // Extend last pixel to be used for all missing inputs
                        /* Accumulate each channel */
                        i = left;
                        while i <= right {
                            let weight_0: f32 = *kern.offset((i - left) as isize);
                            let ix: u32 = if i > int_w - 1 as i32 {
                                (int_w) - 1 as i32
                            } else if i < 0 as i32 {
                                0 as i32
                            } else {
                                i
                            } as u32;
                            let mut j_1: u32 = 0 as i32 as u32;
                            while j_1 < ch_used {
                                *avg.offset(j_1 as isize) += weight_0
                                    * *source_buffer
                                        .offset(ix.wrapping_mul(step).wrapping_add(j_1) as isize);
                                j_1 = j_1.wrapping_add(1)
                            }
                            i += 1
                        }
                    }
                } else {
                    /* Accumulate each channel */
                    i = left;
                    while i <= right {
                        let weight_1: f32 = *kern.offset((i - left) as isize);
                        let mut j_2: u32 = 0 as i32 as u32;
                        while j_2 < ch_used {
                            *avg.offset(j_2 as isize) += weight_1
                                * *source_buffer.offset(
                                    (i as u32).wrapping_mul(step).wrapping_add(j_2) as isize,
                                );
                            j_2 = j_2.wrapping_add(1)
                        }
                        i += 1
                    }
                }
                // Enqueue difference
                memcpy(
                    &mut *buffer.offset((circular_idx as u32).wrapping_mul(ch_used) as isize)
                        as *mut f32 as *mut libc::c_void,
                    avg as *const libc::c_void,
                    (ch_used as u64).wrapping_mul(::std::mem::size_of::<f32>() as u64),
                );
                if threshold_min > 0 as i32 as f32 || threshold_max > 0 as i32 as f32 {
                    let mut change: f32 = 0 as i32 as f32;
                    let mut j_3: u32 = 0 as i32 as u32;
                    while j_3 < ch_used {
                        change += fabs(
                            (*source_buffer
                                .offset(ndx.wrapping_mul(step).wrapping_add(j_3) as isize)
                                - *avg.offset(j_3 as isize)) as f64,
                        ) as f32;
                        j_3 = j_3.wrapping_add(1)
                    }
                    if change < threshold_min || change > threshold_max {
                        memcpy(
                            &mut *buffer
                                .offset((circular_idx as u32).wrapping_mul(ch_used) as isize)
                                as *mut f32 as *mut libc::c_void,
                            &mut *source_buffer.offset(ndx.wrapping_mul(step) as isize) as *mut f32
                                as *const libc::c_void,
                            (ch_used as u64).wrapping_mul(::std::mem::size_of::<f32>() as u64),
                        );
                    }
                }
            }
            circular_idx = ((circular_idx + 1 as i32) as u32).wrapping_rem(buffer_count) as i32;
            ndx = ndx.wrapping_add(1)
        }
        row = row.wrapping_add(1)
    }
    return true;
}
unsafe extern "C" fn BitmapFloat_boxblur_rows(
    _context: *mut flow_c,
    image: *mut flow_bitmap_float,
    radius: u32,
    passes: u32,
    convolve_channels: u32,
    work_buffer: *mut f32,
    from_row: u32,
    row_count: i32,
) -> bool {
    let buffer_count: u32 = radius.wrapping_add(1u32);
    let w: u32 = (*image).w;
    let step: u32 = (*image).channels;
    let until_row: u32 = if row_count < 0 as i32 {
        (*image).h
    } else {
        from_row.wrapping_add(row_count as u32)
    };
    let ch_used: u32 = (*image).channels;
    let buffer: *mut f32 = work_buffer;
    let std_count: u32 = radius.wrapping_mul(2u32).wrapping_add(1u32);
    let std_factor: f32 = 1.0f32 / std_count as f32;
    let mut row: u32 = from_row;
    while row < until_row {
        let source_buffer: *mut f32 = &mut *(*image)
            .pixels
            .offset(row.wrapping_mul((*image).float_stride) as isize)
            as *mut f32;
        let mut pass_index: u32 = 0 as i32 as u32;
        while pass_index < passes {
            let mut circular_idx: i32 = 0 as i32;
            let mut sum: [f32; 4] = [
                0 as i32 as f32,
                0 as i32 as f32,
                0 as i32 as f32,
                0 as i32 as f32,
            ];
            let mut count: u32 = 0 as i32 as u32;
            let mut ndx: u32 = 0 as i32 as u32;
            while ndx < radius {
                let mut ch: u32 = 0 as i32 as u32;
                while ch < convolve_channels {
                    sum[ch as usize] +=
                        *source_buffer.offset(ndx.wrapping_mul(step).wrapping_add(ch) as isize);
                    ch = ch.wrapping_add(1)
                }
                count = count.wrapping_add(1);
                ndx = ndx.wrapping_add(1)
            }
            let mut ndx_0: u32 = 0 as i32 as u32;
            while ndx_0 < w.wrapping_add(buffer_count) {
                // Pixels
                if ndx_0 >= buffer_count {
                    // same as ndx > radius
                    // Remove trailing item from average
                    let mut ch_0: u32 = 0 as i32 as u32;
                    while ch_0 < convolve_channels {
                        sum[ch_0 as usize] -= *source_buffer.offset(
                            ndx_0
                                .wrapping_sub(radius)
                                .wrapping_sub(1u32)
                                .wrapping_mul(step)
                                .wrapping_add(ch_0) as isize,
                        );
                        ch_0 = ch_0.wrapping_add(1)
                    }
                    count = count.wrapping_sub(1);
                    // Flush old value
                    memcpy(
                        &mut *source_buffer
                            .offset(ndx_0.wrapping_sub(buffer_count).wrapping_mul(step) as isize)
                            as *mut f32 as *mut libc::c_void,
                        &mut *buffer.offset((circular_idx as u32).wrapping_mul(ch_used) as isize)
                            as *mut f32 as *const libc::c_void,
                        (ch_used as u64).wrapping_mul(::std::mem::size_of::<f32>() as u64),
                    );
                }
                // Calculate and enqueue new value
                if ndx_0 < w {
                    if ndx_0 < w.wrapping_sub(radius) {
                        let mut ch_1: u32 = 0 as i32 as u32;
                        while ch_1 < convolve_channels {
                            sum[ch_1 as usize] += *source_buffer.offset(
                                ndx_0
                                    .wrapping_add(radius)
                                    .wrapping_mul(step)
                                    .wrapping_add(ch_1) as isize,
                            );
                            ch_1 = ch_1.wrapping_add(1)
                        }
                        count = count.wrapping_add(1)
                    }
                    // Enqueue averaged value
                    if count != std_count {
                        let mut ch_2: u32 = 0 as i32 as u32;
                        while ch_2 < convolve_channels {
                            *buffer.offset(
                                (circular_idx as u32)
                                    .wrapping_mul(ch_used)
                                    .wrapping_add(ch_2) as isize,
                            ) = sum[ch_2 as usize] / count as f32;
                            ch_2 = ch_2.wrapping_add(1)
                            // Recompute factor
                        }
                    } else {
                        let mut ch_3: u32 = 0 as i32 as u32;
                        while ch_3 < convolve_channels {
                            *buffer.offset(
                                (circular_idx as u32)
                                    .wrapping_mul(ch_used)
                                    .wrapping_add(ch_3) as isize,
                            ) = sum[ch_3 as usize] * std_factor;
                            ch_3 = ch_3.wrapping_add(1)
                        }
                    }
                }
                circular_idx = ((circular_idx + 1 as i32) as u32).wrapping_rem(buffer_count) as i32;
                ndx_0 = ndx_0.wrapping_add(1)
            }
            pass_index = pass_index.wrapping_add(1)
        }
        row = row.wrapping_add(1)
    }
    return true;
}
unsafe extern "C" fn BitmapFloat_boxblur_misaligned_rows(
    context: *mut flow_c,
    image: *mut flow_bitmap_float,
    radius: u32,
    align: i32,
    convolve_channels: u32,
    work_buffer: *mut f32,
    from_row: u32,
    row_count: i32,
) -> bool {
    if align != 1 as i32 && align != -(1 as i32) {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1088 as i32,
            (*::std::mem::transmute::<&[u8; 36], &[libc::c_char; 36]>(
                b"BitmapFloat_boxblur_misaligned_rows\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    let buffer_count: u32 = radius.wrapping_add(2u32);
    let w: u32 = (*image).w;
    let step: u32 = (*image).channels;
    let until_row: u32 = if row_count < 0 as i32 {
        (*image).h
    } else {
        from_row.wrapping_add(row_count as u32)
    };
    let ch_used: u32 = (*image).channels;
    let buffer: *mut f32 = work_buffer;
    let write_offset: u32 = if align == -(1 as i32) {
        0 as i32
    } else {
        1 as i32
    } as u32;
    let mut row: u32 = from_row;
    while row < until_row {
        let source_buffer: *mut f32 = &mut *(*image)
            .pixels
            .offset(row.wrapping_mul((*image).float_stride) as isize)
            as *mut f32;
        let mut circular_idx: i32 = 0 as i32;
        let mut sum: [f32; 4] = [
            0 as i32 as f32,
            0 as i32 as f32,
            0 as i32 as f32,
            0 as i32 as f32,
        ];
        let mut count: f32 = 0 as i32 as f32;
        let mut ndx: u32 = 0 as i32 as u32;
        while ndx < radius {
            let factor: f32 = if ndx == radius.wrapping_sub(1u32) {
                0.5f32
            } else {
                1 as i32 as f32
            };
            let mut ch: u32 = 0 as i32 as u32;
            while ch < convolve_channels {
                sum[ch as usize] += *source_buffer
                    .offset(ndx.wrapping_mul(step).wrapping_add(ch) as isize)
                    * factor;
                ch = ch.wrapping_add(1)
            }
            count += factor;
            ndx = ndx.wrapping_add(1)
        }
        let mut ndx_0: u32 = 0 as i32 as u32;
        while ndx_0 < w.wrapping_add(buffer_count).wrapping_sub(write_offset) {
            // Pixels
            // Calculate new value
            if ndx_0 < w {
                if ndx_0 < w.wrapping_sub(radius) {
                    let mut ch_0: u32 = 0 as i32 as u32;
                    while ch_0 < convolve_channels {
                        sum[ch_0 as usize] += *source_buffer.offset(
                            ndx_0
                                .wrapping_add(radius)
                                .wrapping_mul(step)
                                .wrapping_add(ch_0) as isize,
                        ) * 0.5f32;
                        ch_0 = ch_0.wrapping_add(1)
                    }
                    count += 0.5f32
                }
                if ndx_0 < w.wrapping_sub(radius).wrapping_add(1u32) {
                    let mut ch_1: u32 = 0 as i32 as u32;
                    while ch_1 < convolve_channels {
                        sum[ch_1 as usize] += *source_buffer.offset(
                            ndx_0
                                .wrapping_sub(1u32)
                                .wrapping_add(radius)
                                .wrapping_mul(step)
                                .wrapping_add(ch_1) as isize,
                        ) * 0.5f32;
                        ch_1 = ch_1.wrapping_add(1)
                    }
                    count += 0.5f32
                }
                // Remove trailing items from average
                if ndx_0 >= radius {
                    let mut ch_2: u32 = 0 as i32 as u32;
                    while ch_2 < convolve_channels {
                        sum[ch_2 as usize] -= *source_buffer.offset(
                            ndx_0
                                .wrapping_sub(radius)
                                .wrapping_mul(step)
                                .wrapping_add(ch_2) as isize,
                        ) * 0.5f32;
                        ch_2 = ch_2.wrapping_add(1)
                    }
                    count -= 0.5f32
                }
                if ndx_0 >= radius.wrapping_add(1u32) {
                    let mut ch_3: u32 = 0 as i32 as u32;
                    while ch_3 < convolve_channels {
                        sum[ch_3 as usize] -= *source_buffer.offset(
                            ndx_0
                                .wrapping_sub(1u32)
                                .wrapping_sub(radius)
                                .wrapping_mul(step)
                                .wrapping_add(ch_3) as isize,
                        ) * 0.5f32;
                        ch_3 = ch_3.wrapping_add(1)
                    }
                    count -= 0.5f32
                }
            }
            // Flush old value
            if ndx_0 >= buffer_count.wrapping_sub(write_offset) {
                memcpy(
                    &mut *source_buffer.offset(
                        ndx_0
                            .wrapping_add(write_offset)
                            .wrapping_sub(buffer_count)
                            .wrapping_mul(step) as isize,
                    ) as *mut f32 as *mut libc::c_void,
                    &mut *buffer.offset((circular_idx as u32).wrapping_mul(ch_used) as isize)
                        as *mut f32 as *const libc::c_void,
                    (ch_used as u64).wrapping_mul(::std::mem::size_of::<f32>() as u64),
                );
            }
            // enqueue new value
            if ndx_0 < w {
                let mut ch_4: u32 = 0 as i32 as u32; // Never exceed half the size of the buffer.
                while ch_4 < convolve_channels {
                    *buffer.offset(
                        (circular_idx as u32)
                            .wrapping_mul(ch_used)
                            .wrapping_add(ch_4) as isize,
                    ) = sum[ch_4 as usize] / count;
                    ch_4 = ch_4.wrapping_add(1)
                }
            }
            circular_idx = ((circular_idx + 1 as i32) as u32).wrapping_rem(buffer_count) as i32;
            ndx_0 = ndx_0.wrapping_add(1)
        }
        row = row.wrapping_add(1)
    }
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_approx_gaussian_calculate_d(
    sigma: f32,
    bitmap_width: u32,
) -> u32 {
    let mut d: u32 = (1.8799712059732503768118239636082839397552400554574537f32 * sigma + 0.5f32)
        .floor() as i32 as u32;
    d = d.min(bitmap_width.wrapping_sub(1u32).wrapping_div(2u32));
    return d;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_approx_gaussian_buffer_element_count_required(
    sigma: f32,
    bitmap_width: u32,
) -> u32 {
    return flow_bitmap_float_approx_gaussian_calculate_d(sigma, bitmap_width)
        .wrapping_mul(2u32)
        .wrapping_add(12 as i32 as u32);
    // * sizeof(float);
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_approx_gaussian_blur_rows(
    context: *mut flow_c,
    image: *mut flow_bitmap_float,
    sigma: f32,
    buffer: *mut f32,
    buffer_element_count: usize,
    from_row: u32,
    row_count: i32,
) -> bool {
    // Ensure sigma is large enough for approximation to be accurate.
    if sigma < 2 as i32 as f32 {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1173 as i32,
            (*::std::mem::transmute::<&[u8; 44], &[libc::c_char; 44]>(
                b"flow_bitmap_float_approx_gaussian_blur_rows\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    // Ensure the buffer is large enough
    if flow_bitmap_float_approx_gaussian_buffer_element_count_required(sigma, (*image).w) as usize
        > buffer_element_count
    {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1179 as i32,
            (*::std::mem::transmute::<&[u8; 44], &[libc::c_char; 44]>(
                b"flow_bitmap_float_approx_gaussian_blur_rows\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    // http://www.w3.org/TR/SVG11/filters.html#feGaussianBlur
    // For larger values of 's' (s >= 2.0), an approximation can be used :
    // Three successive box - blurs build a piece - wise quadratic convolution kernel, which approximates the Gaussian
    // kernel to within roughly 3 % .
    let d: u32 = flow_bitmap_float_approx_gaussian_calculate_d(sigma, (*image).w);
    //... if d is odd, use three box - blurs of size 'd', centered on the output pixel.
    if d.wrapping_rem(2u32) > 0 as i32 as u32 {
        if !BitmapFloat_boxblur_rows(
            context,
            image,
            d.wrapping_div(2u32),
            3 as i32 as u32,
            (*image).channels,
            buffer,
            from_row,
            row_count,
        ) {
            flow_context_add_to_callstack(
                context,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1191 as i32,
                (*::std::mem::transmute::<&[u8; 44], &[libc::c_char; 44]>(
                    b"flow_bitmap_float_approx_gaussian_blur_rows\x00",
                ))
                .as_ptr(),
            );
            return false;
        }
    } else {
        // ... if d is even, two box - blurs of size 'd'
        // (the first one centered on the pixel boundary between the output pixel and the one to the left,
        //  the second one centered on the pixel boundary between the output pixel and the one to the right)
        // and one box blur of size 'd+1' centered on the output pixel.
        if !BitmapFloat_boxblur_misaligned_rows(
            context,
            image,
            d.wrapping_div(2u32),
            -(1 as i32),
            (*image).channels,
            buffer,
            from_row,
            row_count,
        ) {
            flow_context_add_to_callstack(
                context,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1200 as i32,
                (*::std::mem::transmute::<&[u8; 44], &[libc::c_char; 44]>(
                    b"flow_bitmap_float_approx_gaussian_blur_rows\x00",
                ))
                .as_ptr(),
            );
            return false;
        }
        if !BitmapFloat_boxblur_misaligned_rows(
            context,
            image,
            d.wrapping_div(2u32),
            1 as i32,
            (*image).channels,
            buffer,
            from_row,
            row_count,
        ) {
            flow_context_add_to_callstack(
                context,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1204 as i32,
                (*::std::mem::transmute::<&[u8; 44], &[libc::c_char; 44]>(
                    b"flow_bitmap_float_approx_gaussian_blur_rows\x00",
                ))
                .as_ptr(),
            );
            return false;
        }
        if !BitmapFloat_boxblur_rows(
            context,
            image,
            d.wrapping_div(2u32).wrapping_add(1u32),
            1 as i32 as u32,
            (*image).channels,
            buffer,
            from_row,
            row_count,
        ) {
            flow_context_add_to_callstack(
                context,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1207 as i32,
                (*::std::mem::transmute::<&[u8; 44], &[libc::c_char; 44]>(
                    b"flow_bitmap_float_approx_gaussian_blur_rows\x00",
                ))
                .as_ptr(),
            );
            return false;
        }
    }
    return true;
}
#[inline]
unsafe extern "C" fn transpose4x4_SSE(A: *mut f32, B: *mut f32, lda: i32, ldb: i32) {
    let mut row1: __m128 = _mm_loadu_ps(&mut *A.offset((0 as i32 * lda) as isize));
    let mut row2: __m128 = _mm_loadu_ps(&mut *A.offset((1 as i32 * lda) as isize));
    let mut row3: __m128 = _mm_loadu_ps(&mut *A.offset((2 as i32 * lda) as isize));
    let mut row4: __m128 = _mm_loadu_ps(&mut *A.offset((3 as i32 * lda) as isize));
    let mut tmp3: __m128 = _mm_setzero_ps();
    let mut tmp2: __m128 = _mm_setzero_ps();
    let mut tmp1: __m128 = _mm_setzero_ps();
    let mut tmp0: __m128 = _mm_setzero_ps();
    tmp0 = _mm_unpacklo_ps(row1, row2);
    tmp2 = _mm_unpacklo_ps(row3, row4);
    tmp1 = _mm_unpackhi_ps(row1, row2);
    tmp3 = _mm_unpackhi_ps(row3, row4);
    row1 = _mm_movelh_ps(tmp0, tmp2);
    row2 = _mm_movehl_ps(tmp2, tmp0);
    row3 = _mm_movelh_ps(tmp1, tmp3);
    row4 = _mm_movehl_ps(tmp3, tmp1);
    _mm_storeu_ps(&mut *B.offset((0 as i32 * ldb) as isize), row1);
    _mm_storeu_ps(&mut *B.offset((1 as i32 * ldb) as isize), row2);
    _mm_storeu_ps(&mut *B.offset((2 as i32 * ldb) as isize), row3);
    _mm_storeu_ps(&mut *B.offset((3 as i32 * ldb) as isize), row4);
}
#[inline]
unsafe extern "C" fn transpose_block_SSE4x4(
    A: *mut f32,
    B: *mut f32,
    n: i32,
    m: i32,
    lda: i32,
    ldb: i32,
    block_size: i32,
) {
    //#pragma omp parallel for collapse(2)
    let mut i: i32 = 0 as i32;
    while i < n {
        let mut j: i32 = 0 as i32;
        while j < m {
            let max_i2: i32 = if i + block_size < n {
                (i) + block_size
            } else {
                n
            };
            let max_j2: i32 = if j + block_size < m {
                (j) + block_size
            } else {
                m
            };
            let mut i2: i32 = i;
            while i2 < max_i2 {
                let mut j2: i32 = j;
                while j2 < max_j2 {
                    transpose4x4_SSE(
                        &mut *A.offset((i2 * lda + j2) as isize),
                        &mut *B.offset((j2 * ldb + i2) as isize),
                        lda,
                        ldb,
                    );
                    j2 += 4 as i32
                }
                i2 += 4 as i32
            }
            j += block_size
        }
        i += block_size
    }
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_bgra_transpose(
    c: *mut flow_c,
    from: *mut flow_bitmap_bgra,
    to: *mut flow_bitmap_bgra,
) -> bool {
    if (*from).w != (*to).h || (*from).h != (*to).w || (*from).fmt as u32 != (*to).fmt as u32 {
        flow_context_set_error_get_message_buffer(
            c,
            flow_status_code::Invalid_argument,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1252 as i32,
            (*::std::mem::transmute::<&[u8; 27], &[libc::c_char; 27]>(
                b"flow_bitmap_bgra_transpose\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    if (*from).fmt as u32 != flow_bgra32 as i32 as u32
        && (*from).fmt as u32 != flow_bgr32 as i32 as u32
    {
        if !flow_bitmap_bgra_transpose_slow(c, from, to) {
            flow_context_add_to_callstack(
                c,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1258 as i32,
                (*::std::mem::transmute::<&[u8; 27], &[libc::c_char; 27]>(
                    b"flow_bitmap_bgra_transpose\x00",
                ))
                .as_ptr(),
            );
            return false;
        }
        return true;
    }
    // We require 8 when we only need 4 - in case we ever want to enable avx (like if we make it faster)
    let min_block_size: i32 = 8 as i32;
    // Strides must be multiple of required alignments
    if (*from).stride.wrapping_rem(min_block_size as u32) != 0 as i32 as u32
        || (*to).stride.wrapping_rem(min_block_size as u32) != 0 as i32 as u32
    {
        flow_context_set_error_get_message_buffer(
            c,
            flow_status_code::Invalid_argument,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1269 as i32,
            (*::std::mem::transmute::<&[u8; 27], &[libc::c_char; 27]>(
                b"flow_bitmap_bgra_transpose\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    // 256 (1024x1024 bytes) at 18.18ms, 128 at 18.6ms,  64 at 20.4ms, 16 at 25.71ms
    let block_size: i32 = 128 as i32;
    let cropped_h: i32 = (*from)
        .h
        .wrapping_sub((*from).h.wrapping_rem(min_block_size as u32))
        as i32;
    let cropped_w: i32 = (*from)
        .w
        .wrapping_sub((*from).w.wrapping_rem(min_block_size as u32))
        as i32;
    transpose_block_SSE4x4(
        (*from).pixels as *mut f32,
        (*to).pixels as *mut f32,
        cropped_h,
        cropped_w,
        (*from).stride.wrapping_div(4u32) as i32,
        (*to).stride.wrapping_div(4u32) as i32,
        block_size,
    );
    // Copy missing bits
    let mut x: u32 = cropped_h as u32;
    while x < (*to).w {
        let mut y: u32 = 0 as i32 as u32;
        while y < (*to).h {
            *(&mut *(*to).pixels.offset(
                x.wrapping_mul(4u32)
                    .wrapping_add(y.wrapping_mul((*to).stride)) as isize,
            ) as *mut libc::c_uchar as *mut u32) = *(&mut *(*from).pixels.offset(
                x.wrapping_mul((*from).stride)
                    .wrapping_add(y.wrapping_mul(4u32)) as isize,
            ) as *mut libc::c_uchar
                as *mut u32);
            y = y.wrapping_add(1)
        }
        x = x.wrapping_add(1)
    }
    let mut x_0: u32 = 0 as i32 as u32;
    while x_0 < cropped_h as u32 {
        let mut y_0: u32 = cropped_w as u32;
        while y_0 < (*to).h {
            *(&mut *(*to).pixels.offset(
                x_0.wrapping_mul(4u32)
                    .wrapping_add(y_0.wrapping_mul((*to).stride)) as isize,
            ) as *mut libc::c_uchar as *mut u32) = *(&mut *(*from).pixels.offset(
                x_0.wrapping_mul((*from).stride)
                    .wrapping_add(y_0.wrapping_mul(4u32)) as isize,
            ) as *mut libc::c_uchar
                as *mut u32);
            y_0 = y_0.wrapping_add(1)
        }
        x_0 = x_0.wrapping_add(1)
    }
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_bgra_transpose_slow(
    c: *mut flow_c,
    from: *mut flow_bitmap_bgra,
    to: *mut flow_bitmap_bgra,
) -> bool {
    if (*from).w != (*to).h || (*from).h != (*to).w || (*from).fmt as u32 != (*to).fmt as u32 {
        flow_context_set_error_get_message_buffer(
            c,
            flow_status_code::Invalid_argument,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1300 as i32,
            (*::std::mem::transmute::<&[u8; 32], &[libc::c_char; 32]>(
                b"flow_bitmap_bgra_transpose_slow\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    if (*from).fmt as u32 == flow_bgra32 as i32 as u32
        || (*from).fmt as u32 == flow_bgr32 as i32 as u32
    {
        let mut x: u32 = 0 as i32 as u32;
        while x < (*to).w {
            let mut y: u32 = 0 as i32 as u32;
            while y < (*to).h {
                *(&mut *(*to).pixels.offset(
                    x.wrapping_mul(4u32)
                        .wrapping_add(y.wrapping_mul((*to).stride)) as isize,
                ) as *mut libc::c_uchar as *mut u32) = *(&mut *(*from).pixels.offset(
                    x.wrapping_mul((*from).stride)
                        .wrapping_add(y.wrapping_mul(4u32)) as isize,
                ) as *mut libc::c_uchar
                    as *mut u32);
                y = y.wrapping_add(1)
            }
            x = x.wrapping_add(1)
        }
        return true;
    } else if (*from).fmt as u32 == flow_bgr24 as i32 as u32 {
        let from_stride: i32 = (*from).stride as i32;
        let to_stride: i32 = (*to).stride as i32;
        let mut x_0: u32 = 0 as i32 as u32;
        let mut x_stride: u32 = 0 as i32 as u32;
        let mut x_3: u32 = 0 as i32 as u32;
        while x_0 < (*to).w {
            let mut y_0: u32 = 0 as i32 as u32;
            let mut y_stride: u32 = 0 as i32 as u32;
            let mut y_3: u32 = 0 as i32 as u32;
            while y_0 < (*to).h {
                *(*to).pixels.offset(x_3.wrapping_add(y_stride) as isize) =
                    *(*from).pixels.offset(x_stride.wrapping_add(y_3) as isize);
                *(*to)
                    .pixels
                    .offset(x_3.wrapping_add(y_stride).wrapping_add(1u32) as isize) = *(*from)
                    .pixels
                    .offset(x_stride.wrapping_add(y_3).wrapping_add(1u32) as isize);
                *(*to)
                    .pixels
                    .offset(x_3.wrapping_add(y_stride).wrapping_add(2u32) as isize) = *(*from)
                    .pixels
                    .offset(x_stride.wrapping_add(y_3).wrapping_add(2u32) as isize);
                y_0 = y_0.wrapping_add(1);
                y_stride = (y_stride as u32).wrapping_add(to_stride as u32) as u32 as u32;
                y_3 = (y_3 as u32).wrapping_add(3u32) as u32 as u32
            }
            x_0 = x_0.wrapping_add(1);
            x_stride = (x_stride as u32).wrapping_add(from_stride as u32) as u32 as u32;
            x_3 = (x_3 as u32).wrapping_add(3u32) as u32 as u32
        }
        return true;
    } else {
        flow_context_set_error_get_message_buffer(
            c,
            flow_status_code::Invalid_argument,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1325 as i32,
            (*::std::mem::transmute::<&[u8; 32], &[libc::c_char; 32]>(
                b"flow_bitmap_bgra_transpose_slow\x00",
            ))
            .as_ptr(),
        );
        return false;
    };
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_convert_srgb_to_linear(
    context: *mut flow_c,
    colorcontext: *mut flow_colorcontext_info,
    src: *mut flow_bitmap_bgra,
    from_row: u32,
    dest: *mut flow_bitmap_float,
    dest_row: u32,
    row_count: u32,
) -> bool {
    if ((*src).w != (*dest).w) as i32 as libc::c_long != 0 {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1339 as i32,
            (*::std::mem::transmute::<&[u8; 41], &[libc::c_char; 41]>(
                b"flow_bitmap_float_convert_srgb_to_linear\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    if !(from_row.wrapping_add(row_count) <= (*src).h
        && dest_row.wrapping_add(row_count) <= (*dest).h) as i32 as libc::c_long
        != 0
    {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1345 as i32,
            (*::std::mem::transmute::<&[u8; 41], &[libc::c_char; 41]>(
                b"flow_bitmap_float_convert_srgb_to_linear\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    let w = (*src).w;
    let units: u32 = w * flow_pixel_format_bytes_per_pixel((*src).fmt);
    let from_step: u32 = flow_pixel_format_bytes_per_pixel((*src).fmt);
    let from_copy: u32 = flow_pixel_format_channels(flow_effective_pixel_format(src));
    let to_step: u32 = (*dest).channels;
    let copy_step: u32 = from_copy.min(to_step);
    if copy_step != 3 && copy_step != 4 {
        flow_snprintf(
            flow_context_set_error_get_message_buffer(
                context,
                flow_status_code::Unsupported_pixel_format,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1361 as i32,
                (*::std::mem::transmute::<&[u8; 41], &[libc::c_char; 41]>(
                    b"flow_bitmap_float_convert_srgb_to_linear\x00",
                ))
                .as_ptr(),
            ),
            FLOW_ERROR_MESSAGE_SIZE as usize,
            b"copy_step=%d\x00" as *const u8 as *const libc::c_char,
            copy_step,
        );
        return false;
    }
    if copy_step == 4 && from_step != 4 && to_step != 4 {
        flow_snprintf(
            flow_context_set_error_get_message_buffer(
                context,
                flow_status_code::Unsupported_pixel_format,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1368 as i32,
                (*::std::mem::transmute::<&[u8; 41], &[libc::c_char; 41]>(
                    b"flow_bitmap_float_convert_srgb_to_linear\x00",
                ))
                .as_ptr(),
            ),
            FLOW_ERROR_MESSAGE_SIZE as usize,
            b"copy_step=%d, from_step=%d, to_step=%d\x00" as *const u8 as *const libc::c_char,
            copy_step,
            from_step,
            to_step,
        );
        return false;
    }
    if copy_step == 4 {
        let mut row: u32 = 0 as i32 as u32;
        while row < row_count {
            let src_start: *mut u8 = (*src)
                .pixels
                .offset(from_row.wrapping_add(row).wrapping_mul((*src).stride) as isize);
            let buf: *mut f32 = (*dest).pixels.offset(
                (*dest)
                    .float_stride
                    .wrapping_mul(row.wrapping_add(dest_row)) as isize,
            );
            let mut to_x: u32 = 0 as i32 as u32;
            let mut bix: u32 = 0 as i32 as u32;
            while bix < units {
                let alpha: f32 =
                    *src_start.offset(bix.wrapping_add(3u32) as isize) as f32 / 255.0f32;
                *buf.offset(to_x as isize) = alpha
                    * flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start.offset(bix as isize),
                    );
                *buf.offset(to_x.wrapping_add(1u32) as isize) = alpha
                    * flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start.offset(bix.wrapping_add(1u32) as isize),
                    );
                *buf.offset(to_x.wrapping_add(2u32) as isize) = alpha
                    * flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start.offset(bix.wrapping_add(2u32) as isize),
                    );
                *buf.offset(to_x.wrapping_add(3u32) as isize) = alpha;
                to_x = (to_x as u32).wrapping_add(4u32) as u32 as u32;
                bix = (bix as u32).wrapping_add(4u32) as u32 as u32
            }
            row = row.wrapping_add(1)
        }
    } else if from_step == 3 && to_step == 3 {
        let mut row: u32 = 0 as i32 as u32;
        while row < row_count {
            let src_start_0: *mut u8 = (*src)
                .pixels
                .offset(from_row.wrapping_add(row).wrapping_mul((*src).stride) as isize);
            let buf: *mut f32 = (*dest).pixels.offset(
                (*dest)
                    .float_stride
                    .wrapping_mul(row.wrapping_add(dest_row)) as isize,
            );
            let mut to_x: u32 = 0 as i32 as u32;
            let mut bix: u32 = 0 as i32 as u32;
            while bix < units {
                *buf.offset(to_x as isize) = flow_colorcontext_srgb_to_floatspace(
                    colorcontext,
                    *src_start_0.offset(bix as isize),
                );
                *buf.offset(to_x.wrapping_add(1u32) as isize) =
                    flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start_0.offset(bix.wrapping_add(1u32) as isize),
                    );
                *buf.offset(to_x.wrapping_add(2u32) as isize) =
                    flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start_0.offset(bix.wrapping_add(2u32) as isize),
                    );
                to_x = (to_x as u32).wrapping_add(3u32) as u32 as u32;
                bix = (bix as u32).wrapping_add(3u32) as u32 as u32
            }
            row += 1
        }
    } else if from_step == 4 && to_step == 3 {
        let mut row: u32 = 0 as i32 as u32;
        while row < row_count {
            let src_start: *mut u8 = (*src)
                .pixels
                .offset(from_row.wrapping_add(row).wrapping_mul((*src).stride) as isize);
            let buf: *mut f32 = (*dest).pixels.offset(
                (*dest)
                    .float_stride
                    .wrapping_mul(row.wrapping_add(dest_row)) as isize,
            );
            let mut to_x: u32 = 0 as i32 as u32;
            let mut bix: u32 = 0 as i32 as u32;
            while bix < units {
                *buf.offset(to_x as isize) = flow_colorcontext_srgb_to_floatspace(
                    colorcontext,
                    *src_start.offset(bix as isize),
                );
                *buf.offset(to_x.wrapping_add(1u32) as isize) =
                    flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start.offset(bix.wrapping_add(1u32) as isize),
                    );
                *buf.offset(to_x.wrapping_add(2u32) as isize) =
                    flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start.offset(bix.wrapping_add(2u32) as isize),
                    );
                to_x = (to_x as u32).wrapping_add(3u32) as u32 as u32;
                bix = (bix as u32).wrapping_add(4u32) as u32 as u32
            }
            row += 1
        }
    } else if from_step == 3 && to_step == 4 {
        let mut row: u32 = 0 as i32 as u32;
        while row < row_count {
            let src_start: *mut u8 = (*src)
                .pixels
                .offset(from_row.wrapping_add(row).wrapping_mul((*src).stride) as isize);
            let buf: *mut f32 = (*dest).pixels.offset(
                (*dest)
                    .float_stride
                    .wrapping_mul(row.wrapping_add(dest_row)) as isize,
            );
            let mut to_x: u32 = 0 as i32 as u32;
            let mut bix: u32 = 0 as i32 as u32;
            while bix < units {
                *buf.offset(to_x as isize) = flow_colorcontext_srgb_to_floatspace(
                    colorcontext,
                    *src_start.offset(bix as isize),
                );
                *buf.offset(to_x.wrapping_add(1u32) as isize) =
                    flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start.offset(bix.wrapping_add(1u32) as isize),
                    );
                *buf.offset(to_x.wrapping_add(2u32) as isize) =
                    flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start.offset(bix.wrapping_add(2u32) as isize),
                    );
                to_x = (to_x as u32).wrapping_add(4u32) as u32 as u32;
                bix = (bix as u32).wrapping_add(3u32) as u32 as u32
            }
            row += 1
        }
    } else if from_step == 4 && to_step == 4 {
        let mut row: u32 = 0 as i32 as u32;
        while row < row_count {
            let src_start: *mut u8 = (*src)
                .pixels
                .offset(from_row.wrapping_add(row).wrapping_mul((*src).stride) as isize);
            let buf: *mut f32 = (*dest).pixels.offset(
                (*dest)
                    .float_stride
                    .wrapping_mul(row.wrapping_add(dest_row)) as isize,
            );
            let mut to_x: u32 = 0 as i32 as u32;
            let mut bix: u32 = 0 as i32 as u32;
            while bix < units {
                *buf.offset(to_x as isize) = flow_colorcontext_srgb_to_floatspace(
                    colorcontext,
                    *src_start.offset(bix as isize),
                );
                *buf.offset(to_x.wrapping_add(1u32) as isize) =
                    flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start.offset(bix.wrapping_add(1u32) as isize),
                    );
                *buf.offset(to_x.wrapping_add(2u32) as isize) =
                    flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start.offset(bix.wrapping_add(2u32) as isize),
                    );
                to_x = (to_x as u32).wrapping_add(4u32) as u32 as u32;
                bix = (bix as u32).wrapping_add(4u32) as u32 as u32
            }
            row += 1
        }
    } else {
        flow_snprintf(
            flow_context_set_error_get_message_buffer(
                context,
                flow_status_code::Unsupported_pixel_format,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1411 as i32,
                (*::std::mem::transmute::<&[u8; 41], &[libc::c_char; 41]>(
                    b"flow_bitmap_float_convert_srgb_to_linear\x00",
                ))
                .as_ptr(),
            ),
            FLOW_ERROR_MESSAGE_SIZE as usize,
            b"copy_step=%d, from_step=%d, to_step=%d\x00" as *const u8 as *const libc::c_char,
            copy_step,
            from_step,
            to_step,
        );
        return false;
    }
    return true;
}
/*
static void unpack24bitRow(u32 width, unsigned char* sourceLine, unsigned char* destArray){
    for (u32 i = 0; i < width; i++){

        memcpy(destArray + i * 4, sourceLine + i * 3, 3);
        destArray[i * 4 + 3] = 255;
    }
}
*/
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_bgra_flip_vertical(
    context: *mut flow_c,
    b: *mut flow_bitmap_bgra,
) -> bool {
    let swap: *mut libc::c_void = flow_context_malloc(
        context,
        (*b).stride as usize,
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        context as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        1430 as i32,
    );
    if swap.is_null() {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Out_of_memory,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1432 as i32,
            (*::std::mem::transmute::<&[u8; 31], &[libc::c_char; 31]>(
                b"flow_bitmap_bgra_flip_vertical\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    // Dont' copy the full stride (padding), it could be windowed!
    // Todo: try multiple swap rows? 5ms isn't bad, but could be better
    let row_length: u32 = (*b).stride.min(
        (*b).w
            .wrapping_mul(flow_pixel_format_bytes_per_pixel((*b).fmt)),
    );
    let mut i: u32 = 0 as i32 as u32;
    while i < (*b).h.wrapping_div(2u32) {
        let top: *mut libc::c_void =
            (*b).pixels.offset(i.wrapping_mul((*b).stride) as isize) as *mut libc::c_void;
        let bottom: *mut libc::c_void = (*b).pixels.offset(
            (*b).h
                .wrapping_sub(1u32)
                .wrapping_sub(i)
                .wrapping_mul((*b).stride) as isize,
        ) as *mut libc::c_void;
        memcpy(swap, top, row_length as u64);
        memcpy(top, bottom, row_length as u64);
        memcpy(bottom, swap, row_length as u64);
        i = i.wrapping_add(1)
    }
    flow_deprecated_free(
        context,
        swap,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        1445 as i32,
    );
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_bgra_flip_horizontal(
    _context: *mut flow_c,
    b: *mut flow_bitmap_bgra,
) -> bool {
    if (*b).fmt as u32 == flow_bgra32 as i32 as u32 || (*b).fmt as u32 == flow_bgr32 as i32 as u32 {
        // 12ms simple
        let mut y: u32 = 0 as i32 as u32;
        while y < (*b).h {
            let mut left: *mut u32 =
                (*b).pixels.offset(y.wrapping_mul((*b).stride) as isize) as *mut u32;
            let mut right: *mut u32 = (*b)
                .pixels
                .offset(y.wrapping_mul((*b).stride) as isize)
                .offset((4u32).wrapping_mul((*b).w.wrapping_sub(1u32)) as isize)
                as *mut u32;
            while left < right {
                let swap: u32 = *left;
                *left = *right;
                *right = swap;
                left = left.offset(1);
                right = right.offset(-1)
            }
            y = y.wrapping_add(1)
        }
    } else if (*b).fmt as u32 == flow_bgr24 as i32 as u32 {
        let mut swap_0: [u32; 4] = [0; 4];
        // Dont' copy the full stride (padding), it could be windowed!
        let mut y_0: u32 = 0 as i32 as u32;
        while y_0 < (*b).h {
            let mut left_0: *mut u8 = (*b).pixels.offset(y_0.wrapping_mul((*b).stride) as isize);
            let mut right_0: *mut u8 = (*b)
                .pixels
                .offset(y_0.wrapping_mul((*b).stride) as isize)
                .offset((3u32).wrapping_mul((*b).w.wrapping_sub(1u32)) as isize);
            while left_0 < right_0 {
                memcpy(
                    &mut swap_0 as *mut [u32; 4] as *mut libc::c_void,
                    left_0 as *const libc::c_void,
                    3 as i32 as u64,
                );
                memcpy(
                    left_0 as *mut libc::c_void,
                    right_0 as *const libc::c_void,
                    3 as i32 as u64,
                );
                memcpy(
                    right_0 as *mut libc::c_void,
                    &mut swap_0 as *mut [u32; 4] as *const libc::c_void,
                    3 as i32 as u64,
                );
                left_0 = left_0.offset(3 as i32 as isize);
                right_0 = right_0.offset(-(3 as i32 as isize))
            }
            y_0 = y_0.wrapping_add(1)
        }
    } else {
        let mut swap_1: [u32; 4] = [0; 4];
        // Dont' copy the full stride (padding), it could be windowed!
        let mut y_1: u32 = 0 as i32 as u32;
        while y_1 < (*b).h {
            let mut left_1: *mut u8 = (*b).pixels.offset(y_1.wrapping_mul((*b).stride) as isize);
            let mut right_1: *mut u8 = (*b)
                .pixels
                .offset(y_1.wrapping_mul((*b).stride) as isize)
                .offset(
                    flow_pixel_format_bytes_per_pixel((*b).fmt)
                        .wrapping_mul((*b).w.wrapping_sub(1u32)) as isize,
                );
            while left_1 < right_1 {
                memcpy(
                    &mut swap_1 as *mut [u32; 4] as *mut libc::c_void,
                    left_1 as *const libc::c_void,
                    flow_pixel_format_bytes_per_pixel((*b).fmt) as u64,
                );
                memcpy(
                    left_1 as *mut libc::c_void,
                    right_1 as *const libc::c_void,
                    flow_pixel_format_bytes_per_pixel((*b).fmt) as u64,
                );
                memcpy(
                    right_1 as *mut libc::c_void,
                    &mut swap_1 as *mut [u32; 4] as *const libc::c_void,
                    flow_pixel_format_bytes_per_pixel((*b).fmt) as u64,
                );
                left_1 = left_1.offset(flow_pixel_format_bytes_per_pixel((*b).fmt) as isize);
                right_1 = right_1.offset(-(flow_pixel_format_bytes_per_pixel((*b).fmt) as isize))
            }
            y_1 = y_1.wrapping_add(1)
        }
    }
    return true;
}
unsafe extern "C" fn flow_bitmap_float_blend_matte(
    _context: *mut flow_c,
    colorcontext: *mut flow_colorcontext_info,
    src: *mut flow_bitmap_float,
    from_row: u32,
    row_count: u32,
    matte: *const u8,
) -> bool {
    // We assume that matte is BGRA, regardless.
    let matte_a: f32 = *matte.offset(3 as i32 as isize) as f32 / 255.0f32;
    let b: f32 = flow_colorcontext_srgb_to_floatspace(colorcontext, *matte.offset(0));
    let g: f32 = flow_colorcontext_srgb_to_floatspace(colorcontext, *matte.offset(1));
    let r: f32 = flow_colorcontext_srgb_to_floatspace(colorcontext, *matte.offset(2));
    let mut row: u32 = from_row;
    while row < from_row.wrapping_add(row_count) {
        let start_ix: u32 = row.wrapping_mul((*src).float_stride);
        let end_ix: u32 = start_ix.wrapping_add((*src).w.wrapping_mul((*src).channels));
        let mut ix: u32 = start_ix;
        while ix < end_ix {
            let src_a: f32 = *(*src).pixels.offset(ix.wrapping_add(3u32) as isize);
            let a: f32 = (1.0f32 - src_a) * matte_a;
            let final_alpha: f32 = src_a + a;
            *(*src).pixels.offset(ix as isize) =
                (*(*src).pixels.offset(ix as isize) + b * a) / final_alpha;
            *(*src).pixels.offset(ix.wrapping_add(1u32) as isize) =
                (*(*src).pixels.offset(ix.wrapping_add(1u32) as isize) + g * a) / final_alpha;
            *(*src).pixels.offset(ix.wrapping_add(2u32) as isize) =
                (*(*src).pixels.offset(ix.wrapping_add(2u32) as isize) + r * a) / final_alpha;
            *(*src).pixels.offset(ix.wrapping_add(3u32) as isize) = final_alpha;
            ix = (ix as u32).wrapping_add(4u32) as u32 as u32
        }
        row = row.wrapping_add(1)
    }
    // Ensure alpha is demultiplied
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_demultiply_alpha(
    _context: *mut flow_c,
    src: *mut flow_bitmap_float,
    from_row: u32,
    row_count: u32,
) -> bool {
    let mut row: u32 = from_row;
    while row < from_row.wrapping_add(row_count) {
        let start_ix: u32 = row.wrapping_mul((*src).float_stride);
        let end_ix: u32 = start_ix.wrapping_add((*src).w.wrapping_mul((*src).channels));
        let mut ix: u32 = start_ix;
        while ix < end_ix {
            let alpha: f32 = *(*src).pixels.offset(ix.wrapping_add(3u32) as isize);
            if alpha > 0 as i32 as f32 {
                *(*src).pixels.offset(ix as isize) /= alpha;
                *(*src).pixels.offset(ix.wrapping_add(1u32) as isize) /= alpha;
                *(*src).pixels.offset(ix.wrapping_add(2u32) as isize) /= alpha
            }
            ix = (ix as u32).wrapping_add(4u32) as u32 as u32
        }
        row = row.wrapping_add(1)
    }
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_copy_linear_over_srgb(
    _context: *mut flow_c,
    colorcontext: *mut flow_colorcontext_info,
    src: *mut flow_bitmap_float,
    from_row: u32,
    dest: *mut flow_bitmap_bgra,
    dest_row: u32,
    row_count: u32,
    from_col: u32,
    col_count: u32,
    transpose: bool,
) -> bool {
    let dest_bytes_pp: u32 = flow_pixel_format_bytes_per_pixel((*dest).fmt);
    let srcitems: u32 = from_col
        .wrapping_add(col_count)
        .min((*src).w)
        .wrapping_mul((*src).channels);
    let dest_fmt: flow_pixel_format = flow_effective_pixel_format(dest);
    let ch: u32 = (*src).channels;
    let copy_alpha: bool = dest_fmt as u32 == flow_bgra32 as i32 as u32
        && ch == 4 as i32 as u32
        && (*src).alpha_meaningful as i32 != 0;
    let clean_alpha: bool = !copy_alpha && dest_fmt as u32 == flow_bgra32 as i32 as u32;
    let dest_row_stride: u32 = if transpose as i32 != 0 {
        dest_bytes_pp
    } else {
        (*dest).stride
    };
    let dest_pixel_stride: u32 = if transpose as i32 != 0 {
        (*dest).stride
    } else {
        dest_bytes_pp
    };
    if dest_pixel_stride == 4 as i32 as u32 {
        if ch == 3 as i32 as u32 {
            if copy_alpha && !clean_alpha {
                let mut row: u32 = 0 as i32 as u32;
                while row < row_count {
                    let src_row: *mut f32 =
                        (*src)
                            .pixels
                            .offset(row.wrapping_add(from_row).wrapping_mul((*src).float_stride)
                                as isize);
                    let mut dest_row_bytes: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix: u32 = from_col.wrapping_mul(3u32);
                    while ix < srcitems {
                        *dest_row_bytes.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row.offset(ix as isize),
                        );
                        *dest_row_bytes.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row.offset(ix.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row.offset(ix.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes.offset(3 as i32 as isize) = uchar_clamp_ff(
                            *src_row.offset(ix.wrapping_add(3u32) as isize) * 255.0f32,
                        );
                        dest_row_bytes = dest_row_bytes.offset(4 as i32 as isize);
                        ix = (ix as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row = row.wrapping_add(1)
                }
            }
            if !copy_alpha && !clean_alpha {
                let mut row_0: u32 = 0 as i32 as u32;
                while row_0 < row_count {
                    let src_row_0: *mut f32 = (*src).pixels.offset(
                        row_0
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_0: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_0).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix_0: u32 = from_col.wrapping_mul(3u32);
                    while ix_0 < srcitems {
                        *dest_row_bytes_0.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_0.offset(ix_0 as isize),
                        );
                        *dest_row_bytes_0.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_0.offset(ix_0.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_0.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_0.offset(ix_0.wrapping_add(2u32) as isize),
                        );
                        dest_row_bytes_0 = dest_row_bytes_0.offset(4 as i32 as isize);
                        ix_0 = (ix_0 as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row_0 = row_0.wrapping_add(1)
                }
            }
            if !copy_alpha && clean_alpha {
                let mut row_1: u32 = 0 as i32 as u32;
                while row_1 < row_count {
                    let src_row_1: *mut f32 = (*src).pixels.offset(
                        row_1
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_1: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_1).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix_1: u32 = from_col.wrapping_mul(3u32);
                    while ix_1 < srcitems {
                        *dest_row_bytes_1.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_1.offset(ix_1 as isize),
                        );
                        *dest_row_bytes_1.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_1.offset(ix_1.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_1.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_1.offset(ix_1.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_1.offset(3 as i32 as isize) = 0xff as i32 as u8;
                        dest_row_bytes_1 = dest_row_bytes_1.offset(4 as i32 as isize);
                        ix_1 = (ix_1 as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row_1 = row_1.wrapping_add(1)
                }
            }
        }
        if ch == 4 as i32 as u32 {
            if copy_alpha && !clean_alpha {
                let mut row_2: u32 = 0 as i32 as u32;
                while row_2 < row_count {
                    let src_row_2: *mut f32 = (*src).pixels.offset(
                        row_2
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_2: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_2).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix_2: u32 = from_col.wrapping_mul(4u32);
                    while ix_2 < srcitems {
                        *dest_row_bytes_2.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_2.offset(ix_2 as isize),
                        );
                        *dest_row_bytes_2.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_2.offset(ix_2.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_2.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_2.offset(ix_2.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_2.offset(3 as i32 as isize) = uchar_clamp_ff(
                            *src_row_2.offset(ix_2.wrapping_add(3u32) as isize) * 255.0f32,
                        );
                        dest_row_bytes_2 = dest_row_bytes_2.offset(4 as i32 as isize);
                        ix_2 = (ix_2 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_2 = row_2.wrapping_add(1)
                }
            }
            if !copy_alpha && !clean_alpha {
                let mut row_3: u32 = 0 as i32 as u32;
                while row_3 < row_count {
                    let src_row_3: *mut f32 = (*src).pixels.offset(
                        row_3
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_3: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_3).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix_3: u32 = from_col.wrapping_mul(4u32);
                    while ix_3 < srcitems {
                        *dest_row_bytes_3.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_3.offset(ix_3 as isize),
                        );
                        *dest_row_bytes_3.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_3.offset(ix_3.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_3.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_3.offset(ix_3.wrapping_add(2u32) as isize),
                        );
                        dest_row_bytes_3 = dest_row_bytes_3.offset(4 as i32 as isize);
                        ix_3 = (ix_3 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_3 = row_3.wrapping_add(1)
                }
            }
            if !copy_alpha && clean_alpha {
                let mut row_4: u32 = 0 as i32 as u32;
                while row_4 < row_count {
                    let src_row_4: *mut f32 = (*src).pixels.offset(
                        row_4
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_4: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_4).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix_4: u32 = from_col.wrapping_mul(4u32);
                    while ix_4 < srcitems {
                        *dest_row_bytes_4.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_4.offset(ix_4 as isize),
                        );
                        *dest_row_bytes_4.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_4.offset(ix_4.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_4.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_4.offset(ix_4.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_4.offset(3 as i32 as isize) = 0xff as i32 as u8;
                        dest_row_bytes_4 = dest_row_bytes_4.offset(4 as i32 as isize);
                        ix_4 = (ix_4 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_4 = row_4.wrapping_add(1)
                }
            }
        }
    } else {
        if ch == 3 as i32 as u32 {
            if copy_alpha && !clean_alpha {
                let mut row_5: u32 = 0 as i32 as u32;
                while row_5 < row_count {
                    let src_row_5: *mut f32 = (*src).pixels.offset(
                        row_5
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_5: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_5).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_5: u32 = from_col.wrapping_mul(3u32);
                    while ix_5 < srcitems {
                        *dest_row_bytes_5.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_5.offset(ix_5 as isize),
                        );
                        *dest_row_bytes_5.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_5.offset(ix_5.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_5.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_5.offset(ix_5.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_5.offset(3 as i32 as isize) = uchar_clamp_ff(
                            *src_row_5.offset(ix_5.wrapping_add(3u32) as isize) * 255.0f32,
                        );
                        dest_row_bytes_5 = dest_row_bytes_5.offset(dest_pixel_stride as isize);
                        ix_5 = (ix_5 as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row_5 = row_5.wrapping_add(1)
                }
            }
            if !copy_alpha && !clean_alpha {
                let mut row_6: u32 = 0 as i32 as u32;
                while row_6 < row_count {
                    let src_row_6: *mut f32 = (*src).pixels.offset(
                        row_6
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_6: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_6).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_6: u32 = from_col.wrapping_mul(3u32);
                    while ix_6 < srcitems {
                        *dest_row_bytes_6.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_6.offset(ix_6 as isize),
                        );
                        *dest_row_bytes_6.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_6.offset(ix_6.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_6.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_6.offset(ix_6.wrapping_add(2u32) as isize),
                        );
                        dest_row_bytes_6 = dest_row_bytes_6.offset(dest_pixel_stride as isize);
                        ix_6 = (ix_6 as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row_6 = row_6.wrapping_add(1)
                }
            }
            if !copy_alpha && clean_alpha {
                let mut row_7: u32 = 0 as i32 as u32;
                while row_7 < row_count {
                    let src_row_7: *mut f32 = (*src).pixels.offset(
                        row_7
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_7: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_7).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_7: u32 = from_col.wrapping_mul(3u32);
                    while ix_7 < srcitems {
                        *dest_row_bytes_7.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_7.offset(ix_7 as isize),
                        );
                        *dest_row_bytes_7.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_7.offset(ix_7.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_7.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_7.offset(ix_7.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_7.offset(3 as i32 as isize) = 0xff as i32 as u8;
                        dest_row_bytes_7 = dest_row_bytes_7.offset(dest_pixel_stride as isize);
                        ix_7 = (ix_7 as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row_7 = row_7.wrapping_add(1)
                }
            }
        }
        if ch == 4 as i32 as u32 {
            if copy_alpha && !clean_alpha {
                let mut row_8: u32 = 0 as i32 as u32;
                while row_8 < row_count {
                    let src_row_8: *mut f32 = (*src).pixels.offset(
                        row_8
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_8: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_8).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_8: u32 = from_col.wrapping_mul(4u32);
                    while ix_8 < srcitems {
                        *dest_row_bytes_8.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_8.offset(ix_8 as isize),
                        );
                        *dest_row_bytes_8.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_8.offset(ix_8.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_8.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_8.offset(ix_8.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_8.offset(3 as i32 as isize) = uchar_clamp_ff(
                            *src_row_8.offset(ix_8.wrapping_add(3u32) as isize) * 255.0f32,
                        );
                        dest_row_bytes_8 = dest_row_bytes_8.offset(dest_pixel_stride as isize);
                        ix_8 = (ix_8 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_8 = row_8.wrapping_add(1)
                }
            }
            if !copy_alpha && !clean_alpha {
                let mut row_9: u32 = 0 as i32 as u32;
                while row_9 < row_count {
                    let src_row_9: *mut f32 = (*src).pixels.offset(
                        row_9
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_9: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_9).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_9: u32 = from_col.wrapping_mul(4u32);
                    while ix_9 < srcitems {
                        *dest_row_bytes_9.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_9.offset(ix_9 as isize),
                        );
                        *dest_row_bytes_9.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_9.offset(ix_9.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_9.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_9.offset(ix_9.wrapping_add(2u32) as isize),
                        );
                        dest_row_bytes_9 = dest_row_bytes_9.offset(dest_pixel_stride as isize);
                        ix_9 = (ix_9 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_9 = row_9.wrapping_add(1)
                }
            }
            if !copy_alpha && clean_alpha {
                let mut row_10: u32 = 0 as i32 as u32;
                while row_10 < row_count {
                    let src_row_10: *mut f32 = (*src).pixels.offset(
                        row_10
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_10: *mut u8 =
                        (*dest)
                            .pixels
                            .offset(dest_row.wrapping_add(row_10).wrapping_mul(dest_row_stride)
                                as isize)
                            .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_10: u32 = from_col.wrapping_mul(4u32);
                    while ix_10 < srcitems {
                        *dest_row_bytes_10.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_10.offset(ix_10 as isize),
                        );
                        *dest_row_bytes_10.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_10.offset(ix_10.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_10.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_10.offset(ix_10.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_10.offset(3 as i32 as isize) = 0xff as i32 as u8;
                        dest_row_bytes_10 = dest_row_bytes_10.offset(dest_pixel_stride as isize);
                        ix_10 = (ix_10 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_10 = row_10.wrapping_add(1)
                }
            }
        }
    }
    return true;
}
unsafe extern "C" fn BitmapFloat_compose_linear_over_srgb(
    _context: *mut flow_c,
    colorcontext: *mut flow_colorcontext_info,
    src: *mut flow_bitmap_float,
    from_row: u32,
    dest: *mut flow_bitmap_bgra,
    dest_row: u32,
    row_count: u32,
    from_col: u32,
    col_count: u32,
    transpose: bool,
) -> bool {
    let dest_bytes_pp: u32 = flow_pixel_format_bytes_per_pixel((*dest).fmt);
    let dest_row_stride: u32 = if transpose as i32 != 0 {
        dest_bytes_pp
    } else {
        (*dest).stride
    };
    let dest_pixel_stride: u32 = if transpose as i32 != 0 {
        (*dest).stride
    } else {
        dest_bytes_pp
    };
    let srcitems: u32 = from_col
        .wrapping_add(col_count)
        .min((*src).w)
        .wrapping_mul((*src).channels);
    let ch: u32 = (*src).channels;
    let dest_effective_format: flow_pixel_format = flow_effective_pixel_format(dest);
    let dest_alpha: bool = dest_effective_format as u32 == flow_bgra32 as i32 as u32;
    let dest_alpha_index: u8 = if dest_alpha as i32 != 0 {
        3 as i32
    } else {
        0 as i32
    } as u8;
    let dest_alpha_to_float_coeff: f32 = if dest_alpha as i32 != 0 {
        (1.0f32) / 255.0f32
    } else {
        0.0f32
    };
    let dest_alpha_to_float_offset: f32 = if dest_alpha as i32 != 0 {
        0.0f32
    } else {
        1.0f32
    };
    let mut row: u32 = 0 as i32 as u32;
    while row < row_count {
        // const float * const __restrict src_row = src->pixels + (row + from_row) * src->float_stride;
        let src_row: *mut f32 = (*src)
            .pixels
            .offset(row.wrapping_add(from_row).wrapping_mul((*src).float_stride) as isize);
        let mut dest_row_bytes: *mut u8 = (*dest)
            .pixels
            .offset(dest_row.wrapping_add(row).wrapping_mul(dest_row_stride) as isize)
            .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
        let mut ix: u32 = from_col.wrapping_mul(ch);
        while ix < srcitems {
            let dest_b: u8 = *dest_row_bytes.offset(0);
            let dest_g: u8 = *dest_row_bytes.offset(1);
            let dest_r: u8 = *dest_row_bytes.offset(2);
            let dest_a: u8 = *dest_row_bytes.offset(dest_alpha_index as isize);
            let src_b: f32 = *src_row.offset(ix.wrapping_add(0u32) as isize);
            let src_g: f32 = *src_row.offset(ix.wrapping_add(1u32) as isize);
            let src_r: f32 = *src_row.offset(ix.wrapping_add(2u32) as isize);
            let src_a: f32 = *src_row.offset(ix.wrapping_add(3u32) as isize);
            let a: f32 = (1.0f32 - src_a)
                * (dest_alpha_to_float_coeff * dest_a as i32 as f32 + dest_alpha_to_float_offset);
            let b: f32 = flow_colorcontext_srgb_to_floatspace(colorcontext, dest_b) * a + src_b;
            let g: f32 = flow_colorcontext_srgb_to_floatspace(colorcontext, dest_g) * a + src_g;
            let r: f32 = flow_colorcontext_srgb_to_floatspace(colorcontext, dest_r) * a + src_r;
            let final_alpha: f32 = src_a + a;
            *dest_row_bytes.offset(0) =
                flow_colorcontext_floatspace_to_srgb(colorcontext, b / final_alpha);
            *dest_row_bytes.offset(1) =
                flow_colorcontext_floatspace_to_srgb(colorcontext, g / final_alpha);
            *dest_row_bytes.offset(2) =
                flow_colorcontext_floatspace_to_srgb(colorcontext, r / final_alpha);
            if dest_alpha {
                *dest_row_bytes.offset(3 as i32 as isize) =
                    uchar_clamp_ff(final_alpha * 255 as i32 as f32)
            }
            // TODO: split out 4 and 3 so compiler can vectorize maybe?
            dest_row_bytes = dest_row_bytes.offset(dest_pixel_stride as isize);
            ix = (ix as u32).wrapping_add(ch) as u32 as u32
        }
        row = row.wrapping_add(1)
    }
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_composite_linear_over_srgb(
    context: *mut flow_c,
    colorcontext: *mut flow_colorcontext_info,
    src_mut: *mut flow_bitmap_float,
    from_row: u32,
    dest: *mut flow_bitmap_bgra,
    dest_row: u32,
    row_count: u32,
    transpose: bool,
) -> bool {
    if if transpose as i32 != 0 {
        ((*src_mut).w != (*dest).h) as i32
    } else {
        ((*src_mut).w != (*dest).w) as i32
    } != 0
    {
        // TODO: Add more bounds checks
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1699 as i32,
            (*::std::mem::transmute::<&[u8; 45], &[libc::c_char; 45]>(
                b"flow_bitmap_float_composite_linear_over_srgb\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    if (*dest).compositing_mode as u32 == flow_bitmap_compositing_blend_with_self as i32 as u32
        && (*src_mut).alpha_meaningful as i32 != 0
        && (*src_mut).channels == 4 as i32 as u32
    {
        if !(*src_mut).alpha_premultiplied {
            // Something went wrong. We should always have alpha premultiplied.
            flow_context_set_error_get_message_buffer(
                context,
                flow_status_code::Invalid_internal_state,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1706 as i32,
                (*::std::mem::transmute::<&[u8; 45], &[libc::c_char; 45]>(
                    b"flow_bitmap_float_composite_linear_over_srgb\x00",
                ))
                .as_ptr(),
            );
            return false;
        }
        // Compose
        if !BitmapFloat_compose_linear_over_srgb(
            context,
            colorcontext,
            src_mut,
            from_row,
            dest,
            dest_row,
            row_count,
            0 as i32 as u32,
            (*src_mut).w,
            transpose,
        ) {
            flow_context_add_to_callstack(
                context,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1712 as i32,
                (*::std::mem::transmute::<&[u8; 45], &[libc::c_char; 45]>(
                    b"flow_bitmap_float_composite_linear_over_srgb\x00",
                ))
                .as_ptr(),
            );
            return false;
        }
    } else {
        if (*src_mut).channels == 4 as i32 as u32 && (*src_mut).alpha_meaningful as i32 != 0 {
            let mut demultiply: bool = (*src_mut).alpha_premultiplied;
            if (*dest).compositing_mode as u32
                == flow_bitmap_compositing_blend_with_matte as i32 as u32
            {
                if !flow_bitmap_float_blend_matte(
                    context,
                    colorcontext,
                    src_mut,
                    from_row,
                    row_count,
                    (*dest).matte_color.as_mut_ptr(),
                ) {
                    flow_context_add_to_callstack(
                        context,
                        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                        1722 as i32,
                        (*::std::mem::transmute::<&[u8; 45], &[libc::c_char; 45]>(
                            b"flow_bitmap_float_composite_linear_over_srgb\x00",
                        ))
                        .as_ptr(),
                    );
                    return false;
                }
                demultiply = false
            }
            if demultiply {
                // Demultiply before copy
                if !flow_bitmap_float_demultiply_alpha(context, src_mut, from_row, row_count) {
                    flow_context_add_to_callstack(
                        context,
                        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                        1730 as i32,
                        (*::std::mem::transmute::<&[u8; 45], &[libc::c_char; 45]>(
                            b"flow_bitmap_float_composite_linear_over_srgb\x00",
                        ))
                        .as_ptr(),
                    );
                    return false;
                }
            }
        }
        // Copy/overwrite
        if !flow_bitmap_float_copy_linear_over_srgb(
            context,
            colorcontext,
            src_mut,
            from_row,
            dest,
            dest_row,
            row_count,
            0 as i32 as u32,
            (*src_mut).w,
            transpose,
        ) {
            flow_context_add_to_callstack(
                context,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1738 as i32,
                (*::std::mem::transmute::<&[u8; 45], &[libc::c_char; 45]>(
                    b"flow_bitmap_float_composite_linear_over_srgb\x00",
                ))
                .as_ptr(),
            ); // Don't access rows past the end of the bitmap
            return false;
        }
    } // This algorithm can't handle padding, if present
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_linear_to_luv_rows(
    context: *mut flow_c,
    bit: *mut flow_bitmap_float,
    start_row: u32,
    row_count: u32,
) -> bool {
    if !(start_row.wrapping_add(row_count) <= (*bit).h) {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1751 as i32,
            (*::std::mem::transmute::<&[u8; 37], &[libc::c_char; 37]>(
                b"flow_bitmap_float_linear_to_luv_rows\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    if (*bit).w.wrapping_mul((*bit).channels) != (*bit).float_stride {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1755 as i32,
            (*::std::mem::transmute::<&[u8; 37], &[libc::c_char; 37]>(
                b"flow_bitmap_float_linear_to_luv_rows\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    let start_at: *mut f32 = (*bit)
        .pixels
        .offset((*bit).float_stride.wrapping_mul(start_row) as isize);
    let end_at: *const f32 = (*bit).pixels.offset(
        (*bit)
            .float_stride
            .wrapping_mul(start_row.wrapping_add(row_count)) as isize,
    );
    let mut pix: *mut f32 = start_at;
    while pix < end_at as *mut f32 {
        linear_to_luv(pix);
        pix = pix.offset(1)
    }
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_luv_to_linear_rows(
    context: *mut flow_c,
    bit: *mut flow_bitmap_float,
    start_row: u32,
    row_count: u32,
) -> bool {
    if !(start_row.wrapping_add(row_count) <= (*bit).h) {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1772 as i32,
            (*::std::mem::transmute::<&[u8; 37], &[libc::c_char; 37]>(
                b"flow_bitmap_float_luv_to_linear_rows\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    if (*bit).w.wrapping_mul((*bit).channels) != (*bit).float_stride {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1776 as i32,
            (*::std::mem::transmute::<&[u8; 37], &[libc::c_char; 37]>(
                b"flow_bitmap_float_luv_to_linear_rows\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    let start_at: *mut f32 = (*bit)
        .pixels
        .offset((*bit).float_stride.wrapping_mul(start_row) as isize);
    let end_at: *const f32 = (*bit).pixels.offset(
        (*bit)
            .float_stride
            .wrapping_mul(start_row.wrapping_add(row_count)) as isize,
    );
    let mut pix: *mut f32 = start_at;
    while pix < end_at as *mut f32 {
        luv_to_linear(pix);
        pix = pix.offset(1)
    }
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_bgra_apply_color_matrix(
    context: *mut flow_c,
    bmp: *mut flow_bitmap_bgra,
    row: u32,
    count: u32,
    m: *const *mut f32,
) -> bool {
    let stride: u32 = (*bmp).stride;
    let ch: u32 = flow_pixel_format_bytes_per_pixel((*bmp).fmt);
    let w: u32 = (*bmp).w;
    let h: u32 = row.wrapping_add(count).min((*bmp).h);
    let m40: f32 = *(*m.offset(4 as i32 as isize)).offset(0) * 255.0f32;
    let m41: f32 = *(*m.offset(4 as i32 as isize)).offset(1) * 255.0f32;
    let m42: f32 = *(*m.offset(4 as i32 as isize)).offset(2) * 255.0f32;
    let m43: f32 = *(*m.offset(4 as i32 as isize)).offset(3 as i32 as isize) * 255.0f32;
    if ch == 4 as i32 as u32 {
        let mut y: u32 = row;
        while y < h {
            let mut x: u32 = 0 as i32 as u32;
            while x < w {
                let data: *mut u8 = (*bmp)
                    .pixels
                    .offset(stride.wrapping_mul(y) as isize)
                    .offset(x.wrapping_mul(ch) as isize);
                let r: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(0) * *data.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(0) * *data.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(0) * *data.offset(0) as i32 as f32
                        + *(*m.offset(3 as i32 as isize)).offset(0)
                            * *data.offset(3 as i32 as isize) as i32 as f32
                        + m40,
                );
                let g: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(1) * *data.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(1) * *data.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(1) * *data.offset(0) as i32 as f32
                        + *(*m.offset(3 as i32 as isize)).offset(1)
                            * *data.offset(3 as i32 as isize) as i32 as f32
                        + m41,
                );
                let b: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(2) * *data.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(2) * *data.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(2) * *data.offset(0) as i32 as f32
                        + *(*m.offset(3 as i32 as isize)).offset(2)
                            * *data.offset(3 as i32 as isize) as i32 as f32
                        + m42,
                );
                let a: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(3 as i32 as isize) * *data.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(3 as i32 as isize) * *data.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(3 as i32 as isize) * *data.offset(0) as i32 as f32
                        + *(*m.offset(3 as i32 as isize)).offset(3 as i32 as isize)
                            * *data.offset(3 as i32 as isize) as i32 as f32
                        + m43,
                );
                let newdata: *mut u8 = (*bmp)
                    .pixels
                    .offset(stride.wrapping_mul(y) as isize)
                    .offset(x.wrapping_mul(ch) as isize);
                *newdata.offset(0) = b;
                *newdata.offset(1) = g;
                *newdata.offset(2) = r;
                *newdata.offset(3 as i32 as isize) = a;
                x = x.wrapping_add(1)
            }
            y = y.wrapping_add(1)
        }
    } else if ch == 3 as i32 as u32 {
        let mut y_0: u32 = row;
        while y_0 < h {
            let mut x_0: u32 = 0 as i32 as u32;
            while x_0 < w {
                let data_0: *mut libc::c_uchar = (*bmp)
                    .pixels
                    .offset(stride.wrapping_mul(y_0) as isize)
                    .offset(x_0.wrapping_mul(ch) as isize);
                let r_0: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(0) * *data_0.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(0) * *data_0.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(0) * *data_0.offset(0) as i32 as f32
                        + m40,
                );
                let g_0: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(1) * *data_0.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(1) * *data_0.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(1) * *data_0.offset(0) as i32 as f32
                        + m41,
                );
                let b_0: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(2) * *data_0.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(2) * *data_0.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(2) * *data_0.offset(0) as i32 as f32
                        + m42,
                );
                let newdata_0: *mut u8 = (*bmp)
                    .pixels
                    .offset(stride.wrapping_mul(y_0) as isize)
                    .offset(x_0.wrapping_mul(ch) as isize);
                *newdata_0.offset(0) = b_0;
                *newdata_0.offset(1) = g_0;
                *newdata_0.offset(2) = r_0;
                x_0 = x_0.wrapping_add(1)
            }
            y_0 = y_0.wrapping_add(1)
        }
    } else {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Unsupported_pixel_format,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1838 as i32,
            (*::std::mem::transmute::<&[u8; 36], &[libc::c_char; 36]>(
                b"flow_bitmap_bgra_apply_color_matrix\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    return true;
}
// note: this file isn't exercised by test suite
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_apply_color_matrix(
    context: *mut flow_c,
    bmp: *mut flow_bitmap_float,
    row: u32,
    count: u32,
    m: *mut *mut f32,
) -> bool {
    let stride: u32 = (*bmp).float_stride;
    let ch: u32 = (*bmp).channels;
    let w: u32 = (*bmp).w;
    let h: u32 = row.wrapping_add(count).min((*bmp).h);
    match ch {
        4 => {
            let mut y: u32 = row;
            while y < h {
                let mut x: u32 = 0 as i32 as u32;
                while x < w {
                    let data: *mut f32 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y) as isize)
                        .offset(x.wrapping_mul(ch) as isize);
                    let r: f32 = *(*m.offset(0)).offset(0) * *data.offset(2)
                        + *(*m.offset(1)).offset(0) * *data.offset(1)
                        + *(*m.offset(2)).offset(0) * *data.offset(0)
                        + *(*m.offset(3 as i32 as isize)).offset(0)
                            * *data.offset(3 as i32 as isize)
                        + *(*m.offset(4 as i32 as isize)).offset(0);
                    let g: f32 = *(*m.offset(0)).offset(1) * *data.offset(2)
                        + *(*m.offset(1)).offset(1) * *data.offset(1)
                        + *(*m.offset(2)).offset(1) * *data.offset(0)
                        + *(*m.offset(3 as i32 as isize)).offset(1)
                            * *data.offset(3 as i32 as isize)
                        + *(*m.offset(4 as i32 as isize)).offset(1);
                    let b: f32 = *(*m.offset(0)).offset(2) * *data.offset(2)
                        + *(*m.offset(1)).offset(2) * *data.offset(1)
                        + *(*m.offset(2)).offset(2) * *data.offset(0)
                        + *(*m.offset(3 as i32 as isize)).offset(2)
                            * *data.offset(3 as i32 as isize)
                        + *(*m.offset(4 as i32 as isize)).offset(2);
                    let a: f32 = *(*m.offset(0)).offset(3 as i32 as isize) * *data.offset(2)
                        + *(*m.offset(1)).offset(3 as i32 as isize) * *data.offset(1)
                        + *(*m.offset(2)).offset(3 as i32 as isize) * *data.offset(0)
                        + *(*m.offset(3 as i32 as isize)).offset(3 as i32 as isize)
                            * *data.offset(3 as i32 as isize)
                        + *(*m.offset(4 as i32 as isize)).offset(3 as i32 as isize);
                    let newdata: *mut f32 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y) as isize)
                        .offset(x.wrapping_mul(ch) as isize);
                    *newdata.offset(0) = b;
                    *newdata.offset(1) = g;
                    *newdata.offset(2) = r;
                    *newdata.offset(3 as i32 as isize) = a;
                    x = x.wrapping_add(1)
                }
                y = y.wrapping_add(1)
            }
            return true;
        }
        3 => {
            let mut y_0: u32 = row;
            while y_0 < h {
                let mut x_0: u32 = 0 as i32 as u32;
                while x_0 < w {
                    let data_0: *mut f32 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y_0) as isize)
                        .offset(x_0.wrapping_mul(ch) as isize);
                    let r_0: f32 = *(*m.offset(0)).offset(0) * *data_0.offset(2)
                        + *(*m.offset(1)).offset(0) * *data_0.offset(1)
                        + *(*m.offset(2)).offset(0) * *data_0.offset(0)
                        + *(*m.offset(4 as i32 as isize)).offset(0);
                    let g_0: f32 = *(*m.offset(0)).offset(1) * *data_0.offset(2)
                        + *(*m.offset(1)).offset(1) * *data_0.offset(1)
                        + *(*m.offset(2)).offset(1) * *data_0.offset(0)
                        + *(*m.offset(4 as i32 as isize)).offset(1);
                    let b_0: f32 = *(*m.offset(0)).offset(2) * *data_0.offset(2)
                        + *(*m.offset(1)).offset(2) * *data_0.offset(1)
                        + *(*m.offset(2)).offset(2) * *data_0.offset(0)
                        + *(*m.offset(4 as i32 as isize)).offset(2);
                    let newdata_0: *mut f32 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y_0) as isize)
                        .offset(x_0.wrapping_mul(ch) as isize);
                    *newdata_0.offset(0) = b_0;
                    *newdata_0.offset(1) = g_0;
                    *newdata_0.offset(2) = r_0;
                    x_0 = x_0.wrapping_add(1)
                }
                y_0 = y_0.wrapping_add(1)
            }
            return true;
        }
        _ => {
            flow_context_set_error_get_message_buffer(
                context,
                flow_status_code::Unsupported_pixel_format,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1893 as i32,
                (*::std::mem::transmute::<&[u8; 37], &[libc::c_char; 37]>(
                    b"flow_bitmap_float_apply_color_matrix\x00",
                ))
                .as_ptr(),
            );
            return false;
        }
    };
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_bgra_populate_histogram(
    context: *mut flow_c,
    bmp: *const flow_bitmap_bgra,
    histograms: *mut u64,
    histogram_size_per_channel: u32,
    histogram_count: u32,
    pixels_sampled: *mut u64,
) -> bool {
    let row: u32 = 0;
    let count: u32 = (*bmp).h;
    let stride: u32 = (*bmp).stride;
    let ch: u32 = flow_pixel_format_bytes_per_pixel((*bmp).fmt);
    let w: u32 = (*bmp).w;
    let h: u32 = (row.wrapping_add(count)).min((*bmp).h);
    if histogram_size_per_channel != 256 {
        // We're restricting it to this for speed
        FLOW_error(
            context,
            flow_status_code::Invalid_argument,
            "flow_bitmap_bgra_populate_histogram",
        );
        return false;
    }
    let shift = 0; // 8 - intlog2(histogram_size_per_channel);
    if ch == 4 || ch == 3 {
        if histogram_count == 1 {
            let mut y: u32 = row;
            while y < h {
                let mut x: u32 = 0;
                while x < w {
                    let data: *const u8 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y) as isize)
                        .offset(x.wrapping_mul(ch) as isize);
                    let ref mut fresh9 = *histograms.offset(
                        (306 as i32 * *data.offset(2) as i32
                            + 601 as i32 * *data.offset(1) as i32
                            + 117 as i32 * *data.offset(0) as i32
                            >> shift) as isize,
                    );
                    *fresh9 = (*fresh9).wrapping_add(1);
                    x = x.wrapping_add(1)
                }
                y = y.wrapping_add(1)
            }
        } else if histogram_count == 3 {
            let mut y: u32 = row;
            while y < h {
                let mut x: u32 = 0;
                while x < w {
                    let data: *const u8 = (*bmp)
                        .pixels
                        .offset((stride * y) as isize)
                        .offset((x * ch) as isize);
                    let ref mut fresh10 =
                        *histograms.offset((*data.offset(2) as i32 >> shift) as isize);
                    *fresh10 = (*fresh10).wrapping_add(1);
                    let ref mut fresh11 = *histograms.offset(
                        ((*data.offset(1) as i32 >> shift) as u32)
                            .wrapping_add(histogram_size_per_channel)
                            as isize,
                    );
                    *fresh11 = (*fresh11).wrapping_add(1);
                    let ref mut fresh12 = *histograms.offset(
                        ((*data.offset(0) as i32 >> shift) as u32)
                            .wrapping_add((2u32).wrapping_mul(histogram_size_per_channel))
                            as isize,
                    );
                    *fresh12 = (*fresh12).wrapping_add(1);
                    x = x.wrapping_add(1)
                }
                y = y.wrapping_add(1)
            }
        } else if histogram_count == 2 {
            let mut y_1: u32 = row;
            while y_1 < h {
                let mut x_1: u32 = 0 as i32 as u32;
                while x_1 < w {
                    let data_1: *const u8 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y_1) as isize)
                        .offset(x_1.wrapping_mul(ch) as isize);
                    // Calculate luminosity and saturation
                    let ref mut fresh13 = *histograms.offset(
                        (306 as i32 * *data_1.offset(2) as i32
                            + 601 as i32 * *data_1.offset(1) as i32
                            + 117 as i32 * *data_1.offset(0) as i32
                            >> shift) as isize,
                    );
                    *fresh13 = (*fresh13).wrapping_add(1);
                    let ref mut fresh14 =
                        *histograms.offset(histogram_size_per_channel.wrapping_add(
                            (int_max(
                                255 as i32,
                                int_max(
                                    (*data_1.offset(2) as i32 - *data_1.offset(1) as i32).abs(),
                                    (*data_1.offset(1) as i32 - *data_1.offset(0) as i32).abs(),
                                ),
                            ) >> shift) as u32,
                        ) as isize);
                    *fresh14 = (*fresh14).wrapping_add(1);
                    x_1 = x_1.wrapping_add(1)
                }
                y_1 = y_1.wrapping_add(1)
            }
        } else {
            flow_context_set_error_get_message_buffer(
                context,
                flow_status_code::Invalid_internal_state,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1950 as i32,
                (*::std::mem::transmute::<&[u8; 36], &[libc::c_char; 36]>(
                    b"flow_bitmap_bgra_populate_histogram\x00",
                ))
                .as_ptr(),
            );
            return false;
        }
        *pixels_sampled = h.wrapping_sub(row).wrapping_mul(w) as u64
    } else {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Unsupported_pixel_format,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1956 as i32,
            (*::std::mem::transmute::<&[u8; 36], &[libc::c_char; 36]>(
                b"flow_bitmap_bgra_populate_histogram\x00",
            ))
            .as_ptr(),
        );
        return false;
    }
    return true;
}
// Gamma correction  http://www.4p8.com/eric.brasseur/gamma.html#formulas
#[no_mangle]
pub unsafe extern "C" fn flow_colorcontext_init(
    _context: *mut flow_c,
    mut colorcontext: *mut flow_colorcontext_info,
    space: flow_working_floatspace,
    a: f32,
    _b: f32,
    _c: f32,
) {
    (*colorcontext).floatspace = space;
    (*colorcontext).apply_srgb = (space & flow_working_floatspace_linear) > 0;
    (*colorcontext).apply_gamma = (space & flow_working_floatspace_gamma) > 0;
    /* Code guarded by #ifdef EXPOSE_SIGMOID not translated */
    if (*colorcontext).apply_gamma {
        (*colorcontext).gamma = a;
        (*colorcontext).gamma_inverse = (1.0f64 / a as f64) as f32
    }
    for n in 0..256 {
        (*colorcontext).byte_to_float[n] =
            flow_colorcontext_srgb_to_floatspace_uncached(colorcontext, n as u8);
    }
}
