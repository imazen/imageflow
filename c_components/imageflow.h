#pragma once
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <stdio.h>
#include <setjmp.h>

#ifdef __cplusplus
extern "C" {
#endif
#if defined(_WIN32)

#if defined(imageflow_c_BUILD_SHARED)
/* Cmake will define imageflow_EXPORTS on Windows when it
configures to build a shared library.*/
#define FLOW_EXPORT __declspec(dllexport)
#else
#if defined(imageflow_c_BUILD_STATIC)
#define FLOW_EXPORT
#else
#define FLOW_EXPORT __declspec(dllimport)
#endif
#endif /* imageflow_EXPORTS */
#else /* defined (_WIN32) */
#define FLOW_EXPORT
#endif

#define PUB FLOW_EXPORT

// Version selection is not implemented within imageflow, instead, we let callers do that logic:
// Expose API to evaluate graph and suggest minimum source dimensions.
// Returns "indeterminate" if face or whitespace cropping is in use, or any other conditionals.

// Source/output files and I/O interfaces must be registered with the context. They can survive multiple ImageJobs.

// ImageJobs may eventually associate an opaque cache for dimensions, metadata, and (potentially) bitmap data with these
// I/O
// interfaces.

// * We heavily experimented with LUV and XYZ color spaces, but determined that better results occur using RGB linear.
// * A custom sigmoidized color space could perhaps improve things, but would introduce significant overhead.

typedef enum FLOW_DIRECTION { FLOW_OUTPUT = 8, FLOW_INPUT = 4 } FLOW_DIRECTION;

typedef enum flow_compositing_mode {
    flow_compositing_mode_overwrite,
    flow_compositing_mode_compose,
    flow_compositing_mode_blend_with_matte
} flow_compositing_mode;

typedef enum flow_codec_type {
    flow_codec_type_null = 0,
    flow_codec_type_decode_png = 1,
    flow_codec_type_encode_png = 2,
    flow_codec_type_decode_jpeg = 3,
    flow_codec_type_encode_jpeg = 4,
    flow_codec_type_decode_gif = 5
} flow_codec_type;

typedef enum flow_scanlines_filter_type {
    flow_scanlines_filter_Sharpen, // 3x3, percentage-based
    flow_scanlines_filter_Blur, // 3x box blur to simulate Gaussian
    flow_scanlines_filter_Convolve, // Apply convolution kernel
    flow_scanlines_filter_ColorMatrix, // Apply color matrix
    flow_scanlines_filter_ToLinear,
    flow_scanlines_filter_ToSrgb,
    flow_scanlines_filter_Custom, // Execute custom callback.,
    flow_scanlines_filter__FORCE_ENUM_SIZE_INT32 = 2147483647
} flow_scanlines_filter_type;

typedef enum flow_status_code {
    flow_status_No_Error = 0,
    flow_status_Out_of_memory = 10,
    flow_status_IO_error = 20,
    flow_status_Invalid_internal_state = 30,
    flow_status_Panic = 31,
    flow_status_Not_implemented = 40,
    flow_status_Invalid_argument = 50,
    flow_status_Null_argument = 51,
    flow_status_Invalid_dimensions = 52,
    flow_status_Unsupported_pixel_format = 53,
    flow_status_Item_does_not_exist = 54,

    flow_status_Image_decoding_failed = 60,
    flow_status_Image_encoding_failed = 61,
    flow_status_ErrorReportingInconsistency = 90,
    flow_status_First_rust_error = 200,

    flow_status_Other_error = 1024,
    flow_status____Last_library_error,
    flow_status_First_user_defined_error = 1025,
    flow_status_Last_user_defined_error = 2147483647
} flow_status_code;

typedef enum flow_interpolation_filter {
    flow_interpolation_filter_RobidouxFast = 1,
    flow_interpolation_filter_Robidoux = 2,
    flow_interpolation_filter_RobidouxSharp = 3,
    flow_interpolation_filter_Ginseng = 4,
    flow_interpolation_filter_GinsengSharp = 5,
    flow_interpolation_filter_Lanczos = 6,
    flow_interpolation_filter_LanczosSharp = 7,
    flow_interpolation_filter_Lanczos2 = 8,
    flow_interpolation_filter_Lanczos2Sharp = 9,
    flow_interpolation_filter_CubicFast = 10,
    flow_interpolation_filter_Cubic = 11,
    flow_interpolation_filter_CubicSharp = 12,
    flow_interpolation_filter_CatmullRom = 13,
    flow_interpolation_filter_Mitchell = 14,

    flow_interpolation_filter_CubicBSpline = 15,
    flow_interpolation_filter_Hermite = 16,
    flow_interpolation_filter_Jinc = 17,
    flow_interpolation_filter_RawLanczos3 = 18,
    flow_interpolation_filter_RawLanczos3Sharp = 19,
    flow_interpolation_filter_RawLanczos2 = 20,
    flow_interpolation_filter_RawLanczos2Sharp = 21,
    flow_interpolation_filter_Triangle = 22,
    flow_interpolation_filter_Linear = 23,
    flow_interpolation_filter_Box = 24,
    flow_interpolation_filter_CatmullRomFast = 25,
    flow_interpolation_filter_CatmullRomFastSharp = 26,

    flow_interpolation_filter_Fastest = 27,

    flow_interpolation_filter_MitchellFast = 28,

    flow_interpolation_filter_NCubic = 29,

    flow_interpolation_filter_NCubicSharp = 30
} flow_interpolation_filter;

// TODO: So many more - 8-bit, compressed data,
typedef enum flow_pixel_format { flow_bgr24 = 3, flow_bgra32 = 4, flow_bgr32 = 70, flow_gray8 = 1 } flow_pixel_format;

typedef enum flow_bitmap_compositing_mode {
    flow_bitmap_compositing_replace_self = 0,
    flow_bitmap_compositing_blend_with_self = 1,
    flow_bitmap_compositing_blend_with_matte = 2
} flow_bitmap_compositing_mode;

typedef enum flow_working_floatspace {
    flow_working_floatspace_srgb = 0,
    flow_working_floatspace_as_is = 0,
    flow_working_floatspace_linear = 1,
    flow_working_floatspace_gamma = 2
} flow_working_floatspace;

typedef enum flow_io_mode {
    flow_io_mode_null = 0,
    flow_io_mode_read_sequential = 1,
    flow_io_mode_write_sequential = 2,
    flow_io_mode_read_seekable = 5, // 1 | 4,
    flow_io_mode_write_seekable = 6, // 2 | 4,
    flow_io_mode_read_write_seekable = 15 // 1 | 2 | 4 | 8
} flow_io_mode;

typedef struct flow_context flow_c;
struct flow_codec_definition;
struct flow_codec_instance; // All methods should center around this

struct flow_scanlines_filter;
struct flow_decoder_downscale_hints;
struct flow_bitmap_bgra;

struct flow_encoder_hints {
    bool disable_png_alpha;
    int zlib_compression_level;
};



PUB flow_c * flow_context_create(void);

// When you need to control 100% of heap operations, you can allocate
// flow_context_sizeof_context() bytes and initialize them with flow_context_initialize,
// then call flow_heap_set_custom. Use flow_context_terminate and your matching free() function instead of
// flow_context_destroy
PUB size_t flow_context_sizeof_context_struct(void);
PUB void flow_context_initialize(flow_c * c);
// Did you allocate memory without an owner? Check remaining allocation records after begin_terminate to verify
// correctness.
// Call begin_terminate, then check on error status and memory stats.
// Or, you may call begin/end simply because the context was on the stack
// Or, because you want to check error status after all destructors are called, but before the stacktrace and message
// are freed
PUB bool flow_context_begin_terminate(flow_c * c);
// You should call flow_context_destroy unless the context is stack allocated.
PUB void flow_context_end_terminate(flow_c * c);

// Terminates the context, but does not permit you to check errors that happen during tear-down as begin/end terminate
// do
PUB void flow_context_terminate(flow_c * context);

PUB void flow_context_destroy(flow_c * c); // Don't pass this a pointer on the stack! use begin/end terminate

PUB bool flow_context_error_status_included_in_message(flow_c * context); //Useful for roundtripping
PUB bool flow_context_set_error_get_message_buffer_info(flow_c * context, flow_status_code code, bool status_included_in_buffer, char * * buffer,  size_t * buffer_size);
PUB int64_t flow_context_error_and_stacktrace(flow_c * c, char * buffer, size_t buffer_size, bool full_file_path);
PUB int64_t flow_context_error_message(flow_c * c, char * buffer, size_t buffer_size);
PUB int64_t flow_context_stacktrace(flow_c * c, char * buffer, size_t buffer_size, bool full_file_path);

PUB bool flow_context_has_error(flow_c * c);
PUB int flow_context_error_reason(flow_c * c);

PUB bool flow_context_print_and_exit_if_err(flow_c * c);

PUB void flow_context_clear_error(flow_c * c);

PUB void flow_context_print_error_to(flow_c * c, FILE * stream);

PUB void flow_context_print_memory_info(flow_c * c);

// Flush buffers; close files     ; release underlying resources - the job has been ended.
typedef bool (*flow_destructor_function)(flow_c * c, void * thing);

// Assuming, here, that we never get a pointer to address 42 in memory.
#define FLOW_OWNER_IMMORTAL ((void *)42)

PUB struct flow_io * flow_io_create_for_file(flow_c * c, flow_io_mode mode, const char * filename, void * owner);

PUB struct flow_io * flow_io_create_from_file_pointer(flow_c * c, flow_io_mode mode, FILE * file_pointer,
                                                      int64_t optional_file_length, void * owner);

PUB struct flow_io * flow_io_create_from_memory(flow_c * c, flow_io_mode mode, uint8_t * memory, size_t length,
                                                void * owner, flow_destructor_function memory_free);
PUB struct flow_io * flow_io_create_for_output_buffer(flow_c * c, void * owner);

// Returns false if the flow_io struct is disposed or not an output buffer type (or for any other error)
PUB bool flow_io_get_output_buffer(flow_c * c, struct flow_io * io, uint8_t ** out_pointer_to_buffer,
                                   size_t * out_length);

PUB bool flow_io_write_output_buffer_to_file(flow_c * c, struct flow_io * io, const char * file_path);


PUB uint32_t flow_pixel_format_bytes_per_pixel(flow_pixel_format format);
PUB flow_pixel_format flow_effective_pixel_format(struct flow_bitmap_bgra * b);
PUB uint32_t flow_pixel_format_channels(flow_pixel_format format);

struct flow_decoder_info {
    int64_t codec_id;
    const char * preferred_mime_type;
    const char * preferred_extension;
    size_t frame_count;
    int64_t current_frame_index;
    // Not applicable to TIFF files - will be the first frame instead
    int32_t image_width;
    int32_t image_height;
    flow_pixel_format frame_decodes_into;
    // const char * format_subtype;
    // bool flow_profile_is_srgb;
};

PUB bool flow_bitmap_bgra_write_png(flow_c * c, struct flow_bitmap_bgra * frame, struct flow_io * io);
PUB bool flow_bitmap_bgra_write_png_with_hints(flow_c * c, struct flow_bitmap_bgra * frame, struct flow_io * io, struct flow_encoder_hints * hints);

#undef PUB

#ifdef __cplusplus
}
#endif
