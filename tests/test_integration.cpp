#include <glenn/png/png.h>
#include "catch.hpp"
#include "unistd.h"
#include <sys/types.h>
#include <sys/stat.h>
#include <errno.h>
#include <stdio.h>

#include "fastscaling_private.h"
#include "weighting_test_helpers.h"
#include "trim_whitespace.h"
#include "string.h"
#include "lcms2.h"
#include "png.h"
#include "curl/curl.h"
#include "curl/easy.h"
//TODO: https://github.com/pornel/pngquant/

bool curl_initialized = false;


extern int errno;

size_t nonzero_count(png_bytep array, size_t length){

    size_t nonzero = 0;
    for (size_t  i =0; i < length; i++){
        if (array[i] != 0){
            nonzero++;
        }
    }
    return nonzero;
}

unsigned long djb2(unsigned char *str)
{
    unsigned long hash = 5381;
    int c;

    while ((c = *str++))
        hash = ((hash << 5) + hash) + c; /* hash * 33 + c */

    return hash;
}

void ensure_directory(const char * dir_path){
    struct stat sb;
    int e;
    e = stat(dir_path, &sb);
    printf("e=%d errno=%d\n",e,errno);
    if (e == 0)
    {
        if (sb.st_mode & S_IFDIR)
            printf("%s is a directory.\n",dir_path);
        if (sb.st_mode & S_IFREG)
            printf("%s is a regular file.\n",dir_path);
// etc.
    }
    else
    {
        printf("stat failed.\n");
        if ((errno = ENOENT))
        {
            printf("The directory does not exist. Creating new directory %s...\n", dir_path);
// Add more flags to the mode if necessary.
            e = mkdir(dir_path, S_IRWXU);
            if (e != 0)
            {
                printf("mkdir failed; errno=%d\n",errno);
            }
            else
            {
                printf("created the directory %s\n",dir_path);
            }
        }
    }

}

void copy_file(FILE* from, FILE* to){
    size_t n, m;
    unsigned char buff[8192];
    do {
        n = fread(buff, 1, sizeof buff, from);
        if (n) m = fwrite(buff, 1, n, to);
        else   m = 0;
    } while ((n > 0) && (n == m));
    if (m) perror("copy");
}


uint8_t * read_all_bytes(const char * path, size_t * buffer_size){
    uint8_t *buffer;
    FILE *fh = fopen(path, "rb");
    if ( fh != NULL )
    {
        fseek(fh, 0L, SEEK_END);
        size_t s = ftell(fh);
        rewind(fh);
        buffer = (uint8_t *) malloc(s);
        if ( buffer != NULL )
        {
            fread(buffer, s, 1, fh);
            // we can now close the file
            fclose(fh); fh = NULL;
            *buffer_size = s;
            return buffer;

        }else{
            fprintf(stderr, "Failed to allocate buffer of size: %lu", s);
            exit(8);
        }
        if (fh != NULL) fclose(fh);
    }else {
        fprintf(stderr, "Failed to open for reading: %s", path);
        exit(8);
    }
    return 0;
}
void fetch_image(const char* url, char* dest_path){ /*null-terminated string*/
    if (!curl_initialized){
        curl_initialized = true;
        curl_global_init(CURL_GLOBAL_ALL);
    }

    CURL *curl;
    FILE *fp;
    FILE *real_fp;
    CURLcode res;
    curl = curl_easy_init();
    if (curl) {
        fp = tmpfile();
        if (fp) {
            curl_easy_setopt(curl, CURLOPT_URL, url);
            curl_easy_setopt(curl, CURLOPT_WRITEDATA, fp);
            res = curl_easy_perform(curl);
            if (res != CURLE_OK){
                fprintf(stderr, "CURL HTTP operation failed (error %d) - GET %s, write to  %s",res, url, dest_path);
                exit(4);
            }
        }else{
            fprintf(stderr, "Failed to open temp file");
            exit(3);
        }
        /* always cleanup */
        curl_easy_cleanup(curl);
        rewind(fp);
        real_fp= fopen(dest_path,"wb");
        if (real_fp){
            copy_file(fp, real_fp);
        }else{
            fprintf(stderr, "Failed to open file for writing %s", dest_path);
            exit(3);
        }
        fclose(real_fp);
        fclose(fp);
    }else{
        fprintf(stderr, "Failed to start CURL");
        exit(2);
    }
}

uint8_t* get_bytes_cached(const char * url, size_t * bytes_count_out){

    const size_t MAX_PATH = 255;
    char cache_folder[MAX_PATH];

    snprintf(cache_folder,MAX_PATH, "%s/imageflow_cache", getenv("HOME"));

    ensure_directory(cache_folder);
    char cache_path[MAX_PATH];

    snprintf(cache_path, MAX_PATH, "%s/%lu", cache_folder, djb2((unsigned char *)url));

    if( access( cache_path, F_OK ) == -1 ) {
        // file doesn't exist
        fetch_image(url,cache_path);
    }

    return read_all_bytes(cache_path, bytes_count_out);
}

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



TEST_CASE ("Load png from URL", "[fastscaling]")
{

    bool success = false;

    size_t bytes_count = 0;

    uint8_t * bytes = get_bytes_cached("http://s3.amazonaws.com/resizer-images/sun_256.png", &bytes_count);
    png_size_t image_bytes_count = bytes_count;
    png_const_voidp image_bytes =bytes;

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

        buffer =  (png_bytep)calloc ( PNG_IMAGE_SIZE(image), sizeof(png_bytep));

        if (buffer != NULL)
        {
            if (png_image_finish_read(&image, NULL/*background*/, buffer,
                                      0/*row_stride*/, NULL/*colormap for PNG_FORMAT_FLAG_COLORMAP */))
            {

                int nonzero = nonzero_count(buffer, PNG_IMAGE_SIZE(image));
                if (nonzero > 0){
                    printf("nonzero buffer: %d of %d", nonzero,  PNG_IMAGE_SIZE(image));
                }
                Context context;
                Context_initialize(&context);


                BitmapBgra * source = BitmapBgra_create_header(&context, (unsigned int )(image.width), (unsigned int)(image.height));
                if (source == NULL) {
                    exit(99);
                }
                source->fmt = BitmapPixelFormat::Bgra32;
                source->stride = PNG_IMAGE_ROW_STRIDE(image);
                printf("png stride (%d), calculated (%d)\n",source->stride,  source->w * BitmapPixelFormat_bytes_per_pixel(source->fmt));
                source->alpha_meaningful = true;
                source->pixels = buffer;

                int target_width = 300;
                int target_height = 200;

                BitmapBgra * canvas = BitmapBgra_create(&context, target_width, target_height, true, Bgra32);

                RenderDetails * details = RenderDetails_create_with(&context, InterpolationFilter::Filter_Robidoux);
                details->interpolate_last_percent = 2.1f;
                details->minimum_sample_window_to_interposharpen = 1.5;
                details->havling_acceptable_pixel_loss = 0.26f;

                if (details == NULL) exit(99);
//                details->sharpen_percent_goal = 50;
//                details->post_flip_x = flipx;
//                details->post_flip_y = flipy;
//                details->post_transpose = transpose;
                //details->enable_profiling = profile;

                //Should we even have Renderer_* functions, or just 1 call that does it all?
                //If we add memory use estimation, we should keep Renderer



                if (!RenderDetails_render(&context,details, source, canvas)){

                    char error[255];
                    Context_error_message(&context, error, 255);
                    printf("%s",error);
                    exit(77);
                }
                printf("Rendered!");
                RenderDetails_destroy(&context, details);

                BitmapBgra_destroy(&context, source);
                free(buffer);

                    //TODO, write out PNG here

                png_image target_image;

                /* Only the image structure version number needs to be set. */
                memset(&target_image, 0, sizeof target_image);
                target_image.version = PNG_IMAGE_VERSION;
                target_image.opaque = NULL;
                target_image.width = target_width;
                target_image.height = target_height;
                target_image.format = Bgra32;
                target_image.flags = 0;
                target_image.colormap_entries = 0;

                if (png_image_write_to_file(&target_image, "unipng.png",
                                            0/*convert_to_8bit*/, canvas->pixels, 0/*row_stride*/,
                                            NULL/*colormap*/)) {
                    success = true;
                    int nonzero2 = nonzero_count(canvas->pixels, canvas->stride * canvas->h);
                    printf("nonzero output buffer: %d of %d", nonzero2, canvas->stride * canvas->h);

                }
                else
                    fprintf(stderr, "pngtopng: write failed : %s\n",
                            image.message);


                BitmapBgra_destroy(&context, canvas);
                Context_terminate(&context);


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
