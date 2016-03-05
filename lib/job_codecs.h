#pragma once


typedef enum flow_job_color_profile_source {
    flow_job_color_profile_source_null,
    flow_job_color_profile_source_ICCP,
    flow_job_color_profile_source_ICCP_GRAY,
    flow_job_color_profile_source_GAMA_CHRM,

} flow_job_color_profile_source;



void*flow_job_codecs_aquire_decode_jpeg_on_buffer(flow_context *c, struct flow_job *job,
                                                  struct flow_job_resource_buffer *buffer);
bool flow_job_codecs_jpeg_get_info(flow_context *c, struct flow_job *job, void *codec_state,
                                          struct decoder_frame_info *decoder_frame_info_ref);
bool flow_job_codecs_jpeg_read_frame(flow_context *c, struct flow_job *job, void *codec_state,
                                     flow_bitmap_bgra *canvas);


void*flow_job_codecs_aquire_encode_png_on_buffer(flow_context *c, struct flow_job *job,
                                                 struct flow_job_resource_buffer *buffer);

bool flow_job_codecs_png_get_info(flow_context *c, struct flow_job *job, void *codec_state,
                                  struct decoder_frame_info *decoder_frame_info_ref);
void*flow_job_codecs_aquire_decode_png_on_buffer(flow_context *c, struct flow_job *job,
                                                 struct flow_job_resource_buffer *buffer);
bool flow_job_codecs_png_read_frame(flow_context *c, struct flow_job *job, void *codec_state,
                                           flow_bitmap_bgra *canvas);


png_bytepp flow_job_create_row_pointers(flow_context *c, void *buffer, size_t buffer_size, size_t stride, size_t height);

 bool flow_bitmap_bgra_transform_to_srgb(flow_context *c, cmsHPROFILE current_profile, flow_bitmap_bgra *frame);

//bool flow_bitmap_bgra_write_png(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra* frame);
