#pragma once

#include "imageflow_private.h"
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

struct flow_codec_definition * flow_job_get_codec_definition(flow_c * c, int64_t codec_id);
int64_t flow_job_codec_select(flow_c * c, struct flow_job * job, uint8_t * data, size_t data_bytes);

bool flow_job_initialize_codec(flow_c * c, struct flow_job * job, struct flow_codec_instance * item);

bool flow_job_decoder_get_frame_info(flow_c * c, struct flow_job * job, void * codec_state, int64_t codec_id,
                                     struct flow_decoder_frame_info * decoder_frame_info_ref);

bool flow_job_decoder_read_frame(flow_c * c, struct flow_job * job, void * codec_state, int64_t codec_id,
                                 struct flow_bitmap_bgra * canvas);

png_bytepp flow_job_create_row_pointers(flow_c * c, void * buffer, size_t buffer_size, size_t stride, size_t height);

bool flow_bitmap_bgra_transform_to_srgb(flow_c * c, cmsHPROFILE current_profile, struct flow_bitmap_bgra * frame);

typedef struct jpeg_compress_struct * j_compress_ptr;
typedef struct jpeg_decompress_struct * j_decompress_ptr;

void flow_codecs_jpeg_setup_source_manager(j_decompress_ptr cinfo, struct flow_io * io);
void flow_codecs_jpeg_setup_dest_manager(j_compress_ptr cinfo, struct flow_io * io);

#ifdef __cplusplus
}
#endif
