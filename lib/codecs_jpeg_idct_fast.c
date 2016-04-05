#include <stdio.h>
#include "imageflow_private.h"

#define JPEG_INTERNALS
#include "jpeglib.h"
#include "jdct.h" /* Private declarations for DCT subsystem */
#include "codecs_jpeg.h"
#include "fastapprox.h"

void jpeg_idct_downscale_wrap_islow_fast(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                         JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_downscale_wrap_islow_fast_1x1(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_downscale_wrap_islow_fast_2x2(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_downscale_wrap_islow_fast_3x3(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_downscale_wrap_islow_fast_4x4(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_downscale_wrap_islow_fast_5x5(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_downscale_wrap_islow_fast_6x6(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col);

void jpeg_idct_downscale_wrap_islow_fast_7x7(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col);

//static const float jpeg_scale_to_7_x_7_weights[7][8] = {
//    { 0.9039465785026550293, 0.0960534214973449707, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000,
//      0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000 },
//    { -0.0159397963434457779, 0.8046712279319763184, 0.2112685889005661011, 0.0000000000000000000,
//      0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000 },
//    { 0.0000000000000000000, -0.0307929217815399170, 0.6724552512168884277, 0.3583376407623291016,
//      0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000 },
//    { 0.0000000000000000000, 0.0000000000000000000, -0.0303521882742643356, 0.5303522348403930664,
//      0.5303522348403930664, -0.0303521882742643356, 0.0000000000000000000, 0.0000000000000000000 },
//    { 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.3583376407623291016,
//      0.6724552512168884277, -0.0307929217815399170, 0.0000000000000000000 },
//    { 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000,
//      0.2112685889005661011, 0.8046712279319763184, -0.0159397963434457779 },
//    { 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000,
//      0.0000000000000000000, 0.0960534214973449707, 0.9039465785026550293 },
//};
static const float jpeg_scale_to_6_x_6_weights[6][8] = {
    { 0.7491279840469360352, 0.2508720457553863525, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000,
      0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000 },
    { -0.0224444177001714706, 0.5224443674087524414, 0.5224443674087524414, -0.0224444177001714706,
      0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000 },
    { 0.0000000000000000000, 0.0000000000000000000, 0.2396661937236785889, 0.7156662940979003906, 0.0446674935519695282,
      0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000 },
    { 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0446674935519695282, 0.7156662940979003906,
      0.2396661937236785889, 0.0000000000000000000, 0.0000000000000000000 },
    { 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000,
      -0.0224444177001714706, 0.5224443674087524414, 0.5224443674087524414, -0.0224444177001714706 },
    { 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000,
      0.0000000000000000000, 0.2508720457553863525, 0.7491279840469360352 },
};
static const float jpeg_scale_to_5_x_5_weights[5][8] = {
    { 0.6118828058242797852, 0.4002380371093750000, -0.0121208345517516136, 0.0000000000000000000,
      0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000 },
    { -0.0219573117792606354, 0.2555175423622131348, 0.6176261901855468750, 0.1488135904073715210,
      0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000 },
    { 0.0000000000000000000, 0.0000000000000000000, 0.0161152519285678864, 0.4838847517967224121, 0.4838847517967224121,
      0.0161152519285678864, 0.0000000000000000000, 0.0000000000000000000 },
    { 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.1488135904073715210,
      0.6176261901855468750, 0.2555175423622131348, -0.0219573117792606354 },
    { 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000,
      -0.0121208345517516136, 0.4002380371093750000, 0.6118828058242797852 },
};
static const float jpeg_scale_to_4_x_4_weights[4][8] = {
    { 0.4553350806236267090, 0.4553350806236267090, 0.0893298164010047913, 0.0000000000000000000, 0.0000000000000000000,
      0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000 },
    { 0.0000000000000000000, 0.0820043832063674927, 0.4179956316947937012, 0.4179956316947937012, 0.0820043832063674927,
      0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000 },
    { 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0820043832063674927, 0.4179956316947937012,
      0.4179956316947937012, 0.0820043832063674927, 0.0000000000000000000 },
    { 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000,
      0.0893298164010047913, 0.4553350806236267090, 0.4553350806236267090 },
};
static const float jpeg_scale_to_3_x_3_weights[3][8] = {
    { 0.3126847147941589355, 0.4027547538280487061, 0.2417637705802917480, 0.0427967458963394165, 0.0000000000000000000,
      0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000 },
    { 0.0000000000000000000, 0.0095250364392995834, 0.1524054706096649170, 0.3380694985389709473, 0.3380694985389709473,
      0.1524054706096649170, 0.0095250364392995834, 0.0000000000000000000 },
    { 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0427967458963394165,
      0.2417637705802917480, 0.4027547538280487061, 0.3126847147941589355 },
};
static const float jpeg_scale_to_2_x_2_weights[2][8] = {
    { 0.1866819113492965698, 0.2613925635814666748, 0.2613925635814666748, 0.1866819113492965698, 0.0875365510582923889,
      0.0163145177066326141, 0.0000000000000000000, 0.0000000000000000000 },
    { 0.0000000000000000000, 0.0000000000000000000, 0.0163145177066326141, 0.0875365510582923889, 0.1866819113492965698,
      0.2613925635814666748, 0.2613925635814666748, 0.1866819113492965698 },
};
static const float jpeg_scale_to_1_x_1_weights[1][8] = {
    { 0.0911070853471755981, 0.1178331747651100159, 0.1392842531204223633, 0.1517754793167114258, 0.1517754793167114258,
      0.1392842531204223633, 0.1178331747651100159, 0.0911070853471755981 },
};

#define jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, scaled, summation) \
JSAMPLE result[DCTSIZE2];\
JSAMPROW rows[DCTSIZE] = {&result[0], &result[DCTSIZE], &result[DCTSIZE * 2], &result[DCTSIZE * 3], &result[DCTSIZE * 4], &result[DCTSIZE *5], &result[DCTSIZE * 6], &result[DCTSIZE *7]};\
int i; \
jpeg_idct_islow(cinfo, compptr, coef_block, &rows[0], 0); \
struct flow_job_jpeg_decoder_state * state = (struct flow_job_jpeg_decoder_state *)cinfo->err; \
float linearized[DCTSIZE2];\
for (i = 0; i < DCTSIZE2; i++)\
    linearized[i] = state->lut_to_linear[result[i]]; \
float scaled_h[DCTSIZE2];\
for (int row = 0; row < DCTSIZE; row++) {\
    float * r = &linearized[row * DCTSIZE];\
    float * dest = &scaled_h[row]; \
    summation \
}\
for (int row = 0; row < scaled; row++) {\
    float * r = &scaled_h[row * DCTSIZE]; \
    float * dest = &linearized[row]; \
    summation \
} \
for (int row = 0; row < scaled; row++) {\
    for (int col = 0; col < scaled; col++) {\
        *(output_buf[row] + output_col + col) =  state->flat_lut_linear[(size_t)(linearized[row * DCTSIZE + col] * (sizeof(state->flat_lut_linear) -1))];\
    }\
} \

#define DEFAULT_WEIGHTED_SUM(scaled, matrix) \
  for (int to = 0; to < scaled; to++) {\
        float sum = 0;\
        for (int from = 0; from < DCTSIZE; from++) {\
            sum += matrix[to][from] * r[from];\
        }\
        dest[to * DCTSIZE] = sum; \
    }\

void jpeg_idct_downscale_wrap_islow_fast_1x1(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                         JSAMPARRAY output_buf, JDIMENSION output_col){
    jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, 1, DEFAULT_WEIGHTED_SUM(1, jpeg_scale_to_1_x_1_weights));
}

void jpeg_idct_downscale_wrap_islow_fast_2x2(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col){
    jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, 2, DEFAULT_WEIGHTED_SUM(2, jpeg_scale_to_2_x_2_weights));
}

void jpeg_idct_downscale_wrap_islow_fast_3x3(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col){
    jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, 3, DEFAULT_WEIGHTED_SUM(3, jpeg_scale_to_3_x_3_weights));
}

void jpeg_idct_downscale_wrap_islow_fast_4x4(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col){
    jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, 4, DEFAULT_WEIGHTED_SUM(4, jpeg_scale_to_4_x_4_weights));
}

void jpeg_idct_downscale_wrap_islow_fast_5x5(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col){
    jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, 5, DEFAULT_WEIGHTED_SUM(5, jpeg_scale_to_5_x_5_weights));
}

void jpeg_idct_downscale_wrap_islow_fast_6x6(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col){
    jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, 6, DEFAULT_WEIGHTED_SUM(6, jpeg_scale_to_6_x_6_weights));
}

void jpeg_idct_downscale_wrap_islow_fast_7x7(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col){

#define WEIGHTED_SUM_7x7 \
dest[0 * DCTSIZE] = r[0] * 0.9039465785026550293 + r[1] * 0.0960534214973449707 + 0 + 0 + 0 + 0 + 0 + 0; \
dest[1 * DCTSIZE] = r[0] *-0.0159397963434457779 + r[1] * 0.8046712279319763184 + r[2] * 0.2112685889005661011 + 0 + 0 + 0 + 0 + 0; \
dest[2 * DCTSIZE] = 0 + r[1] * -0.0307929217815399170 +r[2] * 0.6724552512168884277 + r[3] * 0.3583376407623291016 + 0 + 0 + 0 + 0; \
dest[3 * DCTSIZE] = 0 + 0 + r[2] * -0.0303521882742643356 +r[3] * 0.5303522348403930664 + r[4] * 0.5303522348403930664 + r[5] * -0.0303521882742643356 +0 + 0; \
dest[4 * DCTSIZE] = 0 + 0 + 0 + 0 + r[4] * 0.3583376407623291016 + r[5] * 0.6724552512168884277 + r[6] * -0.0307929217815399170 +0; \
dest[5 * DCTSIZE] = 0 + 0 + 0 + 0 + 0 + r[5] * 0.2112685889005661011 + r[6] * 0.8046712279319763184 + r[7] * -0.0159397963434457779; \
dest[6 * DCTSIZE] = 0 + 0 + 0 + 0 + 0 + 0 + r[6] * 0.0960534214973449707 + r[7] * 0.9039465785026550293; \

    jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, 7, WEIGHTED_SUM_7x7);
}


// void jpeg_idct_downscale_wrap_islow_fast(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
//                                              JSAMPARRAY output_buf, JDIMENSION output_col){

// #if JPEG_LIB_VERSION >= 70
//     int scaled = compptr->DCT_h_scaled_size;
// #else
//     int scaled = compptr->DCT_scaled_size;
// #endif


//     jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, scaled, DEFAULT_WEIGHTED_SUM(scaled,&weights_by_target[scaled - 1]));
// }

