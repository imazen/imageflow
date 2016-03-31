#include "helpers_visual.h"

struct named_checksum {
    char * name;
    char * checksum;
};

static bool create_relative_path(flow_c * c, char * filename, size_t max_filename_length, char * format, ...)
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
    return true;
}

/* This code is public domain -- Will Hartung 4/9/09 */
#include <stdio.h>
#include <stdlib.h>

int64_t flow_getline(char **lineptr, size_t *n, FILE *stream) {
    char *bufptr = NULL;
    char *temp_bufptr = NULL;
    char *p = bufptr;
    size_t size;
    int c;

    if (lineptr == NULL) {
        return -1;
    }
    if (stream == NULL) {
        return -1;
    }
    if (n == NULL) {
        return -1;
    }
    bufptr = *lineptr;
    size = *n;

    c = fgetc(stream);
    if (c == EOF) {
        return -1;
    }
    if (bufptr == NULL) {
        bufptr = (char *)malloc(128);
        if (bufptr == NULL) {
            return -1;
        }
        size = 128;
    }
    p = bufptr;
    while (c != EOF) {
        if ((p - bufptr + 1) > (int64_t)size) {
            size = size + 128;
            temp_bufptr = (char *)realloc(bufptr, size);
            if (temp_bufptr == NULL) {          
                return -1;
            }
            else {
                bufptr = temp_bufptr;
                p = p + (int64_t)(temp_bufptr - bufptr);
            }
        }
        *p++ = c;
        if (c == '\n') {
            break;
        }
        c = fgetc(stream);
    }

    *p++ = '\0';
    *lineptr = bufptr;
    *n = size;

    return p - bufptr - 1;
}

static bool load_checksums(flow_c * c, struct named_checksum ** checksums, size_t * checksum_count)
{
    static struct named_checksum * list = NULL;
    static size_t list_size = 0;

    if (list == NULL) {
        char filename[2048];
        if (!create_relative_path(c, filename, 2048, "/visuals/checksums.list")) {
            FLOW_add_to_callstack(c);
            return false;
        }
        FILE * fp;
        char * line_a = NULL;
        size_t len_a = 0;
        int64_t read_a;
        char * line_b = NULL;
        size_t len_b = 0;
        int64_t read_b;

        fp = fopen(filename, "r");
        if (fp == NULL) {
            FLOW_error(c, flow_status_IO_error);
            return false;
        }

        list_size = 200;
        list = (struct named_checksum *)calloc(list_size, sizeof(struct named_checksum));

        size_t index = 0;
        while (true) {
            // Read lines in pairs
            read_a = flow_getline(&line_a, &len_a, fp);
            if (read_a == -1) {
                break;
            }
            read_b = flow_getline(&line_b, &len_b, fp);
            if (read_b == -1) {
                free(line_a);
                break;
            }
            // Drop newlines if present
            if (line_a[read_a - 1] == '\n') {
                line_a[read_a - 1] = '\0';
            }
            if (line_b[read_b - 1] == '\n') {
                line_b[read_b - 1] = '\0';
            }
            // Save
            list[index].name = line_a;
            list[index].checksum = line_b;
            line_a = NULL;
            line_b = NULL;
            index++;
            if (index >= list_size) {
                FLOW_error_msg(c, flow_status_IO_error,
                               "Could not read in entire checksum file. Please increase list_size above %ul.",
                               list_size);
                fclose(fp);
                return false;
            }
        }
        list_size = index;
        fclose(fp);
    }
    *checksum_count = list_size;
    *checksums = list;

    return true;
}
bool append_checksum(flow_c * c, char checksum[34], const char * name);
bool append_checksum(flow_c * c, char checksum[34], const char * name)
{
    char filename[2048];
    if (!create_relative_path(c, filename, 2048, "/visuals/checksums.list")) {
        FLOW_add_to_callstack(c);
        return false;
    }
    FILE * fp = fopen(filename, "a");
    if (fp == NULL) {
        FLOW_error(c, flow_status_IO_error);
        return false;
    }
    fprintf(fp, "%s\n%s\n", name, &checksum[0]);
    fclose(fp);
    return true;
}

static bool checksum_bitmap(flow_c * c, struct flow_bitmap_bgra * bitmap, char * checksum_buffer,
                            size_t checksum_buffer_length)
{
    char info_buffer[256];
    flow_snprintf(&info_buffer[0], sizeof(info_buffer), "%dx%d fmt=%d alpha=%d", bitmap->w, bitmap->h, bitmap->fmt,
                  bitmap->alpha_meaningful);
    int64_t printed_chars = (flow_snprintf(checksum_buffer, checksum_buffer_length, "%016X_%016X",
                                           djb2_buffer((uint8_t *)bitmap->pixels, bitmap->stride * bitmap->h),
                                           djb2((unsigned const char *)&info_buffer[0])));

    return printed_chars != -1;
}

static char * get_checksum_for(flow_c * c, const char * name)
{
    struct named_checksum * checksums = NULL;
    size_t checksum_count = 0;

    if (!load_checksums(c, &checksums, &checksum_count)) {
        FLOW_add_to_callstack(c);
        return NULL;
    }
    for (size_t i = 0; i < checksum_count; i++) {
        if (strcmp(checksums[i].name, name) == 0) {
            return checksums[i].checksum;
        }
    }
    return NULL;
}

static bool download_by_checksum(flow_c * c, struct flow_bitmap_bgra * bitmap, char * checksum)
{
    char filename[2048];
    if (!create_relative_path(c, filename, 2048, "/visuals/%s.png", checksum)) {
        FLOW_add_to_callstack(c);
        return false;
    }

    fprintf(stderr, "%s (trusted)\n", &filename[0]);
    if (access(filename, F_OK) != -1) {
        return true; // Already exists!
    }
    char url[2048];
    flow_snprintf(url, 2048, "http://s3/%s.png", checksum); // TODO: fix actual URL
    if (!fetch_image(url, filename)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    return true;
}

static bool save_bitmap_to_visuals(flow_c * c, struct flow_bitmap_bgra * bitmap, char * checksum)
{
    char filename[2048];
    if (!create_relative_path(c, filename, 2048, "/visuals/%s.png", checksum)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    if (!write_frame_to_disk(c, &filename[0], bitmap)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    fprintf(stderr, "%s (current)\n", &filename[0]);
    return true;
}

static bool generate_image_diff(flow_c * c, char * checksum_a, char * checksum_b)
{
    char filename_a[2048];
    if (!create_relative_path(c, filename_a, 2048, "/visuals/%s.png", checksum_a)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    char filename_b[2048];
    if (!create_relative_path(c, filename_b, 2048, "/visuals/%s.png", checksum_b)) {
        FLOW_add_to_callstack(c);
        return false;
    }
    char filename_c[2048];
    if (!create_relative_path(c, filename_c, 2048, "/visuals/compare_%s_vs_%s.png", checksum_a, checksum_b)) {
        FLOW_add_to_callstack(c);
        return false;
    }

    fprintf(stderr, "%s\n", &filename_c[0]);

    if (access(filename_c, F_OK) != -1) {
        return true; // Already exists!
    }

    char magick_command[4096];
    flow_snprintf(magick_command, 4096, "compare -verbose -metric PSNR %s %s %s", filename_a, filename_b, filename_c);
    int32_t ignore = system(magick_command);
    ignore++;
    return true;
}

static bool append_html(flow_c * c, const char * name, const char * checksum_a, const char * checksum_b)
{
    char filename[2048];
    if (!create_relative_path(c, filename, 2048, "/visuals/visuals.html")) {
        FLOW_add_to_callstack(c);
        return false;
    }
    static bool first_write = true;

    FILE * fp = fopen(filename, first_write ? "w" : "a");
    if (fp == NULL) {
        FLOW_error(c, flow_status_IO_error);
        return false;
    }
    if (first_write) {
        // Write the header here
    }
    if (checksum_b == NULL) {
        fprintf(fp, "<h1>%s</h2>\n<img class=\"current\" src=\"%s.png\"/>", name, checksum_a);
    } else {
        fprintf(fp, "<h1>%s</h2>\n<img class=\"current\" src=\"%s.png\"/><img class=\"correct\" src=\"%s.png\"/><img "
                    "class=\"diff\" src=\"compare_%s_vs_%s.png\"/>",
                name, checksum_a, checksum_b, checksum_a, checksum_b);
    }

    fclose(fp);
    first_write = false;
    return true;
}

bool visual_compare(flow_c * c, struct flow_bitmap_bgra * bitmap, const char * name, bool store_checksums,
                    const char * file_, const char * func_, int line_number)
{

    char checksum[34];

    // compute checksum of bitmap (two checksums, actually - one for configuration, another for bitmap bytes)
    if (!checksum_bitmap(c, bitmap, checksum, 34)) {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }
    // Load stored checksum
    char * stored_checksum = get_checksum_for(c, name);

    // Compare
    if (stored_checksum != NULL && strcmp(checksum, stored_checksum) == 0) {
        return true; // It matches!
    }

    if (stored_checksum == NULL) {
        // No stored checksum for this name
        if (store_checksums) {
            if (!append_checksum(c, checksum, name)) {
                FLOW_error_return(c);
            }
            fprintf(stderr, "===============\n%s\nStoring checksum %s, since FLOW_STORE_CHECKSUMS was set.\n ", name,
                    &checksum[0]);
        } else {
            fprintf(stderr, "===============\n%s\nThere is no stored checksum for this test; #define "
                            "FLOW_STORE_CHECKSUMS and rerun to set the initial value to %s.\n ",
                    name, &checksum[0]);
        }

        fprintf(stderr, "%s:%d in function %s\n", file_, line_number, func_);
    } else {
        fprintf(stderr, "===============\n%s\nThe stored checksum [%s] differs from the current result [%s]. Open "
                        "visuals/visuals.html to comapre.\n ",
                name, stored_checksum, checksum);
        fprintf(stderr, "%s:%d in function %s\n", file_, line_number, func_);
    }

    // The hash differs
    // Save ours so we can see it
    if (!save_bitmap_to_visuals(c, bitmap, checksum)) {
        FLOW_error_return(c);
    }

    if (stored_checksum != NULL) {
        // Try to download "old" png from S3 using the checksums as an address.
        if (!download_by_checksum(c, bitmap, stored_checksum)) {
            FLOW_error_return(c);
        }

        // Diff the two, generate a third PNG. Also get PSNR metrics from imagemagick
        if (!generate_image_diff(c, checksum, stored_checksum)) {
            FLOW_error_return(c);
        }

        // Dump to HTML=
        if (!append_html(c, name, checksum, stored_checksum)) {
            FLOW_error_return(c);
        }
    }

    return false;
}
