#pragma once
#include "imageflow_private.h"
#include "jpeglib.h"
#include "jerror.h"
#include "png.h"



struct flow_jpeg_wrapper_error_state {
    struct jpeg_error_mgr error_mgr;
    jmp_buf error_handler_jmp;
    bool (*error_handler) (void * custom_state, j_common_ptr cinfo, struct jpeg_error_mgr * error_mgr, int error_code, char * error_message_buffer, int error_message_buffer_length);
    void * custom_state;
//    flow_c * context;
//    size_t codec_id;
//    flow_codecs_jpeg_decoder_stage stage;
//    struct jpeg_decompress_struct * cinfo;
//    size_t row_stride;
//    int32_t w;
//    int32_t h;
//    int32_t exif_orientation;
//    int channels;
//    struct flow_io * io;
//    struct flow_bitmap_bgra * canvas;
//    uint8_t * pixel_buffer;
//    size_t pixel_buffer_size;
//    uint8_t ** pixel_buffer_row_pointers;
//
//    struct flow_decoder_color_info color;
//
//    struct flow_decoder_downscale_hints hints;
//    float lut_to_linear[256];
//    uint8_t flat_lut_linear[256 * 13];

};