
#include "helpers.h"

// Moving implementations here inexplicably causes linker errors, even when specifically including this compilation
// unit.

size_t nonzero_count(uint8_t * array, size_t length)
{

    size_t nonzero = 0;
    for (size_t i = 0; i < length; i++) {
        if (array[i] != 0) {
            nonzero++;
        }
    }
    return nonzero;
}

unsigned long djb2(unsigned const char * str)
{
    unsigned long hash = 5381;
    int c;

    while ((c = *str++))
        hash = ((hash << 5) + hash) + c; /* hash * 33 + c */

    return hash;
}

unsigned long djb2_buffer(uint8_t * str, size_t count)
{
    unsigned long hash = 5381;
    int c;

    for (size_t i = 0; i < count; i++) {
        c = *str++;
        hash = ((hash << 5) + hash) + c; /* hash * 33 + c */
    }
    return hash;
}

void copy_file(FILE * from, FILE * to)
{
    size_t n, m;
    unsigned char buff[8192];
    do {
        n = fread(buff, 1, sizeof buff, from);
        if (n)
            m = fwrite(buff, 1, n, to);
        else
            m = 0;
    } while ((n > 0) && (n == m));
    if (m)
        perror("copy");
}

bool write_all_byte(const char * path, char * buffer, size_t size)
{
    FILE * fh = fopen(path, "w");
    if (fh != NULL) {
        if (fwrite(buffer, size, 1, fh) != 1) {
            exit(999);
        }
    }
    fclose(fh);
    return true;
}

uint8_t * read_all_bytes(flow_c * c, size_t * buffer_size, const char * path)
{
    uint8_t * buffer;
    FILE * fh = fopen(path, "rb");
    if (fh != NULL) {
        fseek(fh, 0L, SEEK_END);
        size_t s = ftell(fh);
        rewind(fh);
        buffer = (uint8_t *)FLOW_malloc(c, s);
        if (buffer != NULL) {
            // Returns 1 or 0, not the number of bytes.
            // Technically we're reading 1 element of size s
            size_t read_count = fread(buffer, s, 1, fh);
            // we can now close the file
            fclose(fh);
            fh = NULL;
            *buffer_size = s;
            if (s < 1) {
                // Failed to fill buffer
                fprintf(stderr, "Buffer size: %lu    Result code: %lu", s, read_count);
                exit(8);
            }
            return buffer;

        } else {
            fprintf(stderr, "Failed to allocate buffer of size: %lu", s);
            exit(8);
        }
        if (fh != NULL)
            fclose(fh);
    } else {
        fprintf(stderr, "Failed to open for reading: %s", path);
        exit(8);
    }
    return 0;
}
bool fetch_image(const char * url, char * dest_path)
{ /*null-terminated string*/
    static bool curl_initialized = false;
    if (!curl_initialized) {
        curl_initialized = true;
        curl_global_init(CURL_GLOBAL_ALL);
    }
    fprintf(stdout, "Fetching %s...", url);

    CURL * curl;
    FILE * fp;
    FILE * real_fp;
    CURLcode res;
    curl = curl_easy_init();
    if (curl) {
#ifdef _MSC_VER
        tmpfile_s(&fp);
#else
        fp = tmpfile();
#endif
        if (fp) {
            curl_easy_setopt(curl, CURLOPT_URL, url);
            curl_easy_setopt(curl, CURLOPT_WRITEDATA, fp);
            res = curl_easy_perform(curl);
            long http_code = 0;
            curl_easy_getinfo (curl, CURLINFO_RESPONSE_CODE, &http_code);
            if (res != CURLE_OK || http_code != 200) {
                fprintf(stderr, "CURL HTTP operation failed (error %d, status code %li) - GET %s, write to  %s\n", res, http_code, url, dest_path);
                exit(4);
            }
        } else {
            fprintf(stderr, "Failed to open temp file\n");
            exit(3);
        }
        /* always cleanup */
        curl_easy_cleanup(curl);
        rewind(fp);
        real_fp = fopen(dest_path, "wb");
        if (real_fp) {
            copy_file(fp, real_fp);
        } else {
            fprintf(stderr, "Failed to open file for writing %s\n", dest_path);
            exit(3);
        }
        fclose(real_fp);
        fclose(fp);
        fprintf(stdout, "...done! Written to %s\n", dest_path);
    } else {
        fprintf(stderr, "Failed to start CURL\n");
        exit(2);
    }
    return true;
}

uint8_t * get_bytes_cached(flow_c * c, size_t * bytes_count_out, const char * url)
{

#define FLOW_MAX_PATH 255
    char cache_folder[FLOW_MAX_PATH];
    char cache_path[FLOW_MAX_PATH];
    long long url_hash = djb2((unsigned const char *)url);

    if (!create_relative_path(c, false, &cache_folder[0], sizeof(cache_folder), "")) {
        FLOW_add_to_callstack(c);
        return NULL;
    }
    const char * ext = url + strlen(url) - 6;
    while (*ext != 0 && *ext != '.')
        ext++;

    if (flow_dir_exists_eh(&cache_folder[0])) {
        // The tests folder is still around; we can use it
        if (!create_relative_path(c, false, &cache_path[0], sizeof(cache_path), "/visuals/cache/%lu%s", url_hash,
                                  ext)) {
            FLOW_add_to_callstack(c);
            return NULL;
        }
    } else {
        char * cache_dir = getenv("HOME");
        if (cache_dir == NULL)
            cache_dir = getenv("TEMP");
        flow_snprintf(cache_path, FLOW_MAX_PATH, "%s/imageflow_cache/%lu%s", cache_dir, url_hash, ext);
    }

    flow_recursive_mkdir(&cache_path[0], false);

    if (access(cache_path, F_OK) == -1) {
        // file doesn't exist
        fetch_image(url, cache_path);
    } else {

        // fprintf(stdout, "Using cached image at %s", cache_path);
    }

    return read_all_bytes(c, bytes_count_out, cache_path);
}

bool flow_dir_exists_eh(const char * dir_path)
{
    struct stat sb;
    int e;
    e = stat(dir_path, &sb);
    // I think this logic is vague around permissions. Merits some test if exercised more heavily
    if (e == 0) {
        if ((sb.st_mode & S_IFREG) || !(sb.st_mode & S_IFDIR)) {
            // fprintf(stdout, "%s exists, but is not a directory!\n", dir_path);
            return false;
        }
    } else {
        if ((errno = ENOENT)) {
            return false;
        }
    }
    return true;
}
void flow_utils_ensure_directory_exists(const char * dir_path)
{
    struct stat sb;
    int e;
    e = stat(dir_path, &sb);
    if (e == 0) {
        if ((sb.st_mode & S_IFREG) || !(sb.st_mode & S_IFDIR)) {
            fprintf(stdout, "%s exists, but is not a directory!\n", dir_path);
            exit(1);
        }
    } else {
        if ((errno = ENOENT)) {
// Add more flags to the mode if necessary.
#ifdef _MSC_VER
            e = _mkdir(dir_path); // Windows doesn't support the last param, S_IRWXU);
#else
            e = mkdir(dir_path, S_IRWXU);
#endif
            if (e != 0) {
                fprintf(stdout, "The directory %s does not exist, and creation failed with errno=%d.\n", dir_path,
                        errno);
            } else {
                fprintf(stdout, "The directory %s did not exist. Created successfully.\n", dir_path);
            }
        }
    }
}

bool flow_recursive_mkdir(const char * dir, bool create_last_segment)
{
    char tmp[4096];
    char * p = NULL;
    size_t len;

    flow_snprintf(tmp, sizeof(tmp), "%s", dir);
    len = strlen(tmp);
    if (tmp[len - 1] == '/')
        tmp[len - 1] = 0;
    for (p = tmp + 1; *p; p++)
        if (*p == '/') {
            *p = 0;
#ifdef _MSC_VER
            _mkdir(tmp); // Windows doesn't support the last param, S_IRWXU);
#else
            mkdir(tmp, S_IRWXU);
#endif
            *p = '/';
        }
    if (create_last_segment){
#ifdef _MSC_VER
        _mkdir(tmp); // Windows doesn't support the last param, S_IRWXU);
#else
        mkdir(tmp, S_IRWXU);
#endif
    }
    return true;
}

bool create_relative_path(flow_c * c, bool create_parent_dirs, char * filename, size_t max_filename_length,
                          char * format, ...)
{
    const char * this_file = __FILE__;
    char * last_slash = strrchr(this_file, '/');
    if (last_slash == NULL) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }
    size_t length = last_slash - this_file;

    if (max_filename_length < length + 1) {
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }
    memcpy(&filename[0], this_file, length);
    va_list v;
    va_start(v, format);
    int res = flow_vsnprintf(&filename[length], max_filename_length - length, format, v);
    va_end(v);
    if (res == -1) {
        // Not enough space in filename
        FLOW_error(c, flow_status_Invalid_internal_state);
        return false;
    }
    /// Create directories
    if (create_parent_dirs) {
        flow_recursive_mkdir(&filename[0], false);
    }
    return true;
}

bool has_err(flow_c * c, const char * file, int line, const char * func)
{
    if (flow_context_has_error(c)) {
        flow_context_add_to_callstack(c, file, line, func);
        flow_context_print_error_to(c, stderr);
        return true;
    }
    return false;
}

struct flow_bitmap_bgra * BitmapBgra_create_test_image(flow_c * c)
{
    struct flow_bitmap_bgra * test = flow_bitmap_bgra_create(c, 256, 256, false, flow_bgra32);
    if (test == NULL) {
        FLOW_add_to_callstack(c);
        return NULL;
    }
    uint8_t * pixel;
    for (uint32_t y = 0; y < test->h; y++) {
        pixel = test->pixels + (y * test->stride);
        for (uint32_t x = 0; x < test->w; x++) {
            pixel[0] = (uint8_t)x;
            pixel[1] = (uint8_t)(x / 2);
            pixel[2] = (uint8_t)(x / 3);
            pixel[3] = (uint8_t)y;
            pixel += 4;
        }
    }
    return test;
}

// Returns average delte per channel per pixel. returns (double)INT32_MAX if dimension or channel mismatch
double flow_bitmap_float_compare(flow_c * c, struct flow_bitmap_float * a, struct flow_bitmap_float * b,
                                 float * out_max_delta)
{
    if (a->w != b->w || a->h != b->h || a->channels != b->channels || a->float_count != b->float_count
        || a->float_stride != b->float_stride) {
        return (double)INT32_MAX;
    }
    double difference_total = 0;
    float max_delta = 0;
    for (uint32_t y = 0; y < a->h; y++) {

        double row_delta = 0;
        for (uint32_t x = 0; x < a->w; x++) {
            int pixel = y * a->float_stride + x * a->channels;
            for (uint32_t cx = 0; cx < a->channels; cx++) {
                float delta = (float)fabs(a->pixels[pixel + cx] - b->pixels[pixel + cx]);
                if (delta > max_delta)
                    max_delta = delta;
                row_delta += delta;
            }
        }
        difference_total = row_delta / (float)(a->w * a->channels);
    }
    *out_max_delta = max_delta;
    return difference_total / a->h;
}

struct flow_io * get_io_for_cached_url(flow_c * c, const char * url, void * owner)
{
    size_t bytes_count = 0;
    uint8_t * bytes = get_bytes_cached(c, &bytes_count, url);
    if (bytes == NULL) {
        FLOW_error(c, flow_status_IO_error);
        return NULL;
    }

    struct flow_io * input = flow_io_create_from_memory(c, flow_io_mode_read_seekable, bytes, bytes_count, owner, NULL);
    if (input == NULL) {
        FLOW_add_to_callstack(c);
        return NULL;
    }
    return input;
}
