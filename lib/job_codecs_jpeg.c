#include <stdio.h>
#include <jpeglib.h>
#include "imageflow_private.h"
#include "job.h"
#include "lcms2.h"
#include "job_codecs.h"

typedef enum flow_job_jpeg_decoder_stage {
    flow_job_jpg_decoder_stage_Null = 0,
    flow_job_jpg_decoder_stage_Failed,
    flow_job_jpg_decoder_stage_NotStarted,
    flow_job_jpg_decoder_stage_BeginRead,
    flow_job_jpg_decoder_stage_FinishRead,
} flow_job_jpeg_decoder_stage;

struct flow_job_jpeg_decoder_state {

    struct jpeg_error_mgr error_mgr; // MUST be first
    flow_job_jpeg_decoder_stage stage;
    struct jpeg_decompress_struct* cinfo;
    size_t row_stride;
    size_t w;
    size_t h;
    int channels;
    jmp_buf error_handler_jmp;
    void* file_bytes;
    size_t file_bytes_count;
    uint8_t* pixel_buffer;
    size_t pixel_buffer_size;
    uint8_t** pixel_buffer_row_pointers;
    flow_context* context;
    cmsHPROFILE color_profile;
    flow_job_color_profile_source color_profile_source;
    double gamma;
};

struct flow_job_jpeg_encoder_state {

    struct jpeg_error_mgr error_mgr; // MUST be first
    struct jpeg_compress_struct cinfo;
    jmp_buf error_handler_jmp;

    flow_context* context;
    char* buffer;
    size_t size;
    struct flow_job_resource_buffer* output_resource;
};

static bool flow_job_jpg_decoder_reset(flow_context* c, struct flow_job_jpeg_decoder_state* state);

static void jpeg_decode_error_exit(j_common_ptr cinfo)
{
    /* cinfo->err really points to a my_error_mgr struct, so coerce pointer */
    struct flow_job_jpeg_decoder_state* state = (struct flow_job_jpeg_decoder_state*)cinfo->err;

    /* Always display the message. */
    /* We could postpone this until after returning, if we chose. */
    (*cinfo->err->output_message)(cinfo);

    flow_job_jpg_decoder_reset(state->context, state);
    state->stage = flow_job_jpg_decoder_stage_Failed;
    FLOW_error(state->context, flow_status_Image_decoding_failed);

    /* Return control to the setjmp point */
    longjmp(state->error_handler_jmp, 1);
}

static const JOCTET EOI_BUFFER[1] = { JPEG_EOI };
typedef struct my_source_mgr {
    struct jpeg_source_mgr pub;
    const JOCTET* data;
    size_t len;
} my_source_mgr;

static void my_init_source(j_decompress_ptr cinfo) {}

static boolean my_fill_input_buffer(j_decompress_ptr cinfo)
{
    my_source_mgr* src = (my_source_mgr*)cinfo->src;
    // No more data.  Probably an incomplete image;  just output EOI.
    src->pub.next_input_byte = EOI_BUFFER;
    src->pub.bytes_in_buffer = 1;
    return TRUE;
}
static void my_skip_input_data(j_decompress_ptr cinfo, long num_bytes)
{
    my_source_mgr* src = (my_source_mgr*)cinfo->src;
    if ((long)src->pub.bytes_in_buffer < num_bytes) {
        // Skipping over all of remaining data;  output EOI.
        src->pub.next_input_byte = EOI_BUFFER;
        src->pub.bytes_in_buffer = 1;
    } else {
        // Skipping over only some of the remaining data.
        src->pub.next_input_byte += num_bytes;
        src->pub.bytes_in_buffer -= num_bytes;
    }
}
static void my_term_source(j_decompress_ptr cinfo) {}

static void my_set_source_mgr(j_decompress_ptr cinfo, const char* data, size_t len)
{
    my_source_mgr* src;
    if (cinfo->src == 0) { // if this is first time;  allocate memory
        cinfo->src = (struct jpeg_source_mgr*)(*cinfo->mem->alloc_small)((j_common_ptr)cinfo, JPOOL_PERMANENT,
                                                                         sizeof(my_source_mgr));
    }
    src = (my_source_mgr*)cinfo->src;
    src->pub.init_source = my_init_source;
    src->pub.fill_input_buffer = my_fill_input_buffer;
    src->pub.skip_input_data = my_skip_input_data;
    src->pub.resync_to_restart = jpeg_resync_to_restart; // default
    src->pub.term_source = my_term_source;
    // fill the buffers
    src->data = (const JOCTET*)data;
    src->len = len;
    src->pub.bytes_in_buffer = len;
    src->pub.next_input_byte = src->data;
}

static bool flow_job_jpg_decoder_BeginRead(flow_context* c, struct flow_job_jpeg_decoder_state* state)
{
    if (state->stage != flow_job_jpg_decoder_stage_NotStarted) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }
    if (!flow_job_jpg_decoder_reset(c, state)) {
        state->stage = flow_job_jpg_decoder_stage_Failed;
        FLOW_error_return(c);
    }
    state->stage = flow_job_jpg_decoder_stage_BeginRead;

    state->cinfo = (struct jpeg_decompress_struct*)FLOW_calloc(c, 1, sizeof(struct jpeg_decompress_struct));

    /* We set up the normal JPEG error routines, then override error_exit. */
    state->cinfo->err = jpeg_std_error(&state->error_mgr);
    state->error_mgr.error_exit = jpeg_decode_error_exit;

    if (state->cinfo == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        flow_job_jpg_decoder_reset(c, state);
        state->stage = flow_job_jpg_decoder_stage_Failed;
        return false;
    }
    /* Establish the setjmp return context for jpeg_decode_error_exit to use. */
    if (setjmp(state->error_handler_jmp)) {
        /* If we get here, the JPEG code has signaled an error.
         */
        if (state->stage != flow_job_jpg_decoder_stage_Failed) {
            exit(404); // This should never happen, jpeg_decode_error_exit should fix it.
        }
        return false;
    }
    /* Now we can initialize the JPEG decompression object. */
    jpeg_create_decompress(state->cinfo);

    // Set a source manager for reading from memory
    my_set_source_mgr(state->cinfo, (const char*)state->file_bytes, state->file_bytes_count);

    /* Step 3: read file parameters with jpeg_read_header() */

    (void)jpeg_read_header(state->cinfo, TRUE);
    /* We can ignore the return value from jpeg_read_header since
     *   (a) suspension is not possible with the stdio data source, and
     *   (b) we passed TRUE to reject a tables-only JPEG file as an error.
     * See libjpeg.txt for more info.
     */

    /* Step 4: set parameters for decompression */

    /* In this example, we don't need to change any of the defaults set by
     * jpeg_read_header(), so we do nothing here.
     */

    state->cinfo->out_color_space = JCS_EXT_BGRA;

    /* Step 5: Start decompressor */

    (void)jpeg_start_decompress(state->cinfo);

    /* We may need to do some setup of our own at this point before reading
 * the data.  After jpeg_start_decompress() we have the correct scaled
 * output image dimensions available, as well as the output colormap
 * if we asked for color quantization.
 * In this example, we need to make an output work buffer of the right size.
 */
    /* JSAMPLEs per row in output buffer */
    state->row_stride = state->cinfo->output_width * state->cinfo->output_components;
    state->w = state->cinfo->output_width;
    state->h = state->cinfo->output_height;
    state->channels = state->cinfo->output_components;
    state->gamma = state->cinfo->output_gamma;

    return true;
}

static bool flow_job_jpg_decoder_FinishRead(flow_context* c, struct flow_job_jpeg_decoder_state* state)
{
    if (state->stage != flow_job_jpg_decoder_stage_BeginRead) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }
    // We let the caller create the buffer
    //    state->pixel_buffer =  (jpg_bytep)FLOW_calloc (c, state->pixel_buffer_size, sizeof(jpg_bytep));
    if (state->pixel_buffer == NULL) {
        flow_job_jpg_decoder_reset(c, state);
        state->stage = flow_job_jpg_decoder_stage_Failed;
        FLOW_error(c, flow_status_Out_of_memory);
        return false;
    }

    state->stage = flow_job_jpg_decoder_stage_FinishRead;
    if (setjmp(state->error_handler_jmp)) {
        // Execution comes back to this point if an error happens
        return false;
    }

    state->pixel_buffer_row_pointers
        = flow_job_create_row_pointers(c, state->pixel_buffer, state->pixel_buffer_size, state->row_stride, state->h);
    if (state->pixel_buffer_row_pointers == NULL) {
        flow_job_jpg_decoder_reset(c, state);
        state->stage = flow_job_jpg_decoder_stage_Failed;
        FLOW_error_return(c);
    }

    uint32_t scanlines_read = 0;
    /* Step 6: while (scan lines remain to be read) */
    /*           jpeg_read_scanlines(...); */

    /* Here we use the library's state variable cinfo.output_scanline as the
     * loop counter, so that we don't have to keep track ourselves.
     */
    while (state->cinfo->output_scanline < state->cinfo->output_height) {
        /* jpeg_read_scanlines expects an array of pointers to scanlines.
         * Here the array is only one element long, but you could ask for
         * more than one scanline at a time if that's more convenient.
         */
        scanlines_read = jpeg_read_scanlines(
            state->cinfo, &state->pixel_buffer_row_pointers[state->cinfo->output_scanline], state->h);
    }

    if (scanlines_read < 1) {
        return false;
    }
    /* Step 7: Finish decompression */

    (void)jpeg_finish_decompress(state->cinfo);
    /* We can ignore the return value since suspension is not possible
     * with the stdio data source.
     */

    jpeg_destroy_decompress(state->cinfo);
    FLOW_free(c, state->cinfo);
    state->cinfo = NULL;

    return true;
}

static bool flow_job_jpg_decoder_reset(flow_context* c, struct flow_job_jpeg_decoder_state* state)
{
    if (state->stage == flow_job_jpg_decoder_stage_FinishRead) {
        FLOW_free(c, state->pixel_buffer);
    }
    if (state->stage == flow_job_jpg_decoder_stage_Null) {
        state->pixel_buffer_row_pointers = NULL;
        state->color_profile = NULL;
        state->cinfo = NULL;
    } else {

        if (state->cinfo != NULL) {
            jpeg_destroy_decompress(state->cinfo);
            FLOW_free(c, state->cinfo);
            state->cinfo = NULL;
        }
        memset(&state->error_mgr, 0, sizeof(struct jpeg_error_mgr));

        if (state->color_profile != NULL) {
            cmsCloseProfile(state->color_profile);
            state->color_profile = NULL;
        }
        if (state->pixel_buffer_row_pointers != NULL) {
            FLOW_free(c, state->pixel_buffer_row_pointers);
            state->pixel_buffer_row_pointers = NULL;
        }
    }
    state->color_profile_source = flow_job_color_profile_source_null;
    state->row_stride = 0;
    state->context = c;
    state->w = 0;
    state->h = 0;
    state->gamma = 0.45455;
    state->pixel_buffer = NULL;
    state->pixel_buffer_size = -1;
    state->channels = 0;
    state->stage = flow_job_jpg_decoder_stage_NotStarted;
    return true;
}

void* flow_job_codecs_aquire_decode_jpeg_on_buffer(flow_context* c, struct flow_job* job,
                                                   struct flow_job_resource_buffer* buffer)
{
    // flow_job_jpeg_decoder_state
    if (buffer->codec_state == NULL) {
        struct flow_job_jpeg_decoder_state* state
            = (struct flow_job_jpeg_decoder_state*)FLOW_malloc(c, sizeof(struct flow_job_jpeg_decoder_state));
        if (state == NULL) {
            FLOW_error(c, flow_status_Out_of_memory);
            return NULL;
        }
        state->stage = flow_job_jpg_decoder_stage_Null;

        if (!flow_job_jpg_decoder_reset(c, state)) {
            FLOW_add_to_callstack(c);
            return NULL;
        }
        state->file_bytes = buffer->buffer;
        state->file_bytes_count = buffer->buffer_size;

        buffer->codec_state = (void*)state;
    }
    return buffer->codec_state;
}

bool flow_job_codecs_jpeg_get_info(flow_context* c, struct flow_job* job, void* codec_state,
                                   struct decoder_frame_info* decoder_frame_info_ref)
{
    struct flow_job_jpeg_decoder_state* state = (struct flow_job_jpeg_decoder_state*)codec_state;
    if (state->stage < flow_job_jpg_decoder_stage_BeginRead) {
        if (!flow_job_jpg_decoder_BeginRead(c, state)) {
            FLOW_error_return(c);
        }
    }
    decoder_frame_info_ref->w = state->w;
    decoder_frame_info_ref->h = state->h;
    decoder_frame_info_ref->format = flow_bgra32; // state->channels == 1 ? flow_gray8 : flow_bgr24;
    return true;
}

bool flow_job_codecs_jpeg_read_frame(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra* canvas)
{
    struct flow_job_jpeg_decoder_state* state = (struct flow_job_jpeg_decoder_state*)codec_state;
    if (state->stage == flow_job_jpg_decoder_stage_BeginRead) {
        state->pixel_buffer = canvas->pixels;
        state->pixel_buffer_size = canvas->stride * canvas->h;
        if (!flow_job_jpg_decoder_FinishRead(c, state)) {
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

void* flow_job_codecs_aquire_encode_jpeg_on_buffer(flow_context* c, struct flow_job* job,
                                                   struct flow_job_resource_buffer* buffer)
{
    // flow_job_jpeg_decoder_state
    if (buffer->codec_state == NULL) {
        struct flow_job_jpeg_encoder_state* state
            = (struct flow_job_jpeg_encoder_state*)FLOW_malloc(c, sizeof(struct flow_job_jpeg_encoder_state));
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

static void jpeg_encode_error_exit(j_common_ptr cinfo)
{
    /* cinfo->err really points to a my_error_mgr struct, so coerce pointer */
    struct flow_job_jpeg_encoder_state* state = (struct flow_job_jpeg_encoder_state*)cinfo->err;

    /* Always display the message. */
    /* We could postpone this until after returning, if we chose. */
    (*cinfo->err->output_message)(cinfo);

    // TODO - cleanup??

    FLOW_error(state->context, flow_status_Image_encoding_failed);

    jpeg_destroy_compress(&state->cinfo);

    /* Return control to the setjmp point */
    longjmp(state->error_handler_jmp, 1);
}

bool flow_job_codecs_jpeg_write_frame(flow_context* c, struct flow_job* job, void* codec_state, flow_bitmap_bgra* frame)
{
    struct flow_job_jpeg_encoder_state* state = (struct flow_job_jpeg_encoder_state*)codec_state;
    state->buffer = NULL;
    state->size = 0;
    state->context = c;

    state->cinfo.err = jpeg_std_error(&state->error_mgr);
    state->error_mgr.error_exit = jpeg_encode_error_exit;

    if (setjmp(state->error_handler_jmp)) {
        // Execution comes back to this point if an error happens
        // We assume that the handler already set the context error
        return false;
    }

    jpeg_create_compress(&state->cinfo);

    jpeg_mem_dest(&state->cinfo, (unsigned char**)&state->buffer, &state->size);

    state->cinfo.in_color_space = JCS_EXT_BGRA;
    state->cinfo.image_height = frame->h;
    state->cinfo.image_width = frame->w;
    state->cinfo.input_components = 4;
    state->cinfo.optimize_coding = true;

    jpeg_set_defaults(&state->cinfo);

    int quality = 90;

    jpeg_set_quality(&state->cinfo, quality, TRUE /* limit to baseline-JPEG values */);

    jpeg_simple_progression(&state->cinfo);

    jpeg_start_compress(&state->cinfo, TRUE);

    uint8_t** rows = flow_job_create_row_pointers(c, frame->pixels, frame->stride * frame->h, frame->stride, frame->h);
    if (rows == NULL) {
        FLOW_add_to_callstack(c);
        jpeg_destroy_compress(&state->cinfo);
        return false;
    }

    (void)jpeg_write_scanlines(&state->cinfo, rows, frame->h);

    jpeg_finish_compress(&state->cinfo);

    jpeg_destroy_compress(&state->cinfo);

    if (state->error_mgr.num_warnings > 0) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }

    // Copy the final result to the output resource, if it exists.
    if (state->output_resource != NULL) {
        state->output_resource->buffer = state->buffer;
        state->output_resource->buffer_size = state->size;
    }

    return true;
}
