/*
 * Copyright (c) Imazen LLC.
 * No part of this project, including this file, may be copied, modified,
 * propagated, or distributed except as permitted in COPYRIGHT.txt.
 * Licensed under the GNU Affero General Public License, Version 3.0.
 * Commercial licenses available at http://imageresizing.net/
 */
#pragma once

#ifdef _MSC_VER
#pragma unmanaged
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef struct BitmapFloatStruct BitmapFloat;

typedef struct BitmapBgraStruct BitmapBgra;

typedef struct ContextStruct Context;



static inline float
linear_to_srgb(float clr)
{
    // Gamma correction
    // http://www.4p8.com/eric.brasseur/gamma.html#formulas

    if (clr <= 0.0031308f)
        return 12.92f * clr * 255.0f;

    // a = 0.055; ret ((1+a) * s**(1/2.4) - a) * 255
    return 1.055f  * 255.0f * ((float)pow(clr, 0.41666666f)) - 14.025f;
}


static inline float
srgb_to_linear(float s)
{
    if (s <= 0.04045f)
        return s / 12.92f;
    else
        return (float)pow((s + 0.055f) / (1 + 0.055f), 2.4f);
}

static inline float remove_gamma (Context * context, float value)
{
    return (float)pow (value, context->colorspace.gamma);
}


static inline float apply_gamma (Context * context, float value)
{
    return (float)pow (value, context->colorspace.gamma_inverse);
}

#ifdef EXPOSE_SIGMOID

//y = -(x * 2 - 1) / (abs(x * 2 - 1) - 1.2)
static inline float sigmoid (const SigmoidInfo * info, float x){
    //k = r / (abs(r) + z)
    //r = x * a + b
    //k = (x * a + b) / (abs(x * a + b) + z)
    //y = c * k + d;
    const float r = x * info->x_coeff + info->x_offset;
    return (float)(info->y_coeff * (r / (fabs (r) + info->constant)) + info->y_offset);
}

static inline float sigmoid_inverse (const SigmoidInfo * info, float y){
    //x = (b (-k)+b-k z)/(a (k-1))
    // x = (b (-k) - b + k z) / (a (k + 1))
    const float k = (y - info->y_offset) / info->y_coeff;

    const float signed_k = (info->constant < 0) != (k < 0) ? k : -k;

     //r = k * info->constant / (1 + signed_k)

    return ((k * info->constant / (1 + signed_k)) - info->x_offset) / info->x_coeff;
}

#endif

static inline float Context_srgb_to_floatspace_uncached (Context * context, uint8_t value){
    float v = ((float)value) * (float)(1.0f / 255.0f);
    if (context->colorspace.apply_srgb) v = srgb_to_linear (v);
    else if (context->colorspace.apply_gamma) v = remove_gamma (context, v);
#ifdef EXPOSE_SIGMOID
    if (context->colorspace.apply_sigmoid) v = sigmoid (&context->colorspace.sigmoid, v);
#endif
    return v;
}

static inline float Context_srgb_to_floatspace (Context * context, uint8_t value){
    //if (!context->colorspace.apply_srgb) return Context_srgb_to_floatspace_uncached (context,value);
    // return context->colorspace.floatspace == Floatspace_as_is ? (value * (1.f/255.f)) :    context->colorspace.byte_to_float[value];
    return  context->colorspace.byte_to_float[value]; //2x faster, even if just multiplying by 1/255. 3x faster than the entire calculation.
}

static inline uint8_t Context_floatspace_to_srgb (Context * context, float space_value){
    float v = space_value;
#ifdef EXPOSE_SIGMOID
    v = context->colorspace.apply_sigmoid ? sigmoid_inverse (&context->colorspace.sigmoid, v) : v;
#endif

    if (context->colorspace.apply_gamma) return uchar_clamp_ff(apply_gamma (context, v) * 255.0f);
    if (context->colorspace.apply_srgb) return uchar_clamp_ff (linear_to_srgb (v));
    return uchar_clamp_ff(255.0f * v);
}




static inline void linear_to_yxz(float * bgr)
{

    const float R = bgr[2];
    const float G = bgr[1];
    const float B = bgr[0];

    bgr[0] = 0.212671f*R + 0.71516f *G + 0.072169f*B; //Y
    bgr[1] = 0.412453f*R + 0.35758f *G + 0.180423f*B; //X
    bgr[2] = 0.019334f*R + 0.119193f*G + 0.950227f*B; //Z

}

static inline void linear_to_luv(float * bgr)
{
    //Observer= 2�, Illuminant= D65

    const float xn = 0.312713f;
    const float yn = 0.329016f;
    const float Yn = 1.0f;
    const float un = 4 * xn / (-2 * xn + 12 * yn + 3);
    const float vn = 9 * yn / (-2 * xn + 12 * yn + 3);
    const float y_split = 0.00885645f;
    const float y_adjust = 903.3f;

    const float R = bgr[2];
    const float G = bgr[1];
    const float B = bgr[0];

    if (R == 0 && G == 0 && B == 0) {
        bgr[0] = 0;
        bgr[1] = bgr[2] = 100;
        return;
    }

    const float X = 0.412453f*R + 0.35758f *G + 0.180423f*B;
    const float Y = 0.212671f*R + 0.71516f *G + 0.072169f*B;
    const float Z = 0.019334f*R + 0.119193f*G + 0.950227f*B;



    const float Yd = Y / Yn;

    const float u = 4 * X / (X + 15 * Y + 3 * Z);
    const float v = 9 * Y / (X + 15 * Y + 3 * Z);
    const float L = bgr[0] /* L */ = Yd > y_split ? (116 * ((float)pow(Yd, (float)(1.0f / 3.0f))) - 16) : y_adjust * Yd;
    bgr[1]/* U */ = 13 * L*(u - un) + 100;
    bgr[2] /* V */ = 13 * L*(v - vn) + 100;
}

static inline void luv_to_linear(float * luv)
{
    //D65 white point :
    const float L = luv[0];
    const float U = luv[1] - 100.0f;
    const float V = luv[2] - 100.0f;
    if (L == 0) {
        luv[0] = luv[1] = luv[2] = 0;
        return;
    }

    const float xn = 0.312713f;
    const float yn = 0.329016f;
    const float Yn = 1.0f;
    const float un = 4 * xn / (-2 * xn + 12 * yn + 3);
    const float vn = 9 * yn / (-2 * xn + 12 * yn + 3);
    const float y_adjust_2 = 0.00110705645f;

    const float u = U / (13 * L) + un;
    const float v = V / (13 * L) + vn;
    const float Y = L > 8 ? Yn * ((float)pow((L + 16) / 116, 3)) : Yn * L * y_adjust_2;
    const float X = (9 / 4.0f) * Y * u / v;// -9 * Y * u / ((u - 4) * v - u * v) = (9 / 4) * Y * u / v;
    const float Z = (9 * Y - 15 * v * Y - v * X) / (3 * v);


    const float r = 3.240479f*X - 1.53715f *Y - 0.498535f*Z;
    const float g = -0.969256f*X + 1.875991f*Y + 0.041556f*Z;
    const float b = 0.055648f*X - 0.204043f*Y + 1.057311f*Z;
    luv[0] = b;
    luv[1] = g;
    luv[2] = r;

}

static inline void yxz_to_linear(float * yxz)
{
    //D65 white point :
    const float Y = yxz[0];
    const float X = yxz[1];
    const float Z = yxz[2];

    yxz[2] = 3.240479f*X - 1.53715f *Y - 0.498535f*Z; //r
    yxz[1] = -0.969256f*X + 1.875991f*Y + 0.041556f*Z; //g
    yxz[0] = 0.055648f*X - 0.204043f*Y + 1.057311f*Z; //b

}

bool BitmapFloat_linear_to_luv_rows(Context * context, BitmapFloat * bit, const uint32_t start_row, const  uint32_t row_count);
bool BitmapFloat_luv_to_linear_rows(Context * context, BitmapFloat * bit, const uint32_t start_row, const  uint32_t row_count);


bool BitmapFloat_apply_color_matrix(Context * context, BitmapFloat * bmp, const uint32_t row, const uint32_t count, float*  m[5]);
bool BitmapBgra_apply_color_matrix(Context * context, BitmapBgra * bmp, const uint32_t row, const uint32_t count, float* const __restrict  m[5]);


#ifdef __cplusplus
}
#endif

