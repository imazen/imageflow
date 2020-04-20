
#include "helpers.h"
#include <sys/stat.h>
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

uint64_t djb2(unsigned const char * str)
{
    uint64_t hash = 5381;
    int c;

    while ((c = *str++))
        hash = ((hash << 5) + hash) + c; /* hash * 33 + c */

    return hash;
}

uint64_t djb2_buffer(uint8_t * str, size_t count)
{
    uint64_t hash = 5381;
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

bool flow_dir_exists_eh(const char * dir_path)
{
    struct stat sb;
    int e;
    e = stat(dir_path, &sb);
    // I think this logic is vague around permissions. Merits some test if exercised more heavily
    if (e == 0) {
        if (S_ISREG(sb.st_mode) || !S_ISDIR(sb.st_mode)) {
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
        if (S_ISREG(sb.st_mode) || !S_ISDIR(sb.st_mode)) {
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
    if (tmp[len - 1] == '/' || tmp[len - 1] == '\\')
        tmp[len - 1] = 0;
    for (p = tmp + 1; *p; p++)
        if (*p == '/' || *p == '\\') {
            *p = 0;
#ifdef _MSC_VER
            _mkdir(tmp); // Windows doesn't support the last param, S_IRWXU);
#else
            mkdir(tmp, S_IRWXU);
#endif
#ifdef _MSC_VER
            *p = '\\';
#else
            *p = '/';
#endif
        }
    if (create_last_segment) {
#ifdef _MSC_VER
        _mkdir(tmp); // Windows doesn't support the last param, S_IRWXU);
#else
        mkdir(tmp, S_IRWXU);
#endif
    }
    return true;
}

bool create_path_from_relative(flow_c * c, const char * base_file, bool create_parent_dirs, char * filename,
                               size_t max_filename_length, const char * format, ...)
{
    if (base_file == NULL) {
        FLOW_error(c, flow_status_Null_argument);
        return false;
    }
    const char * this_file = base_file;
    const char * last_slash = strrchr(this_file, '/');
    if (last_slash == NULL) {
        last_slash = strrchr(this_file, '\\');
    }
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

// Returns average delta per channel per pixel. returns (double)INT32_MAX if dimension or channel mismatch
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

bool flow_compare_file_contents(flow_c * c, const char * filename1, const char * filename2,
                                char * difference_message_buffer, size_t buffer_size, bool * are_equal)
{
    FILE * fp1, *fp2;

    if ((fp1 = fopen(filename1, "r")) == NULL) {
        if (difference_message_buffer != NULL)
            flow_snprintf(difference_message_buffer, buffer_size, "Unable to open file A (%s)", filename1);
        *are_equal = false;
        return true;
    }

    if ((fp2 = fopen(filename2, "r")) == NULL) {
        if (difference_message_buffer != NULL)
            flow_snprintf(difference_message_buffer, buffer_size, "Unable to open file B (%s)", filename2);
        *are_equal = false;
        return true;
    }

    int byte_ix = -1;
    bool mismatch = false;
    int f1, f2;
    while (1) {

        do {
            f1 = getc(fp1);
            byte_ix++;
        } while (f1 == 13); // Ignore carriage returns

        do {
            f2 = getc(fp2);
        } while (f2 == 13); // Ignore carriage returns

        if ((f1 == EOF) ^ (f2 == EOF)) {
            // Only one of the files ended
            mismatch = true;

            if (difference_message_buffer != NULL)
                flow_snprintf(difference_message_buffer, buffer_size,
                              "Files are of different lengths: reached EOF at byte %d in %s first.", byte_ix,
                              f1 == EOF ? filename1 : filename2);
            break;
        }

        if (f1 == EOF) {
            break;
        }

        if (f1 != f2) {
            mismatch = true;

            if (difference_message_buffer != NULL)
                flow_snprintf(difference_message_buffer, buffer_size, "Files differ at byte %d: %d vs %d", byte_ix, f1,
                              f2);
            break;
        }
    }
    *are_equal = !mismatch;

    fclose(fp1);
    fclose(fp2);
    return true;
}
