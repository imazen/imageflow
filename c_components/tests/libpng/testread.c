#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include "zlib.h"
#include "png.h"


int main()
{
    printf("\n Testing libpng version %s\n", PNG_LIBPNG_VER_STRING);
    printf("   with zlib   version %s\n", ZLIB_VERSION);
    png_structp png_ptr = png_create_write_struct(PNG_LIBPNG_VER_STRING, NULL, NULL,
                                                  NULL); // makepng_error, makepng_warning);

    uint32_t w = 300;
    uint32_t h = 200;
    uint32_t stride = 1200;

    uint8_t * pixels = (uint8_t *)calloc(1, stride * h);
    uint8_t ** rows = (uint8_t **)calloc(1, sizeof(uint8_t * ) * h);

    unsigned int y;
    for (y = 0; y < h; ++y) {
        rows[y] = ((uint8_t *)pixels + (stride * y));
    }

    png_set_compression_level(png_ptr, Z_BEST_SPEED);
    png_set_text_compression_level(png_ptr, Z_DEFAULT_COMPRESSION);

    png_init_io(png_ptr, fopen("/dev/null", "wb"));
    png_infop info_ptr = NULL;
    info_ptr = png_create_info_struct(png_ptr);
    png_set_rows(png_ptr, info_ptr, rows);
    int color_type = PNG_COLOR_TYPE_RGB_ALPHA;
    int transform = PNG_TRANSFORM_BGR;
    png_set_IHDR(png_ptr, info_ptr, (png_uint_32)w, (png_uint_32)h, 8, color_type,
                 PNG_INTERLACE_NONE, PNG_COMPRESSION_TYPE_BASE, PNG_FILTER_TYPE_BASE);
    png_set_sRGB_gAMA_and_cHRM(png_ptr, info_ptr, PNG_sRGB_INTENT_PERCEPTUAL);
//Uninitialized read happens here:
    png_write_png(png_ptr, info_ptr, transform, NULL);

    png_destroy_write_struct(&png_ptr, &info_ptr);
    free(pixels);
    free(rows);
    return 0;
}
