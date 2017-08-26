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

static bool flow_bitmap_bgra_load_png_fileptr(flow_c * c, struct flow_bitmap_bgra ** b_ref, FILE * file)
{

    png_structp png_ptr;
    png_infop info_ptr;
    uint32_t w;
    uint32_t h;
    int color_type, bit_depth;
    png_bytepp pixel_buffer_row_pointers;
    flow_pixel_format canvas_fmt = flow_bgra32;

    png_ptr = png_create_read_struct(PNG_LIBPNG_VER_STRING, NULL, NULL, NULL);
    if (png_ptr == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        return false;
    }

    info_ptr = png_create_info_struct(png_ptr);
    if (info_ptr == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        // TODO free png_ptr
        return false;
    }
    png_init_io(png_ptr, file);

    // Read header and chunks
    png_read_info(png_ptr, info_ptr);

    // Get dimensions and info
    png_get_IHDR(png_ptr, info_ptr, &w, &h, &bit_depth, &color_type, NULL, NULL, NULL);

    // Now we need to figure out how big our pixel buffer needs to be to hold the entire image.
    // We need to apply some normalization filters so we have fewer variants.

    /* expand palette images to RGB, low-bit-depth grayscale images to 8 bits,
    * transparency chunks to full alpha channel; strip 16-bit-per-sample
    * images to 8 bits per sample; and convert grayscale to RGB[A] */

    // Fill in the alpha channel with FFFF if missing.
    if (!(color_type & PNG_COLOR_MASK_ALPHA)) {
        canvas_fmt = flow_bgr32;
        png_set_expand(png_ptr);
        png_set_filler(png_ptr, 65535L, PNG_FILLER_AFTER);
    }

    // Drop to 8-bit per channel; we can't handle 16-bit yet.
    if (bit_depth == 16) {
        png_set_strip_16(png_ptr);
    }
    // Convert grayscale to RGB.
    if (!(color_type & PNG_COLOR_MASK_COLOR))
        png_set_gray_to_rgb(png_ptr);

    // We use BGRA, not RGBA
    png_set_bgr(png_ptr);
    // We don't want to think about interlacing. Let libpng fix that up.

    // Update our info based on these new settings.
    png_read_update_info(png_ptr, info_ptr);

    // Now we can access a stride that represents the post-transform data.
    // NOT USED: rowbytes = png_get_rowbytes(png_ptr, info_ptr);

    if (png_get_channels(png_ptr, info_ptr) != 4) {
        // TODO: free png_ptr, png_info
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false; // Should always be 4
    }

    struct flow_bitmap_bgra * canvas = flow_bitmap_bgra_create(c, (int)w, (int)h, true, canvas_fmt);
    // We set this, but it's ignored and overwritten by existing callers

    pixel_buffer_row_pointers
        = flow_bitmap_create_row_pointers(c, canvas->pixels, canvas->h * canvas->stride, canvas->stride, h);
    if (pixel_buffer_row_pointers == NULL) {
        // TODO: free memory
        FLOW_error_return(c);
    }
    // The real work
    png_read_image(png_ptr, pixel_buffer_row_pointers);

    png_read_end(png_ptr, NULL);

    // Not sure if we should just call reset instead, or not...
    png_destroy_read_struct(&png_ptr, &info_ptr, NULL);

    *b_ref = canvas;

    return true;
}
bool flow_bitmap_bgra_load_png(flow_c * c, struct flow_bitmap_bgra ** b_ref, const char * path)
{
    FILE * f = fopen(path, "rb");
    if (f == NULL) {
        FLOW_error(c, flow_status_IO_error);
        return false;
    }
    if (!flow_bitmap_bgra_load_png_fileptr(c, b_ref, f)) {
        fclose(f);
        FLOW_error_return(c);
    }
    fclose(f);
    return true;
}
