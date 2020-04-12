#pragma once
#include "codecs.h"
#include "lcms2.h"

#ifdef __cplusplus
extern "C" {
#endif
typedef enum flow_codecs_jpeg_decoder_stage {
    flow_codecs_jpg_decoder_stage_Null = 0,
    flow_codecs_jpg_decoder_stage_Failed,
    flow_codecs_jpg_decoder_stage_NotStarted,
    flow_codecs_jpg_decoder_stage_BeginRead,
    flow_codecs_jpg_decoder_stage_FinishRead,
} flow_codecs_jpeg_decoder_stage;

struct flow_codecs_jpeg_decoder_state;

typedef uint8_t (*flow_codecs_jpeg_linear_to_srgb)(struct flow_codecs_jpeg_decoder_state * state, float v);

struct flow_codecs_jpeg_decoder_state {
    struct jpeg_error_mgr error_mgr;
    jmp_buf error_handler_jmp;
    flow_c * context;
    size_t codec_id;
    flow_codecs_jpeg_decoder_stage stage;
    struct jpeg_decompress_struct * cinfo;
    size_t row_stride;
    int32_t w;
    int32_t h;
    int32_t exif_orientation;
    int channels;
    struct flow_io * io;
    struct flow_bitmap_bgra * canvas;
    uint8_t * pixel_buffer;
    size_t pixel_buffer_size;
    uint8_t ** pixel_buffer_row_pointers;

    struct flow_decoder_color_info color;

    struct flow_decoder_downscale_hints hints;
    float lut_to_linear[256];
    uint8_t flat_lut_linear[256 * 13];

};


#ifdef __cplusplus
}
#endif
