#include "imageflow_private.h"

#include "lcms2.h"
#include "codecs.h"

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

struct flow_codec_definition flow_codec_defs[] = { { .type = flow_codec_type_decode_gif,
                                                     .aquire_on_buffer = NULL,
                                                     .initialize = flow_job_codecs_gif_initialize,
                                                     .get_frame_info = flow_job_codecs_gif_get_info,
                                                     .read_frame = flow_job_codecs_gif_read_frame,
                                                     .dispose = NULL,
                                                     .name = "decode gif",
                                                     .preferred_mime_type = "image/gif",
                                                     .preferred_extension = "gif" },
                                                   { .type = flow_codec_type_decode_png,
                                                     .aquire_on_buffer = NULL,
                                                     .initialize = flow_job_codecs_initialize_decode_png,
                                                     .get_frame_info = flow_job_codecs_png_get_info,
                                                     .read_frame = flow_job_codecs_png_read_frame,
                                                     .dispose = NULL,
                                                     .name = "decode png",
                                                     .preferred_mime_type = "image/png",
                                                     .preferred_extension = "png" },
                                                   { .type = flow_codec_type_encode_png,
                                                     .initialize = flow_job_codecs_initialize_encode_png,
                                                     .write_frame = flow_job_codecs_png_write_frame,
                                                     .dispose = NULL,
                                                     .name = "encode png",
                                                     .preferred_mime_type = "image/png",
                                                     .preferred_extension = "png" },
                                                   { .type = flow_codec_type_decode_jpeg,
                                                     .initialize = flow_job_codecs_initialize_decode_jpeg,
                                                     .get_frame_info = flow_job_codecs_jpeg_get_info,
                                                     .read_frame = flow_job_codecs_jpeg_read_frame,
                                                     .dispose = NULL,
                                                     .name = "decode jpeg",
                                                     .preferred_mime_type = "image/jpeg",
                                                     .preferred_extension = "jpg" },
                                                   { .type = flow_codec_type_encode_jpeg,
                                                     .initialize = flow_job_codecs_initialize_encode_jpeg,
                                                     .write_frame = flow_job_codecs_jpeg_write_frame,
                                                     .dispose = NULL,
                                                     .name = "encode jpeg",
                                                     .preferred_mime_type = "image/jpeg",
                                                     .preferred_extension = "jpg" } };

int32_t flow_codec_defs_count = sizeof(flow_codec_defs) / sizeof(struct flow_codec_definition);
struct flow_codec_definition* flow_job_get_codec_definition(flow_context* c, flow_codec_type type)
{
    int i = 0;
    for (i = 0; i < flow_codec_defs_count; i++) {
        if (flow_codec_defs[i].type == type)
            return &flow_codec_defs[i];
    }
    FLOW_error_msg(c, flow_status_Not_implemented, "No codec found for id %d", type);
    return NULL;
}

uint8_t png_bytes[] = { 0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A };

uint8_t jpeg_bytes_a[] = { 0xFF, 0xD8, 0xFF, 0xDB };
uint8_t jpeg_bytes_b[] = { 0xFF, 0xD8, 0xFF, 0xE0 };
uint8_t jpeg_bytes_c[] = { 0xFF, 0xD8, 0xFF, 0xE1 };

uint8_t gif_bytes[] = { 0x47, 0x49, 0x46, 0x38 };

struct flow_codec_magic_bytes flow_job_codec_magic_bytes_defs[]
    = { {
          .codec_type = flow_codec_type_decode_png, .byte_count = 7, .bytes = (uint8_t*)&png_bytes,
        },
        {
          .codec_type = flow_codec_type_decode_jpeg, .byte_count = 4, .bytes = (uint8_t*)&jpeg_bytes_a,

        },
        {
          .codec_type = flow_codec_type_decode_jpeg, .byte_count = 4, .bytes = (uint8_t*)&jpeg_bytes_b,

        },
        {
          .codec_type = flow_codec_type_decode_jpeg, .byte_count = 4, .bytes = (uint8_t*)&jpeg_bytes_c,

        },
        {
          .codec_type = flow_codec_type_decode_gif, .byte_count = 4, .bytes = (uint8_t*)&gif_bytes,

        } };
int32_t flow_job_codec_magic_bytes_defs_count = sizeof(flow_job_codec_magic_bytes_defs)
                                                / sizeof(struct flow_codec_magic_bytes);

flow_codec_type flow_job_codec_select(flow_context* c, struct flow_job* job, uint8_t* data, size_t data_bytes)
{
    int32_t series_ix = 0;
    for (series_ix = 0; series_ix < flow_job_codec_magic_bytes_defs_count; series_ix++) {
        struct flow_codec_magic_bytes* magic = &flow_job_codec_magic_bytes_defs[series_ix];
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
    return flow_codec_type_null;
}

bool flow_job_initialize_codec(flow_context* c, struct flow_job* job, struct flow_codec_instance* item)
{

    struct flow_codec_definition* def = flow_job_get_codec_definition(c, (flow_codec_type)item->codec_id);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (def->initialize == NULL) {
        FLOW_error_msg(c, flow_status_Not_implemented, ".initialize is not implemented for codec %s", def->name);
        return false;
    }
    return def->initialize(c, job, item);
}

bool flow_job_decoder_get_frame_info(flow_context* c, struct flow_job* job, void* codec_state, flow_codec_type type,
                                     struct decoder_frame_info* decoder_frame_info_ref)
{
    struct flow_codec_definition* def = flow_job_get_codec_definition(c, type);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (!def->get_frame_info(c, job, codec_state, decoder_frame_info_ref)) {
        FLOW_error_return(c);
    }
    return true;
}

bool flow_job_decoder_read_frame(flow_context* c, struct flow_job* job, void* codec_state, flow_codec_type type,
                                 flow_bitmap_bgra* canvas)
{
    struct flow_codec_definition* def = flow_job_get_codec_definition(c, type);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (!def->read_frame(c, job, codec_state, canvas)) {
        FLOW_error_return(c);
    }
    return true;
}

bool flow_job_get_decoder_info(flow_context* c, struct flow_job* job, int32_t by_placeholder_id,
                               struct flow_job_decoder_info* info)
{
    struct flow_codec_instance* current = flow_job_get_codec_instance(c, job, by_placeholder_id);
    if (current == NULL) {
        FLOW_error(c, flow_status_Invalid_argument); // Bad placeholder id
        return false;
    }
    if (current->direction != FLOW_INPUT) {
        FLOW_error(c, flow_status_Invalid_argument); // Bad placeholder id
        return false;
    }
    info->codec_type = (flow_codec_type)current->codec_id;

    if (current->codec_state == NULL) {

        FLOW_error(c, flow_status_Invalid_internal_state); // Codecs should be initialized by this point
        return false;
    }
    struct flow_codec_definition* def = flow_job_get_codec_definition(c, (flow_codec_type)current->codec_id);
    if (def == NULL) {
        FLOW_error_return(c);
    }

    struct decoder_frame_info frame_info;
    if (!flow_job_decoder_get_frame_info(c, job, current->codec_state, (flow_codec_type)current->codec_id,
                                         &frame_info)) {
        FLOW_error_return(c);
    }
    info->frame0_width = frame_info.w;
    info->frame0_height = frame_info.h;
    info->frame0_post_decode_format = frame_info.format;
    info->preferred_mime_type = def->preferred_mime_type;
    info->preferred_extension = def->preferred_extension;

    return true;
}

bool flow_job_initialize_encoder(flow_context* c, struct flow_job* job, int32_t by_placeholder_id,
                                 flow_codec_type codec_id)
{
    struct flow_codec_instance* current = flow_job_get_codec_instance(c, job, by_placeholder_id);
    if (current == NULL) {
        FLOW_error(c, flow_status_Invalid_argument); // Bad placeholder id
        return false;
    }
    if (current->direction != FLOW_OUTPUT) {
        FLOW_error(c, flow_status_Invalid_argument); // Bad placeholder id
        return false;
    }
    current->codec_id = codec_id;

    if (!flow_job_initialize_codec(c, job, current)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    return true;
}

bool flow_job_set_default_encoder(flow_context* c, struct flow_job* job, int32_t by_placeholder_id,
                                  flow_codec_type default_encoder_id)
{
    struct flow_codec_instance* current = flow_job_get_codec_instance(c, job, by_placeholder_id);
    if (current == NULL) {
        FLOW_error(c, flow_status_Invalid_argument); // Bad placeholder id
        return false;
    }
    if (current->direction != FLOW_OUTPUT) {
        FLOW_error(c, flow_status_Invalid_argument); // Bad placeholder id
        return false;
    }
    if (current->codec_state == NULL && current->codec_id == 0) {
        current->codec_id = default_encoder_id;
    }
    return true;
}
