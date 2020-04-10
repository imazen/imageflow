#include "lcms2.h"
#include "imageflow_private.h"

#include "codecs.h"

extern const struct flow_codec_definition flow_codec_definition_decode_jpeg;
extern const struct flow_codec_definition flow_codec_definition_decode_png;

static struct flow_context_codec_set cached_default_codec_set;
static struct flow_codec_definition cached_default_set[6];

struct flow_context_codec_set * flow_context_get_default_codec_set()
{
    size_t i = 0;
    cached_default_set[i++] = flow_codec_definition_decode_jpeg;
    cached_default_set[i++] = flow_codec_definition_decode_png;
    // flow_codec_definition_encode_gif;
    cached_default_codec_set.codecs = &cached_default_set[0];
    cached_default_codec_set.codecs_count = i; // sizeof(cached_default_set) / sizeof(struct flow_codec_definition);
    return &cached_default_codec_set;
}


void flow_decoder_color_info_init(struct flow_decoder_color_info * color){
    memset(color, 0, sizeof(struct flow_decoder_color_info));
    color->gamma = 0.45455;
}

static unsigned long djb2_buffer(unsigned char *bytes, size_t len)
{
    unsigned long hash = 5381;
    for (size_t ix = 0; ix < len; ix++) {
        hash = ((hash << 5) + hash) + bytes[ix]; /* hash * 33 + c */
    }
    return hash;
}

static unsigned long hash_profile_bytes(unsigned char * profile, size_t profile_len){
    if (profile_len <= sizeof(cmsICCHeader)) return 0;

    return djb2_buffer(profile + sizeof(cmsICCHeader), profile_len - sizeof(cmsICCHeader));
}

//static unsigned long hash_close_profile(cmsHPROFILE profile){
//    uint32_t outputsize;
//    if (!cmsSaveProfileToMem(profile, 0, &outputsize)) {
//        cmsCloseProfile(profile);
//        return 0;
//    }
//    unsigned char *buffer = ( unsigned char *) malloc(outputsize);
//    if (buffer == 0){
//        cmsCloseProfile(profile);
//        return 0;
//    }
//    if (!cmsSaveProfileToMem(profile, buffer, &outputsize)){
//        free(buffer);
//        cmsCloseProfile(profile);
//        return 0;
//    }
//    unsigned long hash = hash_profile_bytes(buffer, outputsize);
//    free(buffer);
//    cmsCloseProfile(profile);
//    return hash;
//}

// We save an allocation in png decoding by ignoring an sRGB profile (we assume sRGB anyway).
// We don't save this allocation yet in jpeg decoding, as the profile is segmented.
bool flow_profile_is_srgb(unsigned char * profile, size_t profile_len){

//unsigned long srgbHash = hash_close_profile(cmsCreate_sRGBProfile());
//    fprintf(stdout,"sRGB hash is %lx\n", srgbHash);
//
    unsigned long srgbHash = 0x1b3b4e2f339c1255;
    unsigned long profileHash = hash_profile_bytes(profile, profile_len);

    bool match = (profileHash == srgbHash);
//    fprintf(stdout,"Profile hash is %lx, sRGB hash is %lx\n", profileHash, srgbHash);
    return match;

}

// typedef bool (*codec_dispose_fn)(flow_context *c, struct flow_job * job, void * codec_state);

bool flow_codec_initialize(flow_c * c, struct flow_codec_instance * item)
{

    struct flow_codec_definition * def = flow_codec_get_definition(c, item->codec_id);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (def->initialize == NULL) {
        FLOW_error_msg(c, flow_status_Not_implemented, ".initialize is not implemented for codec %s", def->name);
        return false;
    }
    return def->initialize(c, item);
}

bool flow_codec_decoder_set_downscale_hints(flow_c * c, struct flow_codec_instance * codec,
                                            struct flow_decoder_downscale_hints * hints, bool crash_if_not_implemented)
{
    struct flow_codec_definition * def = flow_codec_get_definition(c, codec->codec_id);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (def->set_downscale_hints == NULL) {
        if (crash_if_not_implemented) {
            FLOW_error_msg(c, flow_status_Not_implemented, ".set_downscale_hints is not implemented for codec %s",
                           def->name);
            return false;
        } else {
            return true;
        }
    }
    if (codec->codec_state == NULL) {
        FLOW_error_msg(c, flow_status_Invalid_internal_state, "Codec has not been initialized.");
        return false;
    }

    if (!def->set_downscale_hints(c, codec, hints)) {
        FLOW_error_return(c);
    }
    return true;
}

bool flow_codec_decoder_get_info(flow_c * c, void * codec_state, int64_t codec_id, struct flow_decoder_info * info)
{
    if (codec_state == NULL) {

        FLOW_error(c, flow_status_Invalid_internal_state); // Codecs should be initialized by this point
        return false;
    }

    struct flow_codec_definition * def = flow_codec_get_definition(c, codec_id);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (def->get_info == NULL) {
        FLOW_error_msg(c, flow_status_Not_implemented, ".get_info is not implemented for codec %s", def->name);
        return false;
    }
    // Reset everything to defaults

    info->frame_decodes_into = flow_bgra32;
    info->image_height = 0;
    info->image_width = 0;
    info->codec_id = codec_id;
    info->current_frame_index = 0;
    info->frame_count = 0;
    info->preferred_extension = NULL;
    info->preferred_mime_type = NULL;

    if (!def->get_info(c, codec_state, info)) {
        FLOW_error_return(c);
    }
    // Fill in fallback values
    if (info->preferred_mime_type == NULL)
        info->preferred_mime_type = def->preferred_mime_type;
    if (info->preferred_extension == NULL)
        info->preferred_extension = def->preferred_extension;

    return true;
}

struct flow_codec_definition * flow_codec_get_definition(flow_c * c, int64_t codec_id)
{
    int i = 0;
    for (i = 0; i < (int)c->codec_set->codecs_count; i++) {
        if (c->codec_set->codecs[i].codec_id == codec_id)
            return &c->codec_set->codecs[i];
    }
    FLOW_error_msg(c, flow_status_Not_implemented, "No codec found for id %d", codec_id);
    return NULL;
}


struct flow_bitmap_bgra * flow_codec_execute_read_frame(flow_c * c, struct flow_codec_instance * codec, struct flow_decoder_color_info * color_info)
{
    struct flow_codec_definition * def = flow_codec_get_definition(c, codec->codec_id);
    if (def == NULL) {
        FLOW_error_return_null(c);
    }
    if (def->get_frame_info == NULL || def->read_frame == NULL) {
        FLOW_error(c, flow_status_Not_implemented);
        return NULL;
    }
    struct flow_decoder_frame_info frame_info;
    if (!def->get_frame_info(c, codec->codec_state, &frame_info)) {
        FLOW_error_return_null(c);
    }

    struct flow_bitmap_bgra * result_bitmap
        = flow_bitmap_bgra_create(c, frame_info.w, frame_info.h, true, frame_info.format);
    if (result_bitmap == NULL) {
        FLOW_error_return_null(c);
    }
    if (!def->read_frame(c, codec->codec_state, result_bitmap, color_info)) {
        FLOW_error_return_null(c);
    }
    return result_bitmap;
}
