#include "codec_jpeg_wrapper.h"

static void jpeg_error_exit(j_common_ptr cinfo) {
    /* cinfo->err really points to a my_error_mgr struct, so coerce pointer */
    struct flow_jpeg_wrapper_error_state *state = (struct flow_jpeg_wrapper_error_state *) cinfo->err;


    /* Acquire the message. */
    char warning_buffer[JMSG_LENGTH_MAX];
    //Q: If this ever fails to set a null byte we are screwed when we format it later
    cinfo->err->format_message(cinfo, warning_buffer);

    bool result = state->error_handler(state->custom_state, cinfo, &state->error_mgr, state->error_mgr.msg_code, &warning_buffer[0], JMSG_LENGTH_MAX );

    if (result) {
        return;
    }else{
        /* Return control to the setjmp point */
        longjmp(state->error_handler_jmp, 1);
    }
    // Uncomment to permit JPEGs with unknown markers
    // if (state->error_mgr.msg_code == JERR_UNKNOWN_MARKER) return;
    // Destroy memory allocs and temp files
    // Specialized routines are wrappers for jpeg_destroy_compress
    jpeg_destroy(cinfo);
}

//! Ignores warnings
static void flow_jpeg_output_message(j_common_ptr cinfo)
{
    // char buffer[JMSG_LENGTH_MAX];
    // cinfo->err->format_message(cinfo, buffer);
    // TODO: maybe create a warnings log in flow_context, and append? Users aren't reading stderr
    // fprintf(stderr, "%s", &buffer[0]);
}

static bool flow_codecs_jpg_decoder_BeginRead(flow_c * c, struct flow_codecs_jpeg_decoder_state * state)
{
    if (state->stage != flow_codecs_jpg_decoder_stage_NotStarted) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }
    if (!flow_codecs_jpg_decoder_reset(c, state)) {
        state->stage = flow_codecs_jpg_decoder_stage_Failed;
        FLOW_error_return(c);
    }
    state->stage = flow_codecs_jpg_decoder_stage_BeginRead;

    state->cinfo = (struct jpeg_decompress_struct *)FLOW_calloc(c, 1, sizeof(struct jpeg_decompress_struct));
    if (state->cinfo == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        flow_codecs_jpg_decoder_reset(c, state);
        state->stage = flow_codecs_jpg_decoder_stage_Failed;
        return false;
    }

    /* We set up the normal JPEG error routines, then override error_exit and output_message. */
    state->cinfo->err = jpeg_std_error(&state->error_mgr);
    state->error_mgr.error_exit = jpeg_error_exit;
    state->error_mgr.output_message = flow_jpeg_output_message; // Prevent USE_WINDOWS_MESSAGEBOX


    /* Establish the setjmp return context for jpeg_error_exit to use. */
    if (setjmp(state->error_handler_jmp)) {
        /* If we get here, the JPEG code has signaled an error.
         */
        if (state->stage != flow_codecs_jpg_decoder_stage_Failed) {
            exit(404); // This should never happen, jpeg_error_exit should fix it.
        }
        return false;
    }
    /* Now we can initialize the JPEG decompression object. */
    jpeg_create_decompress(state->cinfo);

    // Set a source manager for reading from memory
    flow_codecs_jpeg_setup_source_manager(state->cinfo, state->io);

    /* Step 3: read file parameters with jpeg_read_header() */

    /* Tell the library to keep any APP2 data it may find */
    jpeg_save_markers(state->cinfo, ICC_MARKER, 0xFFFF);
    jpeg_save_markers(state->cinfo, EXIF_JPEG_MARKER, 0xffff);

    (void)jpeg_read_header(state->cinfo, TRUE);

    if (!flow_codecs_jpg_decoder_interpret_metadata(c, state)) {
        flow_codecs_jpg_decoder_reset(c, state);
        state->stage = flow_codecs_jpg_decoder_stage_Failed;
        FLOW_error_return(c);
    }
    /* We can ignore the return value from jpeg_read_header since
     *   (a) suspension is not possible with the stdio data source, and
     *   (b) we passed TRUE to reject a tables-only JPEG file as an error.
     * See libjpeg.txt for more info.
     */

    /* Step 4: set parameters for decompression */
    state->cinfo->out_color_space = JCS_EXT_BGRA;

    state->w = state->cinfo->image_width;
    state->h = state->cinfo->image_height;
    return true;
}

static bool flow_codecs_jpg_decoder_FinishRead(flow_c * c, struct flow_codecs_jpeg_decoder_state * state)
{
    if (state->stage != flow_codecs_jpg_decoder_stage_BeginRead) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }
    // We let the caller create the buffer
    //    state->pixel_buffer =  (jpg_bytep)FLOW_calloc (c, state->pixel_buffer_size, sizeof(jpg_bytep));
    if (state->pixel_buffer == NULL || state->canvas == NULL) {
        flow_codecs_jpg_decoder_reset(c, state);
        state->stage = flow_codecs_jpg_decoder_stage_Failed;
        FLOW_error(c, flow_status_Out_of_memory);
        return false;
    }
    if (setjmp(state->error_handler_jmp)) {
        // Execution comes back to this point if an error happens
        return false;
    }
    /* Step 5: Start decompressor */

    (void)jpeg_start_decompress(state->cinfo);

    /* We may need to do some setup of our own at this point before reading
 * the data.  After jpeg_start_decompress() we have the correct scaled
 * output image dimensions available, as well as the output colormap
 * if we asked for color quantization.
 * In this example, we need to make an output work buffer of the right size.
 */
    /* JSAMPLEs per row in output buffer */

    // state->row_stride = state->cinfo->output_width * state->cinfo->output_components;
    state->channels = state->cinfo->output_components;
    state->color.gamma = state->cinfo->output_gamma;

    state->stage = flow_codecs_jpg_decoder_stage_FinishRead;


    state->pixel_buffer_row_pointers = flow_bitmap_create_row_pointers(c, state->pixel_buffer, state->pixel_buffer_size,
                                                                       state->row_stride, state->h);
    if (state->pixel_buffer_row_pointers == NULL) {
        flow_codecs_jpg_decoder_reset(c, state);
        state->stage = flow_codecs_jpg_decoder_stage_Failed;
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
            state->cinfo, &state->pixel_buffer_row_pointers[state->cinfo->output_scanline], (JDIMENSION)state->h);
    }

    if (scanlines_read < 1) {
        return false;
    }

    // We must read the markers before jpeg_finish_decompress destroys them

    if (!flow_codecs_jpg_decoder_interpret_metadata(c, state)) {
        flow_codecs_jpg_decoder_reset(c, state);
        state->stage = flow_codecs_jpg_decoder_stage_Failed;
        FLOW_error_return(c);
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
