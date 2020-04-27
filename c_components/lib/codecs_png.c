#include "zlib.h"
#include <stdio.h>
#include "jpeglib.h"
#include "imageflow_private.h"
#include "lcms2.h"
#include "codecs.h"


typedef enum flow_codecs_png_decoder_stage {
    flow_codecs_png_decoder_stage_Null = 0,
    flow_codecs_png_decoder_stage_Failed,
    flow_codecs_png_decoder_stage_NotStarted,
    flow_codecs_png_decoder_stage_BeginRead,
    flow_codecs_png_decoder_stage_FinishRead,
} flow_codecs_png_decoder_stage;

struct flow_codecs_png_decoder_state {
    flow_codecs_png_decoder_stage stage;
    png_structp png_ptr;
    png_infop info_ptr;
    png_size_t rowbytes;
    png_size_t w;
    png_size_t h;
    flow_pixel_format canvas_fmt;
    jmp_buf error_handler_jmp;
    int color_type, bit_depth;
    struct flow_io * io;
    png_bytep pixel_buffer;
    size_t pixel_buffer_size;
    png_bytepp pixel_buffer_row_pointers;
    flow_c * context;
    struct flow_decoder_color_info color;
};

struct flow_codecs_png_encoder_state {
    flow_c * context;
    struct flow_io * io;
    jmp_buf error_handler_jmp_buf;
};

static bool flow_codecs_png_decoder_reset(flow_c * c, struct flow_codecs_png_decoder_state * state)
{
    if (state->stage == flow_codecs_png_decoder_stage_Null) {
        state->pixel_buffer_row_pointers = NULL;
        state->info_ptr = NULL;
        state->png_ptr = NULL;
    } else {
        if (state->png_ptr != NULL || state->info_ptr != NULL) {
            png_destroy_read_struct(&state->png_ptr, &state->info_ptr, NULL);
        }
        if (state->pixel_buffer_row_pointers != NULL) {
            FLOW_free(c, state->pixel_buffer_row_pointers);
            state->pixel_buffer_row_pointers = NULL;
        }
    }
    flow_decoder_color_info_init(&state->color);
    state->rowbytes = 0;
    state->color_type = 0;
    state->bit_depth = 0;
    state->context = c;
    state->w = 0;
    state->h = 0;
    state->pixel_buffer = NULL;
    state->pixel_buffer_size = -1;
    state->canvas_fmt = flow_bgra32;
    state->stage = flow_codecs_png_decoder_stage_NotStarted;
    return true;
}

static void png_decoder_error_handler(png_structp png_ptr, png_const_charp msg)
{
    struct flow_codecs_png_decoder_state * state = (struct flow_codecs_png_decoder_state *)png_get_error_ptr(png_ptr);

    if (state == NULL) {
        exit(42);
        abort(); // WTF?
    }
    FLOW_error_msg(state->context, flow_status_Image_decoding_failed, "PNG decoding failed: %s", msg);

    // Dispose of everything
    flow_codecs_png_decoder_reset(state->context, state);
    state->stage = flow_codecs_png_decoder_stage_Failed;

    longjmp(state->error_handler_jmp, 1);
}

static void custom_read_data(png_structp png_ptr, png_bytep buffer, png_size_t bytes_requested)
{
    struct flow_codecs_png_decoder_state * state = (struct flow_codecs_png_decoder_state *)png_get_io_ptr(png_ptr);

    if (state == NULL || state->context == NULL) {
        png_error(png_ptr, "Read Error");
    }
    if (state->io == NULL) {
        FLOW_error_msg(state->context, flow_status_Image_decoding_failed,
                       "PNG decoding failed - struct flow_io was null");
        png_error(png_ptr, "Read Error");
    }
    int64_t bytes_read = state->io->read_func(state->context, state->io, buffer, bytes_requested);
    if (bytes_read != (int64_t)bytes_requested) {
        png_error(png_ptr, "Read beyond end of data requested");
        // TODO: Actually check the context error and see if there's a better way
    } else {
        // for (int i =0; i < bytes_read; i++) fprintf(stderr, "%08x", buffer[i]);
    }
}

static bool png_decoder_load_color_profile(flow_c * c, struct flow_codecs_png_decoder_state * state)
{

    // Get gamma
    if (!png_get_valid(state->png_ptr, state->info_ptr, PNG_INFO_sRGB)) {
        png_get_gAMA(state->png_ptr, state->info_ptr, &state->color.gamma);
    }

    // We assume that the underlying buffer can be freed after opening the profile, per
    // http://www.littlecms.com/1/jpegemb.c

    png_bytep profile_buf;
    uint32_t profile_length;

    // Pre-transform color_type (prior to all pre-decode format transforms)
    int is_color_png = state->color_type & PNG_COLOR_MASK_COLOR;

    if (png_get_iCCP(state->png_ptr, state->info_ptr, &(png_charp){ 0 }, &(int){ 0 }, &profile_buf, &profile_length) && profile_length > 0) {
        if (!flow_profile_is_srgb(profile_buf, profile_length)) {

            state->color.profile_buf = (uint8_t *) FLOW_malloc(c, profile_length);
            if (state->color.profile_buf == NULL) {
                FLOW_error(c, flow_status_Out_of_memory);
                return false;
            }
            memcpy(state->color.profile_buf, profile_buf, profile_length);
            state->color.buf_length = profile_length;

            if (is_color_png) {
                state->color.source = flow_codec_color_profile_source_ICCP;
            } else {
                state->color.source = flow_codec_color_profile_source_ICCP_GRAY;
            }
        }else{
            state->color.source = flow_codec_color_profile_source_sRGB;
        }
    }else if(is_color_png && !png_get_valid(state->png_ptr, state->info_ptr, PNG_INFO_sRGB)
        && png_get_valid(state->png_ptr, state->info_ptr, PNG_INFO_gAMA)
        && png_get_valid(state->png_ptr, state->info_ptr, PNG_INFO_cHRM)) {

        // Use cHRM and gAMA to build profile (later)
            png_get_cHRM(state->png_ptr, state->info_ptr, &state->color.white_point.x, &state->color.white_point.y, &state->color.primaries.Red.x,
                     &state->color.primaries.Red.y, &state->color.primaries.Green.x, &state->color.primaries.Green.y, &state->color.primaries.Blue.x, &state->color.primaries.Blue.y);

        state->color.white_point.Y = state->color.primaries.Red.Y = state->color.primaries.Green.Y = state->color.primaries.Blue.Y = 1.0;


        state->color.source = flow_codec_color_profile_source_GAMA_CHRM;
    }

    return true;
}

static bool flow_codecs_png_decoder_BeginRead(flow_c * c, struct flow_codecs_png_decoder_state * state)
{
    if (state->stage != flow_codecs_png_decoder_stage_NotStarted) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }
    if (!flow_codecs_png_decoder_reset(c, state)) {
        state->stage = flow_codecs_png_decoder_stage_Failed;
        FLOW_error_return(c);
    }
    state->stage = flow_codecs_png_decoder_stage_BeginRead;

    state->png_ptr = png_create_read_struct(PNG_LIBPNG_VER_STRING, state, png_decoder_error_handler, NULL);
    if (state->png_ptr == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        flow_codecs_png_decoder_reset(c, state);
        state->stage = flow_codecs_png_decoder_stage_Failed;
        return false;
    }

    state->info_ptr = png_create_info_struct(state->png_ptr);
    if (state->info_ptr == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        flow_codecs_png_decoder_reset(c, state);
        state->stage = flow_codecs_png_decoder_stage_Failed;
        return false;
    }
    // Set up error continuation
    if (setjmp(state->error_handler_jmp)) {
        // Execution comes back to this point if an error happens
        // We assume that the handler already set the context error
        FLOW_add_to_callstack(c);
        return false;
    }
    // Custom read function req.d - reading from memory
    png_set_read_fn(state->png_ptr, state, custom_read_data);

    // Read header and chunks
    png_read_info(state->png_ptr, state->info_ptr);

    png_uint_32 w, h;
    // Get dimensions and info
    png_get_IHDR(state->png_ptr, state->info_ptr, &w, &h, &state->bit_depth, &state->color_type, NULL, NULL, NULL);
    state->w = w;
    state->h = h;

    // Parse gamma and color profile info
    if (!png_decoder_load_color_profile(c, state)) {
        FLOW_add_to_callstack(c);
        flow_codecs_png_decoder_reset(c, state);
        state->stage = flow_codecs_png_decoder_stage_Failed;
        return false;
    }

    // Now we need to figure out how big our pixel buffer needs to be to hold the entire image.
    // We need to apply some normalization filters so we have fewer variants.

    /* expand palette images to RGB, low-bit-depth grayscale images to 8 bits,
    * transparency chunks to full alpha channel; strip 16-bit-per-sample
    * images to 8 bits per sample; and convert grayscale to RGB[A] */

    // Fill in the alpha channel with FFFF if missing.
    if (!(state->color_type & PNG_COLOR_MASK_ALPHA)) {
        png_set_expand(state->png_ptr);
        png_set_filler(state->png_ptr, 65535L, PNG_FILLER_AFTER);
        if (state->color_type == PNG_COLOR_TYPE_PALETTE){
            state->canvas_fmt = flow_bgra32;
        }else{
            state->canvas_fmt = flow_bgr32;
        }
    } else {
        state->canvas_fmt = flow_bgra32;
    }

    // Drop to 8-bit per channel; we can't handle 16-bit yet.
    if (state->bit_depth == 16) {
        png_set_strip_16(state->png_ptr);
    }
    // Convert grayscale to RGB.
    if (!(state->color_type & PNG_COLOR_MASK_COLOR))
        png_set_gray_to_rgb(state->png_ptr);

    // We use BGRA, not RGBA
    png_set_bgr(state->png_ptr);
    // We don't want to think about interlacing. Let libpng fix that up.

    // Update our info based on these new settings.
    png_read_update_info(state->png_ptr, state->info_ptr);

    // Now we can access a stride that represents the post-transform data.
    // state->rowbytes = png_get_rowbytes(state->png_ptr, state->info_ptr);

    if (png_get_channels(state->png_ptr, state->info_ptr) != 4) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false; // Should always be 4
    }
    // We set this, but it's ignored and overwritten by existing callers
    // state->pixel_buffer_size = state->rowbytes * state->h;

    return true;
}

static bool flow_codecs_png_decoder_FinishRead(flow_c * c, struct flow_codecs_png_decoder_state * state)
{
    if (state->stage != flow_codecs_png_decoder_stage_BeginRead) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }
    // We let the caller create the buffer
    if (state->pixel_buffer == NULL) {
        flow_codecs_png_decoder_reset(c, state);
        state->stage = flow_codecs_png_decoder_stage_Failed;
        FLOW_error(c, flow_status_Out_of_memory);
        return false;
    }

    state->stage = flow_codecs_png_decoder_stage_FinishRead;
    if (setjmp(state->error_handler_jmp)) {
        // Execution comes back to this point if an error happens
        return false;
    }

    state->pixel_buffer_row_pointers
        = flow_bitmap_create_row_pointers(c, state->pixel_buffer, state->pixel_buffer_size, state->rowbytes, state->h);
    if (state->pixel_buffer_row_pointers == NULL) {
        flow_codecs_png_decoder_reset(c, state);
        state->stage = flow_codecs_png_decoder_stage_Failed;
        FLOW_error_return(c);
    }

    // The real work
    png_read_image(state->png_ptr, state->pixel_buffer_row_pointers);

    png_read_end(state->png_ptr, NULL);

    // Not sure if we should just call reset instead, or not...
    png_destroy_read_struct(&state->png_ptr, &state->info_ptr, NULL);

    return true;
}

static bool flow_png_cleanup_decoder(flow_c * c, void * state)
{
    if (!flow_codecs_png_decoder_reset(c, (struct flow_codecs_png_decoder_state *)state)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    return true;
}

static bool flow_codecs_initialize_decode_png(flow_c * c, struct flow_codec_instance * item)
{
    // flow_codecs_png_decoder_state
    if (item->codec_state == NULL) {
        struct flow_codecs_png_decoder_state * state
            = (struct flow_codecs_png_decoder_state *)FLOW_malloc(c, sizeof(struct flow_codecs_png_decoder_state));
        if (state == NULL) {
            FLOW_error(c, flow_status_Out_of_memory);
            return false;
        }
        flow_set_destructor(c, state, flow_png_cleanup_decoder);

        state->stage = flow_codecs_png_decoder_stage_Null;

        if (!flow_codecs_png_decoder_reset(c, state)) {
            FLOW_add_to_callstack(c);
            return false;
        }
        state->io = item->io;
        item->codec_state = state;
    }
    return true;
}

static bool flow_codecs_png_get_frame_info(flow_c * c, void * codec_state,
                                           struct flow_decoder_frame_info * decoder_frame_info_ref)
{
    struct flow_codecs_png_decoder_state * state = (struct flow_codecs_png_decoder_state *)codec_state;
    if (state->stage < flow_codecs_png_decoder_stage_BeginRead) {
        if (!flow_codecs_png_decoder_BeginRead(c, state)) {
            FLOW_error_return(c);
        }
    }
    decoder_frame_info_ref->w = (int32_t)state->w;
    decoder_frame_info_ref->h = (int32_t)state->h;
    decoder_frame_info_ref->format = state->canvas_fmt;
    return true;
}
static bool flow_codecs_png_get_info(flow_c * c, void * codec_state, struct flow_decoder_info * info_ref)
{
    struct flow_codecs_png_decoder_state * state = (struct flow_codecs_png_decoder_state *)codec_state;
    if (state->stage < flow_codecs_png_decoder_stage_BeginRead) {
        if (!flow_codecs_png_decoder_BeginRead(c, state)) {
            FLOW_error_return(c);
        }
    }
    info_ref->image_width = (int32_t)state->w;
    info_ref->image_height = (int32_t)state->h;
    info_ref->frame_count = 1;
    info_ref->current_frame_index = 0;
    info_ref->frame_decodes_into = state->canvas_fmt;
    return true;
}

static bool flow_codecs_png_read_frame(flow_c * c, void * codec_state, struct flow_bitmap_bgra * canvas, struct flow_decoder_color_info * color)
{
    struct flow_codecs_png_decoder_state * state = (struct flow_codecs_png_decoder_state *)codec_state;
    if (state->stage == flow_codecs_png_decoder_stage_BeginRead) {
        state->rowbytes = canvas->stride;
        state->pixel_buffer = canvas->pixels;
        state->pixel_buffer_size = canvas->stride * canvas->h;
        if (!flow_codecs_png_decoder_FinishRead(c, state)) {
            FLOW_error_return(c);
        }
        if (color != NULL) {
            *color = state->color;
        }
        return true;
    } else {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }
}

static void png_write_data_callback(png_structp png_ptr, png_bytep data, png_size_t length)
{
    struct flow_codecs_png_encoder_state * p = (struct flow_codecs_png_encoder_state *)png_get_io_ptr(png_ptr);

    if (p->io->write_func(p->context, p->io, data, length) != (int64_t)length) {
        if (!flow_context_has_error(p->context)) {
            FLOW_error_msg(p->context, flow_status_IO_error, "Failed to write %l bytes", length);
        } else {
            FLOW_add_to_callstack(p->context);
        }
        png_error(png_ptr, "Write Error");
    }
}

static void png_encoder_error_handler(png_structp png_ptr, png_const_charp msg)
{
    struct flow_codecs_png_encoder_state * state = (struct flow_codecs_png_encoder_state *)png_get_error_ptr(png_ptr);

    if (state == NULL) {
        exit(42);
        abort(); // WTF?
    }
    FLOW_error_msg(state->context, flow_status_Image_encoding_failed, "PNG encoding failed: %s", msg);

    longjmp(state->error_handler_jmp_buf, 1);
}

static void png_flush_nullop(png_structp png_ptr) {}

static bool flow_codecs_png_write_frame(flow_c * c, void * codec_state, struct flow_bitmap_bgra * frame,
                                        struct flow_encoder_hints * hints)
{
    if (frame->fmt != flow_bgra32 && frame->fmt != flow_bgr24 && frame->fmt != flow_bgr32) {
        FLOW_error(c, flow_status_Unsupported_pixel_format);
        return false;
    }

    struct flow_codecs_png_encoder_state * state = (struct flow_codecs_png_encoder_state *)codec_state;
    state->context = c;

    if (setjmp(state->error_handler_jmp_buf)) {
        // Execution comes back to this point if an error happens
        // We assume that the handler already set the context error
        return false;
    }

    png_structp png_ptr = png_create_write_struct(PNG_LIBPNG_VER_STRING, state, png_encoder_error_handler,
                                                  NULL); // makepng_error, makepng_warning);
    png_infop info_ptr = NULL;
    if (png_ptr == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        return false;
    }
    if hints->zlib_compression_level >= -1 && hints->zlib_compression_level <= 9
    {
        png_set_compression_level(png_ptr, hints->zlib_compression_level);
        png_set_text_compression_level(png_ptr, hints->zlib_compression_level);
    }else{
        png_set_compression_level(png_ptr, Z_BEST_COMPRESSION);
        png_set_text_compression_level(png_ptr, Z_BEST_COMPRESSION);
    }

    png_set_write_fn(png_ptr, state, png_write_data_callback, png_flush_nullop);

    info_ptr = png_create_info_struct(png_ptr);
    if (info_ptr == NULL)
        png_error(png_ptr, "OOM allocating info structure"); // TODO: comprehend png error handling
    {

        png_bytepp rows
            = flow_bitmap_create_row_pointers(c, frame->pixels, frame->stride * frame->h, frame->stride, frame->h);
        if (rows == NULL) {
            FLOW_add_to_callstack(c);
            return false;
        }
        // TODO: check rows for NULL

        png_set_rows(png_ptr, info_ptr, rows);

        int color_type;
        int transform;
        if ((frame->fmt == flow_bgra32 && hints != NULL && hints->disable_png_alpha) || frame->fmt == flow_bgr32) {
            color_type = PNG_COLOR_TYPE_RGB;
            transform = PNG_TRANSFORM_BGR | PNG_TRANSFORM_STRIP_FILLER_AFTER;
        } else if (frame->fmt == flow_bgr24) {
            color_type = PNG_COLOR_TYPE_RGB;
            transform = PNG_TRANSFORM_BGR;
        } else if (frame->fmt == flow_bgra32) {
            color_type = PNG_COLOR_TYPE_RGB_ALPHA;
            transform = PNG_TRANSFORM_BGR;
        } else {
            FLOW_error(c, flow_status_Invalid_argument);
            return false;
        }

        png_set_IHDR(png_ptr, info_ptr, (png_uint_32)frame->w, (png_uint_32)frame->h, 8, color_type, PNG_INTERLACE_NONE,
                     PNG_COMPRESSION_TYPE_BASE, PNG_FILTER_TYPE_BASE);

        png_set_sRGB_gAMA_and_cHRM(png_ptr, info_ptr, PNG_sRGB_INTENT_PERCEPTUAL);

        if (hints != NULL && hints->disable_png_alpha) {
            // png_set_filler(png_ptr, (png_uint_32)0, PNG_FILLER_AFTER);
        }

        png_write_png(png_ptr, info_ptr, transform, NULL);

        FLOW_free(c, rows);
        rows = NULL;
        png_destroy_write_struct(&png_ptr, &info_ptr);
    }
    return true;
}

static bool flow_codecs_initialize_encode_png(flow_c * c, struct flow_codec_instance * item)
{
    // flow_codecs_png_decoder_state
    if (item->codec_state == NULL) {
        struct flow_codecs_png_encoder_state * state
            = (struct flow_codecs_png_encoder_state *)FLOW_malloc(c, sizeof(struct flow_codecs_png_encoder_state));
        if (state == NULL) {
            FLOW_error(c, flow_status_Out_of_memory);
            return false;
        }
        state->context = c;
        state->io = item->io;
        item->codec_state = state;
    }
    return true;
}
bool flow_bitmap_bgra_write_png(flow_c * c, struct flow_bitmap_bgra * frame, struct flow_io * io){
    return flow_bitmap_bgra_write_png_with_hints(c, frame, io, NULL);
}
bool flow_bitmap_bgra_write_png_with_hints(flow_c * c, struct flow_bitmap_bgra * frame, struct flow_io * io, struct flow_encoder_hints * hints)
{
    struct flow_codec_instance instance;
    instance.codec_id = flow_codec_type_encode_png;
    instance.direction = FLOW_OUTPUT;
    instance.io = io;
    instance.codec_state = NULL;
    instance.io_id = 404;

    if (!flow_codecs_initialize_encode_png(c, &instance)) {
        FLOW_error_return(c);
    }

    if (!flow_codecs_png_write_frame(c, instance.codec_state, frame, hints)) {
        FLOW_error_return(c);
    }
    return true;
}

const struct flow_codec_definition flow_codec_definition_decode_png
    = { .codec_id = flow_codec_type_decode_png,
        .initialize = flow_codecs_initialize_decode_png,
        .get_frame_info = flow_codecs_png_get_frame_info,
        .get_info = flow_codecs_png_get_info,
        .read_frame = flow_codecs_png_read_frame,
        .name = "decode png",
        .preferred_mime_type = "image/png",
        .preferred_extension = "png" };

