#pragma once
/**

  Currently we only support BGR24 and BGRA32 pixel formats.
  (And BGR32, where we ignore the alpha channel, but that's not precisely a separate format)
  Eventually we will need to support
  * 8-bit grayscale
  * CMYK
  * YCbCr
  and possibly others. For V1, the API we expose is only used by projects in the same repository, running under the same tests.
  In V2, we can change the API as we wish; we are not constrained to what we design here.
  Perhaps it is best to explicitly limit the structure to represent what we process at this time?
 If our buffers and structures actually describe their contents, then we need to support all permitted values in all functions. This is problematic.
* We heavily experimented with LUV and XYZ color spaces, but determined that better results occur using RGB linear.
* A custom sigmoidized color space could perhaps improve things, but would introduce significant overhead.
**/

/* Proposed changes

*/

#include "fastscaling_enums.h"
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>


#ifdef __cplusplus
extern "C" {
#endif


typedef struct ContextStruct Context;

/** Context: ProfilingLog **/

typedef struct {
    int64_t time;
    const char * name;
    ProfilingEntryFlags flags;
} ProfilingEntry;

typedef struct {
    ProfilingEntry * log;
    uint32_t count;
    uint32_t capacity;
    int64_t ticks_per_second;
} ProfilingLog;

ProfilingLog * Context_get_profiler_log(Context * context);


Context * Context_create(void);
void Context_destroy(Context * context);

const char * Context_error_message(Context * context, char * buffer, size_t buffer_size);

const char * Context_stacktrace (Context * context, char * buffer, size_t buffer_size);

bool Context_has_error(Context * context);
int  Context_error_reason(Context * context);

void Context_free_static_caches(void);


//non-indexed bitmap
typedef struct BitmapBgraStruct {

    //bitmap width in pixels
    uint32_t w;
    //bitmap height in pixels
    uint32_t h;
    //byte length of each row (may include any amount of padding)
    uint32_t stride;
    //pointer to pixel 0,0; should be of length > h * stride
    unsigned char *pixels;
    //If true, we don't dispose of *pixels when we dispose the struct
    bool borrowed_pixels;
    //If false, we can even ignore the alpha channel on 4bpp
    bool alpha_meaningful;
    //If false, we can edit pixels without affecting the stride
    bool pixels_readonly;
    //If false, we can change the stride of the image.
    bool stride_readonly;

    //If true, we can reuse the allocated memory for other purposes.
    bool can_reuse_space;

    BitmapPixelFormat fmt;

    //When using compositing mode blend_with_matte, this color will be used. We should probably define this as always being sRGBA, 4 bytes.
    uint8_t matte_color[4];

    BitmapCompositingMode compositing_mode;

} BitmapBgra;




float Context_byte_to_floatspace (Context * c, uint8_t srgb_value);
uint8_t Context_floatspace_to_byte (Context * c, float space_value);






void Context_set_floatspace (Context * context,  WorkingFloatspace space, float a, float b, float c);

typedef struct RendererStruct Renderer;



struct InterpolationDetailsStruct;
typedef double (*detailed_interpolation_method)(const struct InterpolationDetailsStruct *, double);


typedef struct InterpolationDetailsStruct {
    //1 is the default; near-zero overlapping between windows. 2 overlaps 50% on each side.
    double window;
    //Coefficients for bucubic weighting
    double p1, p2, p3, q1, q2, q3, q4;
    //Blurring factor when > 1, sharpening factor when < 1. Applied to weights.
    double blur;

    //pointer to the weight calculation function
    detailed_interpolation_method filter;
    //How much sharpening we are requesting
    float sharpen_percent_goal;

} InterpolationDetails;


typedef struct ConvolutionKernelStruct {
    float * kernel;
    uint32_t width;
    uint32_t radius;
    float threshold_min_change; //These change values are on a somewhat arbitrary scale between 0 and 4;
    float threshold_max_change;
    float * buffer;
} ConvolutionKernel;

typedef struct RenderDetailsStruct {
    //Interpolation and scaling details
    InterpolationDetails * interpolation;
    //How large the interoplation window needs to be before we even attempt to apply a sharpening
    //percentage to the given filter
    float minimum_sample_window_to_interposharpen;


    // If possible to do correctly, halve the image until it is [interpolate_last_percent] times larger than needed. 3 or greater reccomended. Specify -1 to disable halving.
    float interpolate_last_percent;

    //The number of pixels (in target canvas coordinates) that it is acceptable to discard for better halving performance
    float havling_acceptable_pixel_loss;

    //The actual halving factor to use.
    uint32_t halving_divisor;

    //The first convolution to apply
    ConvolutionKernel * kernel_a;
    //A second convolution to apply
    ConvolutionKernel * kernel_b;


    //If greater than 0, a percentage to sharpen the result along each axis;
    float sharpen_percent_goal;

    //If true, we should apply the color matrix
    bool apply_color_matrix;

    float color_matrix_data[25];
    float *color_matrix[5];

    //Transpose, flipx, flipy - combined, these give you all 90 interval rotations
    bool post_transpose;
    bool post_flip_x;
    bool post_flip_y;

    //Enables profiling
    bool enable_profiling;

} RenderDetails;



BitmapBgra * BitmapBgra_create(Context * context, int sx, int sy, bool zeroed, BitmapPixelFormat format);
BitmapBgra * BitmapBgra_create_header(Context * context, int sx, int sy);
void BitmapBgra_destroy(Context * context, BitmapBgra * im);

RenderDetails * RenderDetails_create(Context * context);
RenderDetails * RenderDetails_create_with(Context * context, InterpolationFilter filter);

bool RenderDetails_render(Context * context, RenderDetails * details, BitmapBgra * source, BitmapBgra * canvas);
bool RenderDetails_render_in_place(Context * context, RenderDetails * details, BitmapBgra * edit_in_place);
void RenderDetails_destroy(Context * context, RenderDetails * d);

bool InterpolationDetails_interpolation_filter_exists(InterpolationFilter filter);
InterpolationDetails * InterpolationDetails_create(Context * context);
InterpolationDetails * InterpolationDetails_create_bicubic_custom(Context * context,double window, double blur, double B, double C);
InterpolationDetails * InterpolationDetails_create_custom(Context * context,double window, double blur, detailed_interpolation_method filter);
InterpolationDetails * InterpolationDetails_create_from(Context * context,InterpolationFilter filter);
double InterpolationDetails_percent_negative_weight(const InterpolationDetails * details);
void InterpolationDetails_destroy(Context * context, InterpolationDetails *);

uint32_t BitmapPixelFormat_bytes_per_pixel (BitmapPixelFormat format);

typedef struct {
    float *Weights;/* Normalized weights of neighboring pixels */
    int Left;      /* Bounds of source pixels window */
    int Right;
} PixelContributions;/* Contirbution information for a single pixel */

typedef struct {
    PixelContributions *ContribRow; /* Row (or column) of contribution weights */
    uint32_t WindowSize;      /* Filter window size (of affecting source pixels) */
    uint32_t LineLength;      /* Length of line (no. or rows / cols) */
    double percent_negative; /* Estimates the sharpening effect actually applied*/
} LineContributions;

LineContributions * LineContributions_create(Context * context, const uint32_t output_line_size, const uint32_t input_line_size, const InterpolationDetails * details);
void LineContributions_destroy(Context * context, LineContributions * p);

ConvolutionKernel * ConvolutionKernel_create(Context * context, uint32_t radius);
void ConvolutionKernel_destroy(Context * context, ConvolutionKernel * kernel);


ConvolutionKernel* ConvolutionKernel_create_guassian(Context * context, double stdDev, uint32_t radius);
//The only error these 2 could generate would be a null pointer. Should they have a context just for this?
double ConvolutionKernel_sum(ConvolutionKernel* kernel);
void ConvolutionKernel_normalize(ConvolutionKernel* kernel, float desiredSum);
ConvolutionKernel* ConvolutionKernel_create_guassian_normalized(Context * context, double stdDev, uint32_t radius);
ConvolutionKernel* ConvolutionKernel_create_guassian_sharpen(Context * context, double stdDev, uint32_t radius);


bool BitmapBgra_populate_histogram (Context * context, BitmapBgra * bmp, uint64_t * histograms, uint32_t histogram_size_per_channel, uint32_t histogram_count, uint64_t * pixels_sampled);


#ifdef __cplusplus
}
#endif
