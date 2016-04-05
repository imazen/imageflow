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

static const float jpeg_scale_to_7_x_7_weights[7][8] = {
    { 0.9039465785026550293, 0.0960534214973449707, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000,
      0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000 },
    { -0.0159397963434457779, 0.8046712279319763184, 0.2112685889005661011, 0.0000000000000000000,
      0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000 },
    { 0.0000000000000000000, -0.0307929217815399170, 0.6724552512168884277, 0.3583376407623291016,
      0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000 },
    { 0.0000000000000000000, 0.0000000000000000000, -0.0303521882742643356, 0.5303522348403930664,
      0.5303522348403930664, -0.0303521882742643356, 0.0000000000000000000, 0.0000000000000000000 },
    { 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.3583376407623291016,
      0.6724552512168884277, -0.0307929217815399170, 0.0000000000000000000 },
    { 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000,
      0.2112685889005661011, 0.8046712279319763184, -0.0159397963434457779 },
    { 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000, 0.0000000000000000000,
      0.0000000000000000000, 0.0960534214973449707, 0.9039465785026550293 },
};
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

static const float * weights_by_target[7]
    = { &jpeg_scale_to_1_x_1_weights[0][0], &jpeg_scale_to_2_x_2_weights[0][0], &jpeg_scale_to_3_x_3_weights[0][0],
        &jpeg_scale_to_4_x_4_weights[0][0], &jpeg_scale_to_5_x_5_weights[0][0], &jpeg_scale_to_6_x_6_weights[0][0],
        &jpeg_scale_to_7_x_7_weights[0][0]

    };



//static inline void jpeg_idct_downscale_wrap_islow_fast_generic(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
//                                         JSAMPARRAY output_buf, JDIMENSION output_col, int scaled)
//{
//
//    JSAMPLE intermediate[DCTSIZE2];
//    JSAMPROW rows[DCTSIZE];
//    int i;
//
//    for (i = 0; i < DCTSIZE; i++)
//        rows[i] = &intermediate[i * DCTSIZE];
//
//    jpeg_idct_islow(cinfo, compptr, coef_block, &rows[0], 0);
//
//    struct flow_job_jpeg_decoder_state * state = (struct flow_job_jpeg_decoder_state *)cinfo->err;
//
//    // Linearize
//    float linearized[DCTSIZE2];
//    for (i = 0; i < DCTSIZE2; i++)
//        linearized[i] = state->lut_to_linear[intermediate[i]];
//
//    // Scale and transpose
//    float scaled_h[DCTSIZE2];
//    for (int row = 0; row < DCTSIZE; row++) {
//        float * linearized_row = &linearized[row * DCTSIZE];
//        for (int to = 0; to < scaled; to++) {
//            const float * weights = weights_by_target[scaled - 1] + DCTSIZE * to;
//            float sum = 0;
//            for (int from = 0; from < DCTSIZE; from++) {
//                sum += weights[from] * linearized_row[from];
//            }
//            scaled_h[to * DCTSIZE + row] = sum;
//        }
//    }
//    // Scale and transpose again
//    for (int row = 0; row < scaled; row++) {
//        for (int to = 0; to < scaled; to++) {
//            const float * weights = weights_by_target[scaled - 1] + DCTSIZE * to;
//            float sum = 0;
//            for (int from = 0; from < DCTSIZE; from++) {
//                sum += weights[from] * scaled_h[row * DCTSIZE + from];
//            }
//            *(output_buf[to] + output_col + row) =  state->flat_lut_linear[(size_t)(sum * (sizeof(state->flat_lut_linear) -1))];
//        }
//    }
//
//}

#define jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, scaled, weight_matrix) \
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
    float * linearized_row = &linearized[row * DCTSIZE];\
    for (int to = 0; to < scaled; to++) {\
        float sum = 0;\
        for (int from = 0; from < DCTSIZE; from++) {\
            sum += weight_matrix[to][from] * linearized_row[from];\
        }\
        scaled_h[to * DCTSIZE + row] = sum;\
    }\
}\
for (int row = 0; row < scaled; row++) {\
    float * transposed_row = &scaled_h[row * DCTSIZE]; \
    for (int to = 0; to < scaled; to++) {\
        float sum = 0;\
        for (int from = 0; from < DCTSIZE; from++) {\
            sum += weight_matrix[to][from] * transposed_row[from];\
        }\
        *(output_buf[to] + output_col + row) =  state->flat_lut_linear[(size_t)(sum * (sizeof(state->flat_lut_linear) -1))];\
    }\
}

void jpeg_idct_downscale_wrap_islow_fast_1x1(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                         JSAMPARRAY output_buf, JDIMENSION output_col){
    jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, 1, jpeg_scale_to_1_x_1_weights);
}

void jpeg_idct_downscale_wrap_islow_fast_2x2(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col){
    jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, 2, jpeg_scale_to_2_x_2_weights);
}

void jpeg_idct_downscale_wrap_islow_fast_3x3(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col){
    jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, 3, jpeg_scale_to_3_x_3_weights);
}

void jpeg_idct_downscale_wrap_islow_fast_4x4(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col){
    jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, 4, jpeg_scale_to_4_x_4_weights);
}

void jpeg_idct_downscale_wrap_islow_fast_5x5(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col){
    jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, 5, jpeg_scale_to_5_x_5_weights);
}

void jpeg_idct_downscale_wrap_islow_fast_6x6(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col){
    jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, 6, jpeg_scale_to_6_x_6_weights);
}

void jpeg_idct_downscale_wrap_islow_fast_7x7(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col){
    jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, 7, jpeg_scale_to_7_x_7_weights);
}


void jpeg_idct_downscale_wrap_islow_fast(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                             JSAMPARRAY output_buf, JDIMENSION output_col){

#if JPEG_LIB_VERSION >= 70
    int scaled = compptr->DCT_h_scaled_size;
#else
    int scaled = compptr->DCT_scaled_size;
#endif


    jpeg_idct_downscale_wrap_islow_fast_generic(cinfo, compptr, coef_block, output_buf, output_col, scaled, (&weights_by_target[scaled - 1]));
}

