#include <stdio.h>
#include "jpeglib.h"
#include "imageflow_private.h"
#include "lcms2.h"
#include "codecs.h"
#include "jerror.h"

#define FLOW_JPEG_INPUT_BUFFER_SIZE 4096
#define FLOW_JPEG_OUTPUT_BUFFER_SIZE 4096

struct flow_jpeg_source_manager {
    struct jpeg_source_mgr pub;
    struct flow_io * io;
    JOCTET * buffer;
    boolean bytes_have_been_read;
};

//! Called by jpeg_read_header() before anything is read.
static void _flow_jpeg_io_source_init(j_decompress_ptr cinfo)
{
    struct flow_jpeg_source_manager * src = (struct flow_jpeg_source_manager *)cinfo->src;

    // If we read multiple sequential files from the same flow_io*, we need to reset this (but not the buffer).
    src->bytes_have_been_read = FALSE;
}

//! Obtains at minimum one more byte, advancing the cursor and resetting the buffer. Always returns true (no suspension
// support)
static boolean _flow_jpeg_io_source_fill_input_buffer(j_decompress_ptr cinfo)
{
    struct flow_jpeg_source_manager * src = (struct flow_jpeg_source_manager *)cinfo->src;

    size_t bytes_read = src->io->read_func(src->io->context, src->io, src->buffer, FLOW_JPEG_INPUT_BUFFER_SIZE);
    if (bytes_read <= 0) {
        // Empty file is a critical error - die fast and release resources/temp files
        if (!src->bytes_have_been_read) {
            if (flow_context_has_error(src->io->context)) {
                FLOW_add_to_callstack(src->io->context);
            } else {
                FLOW_error_msg(src->io->context, flow_status_IO_error, "Input file has zero bytes");
            }
            jpeg_destroy((j_common_ptr)cinfo);

            // Raise error with jpeg and call error_exit which will jump back to our handler
            struct jpeg_error_mgr * err = cinfo->err;
            err->msg_code = JERR_INPUT_EMPTY;
            err->error_exit((j_common_ptr)cinfo);
        }

        // Raise warning that we hie EOF unexpectedly
        struct jpeg_error_mgr * err = cinfo->err;
        err->msg_code = JWRN_JPEG_EOF;
        err->emit_message((j_common_ptr)cinfo, -1);

        // Pretend the file ended normally; we might be able to recover
        src->buffer[0] = (JOCTET)0xFF;
        src->buffer[1] = (JOCTET)JPEG_EOI;
        bytes_read = 2;
    }

    src->pub.next_input_byte = src->buffer;
    src->pub.bytes_in_buffer = bytes_read;
    src->bytes_have_been_read = TRUE;
    return TRUE;
}

//! Skip byte_count bytes, and refill buffer at end. bytes_in_buffer may be zero.
static void _flow_jpeg_io_source_skip_input_data(j_decompress_ptr cinfo, long byte_count)
{
    struct flow_jpeg_source_manager * src = (struct flow_jpeg_source_manager *)cinfo->src;

    // Doesn't support suspension; could be improved to use seek_func instead
    // This would help with skipping undesired segments of the jpeg file (which are rare-ish)
    if (byte_count > 0) {
        while (byte_count > (long)src->pub.bytes_in_buffer) {
            byte_count -= (long)src->pub.bytes_in_buffer;
            (void)_flow_jpeg_io_source_fill_input_buffer(cinfo);
        }

        src->pub.next_input_byte += (size_t)byte_count;
        src->pub.bytes_in_buffer -= (size_t)byte_count;
    }
}

//! Called at the end by jpeg_finish_decompress for the happy path only - not on jpeg_abort/destroy
static void _flow_jpeg_io_source_terminate(j_decompress_ptr cinfo) {}

//! Sets up the jpeg input proxy to work with *io.
void flow_codecs_jpeg_setup_source_manager(j_decompress_ptr cinfo, struct flow_io * io)
{
    struct flow_jpeg_source_manager * src;

    // We're using libjpeg's memory allocation stuff here for no good reason
    // We could use our own ownership-based release system instead, and tie lifetime to *io
    if (cinfo->src == NULL) {
        cinfo->src = (struct jpeg_source_mgr *)(*cinfo->mem->alloc_small)((j_common_ptr)cinfo, JPOOL_PERMANENT,
                                                                          sizeof(struct flow_jpeg_source_manager));

        src = (struct flow_jpeg_source_manager *)cinfo->src;

        src->buffer = (JOCTET *)(*cinfo->mem->alloc_small)((j_common_ptr)cinfo, JPOOL_PERMANENT,
                                                           FLOW_JPEG_INPUT_BUFFER_SIZE * sizeof(JOCTET));
    }

    src = (struct flow_jpeg_source_manager *)cinfo->src;
    src->pub.init_source = _flow_jpeg_io_source_init;
    src->pub.fill_input_buffer = _flow_jpeg_io_source_fill_input_buffer;
    src->pub.skip_input_data = _flow_jpeg_io_source_skip_input_data;
    src->pub.resync_to_restart = jpeg_resync_to_restart; // use default method
    src->pub.term_source = _flow_jpeg_io_source_terminate;
    src->io = io;
    // We don't pre-fill any bytes
    src->pub.bytes_in_buffer = 0;
    src->pub.next_input_byte = NULL;
}
