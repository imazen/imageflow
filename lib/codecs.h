#pragma once

struct flow_job_codec_magic_bytes {
    flow_job_codec_type codec_type;
    size_t byte_count;
    uint8_t* bytes;
};

struct decoder_frame_info {
    int32_t w;
    int32_t h;
    flow_pixel_format format;
};



typedef void* (*codec_aquire_on_buffer_fn)(flow_context* c, struct flow_job* job,
                                           struct flow_job_resource_buffer* buffer);

typedef bool (*codec_get_frame_info_fn)(flow_context* c, struct flow_job* job, void* codec_state,
                                        struct decoder_frame_info* decoder_frame_info_ref);

typedef bool (*codec_read_frame_fn)(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra* canvas);

typedef bool (*codec_write_frame_fn)(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra* frame);

typedef bool (*codec_dispose_fn)(flow_context* c, struct flow_job* job, void* codec_state);

typedef bool (*codec_stringify_fn)(flow_context* c, struct flow_job* job, void* codec_state, char* buffer,
                                   size_t buffer_size);

struct flow_job_codec_definition {
    flow_job_codec_type type;
    codec_aquire_on_buffer_fn aquire_on_buffer;
    codec_get_frame_info_fn get_frame_info;
    codec_read_frame_fn read_frame;
    codec_write_frame_fn write_frame;
    codec_dispose_fn dispose;
    codec_stringify_fn stringify;
    const char* name;
    const char* preferred_mime_type;
    const char* preferred_extension;
};

typedef enum flow_job_color_profile_source {
    flow_job_color_profile_source_null,
    flow_job_color_profile_source_ICCP,
    flow_job_color_profile_source_ICCP_GRAY,
    flow_job_color_profile_source_GAMA_CHRM,

} flow_job_color_profile_source;

// Later we may want to expose this information to the outside
// struct flow_job_decoder_info {
//    cmsHPROFILE color_profile;
//    flow_job_color_profile_source color_profile_source;
//};

// typedef unsigned long png_uint_32;

struct flow_job_codec_definition* flow_job_get_codec_definition(flow_context* c, flow_job_codec_type type);
flow_job_codec_type flow_job_codec_select(flow_context* c, struct flow_job* job, uint8_t* data, size_t data_bytes);

void* flow_job_acquire_codec_over_buffer(flow_context* c, struct flow_job* job, struct flow_job_resource_buffer* buffer,
                                         flow_job_codec_type type);

bool flow_job_decoder_get_frame_info(flow_context* c, struct flow_job* job, void* codec_state, flow_job_codec_type type,
                                     struct decoder_frame_info* decoder_frame_info_ref);

bool flow_job_decoder_read_frame(flow_context* c, struct flow_job* job, void* codec_state, flow_job_codec_type type,
                                 flow_bitmap_bgra* canvas);

struct flow_job_resource_item {
    struct flow_job_resource_item* next;
    int32_t id;
    int32_t graph_placeholder_id;
    FLOW_DIRECTION direction;
    flow_job_resource_type type;
    flow_job_codec_type codec_type;
    void* codec_state;
    void* data;
};

void* flow_job_codecs_aquire_decode_jpeg_on_buffer(flow_context* c, struct flow_job* job,
                                                   struct flow_job_resource_buffer* buffer);
bool flow_job_codecs_jpeg_get_info(flow_context* c, struct flow_job* job, void* codec_state,
                                   struct decoder_frame_info* decoder_frame_info_ref);
bool flow_job_codecs_jpeg_read_frame(flow_context* c, struct flow_job* job, void* codec_state,
                                     flow_bitmap_bgra* canvas);
void* flow_job_codecs_aquire_encode_jpeg_on_buffer(flow_context* c, struct flow_job* job,
                                                   struct flow_job_resource_buffer* buffer);
bool flow_job_codecs_jpeg_write_frame(flow_context* c, struct flow_job* job, void* codec_state,
                                      flow_bitmap_bgra* frame);

void* flow_job_codecs_aquire_encode_png_on_buffer(flow_context* c, struct flow_job* job,
                                                  struct flow_job_resource_buffer* buffer);

bool flow_job_codecs_png_write_frame(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra* frame);

bool flow_job_codecs_png_get_info(flow_context* c, struct flow_job* job, void* codec_state,
                                  struct decoder_frame_info* decoder_frame_info_ref);
void* flow_job_codecs_aquire_decode_png_on_buffer(flow_context* c, struct flow_job* job,
                                                  struct flow_job_resource_buffer* buffer);
bool flow_job_codecs_png_read_frame(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra* canvas);

png_bytepp flow_job_create_row_pointers(flow_context* c, void* buffer, size_t buffer_size, size_t stride,
                                        size_t height);

bool flow_bitmap_bgra_transform_to_srgb(flow_context* c, cmsHPROFILE current_profile, flow_bitmap_bgra* frame);

// bool flow_job_codecs_png_write_frame(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra*
// frame);
