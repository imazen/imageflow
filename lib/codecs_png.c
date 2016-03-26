#include <zlib.h>
#include <stdio.h>
#include <jpeglib.h>
#include "imageflow_private.h"
#include "job.h"
#include "lcms2.h"
#include "codecs.h"

typedef enum flow_job_png_decoder_stage {
    flow_job_png_decoder_stage_Null = 0,
    flow_job_png_decoder_stage_Failed,
    flow_job_png_decoder_stage_NotStarted,
    flow_job_png_decoder_stage_BeginRead,
    flow_job_png_decoder_stage_FinishRead,
} flow_job_png_decoder_stage;

struct flow_job_png_decoder_state {
    flow_job_png_decoder_stage stage;
    png_structp png_ptr;
    png_infop info_ptr;
    png_size_t rowbytes;
    png_size_t w;
    png_size_t h;
    jmp_buf error_handler_jmp;
    int color_type, bit_depth;
    png_const_voidp file_bytes;
    png_size_t file_bytes_count;
    png_size_t file_bytes_read;
    png_bytep pixel_buffer;
    size_t pixel_buffer_size;
    png_bytepp pixel_buffer_row_pointers;
    flow_context* context;
    cmsHPROFILE color_profile;
    flow_codec_color_profile_source color_profile_source;
    double gamma;
};

struct flow_job_png_encoder_state {
    flow_context* context;
    char* buffer;
    size_t size;
    struct flow_job_resource_buffer* output_resource;
    jmp_buf error_handler_jmp_buf;
};

static bool flow_job_png_decoder_reset(flow_context* c, struct flow_job_png_decoder_state* state)
{
    if (state->stage == flow_job_png_decoder_stage_FinishRead) {
        FLOW_free(c, state->pixel_buffer);
    }
    if (state->stage == flow_job_png_decoder_stage_Null) {
        state->pixel_buffer_row_pointers = NULL;
        state->color_profile = NULL;
        state->info_ptr = NULL;
        state->png_ptr = NULL;
    } else {
        if (state->png_ptr != NULL || state->info_ptr != NULL) {
            png_destroy_read_struct(&state->png_ptr, &state->info_ptr, NULL);
        }
        if (state->color_profile != NULL) {
            cmsCloseProfile(state->color_profile);
            state->color_profile = NULL;
        }
        if (state->pixel_buffer_row_pointers != NULL) {
            FLOW_free(c, state->pixel_buffer_row_pointers);
            state->pixel_buffer_row_pointers = NULL;
        }
    }
    state->color_profile_source = flow_codec_color_profile_source_null;
    state->rowbytes = 0;
    state->color_type = 0;
    state->bit_depth = 0;
    state->context = c;
    state->w = 0;
    state->h = 0;
    state->gamma = 0.45455;
    state->pixel_buffer = NULL;
    state->pixel_buffer_size = -1;
    state->file_bytes_read = 0;
    state->stage = flow_job_png_decoder_stage_NotStarted;
    return true;
}

static void png_decoder_error_handler(png_structp png_ptr, png_const_charp msg)
{
    struct flow_job_png_decoder_state* state = (struct flow_job_png_decoder_state*)png_get_error_ptr(png_ptr);

    if (state == NULL) {
        exit(42);
        abort(); // WTF?
    }
    FLOW_error_msg(state->context, flow_status_Image_decoding_failed, "PNG decoding failed");

    // Dispose of everything
    flow_job_png_decoder_reset(state->context, state);
    state->stage = flow_job_png_decoder_stage_Failed;

    longjmp(state->error_handler_jmp, 1);
}

static void custom_read_data(png_structp png_ptr, png_bytep buffer, png_size_t bytes_requested)
{
    struct flow_job_png_decoder_state* state = (struct flow_job_png_decoder_state*)png_get_io_ptr(png_ptr);

    if (state == NULL) {
        png_error(png_ptr, "Read Error");
    }
    if (bytes_requested > state->file_bytes_count - state->file_bytes_read) {
        png_error(png_ptr, "Read beyond end of data requested");
    }
    size_t bytes_read = umin64(state->file_bytes_count - state->file_bytes_read, bytes_requested);
    memcpy(buffer, (const uint8_t*)state->file_bytes + state->file_bytes_read, bytes_read);
    state->file_bytes_read += bytes_read;
}

static bool png_decoder_load_color_profile(flow_context* c, struct flow_job_png_decoder_state* state)
{

    // Get gamma
    if (!png_get_valid(state->png_ptr, state->info_ptr, PNG_INFO_sRGB)) {
        png_get_gAMA(state->png_ptr, state->info_ptr, &state->gamma);
    }

    // We assume that the underlying buffer can be freed after opening the profile, per
    // http://www.littlecms.com/1/jpegemb.c

    png_bytep profile_buf;
    uint32_t profile_length;

    cmsHPROFILE profile = NULL;

    // Pre-transform color_type (prior to all pre-decode format transforms)
    int is_color_png = state->color_type & PNG_COLOR_MASK_COLOR;

    if (png_get_iCCP(state->png_ptr, state->info_ptr, &(png_charp){ 0 }, &(int){ 0 }, &profile_buf, &profile_length)) {
        // Decode the ICC profile from the buffer
        profile = cmsOpenProfileFromMem(profile_buf, profile_length);
        cmsColorSpaceSignature colorspace = cmsGetColorSpace(profile);

        if (colorspace == cmsSigRgbData && is_color_png) {
            state->color_profile_source = flow_codec_color_profile_source_ICCP;
        } else {
            if (colorspace == cmsSigGrayData && !is_color_png) {
                // TODO: warn about this
                state->color_profile_source = flow_codec_color_profile_source_ICCP_GRAY;
                ;
            }
            cmsCloseProfile(profile);
            profile = NULL;
        }
    }

    if (profile == NULL && is_color_png && !png_get_valid(state->png_ptr, state->info_ptr, PNG_INFO_sRGB)
        && png_get_valid(state->png_ptr, state->info_ptr, PNG_INFO_gAMA)
        && png_get_valid(state->png_ptr, state->info_ptr, PNG_INFO_cHRM)) {

        // Use cHRM and gAMA to build profile
        cmsCIExyY white_point;
        cmsCIExyYTRIPLE primaries;

        png_get_cHRM(state->png_ptr, state->info_ptr, &white_point.x, &white_point.y, &primaries.Red.x,
                     &primaries.Red.y, &primaries.Green.x, &primaries.Green.y, &primaries.Blue.x, &primaries.Blue.y);

        white_point.Y = primaries.Red.Y = primaries.Green.Y = primaries.Blue.Y = 1.0;

        cmsToneCurve* gamma_table[3];
        gamma_table[0] = gamma_table[1] = gamma_table[2] = cmsBuildGamma(NULL, 1 / state->gamma);

        profile = cmsCreateRGBProfile(&white_point, &primaries, gamma_table);

        cmsFreeToneCurve(gamma_table[0]);

        state->color_profile_source = flow_codec_color_profile_source_GAMA_CHRM;
    }

    state->color_profile = profile;
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

    state->png_ptr = png_create_read_struct(PNG_LIBPNG_VER_STRING, state, png_decoder_error_handler, NULL);
    if (state->png_ptr == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        flow_job_png_decoder_reset(c, state);
        state->stage = flow_job_png_decoder_stage_Failed;
        return false;
    }

    state->info_ptr = png_create_info_struct(state->png_ptr);
    if (state->info_ptr == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        flow_job_png_decoder_reset(c, state);
        state->stage = flow_job_png_decoder_stage_Failed;
        return false;
    }
    // Set up error continuation
    if (setjmp(state->error_handler_jmp)) {
        // Execution comes back to this point if an error happens
        // We assume that the handler already set the context error
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
        flow_job_png_decoder_reset(c, state);
        state->stage = flow_job_png_decoder_stage_Failed;
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
    state->rowbytes = png_get_rowbytes(state->png_ptr, state->info_ptr);

    if (png_get_channels(state->png_ptr, state->info_ptr) != 4) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false; // Should always be 4
    }
    // We set this, but it's ignored and overwritten by existing callers
    state->pixel_buffer_size = state->rowbytes * state->h;

    return true;
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
        flow_job_png_decoder_reset(c, state);
        state->stage = flow_job_png_decoder_stage_Failed;
        FLOW_error(c, flow_status_Out_of_memory);
        return false;
    }

    state->stage = flow_job_png_decoder_stage_FinishRead;
    if (setjmp(state->error_handler_jmp)) {
        // Execution comes back to this point if an error happens
        return false;
    }

    state->pixel_buffer_row_pointers
        = flow_job_create_row_pointers(c, state->pixel_buffer, state->pixel_buffer_size, state->rowbytes, state->h);
    if (state->pixel_buffer_row_pointers == NULL) {
        flow_job_png_decoder_reset(c, state);
        state->stage = flow_job_png_decoder_stage_Failed;
        FLOW_error_return(c);
    }

    // The real work
    png_read_image(state->png_ptr, state->pixel_buffer_row_pointers);

    png_read_end(state->png_ptr, NULL);

    // Not sure if we should just call reset instead, or not...
    png_destroy_read_struct(&state->png_ptr, &state->info_ptr, NULL);

    return true;
}

void* flow_job_codecs_aquire_decode_png_on_buffer(flow_context* c, struct flow_job* job,
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

bool flow_job_codecs_png_get_info(flow_context* c, struct flow_job* job, void* codec_state,
                                  struct decoder_frame_info* decoder_frame_info_ref)
{
    struct flow_job_png_decoder_state* state = (struct flow_job_png_decoder_state*)codec_state;
    if (state->stage < flow_job_png_decoder_stage_BeginRead) {
        if (!flow_job_png_decoder_BeginRead(c, state)) {
            FLOW_error_return(c);
        }
    }
    decoder_frame_info_ref->w = state->w;
    decoder_frame_info_ref->h = state->h;
    decoder_frame_info_ref->format = flow_bgra32;
    return true;
}

bool flow_job_codecs_png_read_frame(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra* canvas)
{
    struct flow_job_png_decoder_state* state = (struct flow_job_png_decoder_state*)codec_state;
    if (state->stage == flow_job_png_decoder_stage_BeginRead) {
        state->pixel_buffer = canvas->pixels;
        state->pixel_buffer_size = canvas->stride * canvas->h;
        if (!flow_job_png_decoder_FinishRead(c, state)) {
            FLOW_error_return(c);
        }

        if (!flow_bitmap_bgra_transform_to_srgb(c, state->color_profile, canvas)) {
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

static void png_encoder_error_handler(png_structp png_ptr, png_const_charp msg)
{
    struct flow_job_png_encoder_state* state = (struct flow_job_png_encoder_state*)png_get_error_ptr(png_ptr);

    if (state == NULL) {
        exit(42);
        abort(); // WTF?
    }
    FLOW_error_msg(state->context, flow_status_Image_encoding_failed, "PNG encoding failed");

    longjmp(state->error_handler_jmp_buf, 1);
}

static void png_flush_nullop(png_structp png_ptr) {}

bool flow_bitmap_bgra_write_png(flow_context* c, struct flow_job* job, flow_bitmap_bgra* frame,
                                struct flow_job_resource_buffer* buffer)
{

    buffer->codec_state = NULL;

    struct flow_job_png_encoder_state* state
        = (struct flow_job_png_encoder_state*)flow_job_codecs_aquire_encode_png_on_buffer(c, job, buffer);

    if (state == NULL) {
        FLOW_error_return(c);
    }
    if (!flow_job_codecs_png_write_frame(c, job, state, frame)) {
        FLOW_error_return(c);
    }
    FLOW_free(c, buffer->codec_state);
    return true;
}

bool flow_job_codecs_png_write_frame(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra* frame)
{
    struct flow_job_png_encoder_state* state = (struct flow_job_png_encoder_state*)codec_state;
    state->buffer = NULL;
    state->size = 0;
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

    png_set_compression_level(png_ptr, Z_BEST_SPEED);
    png_set_text_compression_level(png_ptr, Z_DEFAULT_COMPRESSION);

    png_set_write_fn(png_ptr, state, png_write_data_callback, png_flush_nullop);

    info_ptr = png_create_info_struct(png_ptr);
    if (info_ptr == NULL)
        png_error(png_ptr, "OOM allocating info structure"); // TODO: comprehend png error handling
    {

        png_bytepp rows
            = flow_job_create_row_pointers(c, frame->pixels, frame->stride * frame->h, frame->stride, frame->h);
        if (rows == NULL) {
            FLOW_add_to_callstack(c);
            return false;
        }
        // TODO: check rows for NULL

        png_set_rows(png_ptr, info_ptr, rows);

        png_set_IHDR(png_ptr, info_ptr, (png_uint_32)frame->w, (png_uint_32)frame->h, 8, PNG_COLOR_TYPE_RGB_ALPHA,
                     PNG_INTERLACE_NONE, PNG_COMPRESSION_TYPE_BASE, PNG_FILTER_TYPE_BASE);

        png_set_sRGB(png_ptr, info_ptr, PNG_sRGB_INTENT_PERCEPTUAL);

        png_write_png(png_ptr, info_ptr, PNG_TRANSFORM_BGR, NULL);

        FLOW_free(c, rows);
        rows = NULL;
        png_destroy_write_struct(&png_ptr, &info_ptr);
        // Copy the final result to the output resource, if it exists.
        if (state->output_resource != NULL) {
            state->output_resource->buffer = state->buffer;
            state->output_resource->buffer_size = state->size;
        }
    }
    // TODO: maybe ? png_destroy_write_struct(&nv_ptr, &nv_info);
    return true;
}

void* flow_job_codecs_aquire_encode_png_on_buffer(flow_context* c, struct flow_job* job,
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
