#include <glenn/png/png.h>
#include "catch.hpp"

#include "fastscaling_private.h"
#include "weighting_test_helpers.h"
#include "trim_whitespace.h"
#include "string.h"
#include "lcms2.h"
#include "png.h"
//TODO: https://github.com/pornel/pngquant/

TEST_CASE ("Load png", "[fastscaling]")
{

    bool success = false;

    uint8_t image_bytes_literal[] = {0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82};
    png_size_t image_bytes_count = sizeof(image_bytes_literal);
    png_const_voidp image_bytes = &image_bytes_literal;

    png_image image;

    /* Only the image structure version number needs to be set. */
    memset(&image, 0, sizeof image);
    image.version = PNG_IMAGE_VERSION;
    image.opaque = NULL;

    if (png_image_begin_read_from_memory(&image,image_bytes, image_bytes_count))
    {
        png_bytep buffer;

        /* Change this to try different formats!  If you set a colormap format
         * then you must also supply a colormap below.
         */
        image.format = PNG_FORMAT_BGRA;

        buffer =  (png_bytep)malloc (PNG_IMAGE_SIZE(image));

        if (buffer != NULL)
        {
            if (png_image_finish_read(&image, NULL/*background*/, buffer,
                                      0/*row_stride*/, NULL/*colormap for PNG_FORMAT_FLAG_COLORMAP */))
            {

                success=true;

//                if (png_image_write_to_file(&image, argv[2],
//                                            0/*convert_to_8bit*/, buffer, 0/*row_stride*/,
//                                            NULL/*colormap*/))
//                    result = 0;
//
//                else
//                    fprintf(stderr, "pngtopng: write %s: %s\n",
//                            image.message);
//
//                free(buffer);
            }

            else
            {
                fprintf(stderr, "png_image_finish_read: %s\n",
                        image.message);

                /* This is the only place where a 'free' is required; libpng does
                 * the cleanup on error and success, but in this case we couldn't
                 * complete the read because of running out of memory.
                 */
                png_image_free(&image);
            }
        }

        else
            fprintf(stderr, "pngtopng: out of memory: %lu bytes\n",
                    (unsigned long)PNG_IMAGE_SIZE(image));
    }

    else
        /* Failed to read the first argument: */
        fprintf(stderr, "png_image_begin_read_from_memory: %s\n", image.message);

    REQUIRE (success);
}
