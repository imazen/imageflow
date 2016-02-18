#include "job.h"




typedef enum flow_job_png_decoder_stage{
    flow_job_png_decoder_stage_Null = 0,
    flow_job_png_decoder_stage_Failed,
    flow_job_png_decoder_stage_NotStarted,
    flow_job_png_decoder_stage_BeginRead,
    flow_job_png_decoder_stage_FinishRead,
} flow_job_png_decoder_stage;

struct flow_job_png_decoder_state {
    flow_job_png_decoder_stage stage;
    png_image image;
    png_const_voidp file_bytes;
    png_size_t file_bytes_count;
    png_bytep pixel_buffer;
    size_t pixel_buffer_size;
};

static bool flow_job_png_decoder_reset(Context * c, struct flow_job_png_decoder_state * state){

    if (state->stage == flow_job_png_decoder_stage_FinishRead){
        CONTEXT_free(c,state->pixel_buffer);
    }
    memset(&state->image, 0, sizeof state->image);
    state->image.version = PNG_IMAGE_VERSION;
    state->image.opaque = NULL;
    state->pixel_buffer = NULL;
    state->pixel_buffer_size = -1;
    state->stage = flow_job_png_decoder_stage_NotStarted;
    return true;
}
static bool flow_job_png_decoder_BeginRead(Context * c, struct flow_job_png_decoder_state * state){
    if (state->stage != flow_job_png_decoder_stage_NotStarted){
        CONTEXT_error(c,Invalid_internal_state);
        return false;
    }
    if (!flow_job_png_decoder_reset(c,state)){
        state->stage = flow_job_png_decoder_stage_Failed;
        CONTEXT_error_return(c);
    }
    state->stage = flow_job_png_decoder_stage_BeginRead;
    if (png_image_begin_read_from_memory(&state->image,state->file_bytes, state->file_bytes_count)) {
        state->image.format = PNG_FORMAT_BGRA;
        state->pixel_buffer_size = PNG_IMAGE_SIZE(state->image);

        return true;
    }else{
        state->stage = flow_job_png_decoder_stage_Failed;
        CONTEXT_error(c, Invalid_argument); //TODO
        return false;
    }
}


static bool flow_job_png_decoder_FinishRead(Context * c, struct flow_job_png_decoder_state * state){
    if (state->stage != flow_job_png_decoder_stage_BeginRead){
        CONTEXT_error(c,Invalid_internal_state);
        return false;
    }

    state->pixel_buffer =  (png_bytep)CONTEXT_calloc (c, state->pixel_buffer_size, sizeof(png_bytep));
    if (state->pixel_buffer == NULL){
        png_image_free(&state->image);
        state->stage = flow_job_png_decoder_stage_Failed;
        CONTEXT_error(c, Out_of_memory);
        return false;
    }

    state->stage = flow_job_png_decoder_stage_FinishRead;
    if (png_image_finish_read(&state->image, NULL/*background*/, state->pixel_buffer,
                              0/*row_stride*/, NULL/*colormap for PNG_FORMAT_FLAG_COLORMAP */)) {

        return true;
    }else{
        state->stage = flow_job_png_decoder_stage_Failed;
        CONTEXT_free(c,state->pixel_buffer);
        fprintf(stderr, "png_image_finish_read: %s\n",
                state->image.message);
        CONTEXT_error(c, Invalid_argument); //TODO
        return false;
    }

}


static void * codec_aquire_decode_png_on_buffer(Context *c, struct flow_job * job, struct flow_job_resource_buffer * buffer){
    //flow_job_png_decoder_state
    if (buffer->codec_state == NULL){
        struct flow_job_png_decoder_state * state = (struct flow_job_png_decoder_state *) CONTEXT_malloc(c, sizeof(struct flow_job_png_decoder_state));
        if (state == NULL){
            CONTEXT_error(c, Out_of_memory);
            return NULL;
        }
        if (!flow_job_png_decoder_reset(c, state)){
            CONTEXT_add_to_callstack(c);
            return NULL;
        }
        state->file_bytes = buffer->buffer;
        state->file_bytes_count = buffer->buffer_size;

        buffer->codec_state = (void *)state;
    }
    return buffer->codec_state;
}


static bool png_get_info(Context *c, struct flow_job * job, void * codec_state, struct decoder_frame_info * decoder_frame_info_ref){
    struct flow_job_png_decoder_state * state = (struct flow_job_png_decoder_state *) codec_state;
    if (state->stage < flow_job_png_decoder_stage_BeginRead){
        if (!flow_job_png_decoder_BeginRead(c,state)){
            CONTEXT_error_return(c);
        }
    }
    decoder_frame_info_ref->w  = state->image.width;
    decoder_frame_info_ref->h = state->image.height;
    return true;
}

static bool png_read_frame(Context *c, struct flow_job * job, void * codec_state, BitmapBgra * canvas){
    struct flow_job_png_decoder_state * state = (struct flow_job_png_decoder_state *) codec_state;
    if (state->stage == flow_job_png_decoder_stage_BeginRead){
        state->pixel_buffer = canvas->pixels;
        state->pixel_buffer_size = canvas->stride * canvas->h;
        if (!flow_job_png_decoder_FinishRead(c,state)){
            CONTEXT_error_return(c);
        }
        return true;
    }else{
        CONTEXT_error(c, Invalid_internal_state);
        return false;
    }
}

//typedef bool (*codec_dispose_fn)(Context *c, struct flow_job * job, void * codec_state);




struct flow_job_codec_definition flow_job_codec_defs[] = {
        {
                .type= flow_job_codec_type_decode_png,
                .aquire_on_buffer = codec_aquire_decode_png_on_buffer,
                .get_frame_info = png_get_info,
                .read_frame = png_read_frame,
                .dispose = NULL,
                .name ="decode png"
        }
};

int32_t flow_job_codec_defs_count = sizeof(flow_job_codec_defs) / sizeof(struct flow_job_codec_definition);
struct flow_job_codec_definition * flow_job_get_codec_definition(Context *c, flow_job_codec_type type){
    int i = 0;
    for (i = 0; i < flow_job_codec_defs_count; i++){
        if (flow_job_codec_defs[i].type == type) return &flow_job_codec_defs[i];
    }
    CONTEXT_error(c, Not_implemented);
    return NULL;
}

uint8_t png_bytes[] = {0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A};

struct flow_job_codec_magic_bytes flow_job_codec_magic_bytes_defs[] = {
        {
                .codec_type = flow_job_codec_type_decode_png,
                .byte_count = 7,
                .bytes = (uint8_t *)&png_bytes,

        }
};
int32_t flow_job_codec_magic_bytes_defs_count = sizeof(flow_job_codec_magic_bytes_defs) / sizeof(struct flow_job_codec_magic_bytes);

flow_job_codec_type flow_job_codec_select(Context *c, struct flow_job * job,  uint8_t * data, size_t data_bytes){
    int32_t series_ix = 0;
    for (series_ix = 0; series_ix < flow_job_codec_magic_bytes_defs_count; series_ix++) {
        struct flow_job_codec_magic_bytes *magic = &flow_job_codec_magic_bytes_defs[series_ix];
        if (data_bytes < magic->byte_count) {
            continue;
        }
        bool match = true;
        uint32_t i;
        for (i = 0; i < magic->byte_count;  i++) {
            if (magic->bytes[i] != data[i]) {
                match = false;
                break;
            }
        }
        if (match) return magic->codec_type;
    }
    return flow_job_codec_type_null;
}



void * flow_job_acquire_decoder_over_buffer(Context *c, struct flow_job *job,
                                                   struct flow_job_resource_buffer *buffer, flow_job_codec_type type) {

    struct flow_job_codec_definition * def = flow_job_get_codec_definition(c, type);
    if (def == NULL){
        CONTEXT_add_to_callstack(c);
        return NULL;
    }
    return def->aquire_on_buffer(c,job,buffer);
}



bool flow_job_decoder_get_frame_info(Context *c, struct flow_job * job, void * codec_state, flow_job_codec_type type, struct decoder_frame_info * decoder_frame_info_ref){
    struct flow_job_codec_definition * def = flow_job_get_codec_definition(c, type);
    if (def == NULL){
        CONTEXT_error_return(c);
    }
    if (!def->get_frame_info(c,job,codec_state,decoder_frame_info_ref)){
        CONTEXT_error_return(c);
    }
    return true;
}

bool flow_job_decoder_read_frame(Context *c, struct flow_job * job, void * codec_state,  flow_job_codec_type type, BitmapBgra * canvas){
    struct flow_job_codec_definition * def = flow_job_get_codec_definition(c, type);
    if (def == NULL){
        CONTEXT_error_return(c);
    }
    if (!def->read_frame(c,job,codec_state,canvas)){
        CONTEXT_error_return(c);
    }
    return true;
}

//typedef bool (*codec_dispose_fn)(Context *c, struct flow_job * job, void * codec_state);
