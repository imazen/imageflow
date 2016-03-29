#include <stdio.h>
#include "gif_lib.h"
#include "imageflow_private.h"
#include "lcms2.h"
#include "codecs.h"

typedef enum flow_job_gif_decoder_stage {
    flow_job_gif_decoder_stage_Null = 0,
    flow_job_gif_decoder_stage_Failed,
    flow_job_gif_decoder_stage_NotStarted,
    flow_job_gif_decoder_stage_BeginRead,
    flow_job_gif_decoder_stage_FinishRead,
} flow_job_gif_decoder_stage;

typedef void (*read_function_data_cleanup)(flow_context* c, void** read_function_data);

struct flow_job_gif_decoder_state {
    GifFileType* gif;
    size_t row_stride;
    size_t w;
    size_t h;

    struct flow_io* io;
    uint8_t* pixel_buffer;
    flow_context* context;
    flow_job_gif_decoder_stage stage;
};

// TODO: context errors must be translated to codec-specific exit flags for every codec (don't think return count is
// enough)

static int flow_job_gif_read_function(GifFileType* gif, GifByteType* buffer, int bytes_please)
{
    struct flow_job_gif_decoder_state* state = (struct flow_job_gif_decoder_state*)gif->UserData;

    int64_t bytes_read = state->io->read_func(state->context, state->io, buffer, bytes_please);
    if (bytes_read != bytes_please) {
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

static bool flow_job_gif_decoder_reset(flow_context* c, struct flow_job_gif_decoder_state* state)
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
    state->row_stride = 0;
    state->context = c;
    state->w = 0;
    state->h = 0;
    state->stage = flow_job_gif_decoder_stage_NotStarted;
    return true;
}

static bool flow_job_gif_decoder_BeginRead(flow_context* c, struct flow_job_gif_decoder_state* state)
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

static bool flow_job_gif_decoder_FinishRead(flow_context* c, struct flow_job_gif_decoder_state* state)
{
    if (state->stage != flow_job_gif_decoder_stage_BeginRead) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }
    // We let the caller create the buffer
    //    state->pixel_buffer =  (gif_bytep)FLOW_calloc (c, state->pixel_buffer_size, sizeof(gif_bytep));
    if (state->pixel_buffer == NULL) {
        flow_job_gif_decoder_reset(c, state);
        state->stage = flow_job_gif_decoder_stage_Failed;
        FLOW_error(c, flow_status_Out_of_memory);
        return false;
    }

    state->stage = flow_job_gif_decoder_stage_FinishRead;

    if (DGifSlurp(state->gif) != GIF_OK) {
        FLOW_error_msg(c, flow_status_Image_decoding_failed, "Failed to open gif: DGifOpen failed with error '%s'",
                       GifErrorString(state->gif->Error));
        return false;
    }

    return true;
}

bool flow_job_codecs_gif_initialize(flow_context* c, struct flow_job* job, struct flow_codec_instance* codec)
{
    // flow_job_gif_decoder_state
    if (codec->codec_state == NULL) {
        struct flow_job_gif_decoder_state* state
            = (struct flow_job_gif_decoder_state*)FLOW_malloc(c, sizeof(struct flow_job_gif_decoder_state));
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

        codec->codec_state = (void*)state;
    }
    return true;
}

bool flow_job_codecs_gif_get_info(flow_context* c, struct flow_job* job, void* codec_state,
                                  struct decoder_frame_info* decoder_frame_info_ref)
{
    struct flow_job_gif_decoder_state* state = (struct flow_job_gif_decoder_state*)codec_state;
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

static bool dequantize(flow_context * c, GifFileType * gif, int frame_index, flow_bitmap_bgra * canvas){
    if (gif->ImageCount <= frame_index){
        FLOW_error_msg(c, flow_status_Invalid_argument, "Frame index must be between [0, %i). Given %i", gif->ImageCount, frame_index);
        return false;
    }
    SavedImage * image = &gif->SavedImages[frame_index];
    int w = image->ImageDesc.Width;
    int h = image->ImageDesc.Height;
    if (w != (int)canvas->w && h != (int)canvas->h){
        FLOW_error_msg(c, flow_status_Invalid_argument, "Canvas size must match gif size");
        return false;
    }
    if (canvas->fmt != flow_bgra32){
        FLOW_error_msg(c, flow_status_Invalid_argument, "Canvas must be bgra32");
        return false;
    }

    struct GraphicsControlBlock gcb;

    DGifSavedExtensionToGCB(gif, frame_index, &gcb);

    //TODO - actually dequantize and store in canvas
    ColorMapObject * palette = image->ImageDesc.ColorMap;
    if (palette == NULL) palette = gif->SColorMap;
    uint8_t * gif_byte = &image->RasterBits[0];
    uint8_t * canvas_byte = canvas->pixels;
    int palette_size = palette->ColorCount;
    GifColorType * colors = palette->Colors;
    int stride_offset = canvas->stride - (canvas->w * 4);


    for (int y = 0; y < h; y++){
        for (int x = 0; x < w; x++){
            uint8_t byte = *gif_byte;
            if (byte > palette_size){
                FLOW_error_msg(c, flow_status_Image_decoding_failed, "Byte at %i,%i had an index (%i) outside the palette range of [0,%i].", x, y, byte, palette_size);
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

bool flow_job_codecs_gif_read_frame(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra* canvas)
{
    struct flow_job_gif_decoder_state* state = (struct flow_job_gif_decoder_state*)codec_state;
    if (state->stage == flow_job_gif_decoder_stage_BeginRead) {
        state->pixel_buffer = canvas->pixels;
        if (!flow_job_gif_decoder_FinishRead(c, state)) {
            FLOW_error_return(c);
        }

        if (!dequantize(c, state->gif, 0, canvas)){
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
