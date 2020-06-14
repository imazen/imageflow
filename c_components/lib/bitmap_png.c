#include "imageflow_private.h"

uint8_t ** flow_bitmap_create_row_pointers(flow_c * c, void * buffer, size_t buffer_size, size_t stride, size_t height)
{
    if (buffer_size < stride * height) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return NULL;
    }
    uint8_t ** rows = (uint8_t **)FLOW_malloc(c, sizeof(uint8_t *) * height);
    if (rows == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        return NULL;
    }
    unsigned int y;
    for (y = 0; y < height; ++y) {
        rows[y] = ((uint8_t *)buffer + (stride * y));
    }
    // printf("Creating row pointers for %p to %p. Last ends at %p\n", buffer, (void *)((uint8_t *)buffer +
    // buffer_size), (void *)(rows[height -1] + stride));
    return rows;
}

bool flow_bitmap_bgra_save_png(flow_c * c, struct flow_bitmap_bgra * b, const char * path)
{

    png_image target_image;
    memset(&target_image, 0, sizeof target_image);
    target_image.version = PNG_IMAGE_VERSION;
    target_image.opaque = NULL;
    target_image.width = b->w;
    target_image.height = b->h;
    target_image.format = PNG_FORMAT_BGRA;
    target_image.flags = 0;
    target_image.colormap_entries = 0;

    if (b->w < 1 || b->h < 1 || b->w > 20000 || b->h > 20000) {
        FLOW_error_msg(c, flow_status_Image_encoding_failed, "Cannot encode image of dimensions %dx%d", b->w, b->h);
        return false;
    }

    if (!png_image_write_to_file(&target_image, path, 0 /*convert_to_8bit*/, b->pixels, b->stride /*row_stride*/,
                                 NULL /*colormap*/)) {
        FLOW_error_msg(c, flow_status_Image_encoding_failed, "Failed to export frame as png: %s  output path: %s.",
                       target_image.message, path);
        return false;
    }
    return true;
}
