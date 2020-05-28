#pragma once
#include "imageflow_private.h"


//typedef enum flow_codec_color_profile_source {
//    flow_codec_color_profile_source_null = 0,
//    flow_codec_color_profile_source_ICCP = 1,
//    flow_codec_color_profile_source_ICCP_GRAY = 2,
//    flow_codec_color_profile_source_GAMA_CHRM = 3,
//    flow_codec_color_profile_source_sRGB = 4,
//
//} flow_codec_color_profile_source;
//
//
//struct flow_decoder_color_info{
//    flow_codec_color_profile_source source;
//    uint8_t * profile_buf;
//    size_t buf_length;
//    cmsCIExyY white_point;
//    cmsCIExyYTRIPLE primaries;
//    double gamma;
//};


static void flow_decoder_color_info_init(struct flow_decoder_color_info * color){
    memset(color, 0, sizeof(struct flow_decoder_color_info));
    color->gamma = 0.45455;
}
