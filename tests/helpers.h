#pragma once

#include "unistd.h"
#include <sys/types.h>
#include <sys/stat.h>
#include <errno.h>
#include <stdio.h>
#include "string.h"
#include <stdbool.h>
#include "curl/curl.h"
#include "curl/easy.h"
#include <stdlib.h>
#include <imageflow.h>
#include <../lib/job.h>

uint8_t *get_bytes_cached(Context *c, size_t *bytes_count_out, const char *url);
void fetch_image(const char* url, char* dest_path);
uint8_t *read_all_bytes(Context *c, size_t *buffer_size, const char *path);
bool write_all_byte(const char *path, char * buffer, size_t size);
void copy_file(FILE* from, FILE* to);

unsigned long djb2(unsigned const char *str);
size_t nonzero_count(uint8_t * array, size_t length);


BitmapBgra * BitmapBgra_create_test_image(Context * c);

size_t nonzero_count(uint8_t * array, size_t length){

    size_t nonzero = 0;
    for (size_t  i =0; i < length; i++){
        if (array[i] != 0){
            nonzero++;
        }
    }
    return nonzero;
}

unsigned long djb2(unsigned const char *str)
{
    unsigned long hash = 5381;
    int c;

    while ((c = *str++))
        hash = ((hash << 5) + hash) + c; /* hash * 33 + c */

    return hash;
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



bool write_all_byte(const char *path, char * buffer, size_t size){
    FILE *fh = fopen(path, "w");
    if ( fh != NULL ) {
        if (fwrite(buffer, size, 1,fh) != 1){
            exit(999);
        }
    }
    fclose(fh);
    return true;
}

uint8_t *read_all_bytes(Context *c, size_t *buffer_size, const char *path) {
    uint8_t *buffer;
    FILE *fh = fopen(path, "rb");
    if ( fh != NULL )
    {
        fseek(fh, 0L, SEEK_END);
        size_t s = ftell(fh);
        rewind(fh);
        buffer = (uint8_t *) CONTEXT_malloc(c, s);
        if ( buffer != NULL )
        {
            //Returns 1 or 0, not the number of bytes.
            //Technically we're reading 1 element of size s
            size_t read_count = fread(buffer, s, 1, fh);
            // we can now close the file
            fclose(fh); fh = NULL;
            *buffer_size = s;
            if (s < 1){
                //Failed to fill buffer
                fprintf(stderr, "Buffer size: %lu    Result code: %lu", s, read_count);
                exit(8);
            }
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
    static bool curl_initialized = false;
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

uint8_t *get_bytes_cached(Context *c, size_t *bytes_count_out, const char *url) {

    #define FLOW_MAX_PATH 255
    char cache_folder[FLOW_MAX_PATH];

    snprintf(cache_folder,FLOW_MAX_PATH, "%s/imageflow_cache", getenv("HOME"));

    flow_utils_ensure_directory_exists(cache_folder);
    char cache_path[FLOW_MAX_PATH];

    snprintf(cache_path, FLOW_MAX_PATH, "%s/%lu", cache_folder, djb2((unsigned const char *)url));

    if( access( cache_path, F_OK ) == -1 ) {
        // file doesn't exist
        fetch_image(url,cache_path);
    }else{

        fprintf(stdout, "Using cached image at %s",cache_path);
    }

    return read_all_bytes(c, bytes_count_out, cache_path);
}


void flow_utils_ensure_directory_exists(const char *dir_path){
    struct stat sb;
    int e;
    e = stat(dir_path, &sb);
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

BitmapBgra * BitmapBgra_create_test_image(Context  * c){
    BitmapBgra * test = BitmapBgra_create(c, 256, 256, false, Bgra32);
    if (test == NULL){
        CONTEXT_add_to_callstack(c);
        return NULL;
    }
    uint8_t * pixel;
    for (uint32_t y = 0; y < test->h; y++){
        pixel = test->pixels + (y * test->stride);
        for (uint32_t x = 0; x < test->w; x++){
            pixel[0] = (uint8_t)x;
            pixel[1] = (uint8_t) (x /2);
            pixel[2] = (uint8_t)(x /3);
            pixel[3] = (uint8_t)y;
            pixel+=4;
        }
    }
    return test;
}
