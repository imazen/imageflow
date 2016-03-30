#pragma once

#include "imageflow_private.h"
#include <jpeglib.h>
#include "lcms2.h"

#ifdef __cplusplus
extern "C" {
#endif

struct flow_decoder_frame_info {
    int32_t w;
    int32_t h;
    flow_pixel_format format;
};

typedef enum flow_codec_color_profile_source {
    flow_codec_color_profile_source_null,
    flow_codec_color_profile_source_ICCP,
    flow_codec_color_profile_source_ICCP_GRAY,
    flow_codec_color_profile_source_GAMA_CHRM,

} flow_codec_color_profile_source;

// Later we may want to expose this information to the outside
// struct flow_decoder_info {
//    cmsHPROFILE color_profile;
//    flow_codec_color_profile_source color_profile_source;
//};

// typedef unsigned long png_uint_32;

struct flow_codec_definition* flow_job_get_codec_definition(flow_context* c, int64_t codec_id);
int64_t flow_job_codec_select(flow_context* c, struct flow_job* job, uint8_t* data, size_t data_bytes);

bool flow_job_initialize_codec(flow_context* c, struct flow_job* job, struct flow_codec_instance* item);

bool flow_job_decoder_get_frame_info(flow_context* c, struct flow_job* job, void* codec_state, int64_t codec_id,
                                     struct flow_decoder_frame_info* decoder_frame_info_ref);

bool flow_job_decoder_read_frame(flow_context* c, struct flow_job* job, void* codec_state, int64_t codec_id,
                                 flow_bitmap_bgra* canvas);

bool flow_job_codecs_initialize_encode_jpeg(flow_context* c, struct flow_job* job, struct flow_codec_instance* item);
bool flow_job_codecs_initialize_decode_jpeg(flow_context* c, struct flow_job* job, struct flow_codec_instance* item);

bool flow_job_codecs_jpeg_get_info(flow_context* c, struct flow_job* job, void* codec_state,
                                   struct flow_decoder_frame_info* decoder_frame_info_ref);
bool flow_job_codecs_jpeg_read_frame(flow_context* c, struct flow_job* job, void* codec_state,
                                     flow_bitmap_bgra* canvas);

bool flow_job_codecs_jpeg_write_frame(flow_context* c, struct flow_job* job, void* codec_state,
                                      flow_bitmap_bgra* frame);

bool flow_job_codecs_initialize_decode_png(flow_context* c, struct flow_job* job, struct flow_codec_instance* item);

bool flow_job_codecs_png_write_frame(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra* frame);

bool flow_job_codecs_png_get_frame_info(flow_context* c, struct flow_job* job, void* codec_state,
                                        struct flow_decoder_frame_info* decoder_frame_info_ref);
bool flow_job_codecs_png_get_info(flow_context* c, struct flow_job* job, void* codec_state,
                                  struct flow_decoder_info* info_ref);

bool flow_job_codecs_initialize_encode_png(flow_context* c, struct flow_job* job, struct flow_codec_instance* item);

bool flow_job_codecs_png_read_frame(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra* canvas);

bool flow_job_codecs_gif_initialize(flow_context* c, struct flow_job* job, struct flow_codec_instance* codec);

bool flow_job_codecs_gif_get_info(flow_context* c, struct flow_job* job, void* codec_state,
                                  struct flow_decoder_info* info_ref);
bool flow_job_codecs_decode_gif_switch_frame(flow_context* c, struct flow_job* job, void* codec_state,
                                             size_t frame_index);

bool flow_job_codecs_gif_get_frame_info(flow_context* c, struct flow_job* job, void* codec_state,
                                        struct flow_decoder_frame_info* info_ref);
bool flow_job_codecs_gif_read_frame(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra* canvas);

png_bytepp flow_job_create_row_pointers(flow_context* c, void* buffer, size_t buffer_size, size_t stride,
                                        size_t height);

bool flow_bitmap_bgra_transform_to_srgb(flow_context* c, cmsHPROFILE current_profile, flow_bitmap_bgra* frame);

// bool flow_job_codecs_png_write_frame(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra*
// frame);

void flow_codecs_jpeg_setup_source_manager(j_decompress_ptr cinfo, struct flow_io* io);
void flow_codecs_jpeg_setup_dest_manager(j_compress_ptr cinfo, struct flow_io* io);

#ifdef __cplusplus
}
#endif
