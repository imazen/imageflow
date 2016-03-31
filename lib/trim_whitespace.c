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

    // left half, middle, ->
    if (!check_region(context, 4, 0, 0.5, 0.5, 0.5, &info)) {
        FLOW_add_to_callstack(context);
        return RectFailure;
    }
    // right half, middle, <-
    if (!check_region(context, 2, 0.5, 1, 0.5, 0.5, &info)) {
        FLOW_add_to_callstack(context);
        return RectFailure;
    }

    // left half, bottom third ->
    if (!check_region(context, 4, 0, 0.5, 0.677f, 0.677f, &info)) {
        FLOW_add_to_callstack(context);
        return RectFailure;
    }
    // right half, bottom third -<
    if (!check_region(context, 2, 0.5, 1, 0.677f, 0.677f, &info)) {
        FLOW_add_to_callstack(context);
        return RectFailure;
    }
    // left half, top third ->
    if (!check_region(context, 4, 0, 0.5, 0.333f, 0.333f, &info)) {
        FLOW_add_to_callstack(context);
        return RectFailure;
    }
    // right half, top third -<
    if (!check_region(context, 2, 0.5, 1, 0.333f, 0.333f, &info)) {
        FLOW_add_to_callstack(context);
        return RectFailure;
    }

    // top half, center \/
    if (!check_region(context, 1, 0.5, 0.5, 0, 0.5, &info)) {
        FLOW_add_to_callstack(context);
        return RectFailure;
    }
    // top half, right third
    if (!check_region(context, 1, 0.677f, 0.677f, 0, 0.5, &info)) {
        FLOW_add_to_callstack(context);
        return RectFailure;
    }
    // top half, left third.
    if (!check_region(context, 1, 0.333f, 0.333f, 0, 0.5, &info)) {
        FLOW_add_to_callstack(context);
        return RectFailure;
    }

    // bottom half, center \/
    if (!check_region(context, 3, 0.5, 0.5, 0.5, 1, &info)) {
        FLOW_add_to_callstack(context);
        return RectFailure;
    }
    // bottom half, right third
    if (!check_region(context, 3, 0.677f, 0.677f, 0.5, 1, &info)) {
        FLOW_add_to_callstack(context);
        return RectFailure;
    }
    // bottom half, left third.
    if (!check_region(context, 3, 0.333f, 0.333f, 0.5, 1, &info)) {
        FLOW_add_to_callstack(context);
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
            return RectFailure;
        }
    } else {

        // Finish by scanning everything that is left. Should be a smaller set.
        // Corners will overlap, and be scanned twice, if they are whitespace.
        if (!check_region(context, 1, 0, 1, 0, 1, &info)) {
            FLOW_add_to_callstack(context);
            return RectFailure;
        }
        if (!check_region(context, 4, 0, 1, 0, 1, &info)) {
            FLOW_add_to_callstack(context);
            return RectFailure;
        }
        if (!check_region(context, 2, 0, 1, 0, 1, &info)) {
            FLOW_add_to_callstack(context);
            return RectFailure;
        }
        if (!check_region(context, 3, 0, 1, 0, 1, &info)) {
            FLOW_add_to_callstack(context);
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
    uint8_t const * __restrict bgra = info->bitmap->pixels + (info->bitmap->stride * info->buf_y)
                                      + (bytes_per_pixel * info->buf_x);
    const uint8_t channels = bytes_per_pixel;
    if (channels == 4 && info->bitmap->alpha_meaningful) {
        uint32_t buf_ix = 0;
        for (uint32_t y = 0; y < h; y++) {
            for (uint32_t x = 0; x < w; x++) {
                info->buf[buf_ix] = (114 * bgra[0] + 587 * bgra[1] + 299 * bgra[2]) * bgra[3] / 255000;
                bgra += 4;
                buf_ix++;
            }
            bgra += remnant;
        }
    } else if (channels == 3 || (channels == 4 && !info->bitmap->alpha_meaningful)) {
        uint32_t buf_ix = 0;
        for (uint32_t y = 0; y < h; y++) {
            for (uint32_t x = 0; x < w; x++) {
                info->buf[buf_ix] = (114 * bgra[0] + 587 * bgra[1] + 299 * bgra[2]) / 255000;
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
                  float y_2_percent, struct flow_SearchInfo * __restrict info)
{
    uint32_t x1 = (uint32_t)umax(0, umin(info->w, (uint32_t)floor(x_1_percent * (float)info->w) - 1));
    uint32_t x2 = (uint32_t)umax(0, umin(info->w, (uint32_t)ceil(x_2_percent * (float)info->w) + 1));

    uint32_t y1 = (uint32_t)umax(0, umin(info->h, (uint32_t)floor(y_1_percent * (float)info->h) - 1));
    uint32_t y2 = (uint32_t)umax(0, umin(info->h, (uint32_t)ceil(y_2_percent * (float)info->h) + 1));

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
        y1 = umax(0, y1 - 1);
        y2 = umin(info->h, y2 + 1);
    }
    while (x2 - x1 < min_region_width) {
        x1 = umax(0, x1 - 1);
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

    const uint32_t vertical_windows = (uint32_t)ceil((float)h / (float)window_height);
    const uint32_t horizantal_windows = (uint32_t)ceil((float)w / (float)window_width);

    for (uint32_t window_row = 0; window_row < vertical_windows; window_row++) {
        for (uint32_t window_column = 0; window_column < horizantal_windows; window_column++) {

            info->buf_x = x1 + (window_width * window_column);
            info->buf_y = y1 + (window_height * window_row);

            info->buf_w = umin(umin(3, x2 - info->buf_x), window_width);
            info->buf_h = umin(umin(3, y2 - info->buf_y), window_height);
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
