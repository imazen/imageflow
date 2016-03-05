#include "imageflow_private.h"
#include "job.h"
#include "lcms2.h"
#include "job_codecs.h"

uint8_t** flow_job_create_row_pointers(flow_context* c, void* buffer, size_t buffer_size, size_t stride, size_t height)
{
    if (buffer_size < stride * height) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return NULL;
    }
    uint8_t** rows = (uint8_t**)FLOW_malloc(c, sizeof(uint8_t*) * height);
    if (rows == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        return NULL;
    }
    unsigned int y;
    for (y = 0; y < height; ++y) {
        rows[y] = ((uint8_t*)buffer + (stride * y));
    }
    return rows;
}

bool flow_bitmap_bgra_transform_to_srgb(flow_context* c, cmsHPROFILE current_profile, flow_bitmap_bgra* frame)
{
    if (current_profile != NULL) {
        cmsHPROFILE target_profile = cmsCreate_sRGBProfile();
        if (target_profile == NULL) {
            FLOW_error(c, flow_status_Out_of_memory);
            return false;
        }
        cmsUInt32Number format = frame->fmt == flow_bgr24 ? TYPE_BGR_8
                                                          : (frame->fmt == flow_bgra32 ? TYPE_BGRA_8 : TYPE_GRAY_8);

        cmsHTRANSFORM transform
            = cmsCreateTransform(current_profile, format, target_profile, format, INTENT_PERCEPTUAL, 0);
        if (transform == NULL) {
            cmsCloseProfile(target_profile);
            FLOW_error(c, flow_status_Out_of_memory);
            return false;
        }
        for (unsigned int i = 0; i < frame->h; i++) {
            cmsDoTransform(transform, frame->pixels + (frame->stride * i), frame->pixels + (frame->stride * i),
                           frame->w);
        }

        cmsDeleteTransform(transform);
        cmsCloseProfile(target_profile);
    }
    return true;
}

// typedef bool (*codec_dispose_fn)(flow_context *c, struct flow_job * job, void * codec_state);

struct flow_job_codec_definition flow_job_codec_defs[]
    = { { .type = flow_job_codec_type_decode_png,
          .aquire_on_buffer = flow_job_codecs_aquire_decode_png_on_buffer,
          .get_frame_info = flow_job_codecs_png_get_info,
          .read_frame = flow_job_codecs_png_read_frame,
          .dispose = NULL,
          .name = "decode png" },
        { .type = flow_job_codec_type_encode_png,
          .aquire_on_buffer = flow_job_codecs_aquire_encode_png_on_buffer,
          .write_frame = flow_job_codecs_png_write_frame,
          .dispose = NULL,
          .name = "encode png" },
        { .type = flow_job_codec_type_decode_jpeg,
          .aquire_on_buffer = flow_job_codecs_aquire_decode_jpeg_on_buffer,
          .get_frame_info = flow_job_codecs_jpeg_get_info,
          .read_frame = flow_job_codecs_jpeg_read_frame,
          .dispose = NULL,
          .name = "decode jpeg" },
        { .type = flow_job_codec_type_encode_jpeg,
          .aquire_on_buffer = flow_job_codecs_aquire_encode_jpeg_on_buffer,
          .write_frame = flow_job_codecs_jpeg_write_frame,
          .dispose = NULL,
          .name = "encode png" } };

int32_t flow_job_codec_defs_count = sizeof(flow_job_codec_defs) / sizeof(struct flow_job_codec_definition);
struct flow_job_codec_definition* flow_job_get_codec_definition(flow_context* c, flow_job_codec_type type)
{
    int i = 0;
    for (i = 0; i < flow_job_codec_defs_count; i++) {
        if (flow_job_codec_defs[i].type == type)
            return &flow_job_codec_defs[i];
    }
    FLOW_error(c, flow_status_Not_implemented);
    return NULL;
}

uint8_t png_bytes[] = { 0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A };

uint8_t jpeg_bytes_a[] = { 0xFF, 0xD8, 0xFF, 0xDB };
uint8_t jpeg_bytes_b[] = { 0xFF, 0xD8, 0xFF, 0xE0 };
uint8_t jpeg_bytes_c[] = { 0xFF, 0xD8, 0xFF, 0xE1 };

struct flow_job_codec_magic_bytes flow_job_codec_magic_bytes_defs[]
    = { {
          .codec_type = flow_job_codec_type_decode_png, .byte_count = 7, .bytes = (uint8_t*)&png_bytes,
        },
        {
          .codec_type = flow_job_codec_type_decode_jpeg, .byte_count = 4, .bytes = (uint8_t*)&jpeg_bytes_a,

        },
        {
          .codec_type = flow_job_codec_type_decode_jpeg, .byte_count = 4, .bytes = (uint8_t*)&jpeg_bytes_b,

        },
        {
          .codec_type = flow_job_codec_type_decode_jpeg, .byte_count = 4, .bytes = (uint8_t*)&jpeg_bytes_c,

        } };
int32_t flow_job_codec_magic_bytes_defs_count = sizeof(flow_job_codec_magic_bytes_defs)
                                                / sizeof(struct flow_job_codec_magic_bytes);

flow_job_codec_type flow_job_codec_select(flow_context* c, struct flow_job* job, uint8_t* data, size_t data_bytes)
{
    int32_t series_ix = 0;
    for (series_ix = 0; series_ix < flow_job_codec_magic_bytes_defs_count; series_ix++) {
        struct flow_job_codec_magic_bytes* magic = &flow_job_codec_magic_bytes_defs[series_ix];
        if (data_bytes < magic->byte_count) {
            continue;
        }
        bool match = true;
        uint32_t i;
        for (i = 0; i < magic->byte_count; i++) {
            if (magic->bytes[i] != data[i]) {
                match = false;
                break;
            }
        }
        if (match)
            return magic->codec_type;
    }
    return flow_job_codec_type_null;
}

void* flow_job_acquire_decoder_over_buffer(flow_context* c, struct flow_job* job,
                                           struct flow_job_resource_buffer* buffer, flow_job_codec_type type)
{

    struct flow_job_codec_definition* def = flow_job_get_codec_definition(c, type);
    if (def == NULL) {
        FLOW_add_to_callstack(c);
        return NULL;
    }
    return def->aquire_on_buffer(c, job, buffer);
}

bool flow_job_decoder_get_frame_info(flow_context* c, struct flow_job* job, void* codec_state, flow_job_codec_type type,
                                     struct decoder_frame_info* decoder_frame_info_ref)
{
    struct flow_job_codec_definition* def = flow_job_get_codec_definition(c, type);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (!def->get_frame_info(c, job, codec_state, decoder_frame_info_ref)) {
        FLOW_error_return(c);
    }
    return true;
}

bool flow_job_decoder_read_frame(flow_context* c, struct flow_job* job, void* codec_state, flow_job_codec_type type,
                                 flow_bitmap_bgra* canvas)
{
    struct flow_job_codec_definition* def = flow_job_get_codec_definition(c, type);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (!def->read_frame(c, job, codec_state, canvas)) {
        FLOW_error_return(c);
    }
    return true;
}

// typedef bool (*codec_dispose_fn)(flow_context *c, struct flow_job * job, void * codec_state);
