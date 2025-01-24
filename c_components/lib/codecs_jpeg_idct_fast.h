#pragma once
#define JPEG_INTERNALS
#include "shared.h"
#include "jpeglib.h"
#include "jdct.h" /* Private declarations for DCT subsystem */



#define PUB FLOW_EXPORT

#ifdef __cplusplus
extern "C" {
#endif



PUB void flow_scale_spatial_srgb_7x7(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_srgb_6x6(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_srgb_5x5(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_srgb_4x4(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_srgb_3x3(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_srgb_2x2(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_srgb_1x1(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_7x7(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_6x6(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_5x5(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_4x4(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_3x3(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_2x2(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

PUB void flow_scale_spatial_1x1(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col);

#undef PUB
#ifdef __cplusplus
}
#endif
