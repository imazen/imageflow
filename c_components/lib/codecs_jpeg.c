#include <stdio.h>
#include "jpeglib.h"
#include "jerror.h"
#include "imageflow_private.h"
#include "lcms2.h"
#include "codecs.h"
#include "codecs_jpeg.h"
#include "fastapprox.h"

#define ICC_MARKER (JPEG_APP0 + 2) /* JPEG marker code for ICC */
#define ICC_OVERHEAD_LEN 14 /* size of non-profile data in APP2 */
#define MAX_BYTES_IN_MARKER 65533 /* maximum data len of a JPEG marker */
#define MAX_DATA_BYTES_IN_MARKER (MAX_BYTES_IN_MARKER - ICC_OVERHEAD_LEN)

static bool flow_codecs_jpg_decoder_reset(flow_c * c, struct flow_codecs_jpeg_decoder_state * state);

static void jpeg_error_exit(j_common_ptr cinfo) {
    /* cinfo->err really points to a my_error_mgr struct, so coerce pointer */
    struct flow_codecs_jpeg_decoder_state *state = (struct flow_codecs_jpeg_decoder_state *) cinfo->err;

    // Uncomment to permit JPEGs with unknown markers
    // if (state->error_mgr.msg_code == JERR_UNKNOWN_MARKER) return;


    /* Acquire the message. */
    char warning_buffer[JMSG_LENGTH_MAX];
    //Q: If this ever fails to set a null byte we are screwed when we format it later
    cinfo->err->format_message(cinfo, warning_buffer);


    // Destroy memory allocs and temp files
    // Specialized routines are wrappers for jpeg_destroy_compress
    jpeg_destroy(cinfo);

    flow_codecs_jpg_decoder_reset(state->context, state);
    state->stage = flow_codecs_jpg_decoder_stage_Failed;
    if (!flow_context_has_error(state->context)) {
        FLOW_error_msg(state->context, flow_status_Image_decoding_failed, "Mozjpeg decoding error: %s", warning_buffer);
    }

    /* Return control to the setjmp point */
    longjmp(state->error_handler_jmp, 1);
}

//! Ignores warnings
static void flow_jpeg_output_message(j_common_ptr cinfo)
{
    // char buffer[JMSG_LENGTH_MAX];
    // cinfo->err->format_message(cinfo, buffer);
    // TODO: maybe create a warnings log in flow_context, and append? Users aren't reading stderr
    // fprintf(stderr, "%s", &buffer[0]);
}

static boolean marker_is_icc(jpeg_saved_marker_ptr marker)
{
    return marker->marker == ICC_MARKER && marker->data_length >= ICC_OVERHEAD_LEN &&
           /* verify the identifying string */
           GETJOCTET(marker->data[0]) == 0x49 && GETJOCTET(marker->data[1]) == 0x43
           && GETJOCTET(marker->data[2]) == 0x43 && GETJOCTET(marker->data[3]) == 0x5F
           && GETJOCTET(marker->data[4]) == 0x50 && GETJOCTET(marker->data[5]) == 0x52
           && GETJOCTET(marker->data[6]) == 0x4F && GETJOCTET(marker->data[7]) == 0x46
           && GETJOCTET(marker->data[8]) == 0x49 && GETJOCTET(marker->data[9]) == 0x4C
           && GETJOCTET(marker->data[10]) == 0x45 && GETJOCTET(marker->data[11]) == 0x0;
}

/*
 * See if there was an ICC profile in the JPEG file being read;
 * if so, reassemble and return the profile data.
 *
 * TRUE is returned if an ICC profile was found, FALSE if not.
 * If TRUE is returned, *icc_data_ptr is set to point to the
 * returned data, and *icc_data_len is set to its length.
 *
 * NOTE: if the file contains invalid ICC APP2 markers, we just silently
 * return FALSE.  You might want to issue an error message instead.
 */

static boolean read_icc_profile(flow_c * c, j_decompress_ptr cinfo, JOCTET ** icc_data_ptr, unsigned int * icc_data_len)
{
    jpeg_saved_marker_ptr marker;
    int num_markers = 0;
    int seq_no;
    JOCTET * icc_data;
    unsigned int total_length;
#define MAX_SEQ_NO 255 /* sufficient since marker numbers are bytes */
    char marker_present[MAX_SEQ_NO + 1]; /* 1 if marker found */
    unsigned int data_length[MAX_SEQ_NO + 1]; /* size of profile data in marker */
    unsigned int data_offset[MAX_SEQ_NO + 1]; /* offset for data in marker */

    *icc_data_ptr = NULL; /* avoid confusion if FALSE return */
    *icc_data_len = 0;

    /* This first pass over the saved markers discovers whether there are
     * any ICC markers and verifies the consistency of the marker numbering.
     */

    for (seq_no = 1; seq_no <= MAX_SEQ_NO; seq_no++)
        marker_present[seq_no] = 0;

    for (marker = cinfo->marker_list; marker != NULL; marker = marker->next) {
        if (marker_is_icc(marker)) {
            if (num_markers == 0)
                num_markers = GETJOCTET(marker->data[13]);
            else if (num_markers != GETJOCTET(marker->data[13]))
                return FALSE; /* inconsistent num_markers fields */
            seq_no = GETJOCTET(marker->data[12]);
            if (seq_no <= 0 || seq_no > num_markers)
                return FALSE; /* bogus sequence number */
            if (marker_present[seq_no])
                return FALSE; /* duplicate sequence numbers */
            marker_present[seq_no] = 1;
            data_length[seq_no] = marker->data_length - ICC_OVERHEAD_LEN;
        }
    }

    if (num_markers == 0)
        return FALSE;

    /* Check for missing markers, count total space needed,
     * compute offset of each marker's part of the data.
     */

    total_length = 0;
    for (seq_no = 1; seq_no <= num_markers; seq_no++) {
        if (marker_present[seq_no] == 0)
            return FALSE; /* missing sequence number */
        data_offset[seq_no] = total_length;
        total_length += data_length[seq_no];
    }

    if (total_length <= 0)
        return FALSE; /* found only empty markers? */

    /* Allocate space for assembled data */
    icc_data = (JOCTET *)FLOW_malloc_owned(c, total_length * sizeof(JOCTET), c);
    if (icc_data == NULL)
        return FALSE; /* oops, out of memory */

    /* and fill it in */
    for (marker = cinfo->marker_list; marker != NULL; marker = marker->next) {
        if (marker_is_icc(marker)) {
            JOCTET FAR * src_ptr;
            JOCTET * dst_ptr;
            unsigned int length;
            seq_no = GETJOCTET(marker->data[12]);
            dst_ptr = icc_data + data_offset[seq_no];
            src_ptr = marker->data + ICC_OVERHEAD_LEN;
            length = data_length[seq_no];
            while (length--) {
                *dst_ptr++ = *src_ptr++;
            }
        }
    }

    *icc_data_ptr = icc_data;
    *icc_data_len = total_length;

    return TRUE;
}

/// From http://src.gnu-darwin.org/ports/x11-toolkits/gtk20/work/gtk+-2.12.3/gdk-pixbuf/io-jpeg.c

// TODO: Remove before relicensing as anything other than LGPL v2+ compatible

/* -*- mode: C; c-file-style: "linux" -*- */
/* GdkPixbuf library - JPEG image loader
 *
 * Copyright (C) 1999 Michael Zucchi
 * Copyright (C) 1999 The Free Software Foundation
 *
 * Progressive loading code Copyright (C) 1999 Red Hat, Inc.
 *
 * Authors: Michael Zucchi <zucchi@zedzone.mmc.com.au>
 *          Federico Mena-Quintero <federico@gimp.org>
 *          Michael Fulbright <drmike@redhat.com>
 *
 * This library is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public
 * License as published by the Free Software Foundation; either
 * version 2 of the License, or (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public
 * License along with this library; if not, write to the
 * Free Software Foundation, Inc., 59 Temple Place - Suite 330,
 * Boston, MA 02111-1307, USA.
 */

#define G_LITTLE_ENDIAN 1234

#define G_BIG_ENDIAN 4321

const char leth[] = { 0x49, 0x49, 0x2a, 0x00 }; // Little endian TIFF header
const char beth[] = { 0x4d, 0x4d, 0x00, 0x2a }; // Big endian TIFF header
const char types[]
    = { 0x00, 0x01, 0x01, 0x02, 0x04, 0x08, 0x00, 0x08, 0x00, 0x04, 0x08 }; // size in bytes for EXIF types

#define GUINT16_SWAP_LE_BE_CONSTANT(val)                                                                               \
    ((uint16_t)((uint16_t)((uint16_t)(val) >> 8) | (uint16_t)((uint16_t)(val) << 8)))

#define GUINT32_SWAP_LE_BE_CONSTANT(val)                                                                               \
    ((uint32_t)((((uint32_t)(val) & (uint32_t)0x000000ffU) << 24) | (((uint32_t)(val) & (uint32_t)0x0000ff00U) << 8)   \
                | (((uint32_t)(val) & (uint32_t)0x00ff0000U) >> 8)                                                     \
                | (((uint32_t)(val) & (uint32_t)0xff000000U) >> 24)))

#define GUINT32_SWAP_LE_BE(val) (GUINT32_SWAP_LE_BE_CONSTANT(val))
#define GUINT16_SWAP_LE_BE(val) (GUINT16_SWAP_LE_BE_CONSTANT(val))

#define GUINT16_TO_LE(val) ((uint16_t)(val))
#define GUINT16_TO_BE(val) (GUINT16_SWAP_LE_BE(val))
#define GUINT32_TO_LE(val) ((uint32_t)(val))
#define GUINT32_TO_BE(val) (GUINT32_SWAP_LE_BE(val))

#define GUINT16_FROM_LE(val) (GUINT16_TO_LE(val))
#define GUINT16_FROM_BE(val) (GUINT16_TO_BE(val))
#define GUINT32_FROM_BE(val) (GUINT32_TO_BE(val))
#define GUINT32_FROM_LE(val) (GUINT32_TO_LE(val))

#define DE_ENDIAN16(val) endian == G_BIG_ENDIAN ? GUINT16_FROM_BE(val) : GUINT16_FROM_LE(val)
#define DE_ENDIAN32(val) endian == G_BIG_ENDIAN ? GUINT32_FROM_BE(val) : GUINT32_FROM_LE(val)

#define ENDIAN16_IT(val) endian == G_BIG_ENDIAN ? GUINT16_TO_BE(val) : GUINT16_TO_LE(val)
#define ENDIAN32_IT(val) endian == G_BIG_ENDIAN ? GUINT32_TO_BE(val) : GUINT32_TO_LE(val)

#define EXIF_JPEG_MARKER JPEG_APP0 + 1
#define EXIF_IDENT_STRING "Exif\000\000"

static unsigned short de_get16(void * ptr, uint32_t endian)
{
    unsigned short val;

    memcpy(&val, ptr, sizeof(val));
    val = DE_ENDIAN16(val);

    return val;
}

static unsigned int de_get32(void * ptr, uint32_t endian)
{
    unsigned int val;

    memcpy(&val, ptr, sizeof(val));
    val = DE_ENDIAN32(val);

    return val;
}

static int32_t get_orientation(j_decompress_ptr cinfo)
{
    /* This function looks through the meta data in the libjpeg decompress structure to
       determine if an EXIF Orientation tag is present and if so return its value (1-8).
       If no EXIF Orientation tag is found 0 (zero) is returned. */

    uint32_t i; /* index into working buffer */
    uint32_t orient_tag_id; /* endianed version of orientation tag ID */
    uint32_t ret; /* Return value */
    uint32_t offset; /* de-endianed offset in various situations */
    uint32_t tags; /* number of tags in current ifd */
    uint32_t type; /* de-endianed type of tag used as index into types[] */
    uint32_t count; /* de-endianed count of elements in a tag */
    uint32_t tiff = 0; /* offset to active tiff header */
    uint32_t endian = 0; /* detected endian of data */

    jpeg_saved_marker_ptr exif_marker; /* Location of the Exif APP1 marker */
    jpeg_saved_marker_ptr cmarker; /* Location to check for Exif APP1 marker */

    /* check for Exif marker (also called the APP1 marker) */
    exif_marker = NULL;
    cmarker = cinfo->marker_list;
    while (cmarker) {
        if (cmarker->marker == EXIF_JPEG_MARKER) {
            /* The Exif APP1 marker should contain a unique
               identification string ("Exif\0\0"). Check for it. */
            if (!memcmp(cmarker->data, EXIF_IDENT_STRING, 6)) {
                exif_marker = cmarker;
            }
        }
        cmarker = cmarker->next;
    }

    /* Did we find the Exif APP1 marker? */
    if (exif_marker == NULL)
        return 0;

    /* Do we have enough data? */
    if (exif_marker->data_length < 32)
        return 0;

    /* Check for TIFF header and catch endianness */
    i = 0;

    /* Just skip data until TIFF header - it should be within 16 bytes from marker start.
       Normal structure relative to APP1 marker -
            0x0000: APP1 marker entry = 2 bytes
               0x0002: APP1 length entry = 2 bytes
            0x0004: Exif Identifier entry = 6 bytes
            0x000A: Start of TIFF header (Byte order entry) - 4 bytes
                        - This is what we look for, to determine endianness.
            0x000E: 0th IFD offset pointer - 4 bytes

            exif_marker->data points to the first data after the APP1 marker
            and length entries, which is the exif identification string.
            The TIFF header should thus normally be found at i=6, below,
            and the pointer to IFD0 will be at 6+4 = 10.
     */

    while (i < 16) {

        /* Little endian TIFF header */
        if (memcmp(&exif_marker->data[i], leth, 4) == 0) {
            endian = G_LITTLE_ENDIAN;
        }

        /* Big endian TIFF header */
        else if (memcmp(&exif_marker->data[i], beth, 4) == 0) {
            endian = G_BIG_ENDIAN;
        }

        /* Keep looking through buffer */
        else {
            i++;
            continue;
        }
        /* We have found either big or little endian TIFF header */
        tiff = i;
        break;
    }

    /* So did we find a TIFF header or did we just hit end of buffer? */
    if (tiff == 0)
        return 0;

    /* Endian the orientation tag ID, to locate it more easily */
    orient_tag_id = ENDIAN16_IT(0x112);

    /* Read out the offset pointer to IFD0 */
    offset = de_get32(&exif_marker->data[i] + 4, endian);
    i = i + offset;

    /* Check that we still are within the buffer and can read the tag count */
    if ((i + 2) > exif_marker->data_length)
        return 0;

    /* Find out how many tags we have in IFD0. As per the TIFF spec, the first
       two bytes of the IFD contain a count of the number of tags. */
    tags = de_get16(&exif_marker->data[i], endian);
    i = i + 2;

    /* Check that we still have enough data for all tags to check. The tags
       are listed in consecutive 12-byte blocks. The tag ID, type, size, and
       a pointer to the actual value, are packed into these 12 byte entries. */
    if ((i + tags * 12) > exif_marker->data_length)
        return 0;

    /* Check through IFD0 for tags of interest */
    while (tags--) {
        type = de_get16(&exif_marker->data[i + 2], endian);
        count = de_get32(&exif_marker->data[i + 4], endian);

        /* Is this the orientation tag? */
        if (memcmp(&exif_marker->data[i], (char *)&orient_tag_id, 2) == 0) {

            /* Check that type and count fields are OK. The orientation field
               will consist of a single (count=1) 2-byte integer (type=3). */
            if (type != 3 || count != 1)
                return 0;

            /* Return the orientation value. Within the 12-byte block, the
               pointer to the actual data is at offset 8. */
            ret = de_get16(&exif_marker->data[i + 8], endian);
            return ret <= 8 ? ret : 0;
        }
        /* move the pointer to the next 12-byte tag field. */
        i = i + 12;
    }

    return 0; /* No EXIF Orientation tag found */
}

//////////////////////////////////////////////////
///// END LGPL licensed code ///////////////////
//////////////////////////////////////////////////

static bool flow_codecs_jpg_decoder_interpret_metadata(flow_c * c, struct flow_codecs_jpeg_decoder_state * state)
{

    // Called twice, avoid repeating work

    JOCTET * icc_buffer;
    unsigned int icc_buffer_len;

    if (state->color.source == flow_codec_color_profile_source_null) {
        if (read_icc_profile(c, state->cinfo, &icc_buffer, &icc_buffer_len)) {
            if (!flow_profile_is_srgb(icc_buffer, icc_buffer_len) && icc_buffer_len > 0) {
                state->color.profile_buf = icc_buffer;
                state->color.buf_length = icc_buffer_len;
                state->color.source = flow_codec_color_profile_source_ICCP;
            }else{
                state->color.source = flow_codec_color_profile_source_sRGB;
            }
        }
    }

    if (state->exif_orientation == 0) {
        state->exif_orientation = get_orientation(state->cinfo);
    }

    // FLOW_error(c, flow_status_Image_decoding_failed);
    return true;
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

int32_t flow_codecs_jpg_decoder_get_exif(flow_c * c, struct flow_codec_instance * codec_instance)
{
    if (codec_instance == NULL || codec_instance->codec_state == NULL
        || codec_instance->codec_id != flow_codec_type_decode_jpeg) {
        return -1;
    }
    struct flow_codecs_jpeg_decoder_state * inner_state
        = (struct flow_codecs_jpeg_decoder_state *)codec_instance->codec_state;
    return inner_state->exif_orientation;
}

static bool flow_codecs_jpg_decoder_reset(flow_c * c, struct flow_codecs_jpeg_decoder_state * state)
{
    if (state->stage == flow_codecs_jpg_decoder_stage_Null) {
        state->pixel_buffer_row_pointers = NULL;
        state->cinfo = NULL;
    } else {

        if (state->cinfo != NULL) {
            // Q: does setjmp need to be called before jpeg_destroy_decompress?
            jpeg_destroy_decompress(state->cinfo);
            FLOW_free(c, state->cinfo);
            state->cinfo = NULL;
        }
        memset(&state->error_mgr, 0, sizeof(struct jpeg_error_mgr));
        if (state->pixel_buffer_row_pointers != NULL) {
            FLOW_free(c, state->pixel_buffer_row_pointers);
            state->pixel_buffer_row_pointers = NULL;
        }
    }
    flow_decoder_color_info_init(&state->color);
    state->row_stride = 0;
    state->exif_orientation = 0;
    state->context = c;
    state->w = 0;
    state->h = 0;
    state->pixel_buffer = NULL;
    state->canvas = NULL;
    state->pixel_buffer_size = -1;
    state->channels = 0;
    state->stage = flow_codecs_jpg_decoder_stage_NotStarted;
    return true;
}

static bool flow_jpeg_cleanup_decoder(flow_c * c, void * state)
{
    if (!flow_codecs_jpg_decoder_reset(c, (struct flow_codecs_jpeg_decoder_state *)state)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    return true;
}

static bool flow_codecs_initialize_decode_jpeg(flow_c * c, struct flow_codec_instance * item)
{
    // flow_codecs_jpeg_decoder_state
    if (item->codec_state == NULL) {

        struct flow_codecs_jpeg_decoder_state * state
            = (struct flow_codecs_jpeg_decoder_state *)FLOW_malloc(c, sizeof(struct flow_codecs_jpeg_decoder_state));
        if (state == NULL) {
            FLOW_error(c, flow_status_Out_of_memory);
            return false;
        }
        flow_set_destructor(c, state, flow_jpeg_cleanup_decoder);

        state->stage = flow_codecs_jpg_decoder_stage_Null;

        state->hints.scale_luma_spatially = false;
        state->hints.gamma_correct_for_srgb_during_spatial_luma_scaling = false;
        state->hints.downscale_if_wider_than = -1;
        state->hints.downscaled_min_width = -1;
        state->hints.downscaled_min_height = -1;
        state->hints.or_if_taller_than = -1;

        if (!flow_codecs_jpg_decoder_reset(c, state)) {
            FLOW_add_to_callstack(c);
            return false;
        }
        state->codec_id = item->codec_id;
        state->io = item->io;
        item->codec_state = state;
    }
    return true;
}
static bool set_downscale_hints(flow_c * c, struct flow_codec_instance * codec,
                                struct flow_decoder_downscale_hints * hints)
{
    struct flow_codecs_jpeg_decoder_state * state = (struct flow_codecs_jpeg_decoder_state *)codec->codec_state;
    memcpy(&state->hints, hints, sizeof(struct flow_decoder_downscale_hints));
    return true;
}

void jpeg_idct_spatial_srgb_1x1(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_spatial_srgb_2x2(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_spatial_srgb_3x3(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_spatial_srgb_4x4(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_spatial_srgb_5x5(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_spatial_srgb_6x6(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_spatial_srgb_7x7(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_spatial_1x1(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                           JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_spatial_2x2(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                           JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_spatial_3x3(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                           JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_spatial_4x4(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                           JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_spatial_5x5(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                           JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_spatial_6x6(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                           JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_spatial_7x7(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                           JSAMPARRAY output_buf, JDIMENSION output_col);

static void flow_jpeg_idct_method_selector(j_decompress_ptr cinfo, jpeg_component_info * compptr,
                                           jpeg_idct_method * set_idct_method, int * set_idct_category)
{
    if (compptr->component_id != 1)
        return;
#if JPEG_LIB_VERSION >= 70
    int scaled = compptr->DCT_h_scaled_size;
#else
    int scaled = compptr->DCT_scaled_size;
#endif

    struct flow_codecs_jpeg_decoder_state * state = (struct flow_codecs_jpeg_decoder_state *)cinfo->err;

    if (scaled > 0 && scaled < 8 && state->hints.scale_luma_spatially) {
        if (state->hints.gamma_correct_for_srgb_during_spatial_luma_scaling) {
            switch (scaled) {
                case 1:
                    *set_idct_method = jpeg_idct_spatial_srgb_1x1;
                    break;
                case 2:
                    *set_idct_method = jpeg_idct_spatial_srgb_2x2;
                    break;
                case 3:
                    *set_idct_method = jpeg_idct_spatial_srgb_3x3;
                    break;
                case 4:
                    *set_idct_method = jpeg_idct_spatial_srgb_4x4;
                    break;
                case 5:
                    *set_idct_method = jpeg_idct_spatial_srgb_5x5;
                    break;
                case 6:
                    *set_idct_method = jpeg_idct_spatial_srgb_6x6;
                    break;
                case 7:
                    *set_idct_method = jpeg_idct_spatial_srgb_7x7;
                    break;
            }
        } else {
            switch (scaled) {
                case 1:
                    *set_idct_method = jpeg_idct_spatial_1x1;
                    break;
                case 2:
                    *set_idct_method = jpeg_idct_spatial_2x2;
                    break;
                case 3:
                    *set_idct_method = jpeg_idct_spatial_3x3;
                    break;
                case 4:
                    *set_idct_method = jpeg_idct_spatial_4x4;
                    break;
                case 5:
                    *set_idct_method = jpeg_idct_spatial_5x5;
                    break;
                case 6:
                    *set_idct_method = jpeg_idct_spatial_6x6;
                    break;
                case 7:
                    *set_idct_method = jpeg_idct_spatial_7x7;
                    break;
            }
        }
        *set_idct_category = JDCT_ISLOW;
        // populate_lookup_tables(state);
    }
}

static bool jpeg_apply_downscaling(flow_c * c, struct flow_codecs_jpeg_decoder_state * state, int32_t * out_w,
                                   int32_t * out_h)
{
    if (state == NULL) {
        FLOW_error(c, flow_status_Null_argument);
        return false;
    }
    if (state->cinfo == NULL) {
        FLOW_error(c, flow_status_Null_argument);
        return false;
    }
    jpeg_set_idct_method_selector(state->cinfo, flow_jpeg_idct_method_selector);
    if (state->hints.downscaled_min_width != -1 && state->hints.downscaled_min_height != 1) {
        if (state->cinfo->image_width > state->hints.downscale_if_wider_than
            || state->cinfo->image_height > state->hints.or_if_taller_than) {

            for (long i = 1; i < 9; i++) {
                if (i == 7)
                    continue; // Because 7/8ths is slower than 8/8
                long new_w = (state->cinfo->image_width * i + 8 - 1L) / 8L;
                long new_h = (state->cinfo->image_height * i + 8 - 1L) / 8L;
                if (new_w >= state->hints.downscaled_min_width && new_h >= state->hints.downscaled_min_height) {
                    state->cinfo->scale_denom = 8;
                    state->cinfo->scale_num = i;
                    *out_w = new_w;
                    *out_h = new_h;
                    return true;
                }
            }
        }
    }
    return true;
}
static bool flow_codecs_jpeg_get_info(flow_c * c, void * codec_state, struct flow_decoder_info * info)
{
    if (codec_state == NULL) {
        FLOW_error(c, flow_status_Null_argument);
        return false;
    }
    struct flow_codecs_jpeg_decoder_state * state = (struct flow_codecs_jpeg_decoder_state *)codec_state;
    if (state->stage < flow_codecs_jpg_decoder_stage_BeginRead) {
        if (!flow_codecs_jpg_decoder_BeginRead(c, state)) {
            FLOW_error_return(c);
        }
    }
    if (state->stage != flow_codecs_jpg_decoder_stage_FinishRead) {
        if (!jpeg_apply_downscaling(c, state, &state->w, &state->h)) {
            FLOW_error_return(c);
        }
    }

    info->current_frame_index = 0;
    info->frame_count = 1;
    info->frame_decodes_into = flow_bgr32;
    info->image_width = state->w;
    info->image_height = state->h;
    return true;
}

static bool flow_codecs_jpeg_get_frame_info(flow_c * c, void * codec_state,
                                            struct flow_decoder_frame_info * decoder_frame_info_ref)
{
    if (codec_state == NULL) {
        FLOW_error(c, flow_status_Null_argument);
        return false;
    }
    struct flow_codecs_jpeg_decoder_state * state = (struct flow_codecs_jpeg_decoder_state *)codec_state;
    if (state->stage < flow_codecs_jpg_decoder_stage_BeginRead) {
        if (!flow_codecs_jpg_decoder_BeginRead(c, state)) {
            FLOW_error_return(c);
        }
    }

    if (state->stage != flow_codecs_jpg_decoder_stage_FinishRead) {
        if (!jpeg_apply_downscaling(c, state, &state->w, &state->h)) {
            FLOW_error_return(c);
        }
    }
    decoder_frame_info_ref->w = state->w;
    decoder_frame_info_ref->h = state->h;
    decoder_frame_info_ref->format = flow_bgr32; // state->channels == 1 ? flow_gray8 : flow_bgr24;
    return true;
}

static bool flow_codecs_jpeg_read_frame(flow_c * c, void * codec_state, struct flow_bitmap_bgra * canvas, struct flow_decoder_color_info * color)
{
    if (codec_state == NULL) {
        FLOW_error(c, flow_status_Null_argument);
        return false;
    }
    struct flow_codecs_jpeg_decoder_state * state = (struct flow_codecs_jpeg_decoder_state *)codec_state;
    if (state->stage == flow_codecs_jpg_decoder_stage_BeginRead) {
        state->pixel_buffer = canvas->pixels;
        state->canvas = canvas;
        state->row_stride = canvas->stride;
        state->pixel_buffer_size = canvas->stride * canvas->h;
        if (!jpeg_apply_downscaling(c, state, &state->w, &state->h)) {
            FLOW_error_return(c);
        }

        if (state->w != (int32_t)canvas->w || state->h != (int32_t)canvas->h) {
            FLOW_error(c, flow_status_Invalid_argument);
            return false;
        }

        if (!flow_codecs_jpg_decoder_FinishRead(c, state)) {
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

const struct flow_codec_definition flow_codec_definition_decode_jpeg
    = { .codec_id = flow_codec_type_decode_jpeg,
        .initialize = flow_codecs_initialize_decode_jpeg,
        .get_info = flow_codecs_jpeg_get_info,
        .get_frame_info = flow_codecs_jpeg_get_frame_info,
        .read_frame = flow_codecs_jpeg_read_frame,
        .set_downscale_hints = set_downscale_hints,
        .name = "decode jpeg",
        .preferred_mime_type = "image/jpeg",
        .preferred_extension = "jpg" };
