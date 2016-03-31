#pragma once
#include "helpers.h"

#ifdef __cplusplus
extern "C" {
#endif

bool visual_compare(flow_c * c, struct flow_bitmap_bgra * bitmap, const char * name, bool store_checksums,
                    const char * file_, const char * func_, int line_number);

#ifdef __cplusplus
}
#endif
