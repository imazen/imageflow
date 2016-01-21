/*
 * Copyright (c) Imazen LLC.
 * No part of this project, including this file, may be copied, modified,
 * propagated, or distributed except as permitted in COPYRIGHT.txt.
 * Licensed under the GNU Affero General Public License, Version 3.0.
 * Commercial licenses available at http://imageresizing.net/
 */
#pragma once

#ifdef _MSC_VER
#pragma unmanaged
#endif

#include "fastscaling_private.h"


typedef struct RectStruct {
    int32_t x1, y1, x2, y2;
} Rect;


typedef struct SearchInfoStruct {
    BitmapBgra * bitmap;
    uint32_t min_x, max_x, min_y, max_y;
    uint32_t w, h;
    uint8_t* buf;
    uint32_t buff_size;
    uint32_t buf_x, buf_y, buf_w, buf_h;
    uint32_t threshold;

} SearchInfo;


#ifdef __cplusplus
extern "C" {
#endif


Rect detect_content(Context * context, BitmapBgra * b, uint8_t threshold);
bool fill_buffer(Context * context, SearchInfo * __restrict info);
bool sobel_scharr_detect(Context * context, SearchInfo* __restrict info );
bool check_region(Context * context,int edgeTRBL, float x_1_percent, float x_2_percent, float y_1_percent, float y_2_percent, SearchInfo* __restrict info);


#ifdef __cplusplus
}
#endif



