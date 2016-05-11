/*
 * Copyright (c) Imazen LLC.
 * No part of this project, including this file, may be copied, modified,
 * propagated, or distributed except as permitted in COPYRIGHT.txt.
 * Licensed under the GNU Affero General Public License, Version 3.0.
 * Commercial licenses available at http://imageresizing.net/
 */

#include "trim_whitespace.h"

#ifdef _MSC_VER
#pragma unmanaged
#endif

const struct flow_rect RectFailure = { -1, -1, -1, -1 };

struct scan_region {
    int edgeTRBL;
    float x_1_percent;
    float x_2_percent;
    float y_1_percent;
    float y_2_percent;
};

struct scan_region quick_strips[] = {
    // left half, middle, ->
    { .edgeTRBL = 4, .x_1_percent = 0, .x_2_percent = 0.5, .y_1_percent = 0.5, .y_2_percent = 0.5 },
    // right half, middle, <-
    { .edgeTRBL = 2, .x_1_percent = 0.5, .x_2_percent = 1, .y_1_percent = 0.5, .y_2_percent = 0.5 },

    // left half, bottom third ->
    { .edgeTRBL = 4, .x_1_percent = 0, .x_2_percent = 0.5, .y_1_percent = 0.677f, .y_2_percent = 0.677f },
    // right half, bottom third -<
    { .edgeTRBL = 2, .x_1_percent = 0.5, .x_2_percent = 1, .y_1_percent = 0.677f, .y_2_percent = 0.677f },
    // left half, top third ->
    { .edgeTRBL = 4, .x_1_percent = 0, .x_2_percent = 0.5, .y_1_percent = 0.333f, .y_2_percent = 0.333f },
    // right half, top third -<
    { .edgeTRBL = 2, .x_1_percent = 0.5, .x_2_percent = 1, .y_1_percent = 0.333f, .y_2_percent = 0.333f },

    // top half, center \/
    { .edgeTRBL = 1, .x_1_percent = 0.5, .x_2_percent = 0.5, .y_1_percent = 0, .y_2_percent = 0.5 },
    // top half, right third
    { .edgeTRBL = 1, .x_1_percent = 0.677f, .x_2_percent = 0.677f, .y_1_percent = 0, .y_2_percent = 0.5 },
    // top half, left third.
    { .edgeTRBL = 1, .x_1_percent = 0.333f, .x_2_percent = 0.333f, .y_1_percent = 0, .y_2_percent = 0.5 },

    // bottom half, center \/
    { .edgeTRBL = 3, .x_1_percent = 0.5, .x_2_percent = 0.5, .y_1_percent = 0.5, .y_2_percent = 1 },
    // bottom half, right third
    { .edgeTRBL = 3, .x_1_percent = 0.677f, .x_2_percent = 0.677f, .y_1_percent = 0.5, .y_2_percent = 1 },
    // bottom half, left third.
    { .edgeTRBL = 3, .x_1_percent = 0.333f, .x_2_percent = 0.333f, .y_1_percent = 0.5, .y_2_percent = 1 },
};

struct scan_region everything_inward[] = {
    { .edgeTRBL = 1, .x_1_percent = 0, .x_2_percent = 1, .y_1_percent = 0, .y_2_percent = 1 },
    { .edgeTRBL = 4, .x_1_percent = 0, .x_2_percent = 1, .y_1_percent = 0, .y_2_percent = 1 },
    { .edgeTRBL = 2, .x_1_percent = 0, .x_2_percent = 1, .y_1_percent = 0, .y_2_percent = 1 },
    { .edgeTRBL = 3, .x_1_percent = 0, .x_2_percent = 1, .y_1_percent = 0, .y_2_percent = 1 },
};

bool check_regions(flow_c * context, struct flow_SearchInfo * info, struct scan_region * regions, size_t region_count);

struct flow_rect detect_content(flow_c * context, struct flow_bitmap_bgra * b, uint8_t threshold)
{
    struct flow_SearchInfo info;
    info.w = b->w;
    info.h = b->h;
    info.buff_size = 2048;
    info.buf = (uint8_t *)FLOW_malloc(context, info.buff_size);
    if (info.buf == NULL) {
        FLOW_error(context, flow_status_Out_of_memory);
        return RectFailure;
    }
    info.max_x = 0;
    info.min_x = b->w;
    info.min_y = b->h;
    info.max_y = 0;
    info.bitmap = b;
    info.threshold = threshold;

    // Let's aim for a minimum dimension of 7px per window
    // We want to glean as much as possible from horizontal strips, as they are faster.

    if (!check_regions(context, &info, &quick_strips[0], sizeof(quick_strips) / sizeof(struct scan_region))) {
        FLOW_add_to_callstack(context);
        FLOW_free(context, info.buf);
        return RectFailure;
    }
    // We should now have a good idea of where boundaries lie. However... if it seems that more than 25% is whitespace,
    // we should do a different type of scan.
    long area_to_scan_separately = info.min_x * info.h + info.min_y * info.w + (info.w - info.max_x) * info.h
                                   + (info.h - info.max_y) * info.h;

    if (area_to_scan_separately > (long)(info.h * info.w)) {
        // Just scan it all at once, non-directionally
        if (!check_region(context, 0, 0, 1, 0, 1, &info)) {
            FLOW_add_to_callstack(context);
            FLOW_free(context, info.buf);
            return RectFailure;
        }
    } else {

        // Finish by scanning everything that is left. Should be a smaller set.
        // Corners will overlap, and be scanned twice, if they are whitespace.
        if (!check_regions(context, &info, &quick_strips[0], sizeof(everything_inward) / sizeof(struct scan_region))) {
            FLOW_add_to_callstack(context);
            FLOW_free(context, info.buf);
            return RectFailure;
        }
    }

    struct flow_rect result;
    result.x1 = info.min_x;
    result.y1 = info.min_y;
    result.y2 = info.max_y;
    result.x2 = info.max_x;

    FLOW_free(context, info.buf);
    return result;
}
bool fill_buffer(flow_c * context, struct flow_SearchInfo * __restrict info)
{

    /* Red: 0.299;
    Green: 0.587;
    Blue: 0.114;
    */
    const uint32_t w = info->buf_w;
    const uint32_t h = info->buf_h;
    const uint32_t bytes_per_pixel = flow_pixel_format_bytes_per_pixel(info->bitmap->fmt);
    const uint32_t remnant = info->bitmap->stride - (bytes_per_pixel * w);
    uint8_t const * __restrict bgra = info->bitmap->pixels + (info->bitmap->stride * (size_t)info->buf_y)
                                      + (bytes_per_pixel * (size_t)info->buf_x);

    size_t bitmap_bytes_accessed = (info->bitmap->stride * ((size_t)info->buf_y + h - 1))
                                   + (bytes_per_pixel * (w + (size_t)info->buf_x));
    if (bitmap_bytes_accessed > info->bitmap->stride * info->bitmap->h) {
        FLOW_error(context, flow_status_Invalid_argument);
        return false; // Invalid w,h, buf_x, or buf_y values
    }
    const uint8_t channels = bytes_per_pixel;
    if (channels == 4 && info->bitmap->alpha_meaningful) {
        uint32_t buf_ix = 0;
        for (uint32_t y = 0; y < h; y++) {
            for (uint32_t x = 0; x < w; x++) {
                // We're rounding up. Should we?
                uint16_t gray
                    = (uint16_t)(((233 * bgra[0] + 1197 * bgra[1] + 610 * bgra[2]) * bgra[3] + 524288 - 1) / 524288);
                info->buf[buf_ix] = gray > 255 ? 255 : gray;
                bgra += 4;
                buf_ix++;
            }
            bgra += remnant;
        }
    } else if (channels == 3 || (channels == 4 && !info->bitmap->alpha_meaningful)) {
        uint32_t buf_ix = 0;
        for (uint32_t y = 0; y < h; y++) {
            for (uint32_t x = 0; x < w; x++) {
                info->buf[buf_ix] = (233 * bgra[0] + 1197 * bgra[1] + 610 * bgra[2]) / 2048;
                bgra += channels;
                buf_ix++;
            }
            bgra += remnant;
        }
    } else {
        uint32_t buf_ix = 0;
        for (uint32_t y = 0; y < h; y++) {
            for (uint32_t x = 0; x < w; x++) {
                uint32_t sum = 0;
                for (uint8_t ch = 0; ch < channels; ch++)
                    sum += bgra[ch];
                info->buf[buf_ix] = sum / channels;
                bgra += channels;
                buf_ix++;
            }
            bgra += remnant;
        }
    }
    return true;
}

bool sobel_scharr_detect(flow_c * context, struct flow_SearchInfo * info)
{
#define COEFFA = 3
#define COEFFB = 10;
    const uint32_t w = info->buf_w;
    const uint32_t h = info->buf_h;
    const uint32_t y_end = h - 1;
    const uint32_t x_end = w - 1;
    const uint32_t threshold = info->threshold;

    uint8_t * __restrict buf = info->buf;
    uint32_t buf_ix = w + 1;
    for (uint32_t y = 1; y < y_end; y++) {
        for (uint32_t x = 1; x < x_end; x++) {

            const int gx = -3 * buf[buf_ix - w - 1] + -10 * buf[buf_ix - 1] + -3 * buf[buf_ix + w - 1]
                           + +3 * buf[buf_ix - w + 1] + 10 * buf[buf_ix + 1] + 3 * buf[buf_ix + w + 1];
            const int gy = 3 * buf[buf_ix - w - 1] + 10 * (buf[buf_ix - w]) + 3 * buf[buf_ix - w + 1]
                           + -3 * buf[buf_ix + w - 1] + -10 * (buf[buf_ix + w]) + -3 * buf[buf_ix + w + 1];
            const size_t value = abs(gx) + abs(gy);
            if (value > threshold) {
                const uint32_t x1 = info->buf_x + x - 1;
                const uint32_t x2 = info->buf_x + x + 1;
                const uint32_t y1 = info->buf_y + y - 1;
                const uint32_t y2 = info->buf_y + y + 1;

                if (x1 < info->min_x) {
                    info->min_x = x1;
                }
                if (x2 > info->max_x) {
                    info->max_x = x2;
                }
                if (y1 < info->min_y) {
                    info->min_y = y1;
                }
                if (y2 > info->max_y) {
                    info->max_y = y2;
                }
            }
            buf_ix++;
        }
        buf_ix += 2;
    }
    return true;
}

bool check_region(flow_c * context, int edgeTRBL, float x_1_percent, float x_2_percent, float y_1_percent,
                  float y_2_percent, struct flow_SearchInfo * info)
{
    uint32_t x1 = (uint32_t)int_max(0, int_min(info->w, (int32_t)floor(x_1_percent * (float)info->w) - 1));
    uint32_t x2 = (uint32_t)int_max(0, int_min(info->w, (int32_t)ceil(x_2_percent * (float)info->w) + 1));

    uint32_t y1 = (uint32_t)int_max(0, int_min(info->h, (int32_t)floor(y_1_percent * (float)info->h) - 1));
    uint32_t y2 = (uint32_t)int_max(0, int_min(info->h, (int32_t)ceil(y_2_percent * (float)info->h) + 1));

    // Snap the boundary depending on which side we're searching
    if (edgeTRBL == 4) {
        x1 = 0;
        x2 = umin(x2, info->min_x);
    }
    if (edgeTRBL == 2) {
        x1 = umax(x1, info->max_x);
        x2 = info->w;
    }
    if (edgeTRBL == 1) {
        y1 = 0;
        y2 = umin(y2, info->min_y);
    }
    if (edgeTRBL == 3) {
        y1 = umax(y1, info->max_y);
        y2 = info->h;
    }
    if (x1 == x2 || y1 == y2)
        return true; // Nothing left to search.

    // Let's make sure that we're searching at least 7 pixels in the perpendicular direction
    uint32_t min_region_width = (edgeTRBL == 2 || edgeTRBL == 4) ? 3 : 7;
    uint32_t min_region_height = (edgeTRBL == 1 || edgeTRBL == 3) ? 3 : 7;

    while (y2 - y1 < min_region_height) {
        y1 = y1 > 0 ? y1 - 1 : 0;
        y2 = umin(info->h, y2 + 1);
    }
    while (x2 - x1 < min_region_width) {
        x1 = x1 > 0 ? x1 - 1 : 0;
        x2 = umin(info->w, x2 + 1);
    }

    // Now we need to split this section into regions that fit in the buffer. Might as well do it vertically, so our
    // scans are minimal.

    const uint32_t w = x2 - x1;
    const uint32_t h = y2 - y1;

    // If we are doing a full scan, make them wide along the X axis. Otherwise, make them square.
    const uint32_t window_width
        = umin(w, (edgeTRBL == 0 ? info->buff_size / 7 : (uint32_t)ceil(sqrt((float)info->buff_size))));
    const uint32_t window_height = umin(h, info->buff_size / window_width);

    const uint32_t vertical_windows = (uint32_t)ceil((float)h / (float)(window_height - 2));
    const uint32_t horizontal_windows = (uint32_t)ceil((float)w / (float)(window_width - 2));

    for (uint32_t window_row = 0; window_row < vertical_windows; window_row++) {
        for (uint32_t window_column = 0; window_column < horizontal_windows; window_column++) {

            info->buf_x = x1 + ((window_width - 2) * window_column);
            info->buf_y = y1 + ((window_height - 2) * window_row);

            info->buf_w = umin(umax(3, x2 - info->buf_x), window_width);
            info->buf_h = umin(umax(3, y2 - info->buf_y), window_height);
            uint32_t buf_x2 = info->buf_x + info->buf_w;
            uint32_t buf_y2 = info->buf_y + info->buf_h;

            const bool excluded_x = (info->min_x <= info->buf_x && info->max_x >= buf_x2);

            const bool excluded_y = (info->min_y <= info->buf_y && info->max_y >= buf_y2);

            if (excluded_x && excluded_y) {
                // Entire window has already been excluded
                continue;
            }
            if (excluded_y && info->min_x < buf_x2 && buf_x2 < info->max_x) {
                info->buf_w = umax(3, info->min_x - info->buf_x);
            } else if (excluded_y && info->max_x > info->buf_x && info->buf_x > info->min_x) {
                info->buf_x = umin(buf_x2 - 3, info->max_x);
                info->buf_w = buf_x2 - info->buf_x;
            }
            if (excluded_x && info->min_y < buf_y2 && buf_y2 < info->max_y) {
                info->buf_h = umax(3, info->min_y - info->buf_y);
            } else if (excluded_x && info->max_y > info->buf_y && info->buf_y > info->min_y) {
                info->buf_y = umin(buf_y2 - 3, info->max_y);
                info->buf_h = buf_y2 - info->buf_y;
            }

            if (info->buf_y + info->buf_h > info->h || info->buf_x + info->buf_w > info->w) {
                // We're out of bounds on the image somehow.
                continue;
            }

            if (!fill_buffer(context, info)) {
                FLOW_add_to_callstack(context);
                return false;
            }
            if (!sobel_scharr_detect(context, info)) {
                FLOW_add_to_callstack(context);
                return false;
            }
        }
    }
    return true;
}

bool check_regions(flow_c * context, struct flow_SearchInfo * info, struct scan_region * regions, size_t region_count)
{
    for (size_t i = 0; i < region_count; i++) {
        if (!check_region(context, regions[i].edgeTRBL, regions[i].x_1_percent, regions[i].x_2_percent,
                          regions[i].y_1_percent, regions[i].y_2_percent, info)) {
            FLOW_add_to_callstack(context);
            return false;
        }
    }
    return true;
}
