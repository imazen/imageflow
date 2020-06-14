#pragma once

#include <sys/types.h>
#include <sys/stat.h>
#include <errno.h>
#include <stdio.h>
#include "string.h"
#include <stdbool.h>
#include <stdlib.h>
#include "imageflow.h"
#include "lib/imageflow_private.h"
#ifdef _MSC_VER
#include "direct.h" //for _mkdir
#endif

#ifdef __cplusplus
extern "C" {
#endif


#include <sys/stat.h>


#ifdef _MSC_VER
#include "io.h"
#pragma warning(error : 4005)
    #define S_ISREG(m) (((m) & S_IFMT) == S_IFREG)
    #define S_ISDIR(m) (((m) & S_IFMT) == S_IFDIR)
#ifndef _UNISTD_H
#define _UNISTD_H 1

/* This file intended to serve as a drop-in replacement for
*  unistd.h on Windows
*  Please add functionality as needed
*/

#include <stdlib.h>
#include <io.h>
#include <process.h> /* for getpid() and the exec..() family */
#include <direct.h> /* for _getcwd() and _chdir() */

#define srandom srand
#define random rand

/* Values for the second argument to access.
These may be OR'd together.  */
#define R_OK 4 /* Test for read permission.  */
#define W_OK 2 /* Test for write permission.  */
//#define   X_OK    1       /* execute permission - unsupported in windows*/
#define F_OK 0 /* Test for existence.  */

#define access _access
#define dup2 _dup2
#define execve _execve
#define ftruncate _chsize
#define unlink _unlink
#define fileno _fileno
#define getcwd _getcwd
#define chdir _chdir
#define isatty _isatty
#define lseek _lseek
/* read, write, and close are NOT being #defined here, because while there are file handle specific versions for
 * Windows, they probably don't work for sockets. You need to look at your app and consider whether to call e.g.
 * closesocket(). */

#define ssize_t int

#define STDIN_FILENO 0
#define STDOUT_FILENO 1
#define STDERR_FILENO 2

#define S_IRWXU = (400 | 200 | 100)
#endif
#else
#include "unistd.h"
#endif

uint8_t * get_bytes_cached(flow_c * c, size_t * bytes_count_out, const char * url, const char * storage_relative_to);
bool fetch_image(const char * url, char * dest_path);
uint8_t * read_all_bytes(flow_c * c, size_t * buffer_size, const char * path);
bool write_all_byte(const char * path, char * buffer, size_t size);
void copy_file(FILE * from, FILE * to);

bool create_path_from_relative(flow_c * c, const char * base_file, bool create_parent_dirs, char * filename,
                               size_t max_filename_length, const char * format, ...);

uint64_t djb2(unsigned const char * str);
uint64_t djb2_buffer(uint8_t * str, size_t count);
size_t nonzero_count(uint8_t * array, size_t length);

struct flow_bitmap_bgra * BitmapBgra_create_test_image(flow_c * c);
double flow_bitmap_float_compare(flow_c * c, struct flow_bitmap_float * a, struct flow_bitmap_float * b,
                                 float * out_max_delta);

struct flow_io * get_io_for_cached_url(flow_c * c, const char * url, void * owner, const char * storage_relative_to);

bool has_err(flow_c * c, const char * file, int line, const char * func);

bool flow_recursive_mkdir(const char * dir, bool create_last_segment);

void flow_utils_ensure_directory_exists(const char * dir_path);
bool flow_dir_exists_eh(const char * dir_path);

bool flow_compare_file_contents(flow_c * c, const char * filename1, const char * filename2,
                                char * difference_message_buffer, size_t buffer_size, bool * are_equal);

#define ERR(c) REQUIRE_FALSE(has_err(c, __FILE__, __LINE__, __func__))
#define PRINT_IF_ERR(c) has_err(c, __FILE__, __LINE__, __func__)

#ifdef __cplusplus
}
#endif
