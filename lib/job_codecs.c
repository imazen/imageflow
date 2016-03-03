#include <zlib.h>
#include "job.h"

typedef enum flow_job_png_decoder_stage {
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

static bool flow_job_png_decoder_reset(flow_context* c, struct flow_job_png_decoder_state* state)
{

    if (state->stage == flow_job_png_decoder_stage_FinishRead) {
        FLOW_free(c, state->pixel_buffer);
    }
    memset(&state->image, 0, sizeof state->image);
    state->image.version = PNG_IMAGE_VERSION;
    state->image.opaque = NULL;
    state->pixel_buffer = NULL;
    state->pixel_buffer_size = -1;
    state->stage = flow_job_png_decoder_stage_NotStarted;
    return true;
}
static bool flow_job_png_decoder_BeginRead(flow_context* c, struct flow_job_png_decoder_state* state)
{
    if (state->stage != flow_job_png_decoder_stage_NotStarted) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }
    if (!flow_job_png_decoder_reset(c, state)) {
        state->stage = flow_job_png_decoder_stage_Failed;
        FLOW_error_return(c);
    }
    state->stage = flow_job_png_decoder_stage_BeginRead;
    if (png_image_begin_read_from_memory(&state->image, state->file_bytes, state->file_bytes_count)) {
        state->image.format = PNG_FORMAT_BGRA;
        state->pixel_buffer_size = PNG_IMAGE_SIZE(state->image);

        return true;
    } else {
        state->stage = flow_job_png_decoder_stage_Failed;
        FLOW_error(c, flow_status_Invalid_argument); // TODO
        return false;
    }
}

static bool flow_job_png_decoder_FinishRead(flow_context* c, struct flow_job_png_decoder_state* state)
{
    if (state->stage != flow_job_png_decoder_stage_BeginRead) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }
    // We let the caller create the buffer
    //    state->pixel_buffer =  (png_bytep)FLOW_calloc (c, state->pixel_buffer_size, sizeof(png_bytep));
    if (state->pixel_buffer == NULL) {
        png_image_free(&state->image);
        state->stage = flow_job_png_decoder_stage_Failed;
        FLOW_error(c, flow_status_Out_of_memory);
        return false;
    }

    state->stage = flow_job_png_decoder_stage_FinishRead;
    if (png_image_finish_read(&state->image, NULL /*background*/, state->pixel_buffer, 0 /*row_stride*/,
                              NULL /*colormap for PNG_FORMAT_FLAG_COLORMAP */)) {

        return true;
    } else {
        state->stage = flow_job_png_decoder_stage_Failed;
        FLOW_free(c, state->pixel_buffer);
        fprintf(stderr, "png_image_finish_read: %s\n", state->image.message);
        FLOW_error(c, flow_status_Invalid_argument); // TODO
        return false;
    }
}

static void* codec_aquire_decode_png_on_buffer(flow_context* c, struct flow_job* job,
                                               struct flow_job_resource_buffer* buffer)
{
    // flow_job_png_decoder_state
    if (buffer->codec_state == NULL) {
        struct flow_job_png_decoder_state* state
            = (struct flow_job_png_decoder_state*)FLOW_malloc(c, sizeof(struct flow_job_png_decoder_state));
        if (state == NULL) {
            FLOW_error(c, flow_status_Out_of_memory);
            return NULL;
        }
        state->stage = flow_job_png_decoder_stage_Null;
        if (!flow_job_png_decoder_reset(c, state)) {
            FLOW_add_to_callstack(c);
            return NULL;
        }
        state->file_bytes = buffer->buffer;
        state->file_bytes_count = buffer->buffer_size;

        buffer->codec_state = (void*)state;
    }
    return buffer->codec_state;
}

static bool png_get_info(flow_context* c, struct flow_job* job, void* codec_state,
                         struct decoder_frame_info* decoder_frame_info_ref)
{
    struct flow_job_png_decoder_state* state = (struct flow_job_png_decoder_state*)codec_state;
    if (state->stage < flow_job_png_decoder_stage_BeginRead) {
        if (!flow_job_png_decoder_BeginRead(c, state)) {
            FLOW_error_return(c);
        }
    }
    decoder_frame_info_ref->w = state->image.width;
    decoder_frame_info_ref->h = state->image.height;
    return true;
}

static bool png_read_frame(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra* canvas)
{
    struct flow_job_png_decoder_state* state = (struct flow_job_png_decoder_state*)codec_state;
    if (state->stage == flow_job_png_decoder_stage_BeginRead) {
        state->pixel_buffer = canvas->pixels;
        state->pixel_buffer_size = canvas->stride * canvas->h;
        if (!flow_job_png_decoder_FinishRead(c, state)) {
            FLOW_error_return(c);
        }
        return true;
    } else {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }
}

static void png_write_data_callback(png_structp png_ptr, png_bytep data, png_size_t length)
{
    struct flow_job_png_encoder_state* p = (struct flow_job_png_encoder_state*)png_get_io_ptr(png_ptr);
    size_t nsize = p->size + length;

    /* allocate or grow buffer */
    if (p->buffer)
        p->buffer = (char*)FLOW_realloc(p->context, p->buffer, nsize);
    else
        p->buffer = (char*)FLOW_malloc(p->context, nsize);

    if (!p->buffer)
        png_error(png_ptr, "Write Error"); // TODO: comprehend png error handling

    /* copy new bytes to end of buffer */
    memcpy(p->buffer + p->size, data, length);
    p->size += length;
}
static void png_flush_nullop(png_structp png_ptr) {}

bool flow_bitmap_bgra_write_png(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra* frame)
{
    struct flow_job_png_encoder_state* state = (struct flow_job_png_encoder_state*)codec_state;
    state->buffer = NULL;
    state->size = 0;
    state->context = c;

    png_structp png_ptr = png_create_write_struct(PNG_LIBPNG_VER_STRING, "writing to memory", NULL,
                                                  NULL); // makepng_error, makepng_warning);
    png_infop info_ptr = NULL;
    if (png_ptr == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        return false;
    }
    png_set_compression_level(png_ptr, Z_DEFAULT_COMPRESSION);
    png_set_text_compression_level(png_ptr, Z_DEFAULT_COMPRESSION);

    png_set_write_fn(png_ptr, state, png_write_data_callback, png_flush_nullop);

    info_ptr = png_create_info_struct(png_ptr);
    if (info_ptr == NULL)
        png_error(png_ptr, "OOM allocating info structure"); // TODO: comprehend png error handling
    {

        png_bytepp rows = (png_bytepp)FLOW_malloc(c, sizeof(png_bytep) * frame->h);
        unsigned int y;
        for (y = 0; y < frame->h; ++y) {
            rows[y] = frame->pixels + (frame->stride * y);
        }

        png_set_rows(png_ptr, info_ptr, rows);

        png_set_IHDR(png_ptr, info_ptr, (png_uint_32)frame->w, (png_uint_32)frame->h, 8, PNG_COLOR_TYPE_RGB_ALPHA,
                     PNG_INTERLACE_NONE, PNG_COMPRESSION_TYPE_BASE, PNG_FILTER_TYPE_BASE);

        png_set_sRGB(png_ptr, info_ptr, PNG_sRGB_INTENT_PERCEPTUAL);

        png_write_png(png_ptr, info_ptr, PNG_TRANSFORM_BGR, NULL);

        FLOW_free(c, rows);
        rows = NULL;
        png_destroy_write_struct(&png_ptr, &info_ptr);
        state->output_resource->buffer = state->buffer;
        state->output_resource->buffer_size = state->size;
    }
    // TODO: maybe ? png_destroy_write_struct(&nv_ptr, &nv_info);
    return true;
}

static void* codec_aquire_encode_png_on_buffer(flow_context* c, struct flow_job* job,
                                               struct flow_job_resource_buffer* buffer)
{
    // flow_job_png_decoder_state
    if (buffer->codec_state == NULL) {
        struct flow_job_png_encoder_state* state
            = (struct flow_job_png_encoder_state*)FLOW_malloc(c, sizeof(struct flow_job_png_encoder_state));
        if (state == NULL) {
            FLOW_error(c, flow_status_Out_of_memory);
            return NULL;
        }
        state->buffer = NULL;
        state->size = 0;
        state->context = c;
        state->output_resource = buffer;

        buffer->codec_state = (void*)state;
    }
    return buffer->codec_state;
}

// typedef bool (*codec_dispose_fn)(flow_context *c, struct flow_job * job, void * codec_state);

struct flow_job_codec_definition flow_job_codec_defs[] = { { .type = flow_job_codec_type_decode_png,
                                                             .aquire_on_buffer = codec_aquire_decode_png_on_buffer,
                                                             .get_frame_info = png_get_info,
                                                             .read_frame = png_read_frame,
                                                             .dispose = NULL,
                                                             .name = "decode png" },
                                                           { .type = flow_job_codec_type_encode_png,
                                                             .aquire_on_buffer = codec_aquire_encode_png_on_buffer,
                                                             .write_frame = flow_bitmap_bgra_write_png,
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

struct flow_job_codec_magic_bytes flow_job_codec_magic_bytes_defs[] = { {
    .codec_type = flow_job_codec_type_decode_png, .byte_count = 7, .bytes = (uint8_t*)&png_bytes,

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
