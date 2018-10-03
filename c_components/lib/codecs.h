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


void flow_decoder_color_info_init(struct flow_decoder_color_info * color);

struct flow_codec_definition * flow_codec_get_definition(flow_c * c, int64_t codec_id);
bool flow_codec_decoder_get_info(flow_c * c, void * codec_state, int64_t codec_id, struct flow_decoder_info * info);

int64_t flow_codec_select(flow_c * c, uint8_t * data, size_t data_bytes);

bool flow_codec_initialize(flow_c * c, struct flow_codec_instance * item);

bool flow_codec_decoder_get_frame_info(flow_c * c, void * codec_state, int64_t codec_id,
                                       struct flow_decoder_frame_info * decoder_frame_info_ref);

bool flow_codec_decoder_read_frame(flow_c * c, void * codec_state, int64_t codec_id, struct flow_bitmap_bgra * canvas, struct flow_decoder_color_info * color_info);

typedef struct jpeg_decompress_struct * j_decompress_ptr;

void flow_codecs_jpeg_setup_source_manager(j_decompress_ptr cinfo, struct flow_io * io);


#ifdef __cplusplus
}
#endif
