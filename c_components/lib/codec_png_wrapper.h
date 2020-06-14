#pragma once
#include "codec_wrappers.h"
#include "png.h"

#define PUB FLOW_EXPORT

#ifdef __cplusplus
extern "C" {
#endif

/*
 *
typedef enum flow_codec_color_profile_source {
    flow_codec_color_profile_source_null = 0,
    flow_codec_color_profile_source_ICCP = 1,
    flow_codec_color_profile_source_ICCP_GRAY = 2,
    flow_codec_color_profile_source_GAMA_CHRM = 3,
    flow_codec_color_profile_source_sRGB = 4,

} flow_codec_color_profile_source;


struct flow_decoder_color_info{
    flow_codec_color_profile_source source;
    uint8_t * profile_buf;
    size_t buf_length;
    cmsCIExyY white_point;
    cmsCIExyYTRIPLE primaries;
    double gamma;
};
 */
struct wrap_png_decoder_state;

typedef void (*wrap_png_error_handler) (png_structp png_ptr, void * custom_state, const char * error_message);

typedef  bool (*wrap_png_custom_read_function) (png_structp png_ptr, void * custom_state, uint8_t * buffer, size_t bytes_requested, size_t * out_bytes_read);


PUB size_t wrap_png_decoder_state_bytes(void);

PUB bool wrap_png_decoder_state_init(struct wrap_png_decoder_state * state, void * custom_state,
                                        wrap_png_error_handler error_handler, wrap_png_custom_read_function read_function);

PUB bool wrap_png_decode_image_info(struct wrap_png_decoder_state * state);

PUB bool wrap_png_decode_finish(struct wrap_png_decoder_state * state, uint8_t * * row_pointers, size_t row_count, size_t row_bytes);

PUB void * wrap_png_decoder_get_png_ptr(struct wrap_png_decoder_state * state);

PUB void * wrap_png_decoder_get_info_ptr(struct wrap_png_decoder_state * state);


PUB bool wrap_png_decoder_destroy(struct wrap_png_decoder_state * state);

PUB bool wrap_png_decoder_get_info(struct wrap_png_decoder_state * state, uint32_t * w, uint32_t * h, bool * uses_alpha);

PUB struct flow_decoder_color_info * wrap_png_decoder_get_color_info(struct wrap_png_decoder_state * state);

typedef  bool (*wrap_png_custom_write_function) (png_structp png_ptr, void * custom_state, uint8_t * buffer, size_t buffer_length);



PUB bool wrap_png_encoder_write_png(void * custom_state,
                                    wrap_png_error_handler error_handler,
                                    wrap_png_custom_write_function write_function,
                                    uint8_t * * row_pointers,
                                    size_t w,
                                    size_t h,
                                    bool disable_png_alpha,
                                    int zlib_compression_level,
                                    flow_pixel_format pixel_format);

#ifdef __cplusplus
}
#endif

