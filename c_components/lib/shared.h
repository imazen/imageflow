
// stdio.h

#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <stdio.h>
#include <setjmp.h>
#include <stdarg.h>
#include <string.h>
#include <sys/stat.h>
#include <errno.h>

#define __STDC_FORMAT_MACROS
#include <inttypes.h>
#undef __STDC_FORMAT_MACROS

#if defined(_WIN32)

#if defined(imageflow_c_BUILD_SHARED)
/* Cmake will define imageflow_EXPORTS on Windows when it
configures to build a shared library.*/
#define FLOW_EXPORT __declspec(dllexport)
#else
#if defined(imageflow_c_BUILD_STATIC)
#define FLOW_EXPORT
#else
#define FLOW_EXPORT __declspec(dllimport)
#endif
#endif /* imageflow_EXPORTS */
#else /* defined (_WIN32) */
#define FLOW_EXPORT
#endif

typedef enum flow_pixel_format { flow_bgr24 = 3, flow_bgra32 = 4, flow_bgr32 = 70, flow_gray8 = 1 } flow_pixel_format;
