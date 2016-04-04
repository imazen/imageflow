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

struct flow_jpeg_destination_manager {
    struct jpeg_destination_mgr pub;
    struct flow_io * io;
    JOCTET * buffer;
};

//! called by jpeg_finish_compress(). It must leave next_output_byte and free_in_buffer with space available for
// immediate writing.
static void _flow_jpeg_io_dest_init(j_compress_ptr cinfo)
{
    struct flow_jpeg_destination_manager * dest = (struct flow_jpeg_destination_manager *)cinfo->dest;

    dest->buffer = (JOCTET *)(*cinfo->mem->alloc_small)((j_common_ptr)cinfo, JPOOL_IMAGE,
                                                        FLOW_JPEG_OUTPUT_BUFFER_SIZE * sizeof(JOCTET));
    dest->pub.next_output_byte = dest->buffer;
    dest->pub.free_in_buffer = FLOW_JPEG_OUTPUT_BUFFER_SIZE;
}

//! Called when free_in_buffer == 0; should flush entire underlying buffer and reset pointer/count. Always returns TRUE
//- suspension not supported.
static boolean _flow_jpeg_io_dest_empty_output_buffer(j_compress_ptr cinfo)
{
    struct flow_jpeg_destination_manager * dest = (struct flow_jpeg_destination_manager *)cinfo->dest;

    if (dest->io->write_func(dest->io->context, dest->io, dest->buffer, FLOW_JPEG_OUTPUT_BUFFER_SIZE)
        != FLOW_JPEG_OUTPUT_BUFFER_SIZE) {
        if (flow_context_has_error(dest->io->context)) {
            FLOW_add_to_callstack(dest->io->context);
        } else {
            FLOW_error_msg(dest->io->context, flow_status_IO_error, "Failed to write all %l bytes to output flow_io *",
                           FLOW_JPEG_OUTPUT_BUFFER_SIZE);
        }
        jpeg_destroy((j_common_ptr)cinfo);

        // Raise error with jpeg and call error_exit which will jump back to our handler
        struct jpeg_error_mgr * err = cinfo->err;
        err->msg_code = JERR_FILE_WRITE;
        err->error_exit((j_common_ptr)cinfo);
    }

    dest->pub.next_output_byte = dest->buffer;
    dest->pub.free_in_buffer = FLOW_JPEG_OUTPUT_BUFFER_SIZE;

    return TRUE;
}

//! called by jpeg_finish_compress() to flush remaining bytes after last write
static void _flow_jpeg_io_dest_terminate(j_compress_ptr cinfo)
{
    struct flow_jpeg_destination_manager * dest = (struct flow_jpeg_destination_manager *)cinfo->dest;

    size_t remaining_bytes = FLOW_JPEG_OUTPUT_BUFFER_SIZE - dest->pub.free_in_buffer;
    // Flush to underlying io
    if (remaining_bytes > 0) {
        if (dest->io->write_func(dest->io->context, dest->io, dest->buffer, (size_t)remaining_bytes)
            != (int64_t)remaining_bytes) {
            if (flow_context_has_error(dest->io->context)) {
                FLOW_add_to_callstack(dest->io->context);
            } else {
                FLOW_error_msg(dest->io->context, flow_status_IO_error,
                               "Failed to write all %l bytes to output flow_io *", remaining_bytes);
            }
            jpeg_destroy((j_common_ptr)cinfo);
            // Raise error with jpeg and call error_exit which will jump back to our handler
            struct jpeg_error_mgr * err = cinfo->err;
            err->msg_code = JERR_FILE_WRITE;
            err->error_exit((j_common_ptr)cinfo);
        }
    }
}

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

//! Sets up the jpeg output proxy to work with *io.
void flow_codecs_jpeg_setup_dest_manager(j_compress_ptr cinfo, struct flow_io * io)
{

    struct flow_jpeg_destination_manager * dest;

    if (cinfo->dest == NULL) {
        cinfo->dest = (struct jpeg_destination_mgr *)(*cinfo->mem->alloc_small)(
            (j_common_ptr)cinfo, JPOOL_PERMANENT, sizeof(struct flow_jpeg_destination_manager));
    }

    dest = (struct flow_jpeg_destination_manager *)cinfo->dest;
    dest->pub.init_destination = _flow_jpeg_io_dest_init;
    dest->pub.empty_output_buffer = _flow_jpeg_io_dest_empty_output_buffer;
    dest->pub.term_destination = _flow_jpeg_io_dest_terminate;
    dest->io = io;
}
