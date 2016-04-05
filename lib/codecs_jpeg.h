#pragma once
#include "codecs.h"
#include "lcms2.h"

#ifdef __cplusplus
extern "C" {
#endif
typedef enum flow_job_jpeg_decoder_stage {
    flow_job_jpg_decoder_stage_Null = 0,
    flow_job_jpg_decoder_stage_Failed,
    flow_job_jpg_decoder_stage_NotStarted,
    flow_job_jpg_decoder_stage_BeginRead,
    flow_job_jpg_decoder_stage_FinishRead,
} flow_job_jpeg_decoder_stage;

struct flow_job_jpeg_decoder_state;

typedef uint8_t (*flow_job_jpeg_linear_to_srgb)(struct flow_job_jpeg_decoder_state * state, float v);

struct flow_job_jpeg_decoder_state {
    struct jpeg_error_mgr error_mgr; // MUST be first
    jmp_buf error_handler_jmp; // MUST be second
    flow_c * context; // MUST be third
    size_t codec_id; // MUST be fourth
    flow_job_jpeg_decoder_stage stage;
    struct jpeg_decompress_struct * cinfo;
    size_t row_stride;
    int32_t w;
    int32_t h;
    int channels;
    struct flow_io * io;
    uint8_t * pixel_buffer;
    size_t pixel_buffer_size;
    uint8_t ** pixel_buffer_row_pointers;

    cmsHPROFILE color_profile;
    flow_codec_color_profile_source color_profile_source;
    double gamma;

    struct flow_decoder_downscale_hints hints;
    float lut_to_linear[256];
    uint8_t flat_lut_linear[256 * 13];
};


struct flow_job_jpeg_codec_state_common {
    struct jpeg_error_mgr error_mgr; // MUST be first
    jmp_buf error_handler_jmp; // MUST be second
    flow_c * context; // MUST be third
    size_t codec_id; // MUST be fourth
};
struct flow_job_jpeg_encoder_state {
    struct jpeg_error_mgr error_mgr; // MUST be first
    jmp_buf error_handler_jmp; // MUST be second
    flow_c * context; // MUST be third
    size_t codec_id; // MUST be fourth
    struct jpeg_compress_struct cinfo;
    struct flow_io * io;
};

#ifdef __cplusplus
}
#endif
