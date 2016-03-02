
#include "catch.hpp"

#include "fastscaling_private.h"
#include "weighting_test_helpers.h"
#include "trim_whitespace.h"
#include "string.h"
#include "helpers.h"


bool test(int sx, int sy, flow_pixel_format sbpp, int cx, int cy, flow_pixel_format cbpp, bool transpose, bool flipx,
          bool flipy, bool profile, flow_interpolation_filter filter)
{
    flow_context context;
    flow_context_initialize(&context);

    flow_bitmap_bgra* source = flow_bitmap_bgra_create(&context, sx, sy, true, sbpp);
    flow_bitmap_bgra* canvas = flow_bitmap_bgra_create(&context, cx, cy, true, cbpp);
    if (canvas == NULL || source == NULL)
        return false;
    flow_RenderDetails* details = flow_RenderDetails_create_with(&context, filter);
    if (details == NULL)
        return false;
    details->sharpen_percent_goal = 50;
    details->post_flip_x = flipx;
    details->post_flip_y = flipy;
    details->post_transpose = transpose;
    details->enable_profiling = profile;

    // Should we even have Renderer_* functions, or just 1 call that does it all?
    // If we add memory use estimation, we should keep flow_Renderer

    if (!flow_RenderDetails_render(&context, details, source, canvas)) {
        return false;
    }

    flow_context_terminate(&context);
    return true;
}

bool test_in_place(int sx, int sy, flow_pixel_format sbpp, bool flipx, bool flipy, bool profile, float sharpen,
                   uint32_t kernelRadius)
{
    flow_context context;
    flow_context_initialize(&context);
    flow_bitmap_bgra* source = flow_bitmap_bgra_create(&context, sx, sy, true, sbpp);

    flow_RenderDetails* details = flow_RenderDetails_create(&context);

    details->sharpen_percent_goal = sharpen;
    details->post_flip_x = flipx;
    details->post_flip_y = flipy;
    details->enable_profiling = profile;
    if (kernelRadius > 0) {
        details->kernel_a = flow_convolution_kernel_create_gaussian_normalized(&context, 1.4, kernelRadius);
    }

    flow_RenderDetails_render_in_place(&context, details, source);

    flow_context_terminate(&context);
    return true;
}

const flow_interpolation_filter DEFAULT_FILTER = flow_interpolation_filter_Robidoux;

TEST_CASE("Render without crashing", "[fastscaling]")
{
    REQUIRE(test(400, 300, flow_bgra32, 200, 40, flow_bgra32, false, false, false, false, DEFAULT_FILTER));
}

TEST_CASE("Render - upscale", "[fastscaling]")
{
    REQUIRE(test(200, 40, flow_bgra32, 500, 300, flow_bgra32, false, false, false, false, DEFAULT_FILTER));
}

TEST_CASE("Render - downscale 24->32", "[fastscaling]")
{
    REQUIRE(test(400, 200, flow_bgr24, 200, 100, flow_bgra32, false, false, false, false, DEFAULT_FILTER));
}

TEST_CASE("Render and rotate", "[fastscaling]")
{
    REQUIRE(test(200, 40, flow_bgra32, 500, 300, flow_bgra32, true, true, true, false, DEFAULT_FILTER));
}

TEST_CASE("Render and rotate with profiling", "[fastscaling]")
{
    REQUIRE(test(200, 40, flow_bgra32, 500, 300, flow_bgra32, true, true, true, true, DEFAULT_FILTER));
}

TEST_CASE("Flip in place", "[fastscaling]") { REQUIRE(test_in_place(400, 300, flow_bgra32, true, true, false, 0, 0)); }
TEST_CASE("Flip in place 24 bit", "[fastscaling]")
{
    REQUIRE(test_in_place(400, 300, flow_bgr24, true, true, false, 0, 0));
}
// segfaults the process
TEST_CASE("Sharpen and convolve in place", "[fastscaling]")
{
    REQUIRE(test_in_place(400, 300, flow_bgr24, false, false, false, 0.5, 0));
}
//*/

flow_bitmap_bgra* crop_window(flow_context* context, flow_bitmap_bgra* source, uint32_t x, uint32_t y, uint32_t w,
                              uint32_t h)
{
    flow_bitmap_bgra* cropped = flow_bitmap_bgra_create_header(context, w, h);
    cropped->fmt = source->fmt;
    const uint32_t bytes_pp = flow_pixel_format_bytes_per_pixel(source->fmt);
    cropped->pixels = source->pixels + (y * source->stride) + (x * bytes_pp);
    cropped->stride = source->stride;
    return cropped;
}

void clear_bitmap(flow_bitmap_bgra* b, uint8_t fill_red, uint8_t fill_green, uint8_t fill_blue, uint8_t fill_alpha)
{
    const uint32_t bytes_pp = flow_pixel_format_bytes_per_pixel(b->fmt);
    const uint32_t row_bytes = bytes_pp * b->w;
    for (uint32_t i = 0; i < row_bytes; i += 4) {
        b->pixels[i] = fill_blue;
        b->pixels[i + 1] = fill_green;
        b->pixels[i + 2] = fill_red;
        if (bytes_pp == 4) {
            b->pixels[i + 3] = fill_alpha;
        }
    }
    for (uint32_t i = 0; i < b->h; i++) {
        memcpy(b->pixels + (i * b->stride), b->pixels, row_bytes);
    }
}

void fill_rect(flow_context* context, flow_bitmap_bgra* b, uint32_t x, uint32_t y, uint32_t w, uint32_t h,
               uint8_t fill_red, uint8_t fill_green, uint8_t fill_blue, uint8_t fill_alpha)
{
    flow_bitmap_bgra* cropped = crop_window(context, b, x, y, w, h);
    clear_bitmap(cropped, fill_red, fill_green, fill_blue, fill_alpha);
    flow_bitmap_bgra_destroy(context, cropped);
}

/*/ segfaults the process if you uncomment this
TEST_CASE ("Trim whitespace in 32-bit image", "[fastscaling]") {
    flow_bitmap_bgra* b = create_bitmap_bgra (200, 150, true, flow_bgra32);

    fill_rect (b, 30, 20, 100, 75, 30, 30, 30, 255);

    Rect r = detect_content (b, 20);

    CHECK (r.x1 == 30);
    CHECK (r.y1 == 20);
    CHECK (r.x2 == 20 + 75);
    CHECK (r.y2 == 30 + 100);

    destroy_bitmap_bgra (b);
}

//*/

TEST_CASE("Test contrib windows", "[fastscaling]")
{

    char msg[256];

    flow_context context;
    flow_context_initialize(&context);

    bool r = test_contrib_windows(&context, msg);

    if (!r)
        FAIL(msg);
    REQUIRE(r);
    flow_context_terminate(&context);
}

TEST_CASE("Test Weighting", "[fastscaling]")
{

    char msg[256];

    flow_context context;
    flow_context_initialize(&context);

    // These have window = 1, and shouldnt' have negative values. They should also end at 1
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Hermite, msg, 0, 0, 0.99, 0.08, 1)
          == nullptr);
    // Also called a linear filter
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Triangle, msg, 0, 0, 0.99, 0.08, 1)
          == nullptr);
    // Box should only return a value from -0.5..0.5
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Box, msg, 0, 0, 0.51, 0.001, 0.51)
          == nullptr);

    // These should go negative between x=1 and x=2, but should end at x=2
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_CatmullRom, msg, 1, 2, 1, 0.08, 2)
          == nullptr);
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_CubicFast, msg, 1, 2, 1, 0.08, 2)
          == nullptr);
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Cubic, msg, 1, 2, 1, 0.08, 2)
          == nullptr);

    // BSpline is a smoothing filter, always positive
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_CubicBSpline, msg, 0, 0, 1.75,
                      0.08, 2) == nullptr);

    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Mitchell, msg, 1.0f, 1.75f, 1,
                      0.08, 1.75) == nullptr);

    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Robidoux, msg, 1, 1.65, 1, 0.08,
                      1.75) == nullptr);
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_RobidouxSharp, msg, 1, 1.8, 1,
                      0.08, 1.8) == nullptr);

    // Sinc filters. These have second crossings.
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_RawLanczos2, msg, 1, 2, 1, 0.08, 2)
          == nullptr);
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_RawLanczos2Sharp, msg, 0.954, 1.86,
                      1, 0.08, 2) == nullptr);

    // These should be negative between x=1 and x=2, positive between 2 and 3, but should end at 3

    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_RawLanczos3, msg, 1, 2, 1, 0.1, 3)
          == nullptr);
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_RawLanczos3Sharp, msg, 0.98,
                      1.9625, 1, 0.1, 3) == nullptr);

    ///
    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Lanczos2, msg, 1, 2, 1, 0.08, 2)
          == nullptr);

    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Lanczos2Sharp, msg, 0.954, 1.86, 1,
                      0.08, 2) == nullptr);

    // These should be negative between x=1 and x=2, positive between 2 and 3, but should end at 3

    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_Lanczos, msg, 1, 2, 1, 0.1, 3)
          == nullptr);

    CHECK(test_filter(&context, flow_interpolation_filter::flow_interpolation_filter_LanczosSharp, msg, 0.98, 1.9625, 1,
                      0.1, 2.943) == nullptr);

    flow_context_terminate(&context);
}

TEST_CASE("Test Linear RGB 000 -> LUV ", "[fastscaling]")
{
    float bgra[4] = { 0, 0, 0, 0 };

    linear_to_luv(bgra);

    CHECK(bgra[0] == 0.0f);
    CHECK(bgra[1] == 100.0f);
    CHECK(bgra[2] == 100.0f);
}

TEST_CASE("Roundtrip RGB<->LUV 0.2,0.2,0.2 ", "[fastscaling]")
{
    float bgra[4] = { 0.2f, 0.2f, 0.2f, 1 };

    linear_to_luv(bgra);
    luv_to_linear(bgra);

    CHECK(bgra[0] == Approx(0.2f));
    CHECK(bgra[1] == Approx(0.2f));
    CHECK(bgra[2] == Approx(0.2f));
}

TEST_CASE("Roundtrip sRGB<->linear RGB<->LUV", "[fastscaling]")
{
    for (int x = 0; x < 256; x++) {
        CHECK(x == uchar_clamp_ff(linear_to_srgb(srgb_to_linear((float)x / 255.0f))));
    }
}

TEST_CASE("Roundtrip RGB<->LUV 0,0,0,0 ", "[fastscaling]")
{
    float bgra[4] = { 0, 0, 0, 0 };

    linear_to_luv(bgra);
    luv_to_linear(bgra);

    CHECK(bgra[0] == 0.0f);
    CHECK(bgra[1] == 0.0f);
    CHECK(bgra[2] == 0.0f);
}

TEST_CASE("Test guassian blur approximation.", "[fastscaling]")
{
    flow_context context;
    flow_context_initialize(&context);


    float sigma = 2.0;

    //We figure this out just for the actual guassian function
    int kernel_radius = max(1,(int)ceil (sigma * 3.11513411073090629014797467185716068837128426554157826035269 - 0.5)); //Should provide at least 7 bits of precision, and almost always 8.


    uint32_t bitmap_width = 300;
    uint32_t d =  flow_bitmap_float_approx_gaussian_calculate_d( sigma,  bitmap_width);

    uint32_t buffer_elements = flow_bitmap_float_approx_gaussian_buffer_element_count_required( sigma,  bitmap_width);

    float * buffer = FLOW_calloc_array(&context, buffer_elements, float);
    CHECK_FALSE (buffer == NULL);

    //Preferably test premultiplication

    flow_bitmap_float* image = flow_bitmap_float_create(&context, bitmap_width, 1, 4, true);
    CHECK_FALSE (image == NULL);

    for (int i =0; i < image->w * 4; i++){

        image->pixels[i] = (i % 8 == 0 ? 0.5 : 0) +  (i % 12 == 0 ? 0.4 : 0.1);
    }
    flow_bitmap_float* image_b = flow_bitmap_float_create(&context, bitmap_width, 1, 4, true);
    memcpy(image_b->pixels, image->pixels, image->float_stride * sizeof(float));

    CHECK(flow_bitmap_float_approx_gaussian_blur_rows(&context, image,  sigma,buffer,  buffer_elements, 0, 1));


    flow_convolution_kernel* gaussian = flow_convolution_kernel_create_gaussian_normalized(&context, sigma, kernel_radius);
    CHECK(gaussian != NULL);
    CHECK(flow_bitmap_float_convolve_rows(&context, image_b, gaussian, 4, 0, 1));

    //Compare image_a and image_b
    float max_delta = 0;
    double avg_delta = flow_bitmap_float_compare(&context, image, image_b, &max_delta);

    CHECK(max_delta < 0.12);
    CHECK(avg_delta < 0.03);


    flow_context_terminate(&context);
}
/*
If we need to research fixed point, convert this test
//Looks like we need an 11 bit integer to safely store a sRGB byte in linear form.

int skipfirst = 0; //Skip the first N values, as if we will handle those individually with branches.
for (int bits = 16; bits > 11; bits--){
    array<Byte, 1>^ from = gcnew array<Byte, 1> (257);
    array<Byte, 1>^ to = gcnew array<Byte, 1> (257);

    for (int x = skipfirst - 1; x < 256; x++){
        float linear = srgb_to_linear (x / 255.0f);

        int divisor = (int)pow (2.0, bits);

        int rounded = lround (linear * divisor);

        from[x + 1] = x;
        to[x + 1] = uchar_clamp_ff (linear_to_srgb ((float)rounded / (float)divisor));
    }
    from[0] = bits;
    to[0] = bits;

    Assert::Equal (from, to);
}*/

SCENARIO("sRGB roundtrip", "[fastscaling]")
{
    GIVEN("A 256x256 image, grayscale gradient along the x axis, alpha along the y")
    {
        int w = 256;
        int h = 256;
        flow_context context;
        flow_context_initialize(&context);
        flow_bitmap_bgra* bit = flow_bitmap_bgra_create(&context, w, h, true, flow_bgra32);
        const uint32_t bytes_pp = flow_pixel_format_bytes_per_pixel(bit->fmt);

        for (size_t y = 1; y < bit->h; y++) {
            for (size_t x = 0; x < bit->w; x++) {
                uint8_t* pix = bit->pixels + (y * bit->stride) + (x * bytes_pp);

                *pix = (uint8_t)x;
                *(pix + 1) = (uint8_t)x;
                *(pix + 2) = (uint8_t)x;
                *(pix + 3) = (uint8_t)y;
            }
        }

        flow_bitmap_bgra* final = flow_bitmap_bgra_create(&context, w, h, true, flow_bgra32);
        // flow_bitmap_float* buf = create_bitmap_float(w, h, 4, true);

        WHEN("we do stuff")
        {

            flow_RenderDetails* details = flow_RenderDetails_create(&context);
            CHECK(flow_RenderDetails_render(&context, details, bit, final));

            // convert_srgb_to_linear(bit, 0, buf, 0, h);
            // demultiply_alpha(buf, 0, h);
            // copy_linear_over_srgb(buf, 0, final, 0, h, 0, buf->w, false);

            THEN(" and so forth ")
            {

                bool exact_match = true;
                for (size_t y = 0; y < bit->h; y++) {
                    for (size_t x = 0; x < bit->w; x++) {
                        uint8_t* from = bit->pixels + (y * bit->stride) + (x * bytes_pp);
                        uint8_t* to = final->pixels + (y * final->stride) + (x * bytes_pp);

                        if (*from != *to)
                            exact_match = false;
                        from++;
                        to++;
                        if (*from != *to)
                            exact_match = false;
                        from++;
                        to++;
                        if (*from != *to)
                            exact_match = false;
                        from++;
                        to++;
                        if (*from != *to)
                            exact_match = false;
                        from++;
                        to++;
                    }
                }
                REQUIRE(exact_match);
            }
            flow_RenderDetails_destroy(&context, details);
        }
        flow_bitmap_bgra_destroy(&context, final);
        flow_bitmap_bgra_destroy(&context, bit);
        flow_context_terminate(&context);
    }
}
