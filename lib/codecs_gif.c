#include <stdio.h>
#include "gif_lib.h"
#include "imageflow_private.h"
#include "lcms2.h"
#include "codecs.h"

static uint8_t gif_bytes[] = { 0x47, 0x49, 0x46, 0x38 };

static struct flow_codec_magic_bytes gif_magic_bytes[] = { {
    .byte_count = 4, .bytes = (uint8_t *)&gif_bytes,

} };

typedef enum flow_job_gif_decoder_stage {
    flow_job_gif_decoder_stage_Null = 0,
    flow_job_gif_decoder_stage_Failed,
    flow_job_gif_decoder_stage_NotStarted,
    flow_job_gif_decoder_stage_BeginRead,
    flow_job_gif_decoder_stage_FinishRead,
} flow_job_gif_decoder_stage;

typedef void (*read_function_data_cleanup)(flow_c * c, void ** read_function_data);

struct flow_job_gif_decoder_state {
    GifFileType * gif;
    size_t w;
    size_t h;
    int64_t current_frame_index;
    struct flow_io * io;
    flow_c * context;
    flow_job_gif_decoder_stage stage;
};

// TODO: context errors must be translated to codec-specific exit flags for every codec (don't think return count is
// enough)

static int flow_job_gif_read_function(GifFileType * gif, GifByteType * buffer, int bytes_please)
{
    struct flow_job_gif_decoder_state * state = (struct flow_job_gif_decoder_state *)gif->UserData;
    if (state == NULL || state->io == NULL || state->io->read_func == NULL || state->context == NULL) {
        fprintf(stderr, "Fatal invocation of gif_read_function\n");
    }
    int64_t bytes_read = state->io->read_func(state->context, state->io, buffer, bytes_please);
    if (bytes_read != bytes_please) {
        fprintf(stderr, "Read only %" PRIu64 " of %i requested bytes\n", bytes_read, bytes_please);
        if (flow_context_has_error(state->context)) {
            FLOW_add_to_callstack(state->context);
        } else {
            FLOW_error_msg(state->context, flow_status_IO_error, "Failed to read %i bytes (only %i copied)",
                           bytes_please, bytes_read);
        }
    }
    return bytes_read;
}

// static int flow_job_gif_write_function (GifFileType * gif, const GifByteType * buffer, int count){
//    struct flow_job_gif_decoder_state * state = (struct flow_job_gif_decoder_state *)gif->UserData;
//
//    int64_t bytes_written = state->io->write_func(state->context,state->io,buffer,count);
//    if (bytes_written != count){
//        if (flow_context_has_error(state->context)) {
//            FLOW_add_to_callstack(state->context);
//        }else{
//            FLOW_error_msg(state->context, flow_status_IO_error, "Failed to write %i bytes (only %i flushed)", count,
//            bytes_written);
//        }
//    }
//    return bytes_written;
//}

// Flush buffers; close files     ; release underlying resources - the job has been ended.
static bool flow_job_gif_dispose(flow_c * c, void * codec_state)
{
    struct flow_job_gif_decoder_state * state = (struct flow_job_gif_decoder_state *)codec_state;
    if (state->gif != NULL) {
        int error = 0;
        // fprintf(stderr, "Closing gif %p\n", (void *)state->gif);
        if (DGifCloseFile(state->gif, &error) != D_GIF_SUCCEEDED) {
            FLOW_error_msg(c, flow_status_Image_decoding_failed,
                           "Failed to close gif: DGifCloseFile failed with error '%s'", GifErrorString(error));
            return false;
        }
        state->gif = NULL;
    }
    return true;
}

static bool flow_job_gif_decoder_reset(flow_c * c, struct flow_job_gif_decoder_state * state)
{
    if (state->stage == flow_job_gif_decoder_stage_FinishRead) {
        // FLOW_free(c, state->pixel_buffer);
    }
    if (state->stage == flow_job_gif_decoder_stage_Null) {
        state->gif = NULL;

    } else {

        if (state->gif != NULL) {
            int error_code = 0;
            if (DGifCloseFile(state->gif, &error_code) != GIF_OK) {
                FLOW_error_msg(c, flow_status_Image_decoding_failed, "Failed to close GifFileType: %s",
                               GifErrorString(error_code));
                return false;
            }
            state->gif = NULL;
        }
    }
    state->current_frame_index = 0;
    state->context = c;
    state->w = 0;
    state->h = 0;
    state->stage = flow_job_gif_decoder_stage_NotStarted;
    return true;
}

static bool flow_job_gif_decoder_FinishRead(flow_c * c, struct flow_job_gif_decoder_state * state)
{
    if (state->stage < flow_job_gif_decoder_stage_BeginRead) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }
    if (state->stage == flow_job_gif_decoder_stage_FinishRead) {
        return true;
    }

    state->stage = flow_job_gif_decoder_stage_FinishRead;

    // fprintf(stderr, "DGifSlurp on %p\n", (void *)state->gif);
    if (DGifSlurp(state->gif) != GIF_OK) {
        FLOW_error_msg(c, flow_status_Image_decoding_failed, "Failed to read gif: DGifSlurp(%p) failed with error '%s'",
                       (void *)state->gif, GifErrorString(state->gif->Error));
        return false;
    }

    return true;
}

static bool flow_job_gif_decoder_BeginRead(flow_c * c, struct flow_job_gif_decoder_state * state)
{
    if (state->stage != flow_job_gif_decoder_stage_NotStarted) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }
    if (!flow_job_gif_decoder_reset(c, state)) {
        state->stage = flow_job_gif_decoder_stage_Failed;
        FLOW_error_return(c);
    }
    state->stage = flow_job_gif_decoder_stage_BeginRead;

    int error = 0;
    state->gif = DGifOpen(state, flow_job_gif_read_function, &error);
    // fprintf(stderr, "DGifOpen returned %p\n", (void *) state->gif);

    if (error != D_GIF_SUCCEEDED) {
        FLOW_error_msg(c, flow_status_Image_decoding_failed, "Failed to open gif: DGifOpen failed with error '%s'",
                       GifErrorString(error));
        return false;
    }

    if (state->gif == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        flow_job_gif_decoder_reset(c, state);
        state->stage = flow_job_gif_decoder_stage_Failed;
        return false;
    }
    state->w = state->gif->SWidth;
    state->h = state->gif->SHeight;

    return true;
}

static bool flow_job_codecs_gif_initialize(flow_c * c, struct flow_job * job, struct flow_codec_instance * codec)
{
    // flow_job_gif_decoder_state
    if (codec->codec_state == NULL) {
        struct flow_job_gif_decoder_state * state = (struct flow_job_gif_decoder_state *)flow_context_malloc(
            c, sizeof(struct flow_job_gif_decoder_state), flow_job_gif_dispose, job, __FILE__, __LINE__);
        if (state == NULL) {
            FLOW_error(c, flow_status_Out_of_memory);
            return false;
        }
        state->stage = flow_job_gif_decoder_stage_Null;

        if (!flow_job_gif_decoder_reset(c, state)) {
            FLOW_add_to_callstack(c);
            return false;
        }
        state->io = codec->io;
        state->context = c;

        codec->codec_state = (void *)state;
    }
    return true;
}
static bool flow_job_codecs_decode_gif_switch_frame(flow_c * c, struct flow_job * job, void * codec_state,
                                                    size_t frame_index)
{
    struct flow_job_gif_decoder_state * state = (struct flow_job_gif_decoder_state *)codec_state;
    if (state->stage < flow_job_gif_decoder_stage_BeginRead) {
        if (!flow_job_gif_decoder_BeginRead(c, state)) {
            FLOW_error_return(c);
        }
    }
    state->current_frame_index = frame_index;
    return true;
}
static bool flow_job_codecs_gif_get_info(flow_c * c, struct flow_job * job, void * codec_state,
                                         struct flow_decoder_info * info_ref)
{
    struct flow_job_gif_decoder_state * state = (struct flow_job_gif_decoder_state *)codec_state;
    if (state->stage < flow_job_gif_decoder_stage_BeginRead) {
        if (!flow_job_gif_decoder_BeginRead(c, state)) {
            FLOW_error_return(c);
        }
    }
    if (state->stage < flow_job_gif_decoder_stage_FinishRead) {
        if (!flow_job_gif_decoder_FinishRead(c, state)) {
            FLOW_error_return(c);
        }
    }
    info_ref->frame0_width = state->gif->SWidth;
    info_ref->frame0_height = state->gif->SHeight;
    info_ref->frame_count = state->gif->ImageCount;
    info_ref->current_frame_index = state->current_frame_index;
    info_ref->frame0_post_decode_format = flow_bgra32;
    return true;
}

static bool flow_job_codecs_gif_get_frame_info(flow_c * c, struct flow_job * job, void * codec_state,
                                               struct flow_decoder_frame_info * decoder_frame_info_ref)
{
    struct flow_job_gif_decoder_state * state = (struct flow_job_gif_decoder_state *)codec_state;
    if (state->stage < flow_job_gif_decoder_stage_BeginRead) {
        if (!flow_job_gif_decoder_BeginRead(c, state)) {
            FLOW_error_return(c);
        }
    }
    decoder_frame_info_ref->w = state->w;
    decoder_frame_info_ref->h = state->h;
    decoder_frame_info_ref->format = flow_bgra32; // state->channels == 1 ? flow_gray8 : flow_bgr24;
    return true;
}

static bool dequantize(flow_c * c, GifFileType * gif, int frame_index, struct flow_bitmap_bgra * canvas)
{
    if (gif->ImageCount <= frame_index) {
        FLOW_error_msg(c, flow_status_Invalid_argument, "Frame index must be between [0, %i). Given %i",
                       gif->ImageCount, frame_index);
        return false;
    }
    SavedImage * image = &gif->SavedImages[frame_index];
    int w = image->ImageDesc.Width;
    int h = image->ImageDesc.Height;
    int left = image->ImageDesc.Left;
    int top = image->ImageDesc.Top;
    if (w + left > (int)canvas->w && h + top > (int)canvas->h) {
        FLOW_error_msg(c, flow_status_Invalid_argument, "Canvas size must be >= gif size");
        return false;
    }
    if (canvas->fmt != flow_bgra32) {
        FLOW_error_msg(c, flow_status_Invalid_argument, "Canvas must be bgra32");
        return false;
    }

    struct GraphicsControlBlock gcb;

    DGifSavedExtensionToGCB(gif, frame_index, &gcb);

    // TODO - actually dequantize and store in canvas
    ColorMapObject * palette = image->ImageDesc.ColorMap;
    if (palette == NULL)
        palette = gif->SColorMap;
    uint8_t * gif_byte = &image->RasterBits[0];
    uint8_t * canvas_byte = canvas->pixels + (left * 4) + (top * canvas->stride);
    int palette_size = palette->ColorCount;
    GifColorType * colors = palette->Colors;
    int stride_offset = canvas->stride - (w * 4);

    for (int y = 0; y < h; y++) {
        for (int x = 0; x < w; x++) {
            uint8_t byte = *gif_byte;
            if (byte > palette_size) {
                FLOW_error_msg(c, flow_status_Image_decoding_failed,
                               "Byte at %i,%i had an index (%i) outside the palette range of [0,%i].", x, y, byte,
                               palette_size);
                return false;
            }
            *canvas_byte++ = colors[byte].Blue;
            *canvas_byte++ = colors[byte].Green;
            *canvas_byte++ = colors[byte].Red;
            *canvas_byte++ = gcb.TransparentColor == byte ? 0 : 255;
            gif_byte++;
        }
        canvas_byte += stride_offset;
    }
    return true;
}

static bool flow_job_codecs_gif_read_frame(flow_c * c, struct flow_job * job, void * codec_state,
                                           struct flow_bitmap_bgra * canvas)
{
    struct flow_job_gif_decoder_state * state = (struct flow_job_gif_decoder_state *)codec_state;
    if (state->stage >= flow_job_gif_decoder_stage_BeginRead) {

        if (canvas == NULL || canvas->pixels == NULL) {
            flow_job_gif_decoder_reset(c, state);
            state->stage = flow_job_gif_decoder_stage_Failed;
            FLOW_error(c, flow_status_Out_of_memory);
            return false;
        }

        if (!flow_job_gif_decoder_FinishRead(c, state)) {
            FLOW_error_return(c);
        }

        if (!dequantize(c, state->gif, state->current_frame_index, canvas)) {
            FLOW_error_return(c);
        }

        return true;
    } else {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }
}

//
// void* flow_job_codecs_aquire_encode_gif_on_buffer(flow_context* c, struct flow_job* job,
//                                                   struct flow_job_resource_buffer* buffer)
//{
//    // flow_job_gif_decoder_state
//    if (buffer->codec_state == NULL) {
//        struct flow_job_gif_encoder_state* state
//                = (struct flow_job_gif_encoder_state*)FLOW_malloc(c, sizeof(struct flow_job_gif_encoder_state));
//        if (state == NULL) {
//            FLOW_error(c, flow_status_Out_of_memory);
//            return NULL;
//        }
//        state->buffer = NULL;
//        state->size = 0;
//        state->context = c;
//        state->output_resource = buffer;
//
//        buffer->codec_state = (void*)state;
//    }
//    return buffer->codec_state;
//}
//
// bool flow_job_codecs_gif_write_frame(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra*
// frame)
//{
//    struct flow_job_gif_encoder_state* state = (struct flow_job_gif_encoder_state*)codec_state;
//    state->buffer = NULL;
//    state->size = 0;
//    state->context = c;
//
//    state->cinfo.err = gif_std_error(&state->error_mgr);
//    state->error_mgr.error_exit = gif_encode_error_exit;
//
//    if (setjmp(state->error_handler_jmp)) {
//        // Execution comes back to this point if an error happens
//        // We assume that the handler already set the context error
//        return false;
//    }
//
//    gif_create_compress(&state->cinfo);
//
//    gif_mem_dest(&state->cinfo, (unsigned char**)&state->buffer, &state->size);
//
//    state->cinfo.in_color_space = JCS_EXT_BGRA;
//    state->cinfo.image_height = frame->h;
//    state->cinfo.image_width = frame->w;
//    state->cinfo.input_components = 4;
//    state->cinfo.optimize_coding = true;
//
//    gif_set_defaults(&state->cinfo);
//
//    int quality = 90;
//
//    gif_set_quality(&state->cinfo, quality, TRUE /* limit to baseline-JPEG values */);
//
//    gif_simple_progression(&state->cinfo);
//
//    gif_start_compress(&state->cinfo, TRUE);
//
//    uint8_t** rows = flow_job_create_row_pointers(c, frame->pixels, frame->stride * frame->h, frame->stride,
//    frame->h);
//    if (rows == NULL) {
//        FLOW_add_to_callstack(c);
//        gif_destroy_compress(&state->cinfo);
//        return false;
//    }
//
//    (void)gif_write_scanlines(&state->cinfo, rows, frame->h);
//
//    gif_finish_compress(&state->cinfo);
//
//    gif_destroy_compress(&state->cinfo);
//
//    if (state->error_mgr.num_warnings > 0) {
//        FLOW_error(c, flow_status_Invalid_internal_state);
//        return false;
//    }
//
//    // Copy the final result to the output resource, if it exists.
//    if (state->output_resource != NULL) {
//        state->output_resource->buffer = state->buffer;
//        state->output_resource->buffer_size = state->size;
//    }
//
//    return true;
//}

const struct flow_codec_definition flow_codec_definition_decode_gif
    = { .codec_id = flow_codec_type_decode_gif,
        .initialize = flow_job_codecs_gif_initialize,
        .get_frame_info = flow_job_codecs_gif_get_frame_info,
        .get_info = flow_job_codecs_gif_get_info,
        .switch_frame = flow_job_codecs_decode_gif_switch_frame,
        .read_frame = flow_job_codecs_gif_read_frame,
        .magic_byte_sets = &gif_magic_bytes[0],
        .magic_byte_sets_count = sizeof(gif_magic_bytes) / sizeof(struct flow_codec_magic_bytes),
        .name = "decode gif",
        .preferred_mime_type = "image/gif",
        .preferred_extension = "gif" };
