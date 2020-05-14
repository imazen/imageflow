/*
 * Copyright (c) Imazen LLC.
 * No part of this project, including this file, may be copied, modified,
 * propagated, or distributed except as permitted in COPYRIGHT.txt.
 * Licensed under the GNU Affero General Public License, Version 3.0.
 * Commercial licenses available at http://imageresizing.net/
 */
#ifdef _MSC_VER
#pragma unmanaged
#endif

#include "imageflow_private.h"
#include <string.h>
#include <immintrin.h>

#ifndef _MSC_VER
#include <alloca.h>
#else
#pragma unmanaged
#ifndef alloca
#include <malloc.h>
#define alloca _alloca
#endif
#endif

struct flow_convolution_kernel * flow_convolution_kernel_create(flow_c * context, uint32_t radius)
{
    struct flow_convolution_kernel * k = FLOW_calloc_array(context, 1, struct flow_convolution_kernel);
    // For the actual array;
    float * a = FLOW_calloc_array(context, radius * 2 + 1, float);
    // we assume a maximum of 4 channels are going to need buffering during convolution
    float * buf = (float *)FLOW_malloc(context, (radius + 2) * 4 * sizeof(float));

    if (k == NULL || a == NULL || buf == NULL) {
        FLOW_free(context, k);
        FLOW_free(context, a);
        FLOW_free(context, buf);
        FLOW_error(context, flow_status_Out_of_memory);
        return NULL;
    }
    k->kernel = a;
    k->width = radius * 2 + 1;
    k->buffer = buf;
    k->radius = radius;
    return k;
}
void flow_convolution_kernel_destroy(flow_c * context, struct flow_convolution_kernel * kernel)
{
    if (kernel != NULL) {
        FLOW_free(context, kernel->kernel);
        FLOW_free(context, kernel->buffer);
        kernel->kernel = NULL;
        kernel->buffer = NULL;
    }
    FLOW_free(context, kernel);
}

struct flow_convolution_kernel * flow_convolution_kernel_create_guassian(flow_c * context, double stdDev,
                                                                         uint32_t radius)
{
    struct flow_convolution_kernel * k = flow_convolution_kernel_create(context, radius);
    if (k != NULL) {
        for (uint32_t i = 0; i < k->width; i++) {

            k->kernel[i] = (float)ir_guassian(abs((int)radius - (int)i), stdDev);
        }
    }
    return k;
}

double flow_convolution_kernel_sum(struct flow_convolution_kernel * kernel)
{
    double sum = 0;
    for (uint32_t i = 0; i < kernel->width; i++) {
        sum += kernel->kernel[i];
    }
    return sum;
}

void flow_convolution_kernel_normalize(struct flow_convolution_kernel * kernel, float desiredSum)
{
    double sum = flow_convolution_kernel_sum(kernel);
    if (sum == 0)
        return; // nothing to do here, zeroes are as normalized as you can get ;)
    float factor = (float)(desiredSum / sum);
    for (uint32_t i = 0; i < kernel->width; i++) {
        kernel->kernel[i] *= factor;
    }
}
struct flow_convolution_kernel * flow_convolution_kernel_create_gaussian_normalized(flow_c * context, double stdDev,
                                                                                    uint32_t radius)
{
    struct flow_convolution_kernel * kernel = flow_convolution_kernel_create_guassian(context, stdDev, radius);
    if (kernel != NULL) {
        flow_convolution_kernel_normalize(kernel, 1);
    }
    return kernel;
}

struct flow_convolution_kernel * flow_convolution_kernel_create_gaussian_sharpen(flow_c * context, double stdDev,
                                                                                 uint32_t radius)
{
    struct flow_convolution_kernel * kernel = flow_convolution_kernel_create_guassian(context, stdDev, radius);
    if (kernel != NULL) {
        double sum = flow_convolution_kernel_sum(kernel);
        for (uint32_t i = 0; i < kernel->width; i++) {
            if (i == radius) {
                kernel->kernel[i] = (float)(2 * sum - kernel->kernel[i]);
            } else {
                kernel->kernel[i] *= -1;
            }
        }
        flow_convolution_kernel_normalize(kernel, 1);
    }
    return kernel;
}

bool flow_bitmap_float_convolve_rows(flow_c * context, struct flow_bitmap_float * buf,
                                     struct flow_convolution_kernel * kernel, uint32_t convolve_channels,
                                     uint32_t from_row, int row_count)
{

    const uint32_t radius = kernel->radius;
    const float threshold_min = kernel->threshold_min_change;
    const float threshold_max = kernel->threshold_max_change;

    // Do nothing unless the image is at least half as wide as the kernel.
    if (buf->w < radius + 1)
        return true;

    const uint32_t buffer_count = radius + 1;
    const uint32_t w = buf->w;
    const int32_t int_w = (int32_t)buf->w;
    const uint32_t step = buf->channels;

    const uint32_t until_row = row_count < 0 ? buf->h : from_row + (unsigned)row_count;

    const uint32_t ch_used = convolve_channels;

    float * __restrict buffer = kernel->buffer;
    float * __restrict avg = &kernel->buffer[buffer_count * ch_used];

    const float * __restrict kern = kernel->kernel;

    const int wrap_mode = 0;

    for (uint32_t row = from_row; row < until_row; row++) {

        float * __restrict source_buffer = &buf->pixels[row * buf->float_stride];
        int circular_idx = 0;

        for (uint32_t ndx = 0; ndx < w + buffer_count; ndx++) {
            // Flush old value
            if (ndx >= buffer_count) {
                memcpy(&source_buffer[(ndx - buffer_count) * step], &buffer[circular_idx * ch_used],
                       ch_used * sizeof(float));
            }
            // Calculate and enqueue new value
            if (ndx < w) {
                const int left = ndx - radius;
                const int right = ndx + radius;
                int i;

                memset(avg, 0, sizeof(float) * ch_used);

                if (left < 0 || right >= (int32_t)w) {
                    if (wrap_mode == 0) {
                        // Only sample what's present, and fix the average later.
                        float total_weight = 0;
                        /* Accumulate each channel */
                        for (i = left; i <= right; i++) {
                            if (i > 0 && i < int_w) {
                                const float weight = kern[i - left];
                                total_weight += weight;
                                for (uint32_t j = 0; j < ch_used; j++)
                                    avg[j] += weight * source_buffer[i * step + j];
                            }
                        }
                        for (uint32_t j = 0; j < ch_used; j++)
                            avg[j] = avg[j] / total_weight;
                    } else if (wrap_mode == 1) {
                        // Extend last pixel to be used for all missing inputs
                        /* Accumulate each channel */
                        for (i = left; i <= right; i++) {
                            const float weight = kern[i - left];
                            const uint32_t ix = EVIL_CLAMP(i, 0, int_w - 1);
                            for (uint32_t j = 0; j < ch_used; j++)
                                avg[j] += weight * source_buffer[ix * step + j];
                        }
                    }
                } else {
                    /* Accumulate each channel */
                    for (i = left; i <= right; i++) {
                        const float weight = kern[i - left];
                        for (uint32_t j = 0; j < ch_used; j++)
                            avg[j] += weight * source_buffer[i * step + j];
                    }
                }

                // Enqueue difference
                memcpy(&buffer[circular_idx * ch_used], avg, ch_used * sizeof(float));

                if (threshold_min > 0 || threshold_max > 0) {
                    float change = 0;
                    for (uint32_t j = 0; j < ch_used; j++)
                        change += (float)fabs(source_buffer[ndx * step + j] - avg[j]);

                    if (change < threshold_min || change > threshold_max) {
                        memcpy(&buffer[circular_idx * ch_used], &source_buffer[ndx * step], ch_used * sizeof(float));
                    }
                }
            }
            circular_idx = (circular_idx + 1) % buffer_count;
        }
    }
    return true;
}

static bool BitmapFloat_boxblur_rows(flow_c * context, struct flow_bitmap_float * image, uint32_t radius,
                                     uint32_t passes, const uint32_t convolve_channels, float * work_buffer,
                                     uint32_t from_row, int row_count)
{
    const uint32_t buffer_count = radius + 1;
    const uint32_t w = image->w;
    const uint32_t step = image->channels;
    const uint32_t until_row = row_count < 0 ? image->h : from_row + (unsigned)row_count;
    const uint32_t ch_used = image->channels;
    float * __restrict buffer = work_buffer;
    const uint32_t std_count = radius * 2 + 1;
    const float std_factor = 1.0f / (float)(std_count);
    for (uint32_t row = from_row; row < until_row; row++) {
        float * __restrict source_buffer = &image->pixels[row * image->float_stride];
        for (uint32_t pass_index = 0; pass_index < passes; pass_index++) {
            int circular_idx = 0;
            float sum[4] = { 0, 0, 0, 0 };
            uint32_t count = 0;
            for (uint32_t ndx = 0; ndx < radius; ndx++) {
                for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                    sum[ch] += source_buffer[ndx * step + ch];
                }
                count++;
            }
            for (uint32_t ndx = 0; ndx < w + buffer_count; ndx++) { // Pixels
                if (ndx >= buffer_count) { // same as ndx > radius
                    // Remove trailing item from average
                    for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                        sum[ch] -= source_buffer[(ndx - radius - 1) * step + ch];
                    }
                    count--;
                    // Flush old value
                    memcpy(&source_buffer[(ndx - buffer_count) * step], &buffer[circular_idx * ch_used],
                           ch_used * sizeof(float));
                }
                // Calculate and enqueue new value
                if (ndx < w) {
                    if (ndx < w - radius) {
                        for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                            sum[ch] += source_buffer[(ndx + radius) * step + ch];
                        }
                        count++;
                    }
                    // Enqueue averaged value
                    if (count != std_count) {
                        for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                            buffer[circular_idx * ch_used + ch] = sum[ch] / (float)count; // Recompute factor
                        }
                    } else {
                        for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                            buffer[circular_idx * ch_used + ch] = sum[ch] * std_factor;
                        }
                    }
                }
                circular_idx = (circular_idx + 1) % buffer_count;
            }
        }
    }
    return true;
}
static bool BitmapFloat_boxblur_misaligned_rows(flow_c * context, struct flow_bitmap_float * image, uint32_t radius,
                                                int align, const uint32_t convolve_channels, float * work_buffer,
                                                uint32_t from_row, int row_count)
{
    if (align != 1 && align != -1) {
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false;
    }
    const uint32_t buffer_count = radius + 2;
    const uint32_t w = image->w;
    const uint32_t step = image->channels;
    const uint32_t until_row = row_count < 0 ? image->h : from_row + (unsigned)row_count;
    const uint32_t ch_used = image->channels;
    float * __restrict buffer = work_buffer;
    const uint32_t write_offset = align == -1 ? 0 : 1;
    for (uint32_t row = from_row; row < until_row; row++) {
        float * __restrict source_buffer = &image->pixels[row * image->float_stride];
        int circular_idx = 0;
        float sum[4] = { 0, 0, 0, 0 };
        float count = 0;
        for (uint32_t ndx = 0; ndx < radius; ndx++) {
            float factor = (ndx == radius - 1) ? 0.5f : 1;
            for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                sum[ch] += source_buffer[ndx * step + ch] * factor;
            }
            count += factor;
        }
        for (uint32_t ndx = 0; ndx < w + buffer_count - write_offset; ndx++) { // Pixels
            // Calculate new value
            if (ndx < w) {
                if (ndx < w - radius) {
                    for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                        sum[ch] += source_buffer[(ndx + radius) * step + ch] * 0.5f;
                    }
                    count += 0.5f;
                }
                if (ndx < w - radius + 1) {
                    for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                        sum[ch] += source_buffer[(ndx - 1 + radius) * step + ch] * 0.5f;
                    }
                    count += 0.5f;
                }
                // Remove trailing items from average
                if (ndx >= radius) {
                    for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                        sum[ch] -= source_buffer[(ndx - radius) * step + ch] * 0.5f;
                    }
                    count -= 0.5f;
                }
                if (ndx >= radius + 1) {
                    for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                        sum[ch] -= source_buffer[(ndx - 1 - radius) * step + ch] * 0.5f;
                    }
                    count -= 0.5f;
                }
            }
            // Flush old value
            if (ndx >= buffer_count - write_offset) {
                memcpy(&source_buffer[(ndx + write_offset - buffer_count) * step], &buffer[circular_idx * ch_used],
                       ch_used * sizeof(float));
            }
            // enqueue new value
            if (ndx < w) {
                for (uint32_t ch = 0; ch < convolve_channels; ch++) {
                    buffer[circular_idx * ch_used + ch] = sum[ch] / (float)count;
                }
            }
            circular_idx = (circular_idx + 1) % buffer_count;
        }
    }
    return true;
}

uint32_t flow_bitmap_float_approx_gaussian_calculate_d(float sigma, uint32_t bitmap_width)
{
    uint32_t d = (int)floorf(1.8799712059732503768118239636082839397552400554574537f * sigma + 0.5f);
    d = umin(d, (bitmap_width - 1) / 2); // Never exceed half the size of the buffer.
    return d;
}

uint32_t flow_bitmap_float_approx_gaussian_buffer_element_count_required(float sigma, uint32_t bitmap_width)
{
    return flow_bitmap_float_approx_gaussian_calculate_d(sigma, bitmap_width) * 2 + 12; // * sizeof(float);
}
bool flow_bitmap_float_approx_gaussian_blur_rows(flow_c * context, struct flow_bitmap_float * image, float sigma,
                                                 float * buffer, size_t buffer_element_count, uint32_t from_row,
                                                 int row_count)
{
    // Ensure sigma is large enough for approximation to be accurate.
    if (sigma < 2) {
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false;
    }

    // Ensure the buffer is large enough
    if (flow_bitmap_float_approx_gaussian_buffer_element_count_required(sigma, image->w) > buffer_element_count) {
        FLOW_error(context, flow_status_Invalid_internal_state);
        return false;
    }

    // http://www.w3.org/TR/SVG11/filters.html#feGaussianBlur
    // For larger values of 's' (s >= 2.0), an approximation can be used :
    // Three successive box - blurs build a piece - wise quadratic convolution kernel, which approximates the Gaussian
    // kernel to within roughly 3 % .
    uint32_t d = flow_bitmap_float_approx_gaussian_calculate_d(sigma, image->w);
    //... if d is odd, use three box - blurs of size 'd', centered on the output pixel.
    if (d % 2 > 0) {
        if (!BitmapFloat_boxblur_rows(context, image, d / 2, 3, image->channels, buffer, from_row, row_count)) {
            FLOW_error_return(context);
        }
    } else {
        // ... if d is even, two box - blurs of size 'd'
        // (the first one centered on the pixel boundary between the output pixel and the one to the left,
        //  the second one centered on the pixel boundary between the output pixel and the one to the right)
        // and one box blur of size 'd+1' centered on the output pixel.
        if (!BitmapFloat_boxblur_misaligned_rows(context, image, d / 2, -1, image->channels, buffer, from_row,
                                                 row_count)) {
            FLOW_error_return(context);
        }
        if (!BitmapFloat_boxblur_misaligned_rows(context, image, d / 2, 1, image->channels, buffer, from_row,
                                                 row_count)) {
            FLOW_error_return(context);
        }
        if (!BitmapFloat_boxblur_rows(context, image, d / 2 + 1, 1, image->channels, buffer, from_row, row_count)) {
            FLOW_error_return(context);
        }
    }
    return true;
}


FLOW_HINT_HOT FLOW_HINT_UNSAFE_MATH_OPTIMIZATIONS static inline void transpose4x4_SSE(float * A, float * B,
                                                                                      const int lda, const int ldb)
{
    __m128 row1 = _mm_loadu_ps(&A[0 * lda]);
    __m128 row2 = _mm_loadu_ps(&A[1 * lda]);
    __m128 row3 = _mm_loadu_ps(&A[2 * lda]);
    __m128 row4 = _mm_loadu_ps(&A[3 * lda]);
    _MM_TRANSPOSE4_PS(row1, row2, row3, row4);
    _mm_storeu_ps(&B[0 * ldb], row1);
    _mm_storeu_ps(&B[1 * ldb], row2);
    _mm_storeu_ps(&B[2 * ldb], row3);
    _mm_storeu_ps(&B[3 * ldb], row4);
}

FLOW_HINT_HOT
static inline void transpose_block_SSE4x4(float * A, float * B, const int n, const int m, const int lda, const int ldb,
                                          const int block_size)
{
    //#pragma omp parallel for collapse(2)
    for (int i = 0; i < n; i += block_size) {
        for (int j = 0; j < m; j += block_size) {
            int max_i2 = i + block_size < n ? i + block_size : n;
            int max_j2 = j + block_size < m ? j + block_size : m;
            for (int i2 = i; i2 < max_i2; i2 += 4) {
                for (int j2 = j; j2 < max_j2; j2 += 4) {
                    transpose4x4_SSE(&A[i2 * lda + j2], &B[j2 * ldb + i2], lda, ldb);
                }
            }
        }
    }
}

//
// FLOW_HINT_HOT FLOW_HINT_UNSAFE_MATH_OPTIMIZATIONS
// static inline void transpose8x8_AVX(float* mat,  float* matT, const int stride, const int matT_stride) {
//    __m256  r0, r1, r2, r3, r4, r5, r6, r7;
//    __m256  t0, t1, t2, t3, t4, t5, t6, t7;
//
//    r0 = _mm256_insertf128_ps(_mm256_castps128_ps256(_mm_loadu_ps(&mat[0*stride+0])), _mm_loadu_ps(&mat[4*stride+0]),
//    1);
//    r1 = _mm256_insertf128_ps(_mm256_castps128_ps256(_mm_loadu_ps(&mat[1*stride+0])), _mm_loadu_ps(&mat[5*stride+0]),
//    1);
//    r2 = _mm256_insertf128_ps(_mm256_castps128_ps256(_mm_loadu_ps(&mat[2*stride+0])), _mm_loadu_ps(&mat[6*stride+0]),
//    1);
//    r3 = _mm256_insertf128_ps(_mm256_castps128_ps256(_mm_loadu_ps(&mat[3*stride+0])), _mm_loadu_ps(&mat[7*stride+0]),
//    1);
//    r4 = _mm256_insertf128_ps(_mm256_castps128_ps256(_mm_loadu_ps(&mat[0*stride+4])), _mm_loadu_ps(&mat[4*stride+4]),
//    1);
//    r5 = _mm256_insertf128_ps(_mm256_castps128_ps256(_mm_loadu_ps(&mat[1*stride+4])), _mm_loadu_ps(&mat[5*stride+4]),
//    1);
//    r6 = _mm256_insertf128_ps(_mm256_castps128_ps256(_mm_loadu_ps(&mat[2*stride+4])), _mm_loadu_ps(&mat[6*stride+4]),
//    1);
//    r7 = _mm256_insertf128_ps(_mm256_castps128_ps256(_mm_loadu_ps(&mat[3*stride+4])), _mm_loadu_ps(&mat[7*stride+4]),
//    1);
//
//    t0 = _mm256_unpacklo_ps(r0,r1);
//    t1 = _mm256_unpackhi_ps(r0,r1);
//    t2 = _mm256_unpacklo_ps(r2,r3);
//    t3 = _mm256_unpackhi_ps(r2,r3);
//    t4 = _mm256_unpacklo_ps(r4,r5);
//    t5 = _mm256_unpackhi_ps(r4,r5);
//    t6 = _mm256_unpacklo_ps(r6,r7);
//    t7 = _mm256_unpackhi_ps(r6,r7);
//
//    //__m256 v;
//
//    r0 = _mm256_shuffle_ps(t0,t2, 0x44);
//    r1 = _mm256_shuffle_ps(t0,t2, 0xEE);
////    v = _mm256_shuffle_ps(t0,t2, 0x4E);
////    r0 = _mm256_blend_ps(t0, v, 0xCC);
////    r1 = _mm256_blend_ps(t2, v, 0x33);
//
//    r2 = _mm256_shuffle_ps(t1,t3, 0x44);
//    r3 = _mm256_shuffle_ps(t1,t3, 0xEE);
////    v = _mm256_shuffle_ps(t1,t3, 0x4E);
////    r2 = _mm256_blend_ps(t1, v, 0xCC);
////    r3 = _mm256_blend_ps(t3, v, 0x33);
//
//    r4 = _mm256_shuffle_ps(t4,t6, 0x44);
//    r5 = _mm256_shuffle_ps(t4,t6, 0xEE);
////    v = _mm256_shuffle_ps(t4,t6, 0x4E);
////    r4 = _mm256_blend_ps(t4, v, 0xCC);
////    r5 = _mm256_blend_ps(t6, v, 0x33);
//
//    r6 = _mm256_shuffle_ps(t5,t7, 0x44);
//    r7 = _mm256_shuffle_ps(t5,t7, 0xEE);
////    v = _mm256_shuffle_ps(t5,t7, 0x4E);
////    r6 = _mm256_blend_ps(t5, v, 0xCC);
////    r7 = _mm256_blend_ps(t7, v, 0x33);
//
//    _mm256_storeu_ps(&matT[0*matT_stride], r0);
//    _mm256_storeu_ps(&matT[1*matT_stride], r1);
//    _mm256_storeu_ps(&matT[2*matT_stride], r2);
//    _mm256_storeu_ps(&matT[3*matT_stride], r3);
//    _mm256_storeu_ps(&matT[4*matT_stride], r4);
//    _mm256_storeu_ps(&matT[5*matT_stride], r5);
//    _mm256_storeu_ps(&matT[6*matT_stride], r6);
//    _mm256_storeu_ps(&matT[7*matT_stride], r7);
//}
//
//
// FLOW_HINT_HOT
// static inline void transpose_block_AVX8x8(float *A, float *B, const int n, const int m, const int lda, const int ldb
// ,const int block_size) {
////#pragma omp parallel for
//    for(int i=0; i<n; i+=block_size) {
//        for(int j=0; j<m; j+=block_size) {
//            int max_i2 = i+block_size < n ? i + block_size : n;
//            int max_j2 = j+block_size < m ? j + block_size : m;
//            for(int i2=i; i2<max_i2; i2+=8) {
//                for(int j2=j; j2<max_j2; j2+=8) {
//                    transpose8x8_AVX(&A[i2*lda +j2], &B[j2*ldb + i2], lda, ldb);
//                }
//            }
//        }
//    }
//}

FLOW_HINT_HOT FLOW_HINT_UNSAFE_MATH_OPTIMIZATIONS

    bool
    flow_bitmap_bgra_transpose(flow_c * c, struct flow_bitmap_bgra * from, struct flow_bitmap_bgra * to)
{
    if (from->w != to->h || from->h != to->w || from->fmt != to->fmt) {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }

    if (from->fmt != flow_bgra32 && from->fmt != flow_bgr32) {
        if (!flow_bitmap_bgra_transpose_slow(c, from, to)) {
            FLOW_add_to_callstack(c);
            return false;
        }
        return true;
    }

    // We require 8 when we only need 4 - in case we ever want to enable avx (like if we make it faster)
    const int min_block_size = 8;

    // Strides must be multiple of required alignments
    if (from->stride % min_block_size != 0 || to->stride % min_block_size != 0) {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }
    // 256 (1024x1024 bytes) at 18.18ms, 128 at 18.6ms,  64 at 20.4ms, 16 at 25.71ms
    int block_size = 128;

    int cropped_h = from->h - from->h % min_block_size;
    int cropped_w = from->w - from->w % min_block_size;

    transpose_block_SSE4x4((float *)from->pixels, (float *)to->pixels, cropped_h, cropped_w, from->stride / 4,
                           to->stride / 4, block_size);

    // Copy missing bits
    for (uint32_t x = cropped_h; x < to->w; x++) {
        for (uint32_t y = 0; y < to->h; y++) {
            *((uint32_t *)&to->pixels[x * 4 + y * to->stride]) = *((uint32_t *)&from->pixels[x * from->stride + y * 4]);
        }
    }

    for (uint32_t x = 0; x < (uint32_t)cropped_h; x++) {
        for (uint32_t y = cropped_w; y < to->h; y++) {
            *((uint32_t *)&to->pixels[x * 4 + y * to->stride]) = *((uint32_t *)&from->pixels[x * from->stride + y * 4]);
        }
    }

    return true;
}

bool flow_bitmap_bgra_transpose_slow(flow_c * c, struct flow_bitmap_bgra * from, struct flow_bitmap_bgra * to)
{
    if (from->w != to->h || from->h != to->w || from->fmt != to->fmt) {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }

    if (from->fmt == flow_bgra32 || from->fmt == flow_bgr32) {
        for (uint32_t x = 0; x < to->w; x++) {
            for (uint32_t y = 0; y < to->h; y++) {
                *((uint32_t *)&to->pixels[x * 4 + y * to->stride])
                    = *((uint32_t *)&from->pixels[x * from->stride + y * 4]);
            }
        }
        return true;
    } else if (from->fmt == flow_bgr24) {
        int from_stride = from->stride;
        int to_stride = to->stride;
        for (uint32_t x = 0, x_stride = 0, x_3 = 0; x < to->w; x++, x_stride += from_stride, x_3 += 3) {
            for (uint32_t y = 0, y_stride = 0, y_3 = 0; y < to->h; y++, y_stride += to_stride, y_3 += 3) {

                to->pixels[x_3 + y_stride] = from->pixels[x_stride + y_3];
                to->pixels[x_3 + y_stride + 1] = from->pixels[x_stride + y_3 + 1];
                to->pixels[x_3 + y_stride + 2] = from->pixels[x_stride + y_3 + 2];
            }
        }
        return true;
    } else {
        FLOW_error(c, flow_status_Invalid_argument);
        return false;
    }
}
