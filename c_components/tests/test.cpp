#include "catch.hpp"
#include "helpers.h"

extern "C" void keep2() {}

struct flow_bitmap_bgra * crop_window(flow_c * context, struct flow_bitmap_bgra * source, uint32_t x, uint32_t y,
                                      uint32_t w, uint32_t h)
{
    struct flow_bitmap_bgra * cropped = flow_bitmap_bgra_create_header(context, w, h);
    cropped->fmt = source->fmt;
    const uint32_t bytes_pp = flow_pixel_format_bytes_per_pixel(source->fmt);
    cropped->pixels = source->pixels + (y * source->stride) + (x * bytes_pp);
    cropped->stride = source->stride;
    return cropped;
}

void fill_rect(flow_c * context, struct flow_bitmap_bgra * b, uint32_t x, uint32_t y, uint32_t w, uint32_t h,
               uint8_t fill_red, uint8_t fill_green, uint8_t fill_blue, uint8_t fill_alpha)
{
    flow_bitmap_bgra_fill_rect(context, b, x, y, x + w, y + h,
                               (fill_blue << 24) | (fill_red << 16) | (fill_green << 8) | fill_alpha);
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

TEST_CASE("Test gaussian blur approximation.", "[fastscaling]")
{
    flow_c context;
    flow_context_initialize(&context);

    float sigma = 2.0;

    // We figure this out just for the actual gaussian function
    int kernel_radius
        = int_max(1, (int)ceil(sigma * 3.11513411073090629014797467185716068837128426554157826035269
                               - 0.5)); // Should provide at least 7 bits of precision, and almost always 8.

    uint32_t bitmap_width = 300;
    uint32_t d = flow_bitmap_float_approx_gaussian_calculate_d(sigma, bitmap_width);

    uint32_t buffer_elements = flow_bitmap_float_approx_gaussian_buffer_element_count_required(sigma, bitmap_width);

    float * buffer = FLOW_calloc_array(&context, buffer_elements, float);
    CHECK_FALSE(buffer == NULL);

    // Preferably test premultiplication

    struct flow_bitmap_float * image = flow_bitmap_float_create(&context, bitmap_width, 1, 4, true);
    CHECK_FALSE(image == NULL);

    for (uint32_t i = 0; i < image->w * 4; i++) {

        image->pixels[i] = (float)((i % 8 == 0 ? 0.5 : 0) + (i % 12 == 0 ? 0.4 : 0.1));
    }
    struct flow_bitmap_float * image_b = flow_bitmap_float_create(&context, bitmap_width, 1, 4, true);
    memcpy(image_b->pixels, image->pixels, image->float_stride * sizeof(float));

    CHECK(flow_bitmap_float_approx_gaussian_blur_rows(&context, image, sigma, buffer, buffer_elements, 0, 1));

    struct flow_convolution_kernel * gaussian
        = flow_convolution_kernel_create_gaussian_normalized(&context, sigma, kernel_radius);
    CHECK(gaussian != NULL);
    CHECK(flow_bitmap_float_convolve_rows(&context, image_b, gaussian, 4, 0, 1));

    // Compare image_a and image_b
    float max_delta = 0;
    double avg_delta = flow_bitmap_float_compare(&context, image, image_b, &max_delta);

    CHECK(max_delta < 0.12);
    CHECK(avg_delta < 0.03);

    REQUIRE(flow_context_begin_terminate(&context) == true);
    flow_context_end_terminate(&context);
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
