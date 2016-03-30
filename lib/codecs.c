#include "imageflow_private.h"

#include "lcms2.h"
#include "codecs.h"

extern const struct flow_codec_definition flow_codec_definition_decode_jpeg;
extern const struct flow_codec_definition flow_codec_definition_decode_png;
extern const struct flow_codec_definition flow_codec_definition_decode_gif;
extern const struct flow_codec_definition flow_codec_definition_encode_jpeg;
extern const struct flow_codec_definition flow_codec_definition_encode_png;
// extern const struct flow_codec_definition flow_codec_definition_encode_gif;

static struct flow_context_codec_set cached_default_codec_set;
static struct flow_codec_definition cached_default_set[6];

struct flow_context_codec_set* flow_context_get_default_codec_set()
{
    int i = 0;
    cached_default_set[i++] = flow_codec_definition_decode_jpeg;
    cached_default_set[i++] = flow_codec_definition_decode_png;
    cached_default_set[i++] = flow_codec_definition_decode_gif;
    cached_default_set[i++] = flow_codec_definition_encode_jpeg;
    cached_default_set[i++] = flow_codec_definition_encode_png;
    cached_default_set[i++] = flow_codec_definition_encode_png; // flow_codec_definition_encode_gif;
    cached_default_codec_set.codecs = &cached_default_set[0];
    cached_default_codec_set.codecs_count = sizeof(cached_default_set) / sizeof(struct flow_codec_definition);
    return &cached_default_codec_set;
}

uint8_t** flow_job_create_row_pointers(flow_c* c, void* buffer, size_t buffer_size, size_t stride, size_t height)
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

bool flow_bitmap_bgra_transform_to_srgb(flow_c* c, cmsHPROFILE current_profile, struct flow_bitmap_bgra* frame)
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

bool flow_job_initialize_codec(flow_c* c, struct flow_job* job, struct flow_codec_instance* item)
{

    struct flow_codec_definition* def = flow_job_get_codec_definition(c, item->codec_id);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (def->initialize == NULL) {
        FLOW_error_msg(c, flow_status_Not_implemented, ".initialize is not implemented for codec %s", def->name);
        return false;
    }
    return def->initialize(c, job, item);
}

bool flow_job_decoder_switch_frame(flow_c* c, struct flow_job* job, int32_t by_placeholder_id, int64_t frame_index)
{
    struct flow_codec_instance* current = flow_job_get_codec_instance(c, job, by_placeholder_id);
    if (current == NULL) {
        FLOW_error(c, flow_status_Invalid_argument); // Bad placeholder id
        return false;
    }
    if (current->codec_state == NULL) {
        FLOW_error(c, flow_status_Invalid_internal_state); // Codecs should be initialized by this point
        return false;
    }
    struct flow_codec_definition* def = flow_job_get_codec_definition(c, current->codec_id);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (def->switch_frame == NULL) {
        FLOW_error_msg(c, flow_status_Not_implemented, ".switch_frame is not implemented for codec %s", def->name);
        return false;
    }
    if (!def->switch_frame(c, job, current->codec_state, frame_index)) {
        FLOW_error_return(c);
    }
    return true;
}

bool flow_job_decoder_get_frame_info(flow_c* c, struct flow_job* job, void* codec_state, int64_t type,
                                     struct flow_decoder_frame_info* decoder_frame_info_ref)
{
    struct flow_codec_definition* def = flow_job_get_codec_definition(c, type);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (def->get_frame_info == NULL) {
        FLOW_error_msg(c, flow_status_Not_implemented, ".get_frame_info is not implemented for codec %s", def->name);
        return false;
    }
    if (!def->get_frame_info(c, job, codec_state, decoder_frame_info_ref)) {
        FLOW_error_return(c);
    }
    return true;
}

bool flow_job_decoder_read_frame(flow_c* c, struct flow_job* job, void* codec_state, int64_t type,
                                 struct flow_bitmap_bgra* canvas)
{
    struct flow_codec_definition* def = flow_job_get_codec_definition(c, type);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (def->read_frame == NULL) {
        FLOW_error_msg(c, flow_status_Not_implemented, ".read_frame is not implemented for codec %s", def->name);
        return false;
    }
    if (!def->read_frame(c, job, codec_state, canvas)) {
        FLOW_error_return(c);
    }
    return true;
}

static bool flow_job_decoder_get_info(flow_c* c, struct flow_job* job, void* codec_state, int64_t type,
                                      struct flow_decoder_info* decoder_info_ref)
{
    struct flow_codec_definition* def = flow_job_get_codec_definition(c, type);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (def->get_info == NULL) {
        FLOW_error_msg(c, flow_status_Not_implemented, ".get_info is not implemented for codec %s", def->name);
        return false;
    }
    if (!def->get_info(c, job, codec_state, decoder_info_ref)) {
        FLOW_error_return(c);
    }
    return true;
}

bool flow_job_get_decoder_info(flow_c* c, struct flow_job* job, int32_t by_placeholder_id,
                               struct flow_decoder_info* info)
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
    info->codec_id = current->codec_id;

    if (current->codec_state == NULL) {

        FLOW_error(c, flow_status_Invalid_internal_state); // Codecs should be initialized by this point
        return false;
    }
    info->frame0_post_decode_format = flow_bgra32;
    info->frame0_height = 0;
    info->frame0_width = 0;
    info->codec_id = current->codec_id;
    info->current_frame_index = 0;
    info->frame_count = 0;
    info->preferred_extension = NULL;
    info->preferred_mime_type = NULL;

    if (!flow_job_decoder_get_info(c, job, current->codec_state, current->codec_id, info)) {
        FLOW_error_return(c);
    }
    // Fill in defaults
    struct flow_codec_definition* def = flow_job_get_codec_definition(c, current->codec_id);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (info->preferred_mime_type == NULL)
        info->preferred_mime_type = def->preferred_mime_type;
    if (info->preferred_extension == NULL)
        info->preferred_extension = def->preferred_extension;

    return true;
}

bool flow_job_initialize_encoder(flow_c* c, struct flow_job* job, int32_t by_placeholder_id, int64_t codec_id)
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

bool flow_job_set_default_encoder(flow_c* c, struct flow_job* job, int32_t by_placeholder_id,
                                  int64_t default_encoder_id)
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

struct flow_codec_definition* flow_job_get_codec_definition(flow_c* c, int64_t codec_id)
{
    int i = 0;
    for (i = 0; i < (int)c->codec_set->codecs_count; i++) {
        if (c->codec_set->codecs[i].codec_id == codec_id)
            return &c->codec_set->codecs[i];
    }
    FLOW_error_msg(c, flow_status_Not_implemented, "No codec found for id %d", codec_id);
    return NULL;
}

int64_t flow_job_codec_select(flow_c* c, struct flow_job* job, uint8_t* data, size_t data_bytes)
{
    int32_t codec_ix = 0;
    for (codec_ix = 0; codec_ix < (int)c->codec_set->codecs_count; codec_ix++) {
        int32_t series_ix = 0;
        struct flow_codec_definition* def = &c->codec_set->codecs[codec_ix];
        for (series_ix = 0; series_ix < (int)def->magic_byte_sets_count; series_ix++) {
            struct flow_codec_magic_bytes* magic = &def->magic_byte_sets[series_ix];
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
                return def->codec_id;
        }
    }
    return flow_codec_type_null;
}
