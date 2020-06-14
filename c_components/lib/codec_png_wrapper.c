#include "codec_png_wrapper.h"

struct wrap_png_decoder_state {
    png_structp png_ptr;
    png_infop info_ptr;
    wrap_png_error_handler error_handler;
    wrap_png_custom_read_function read_function;
    void * custom_state;
    jmp_buf error_handler_jmp;
    png_size_t w;
    png_size_t h;
    int color_type;
    int bit_depth;
    bool alpha_used;
    struct flow_decoder_color_info color;
};

size_t wrap_png_decoder_state_bytes(){
    return sizeof(struct wrap_png_decoder_state);
}

static void wrap_png_decoder_error_handler(png_structp png_ptr, png_const_charp msg)
{
    struct wrap_png_decoder_state * state = (struct wrap_png_decoder_state *)png_get_error_ptr(png_ptr);

    state->error_handler(png_ptr, state->custom_state, msg);

    //TODO: determine if we should do cleanup here or leave it to rust code

    /* Return control to the setjmp point */
    longjmp(state->error_handler_jmp, 1);
}

static void wrap_png_custom_read_data(png_structp png_ptr, png_bytep buffer, png_size_t bytes_requested)
{
    struct wrap_png_decoder_state * state = (struct wrap_png_decoder_state *)png_get_io_ptr(png_ptr);

    if (state == NULL || state->custom_state == NULL) {
        png_error(png_ptr, "Read Error - state or custom_state null");
    }

    if (state->read_function == NULL) {
        png_error(png_ptr, "Read Error - read_function null");
    }

    size_t bytes_read = 0;

    if (!state->read_function(png_ptr, state->custom_state, buffer, bytes_requested, &bytes_read)){
        png_error(png_ptr, "Read error in read_function callback");
    }

    if (bytes_read != bytes_requested) {
        png_error(png_ptr, "Read beyond end of data requested");
    }
}

bool wrap_png_decoder_state_init(struct wrap_png_decoder_state * state, void * custom_state,
                                        wrap_png_error_handler error_handler, wrap_png_custom_read_function read_function)
{
    memset(state, 0, sizeof(struct wrap_png_decoder_state));
    state->custom_state = custom_state;
    state->read_function = read_function;
    state->error_handler = error_handler;
    flow_decoder_color_info_init(&state->color);
    return true;
}


static bool wrap_png_decoder_load_color_profile(struct wrap_png_decoder_state * state)
{

    // Get gamma
    if (!png_get_valid(state->png_ptr, state->info_ptr, PNG_INFO_sRGB)) {
        png_get_gAMA(state->png_ptr, state->info_ptr, &state->color.gamma);
    }

    // We assume that the underlying buffer can be freed after opening the profile, per
    // http://www.littlecms.com/1/jpegemb.c

    png_bytep profile_buf;
    uint32_t profile_length;

    // Pre-transform color_type (prior to all pre-decode format transforms)
    int is_color_png = state->color_type & PNG_COLOR_MASK_COLOR;

    if (png_get_iCCP(state->png_ptr, state->info_ptr, &(png_charp){ 0 }, &(int){ 0 }, &profile_buf, &profile_length) && profile_length > 0) {

            state->color.profile_buf = profile_buf;
            state->color.buf_length = profile_length;

            if (is_color_png) {
                state->color.source = flow_codec_color_profile_source_ICCP;
            } else {
                state->color.source = flow_codec_color_profile_source_ICCP_GRAY;
            }

    }else if(is_color_png && !png_get_valid(state->png_ptr, state->info_ptr, PNG_INFO_sRGB)
             && png_get_valid(state->png_ptr, state->info_ptr, PNG_INFO_gAMA)
             && png_get_valid(state->png_ptr, state->info_ptr, PNG_INFO_cHRM)) {

        // Use cHRM and gAMA to build profile (later)
        png_get_cHRM(state->png_ptr, state->info_ptr, &state->color.white_point.x, &state->color.white_point.y, &state->color.primaries.Red.x,
                     &state->color.primaries.Red.y, &state->color.primaries.Green.x, &state->color.primaries.Green.y, &state->color.primaries.Blue.x, &state->color.primaries.Blue.y);

        state->color.white_point.Y = state->color.primaries.Red.Y = state->color.primaries.Green.Y = state->color.primaries.Blue.Y = 1.0;


        state->color.source = flow_codec_color_profile_source_GAMA_CHRM;
    }

    return true;
}

struct flow_decoder_color_info * wrap_png_decoder_get_color_info(struct wrap_png_decoder_state * state){
    return &state->color;
}


bool wrap_png_decode_image_info(struct wrap_png_decoder_state * state)
{
    state->png_ptr = png_create_read_struct(PNG_LIBPNG_VER_STRING, state, wrap_png_decoder_error_handler, NULL);
    if (state->png_ptr == NULL) {
        state->error_handler(state->png_ptr, state->custom_state, "OOM in wrap_png_decode_image_info: png_create_read_struct failed. Out of memory.\"");
        return false;
    }

    // Set up error continuation
    if (setjmp(state->error_handler_jmp)) {
        // Execution comes back to this point if an error happens
        // We assume that the handler already set the context error
        return false;
    }

    state->info_ptr = png_create_info_struct(state->png_ptr);
    if (state->info_ptr == NULL) {
        state->error_handler(state->png_ptr, state->custom_state, "OOM in wrap_png_decode_image_info: png_create_info_struct failed. Out of memory.");
        return false;
    }

    // Custom read function req.d - reading from memory
    png_set_read_fn(state->png_ptr, state, wrap_png_custom_read_data);

    // Read header and chunks
    png_read_info(state->png_ptr, state->info_ptr);

    png_uint_32 w, h;
    // Get dimensions and info
    png_get_IHDR(state->png_ptr, state->info_ptr, &w, &h, &state->bit_depth, &state->color_type, NULL, NULL, NULL);
    state->w = w;
    state->h = h;

    if (!wrap_png_decoder_load_color_profile(state)){
        return false;
    }

    // Now we need to figure out how big our pixel buffer needs to be to hold the entire image.
    // We need to apply some normalization filters so we have fewer variants.

    /* expand palette images to RGB, low-bit-depth grayscale images to 8 bits,
    * transparency chunks to full alpha channel; strip 16-bit-per-sample
    * images to 8 bits per sample; and convert grayscale to RGB[A] */

    // Fill in the alpha channel with FFFF if missing.
    if (!(state->color_type & PNG_COLOR_MASK_ALPHA)) {
        png_set_expand(state->png_ptr);
        png_set_filler(state->png_ptr, 65535L, PNG_FILLER_AFTER);
        if (state->color_type == PNG_COLOR_TYPE_PALETTE){
            state->alpha_used = true;
        }else{
            state->alpha_used = false;
        }
    } else {
        state->alpha_used = true;
    }

    // Drop to 8-bit per channel; we can't handle 16-bit yet.
    if (state->bit_depth == 16) {
        png_set_strip_16(state->png_ptr);
    }
    // Convert grayscale to RGB.
    if (!(state->color_type & PNG_COLOR_MASK_COLOR))
        png_set_gray_to_rgb(state->png_ptr);

    // We use BGRA, not RGBA
    png_set_bgr(state->png_ptr);
    // We don't want to think about interlacing. Let libpng fix that up.

    // Update our info based on these new settings.
    png_read_update_info(state->png_ptr, state->info_ptr);

    // Now we can access a stride that represents the post-transform data.
    // state->rowbytes = png_get_rowbytes(state->png_ptr, state->info_ptr);

    if (png_get_channels(state->png_ptr, state->info_ptr) != 4) {
        state->error_handler(state->png_ptr, state->custom_state, "libpng channels != 4 (should convert to BGRA)");
        return false;
    }
    return true;
}


bool wrap_png_decode_finish(struct wrap_png_decoder_state * state, uint8_t * * row_pointers, size_t row_count, size_t row_bytes)
{

    // We let the caller create the buffer
    if (row_pointers == NULL) {
        state->error_handler(state->png_ptr, state->custom_state, "wrap_png_decode_finish: row_pointers == NULL");
        return false;
    }

    if (state->h != row_count){
        state->error_handler(state->png_ptr, state->custom_state, "wrap_png_decode_finish: row_pointers row_count != 0");
        return false;
    }

    if (state->w * 4 != row_bytes){
        state->error_handler(state->png_ptr, state->custom_state, "wrap_png_decode_finish: row_bytes != w * 4");
        return false;
    }


    if (setjmp(state->error_handler_jmp)) {
        // Execution comes back to this point if an error happens
        return false;
    }

    // The real work
    png_read_image(state->png_ptr, row_pointers);

    png_read_end(state->png_ptr, NULL);

    return true;
}

void * wrap_png_decoder_get_png_ptr(struct wrap_png_decoder_state * state){
    return &state->png_ptr;
}

void * wrap_png_decoder_get_info_ptr(struct wrap_png_decoder_state * state){
    return &state->info_ptr;
}

bool wrap_png_decoder_destroy(struct wrap_png_decoder_state * state){
    if (setjmp(state->error_handler_jmp)) {
        // Execution comes back to this point if an error happens
        return false;
    }

    png_destroy_read_struct(&state->png_ptr, &state->info_ptr, NULL);
    return true;
}

bool wrap_png_decoder_get_info(struct wrap_png_decoder_state * state, uint32_t * w, uint32_t * h, bool * uses_alpha){
    *w = state->w;
    *h = state->h;
    *uses_alpha = state->alpha_used;
    return true;
}

/******************************************* ENCODER **************************************************/


struct wrap_png_encoder_state {
    wrap_png_error_handler error_handler;
    wrap_png_custom_write_function write_function;
    void * custom_state;
    jmp_buf error_handler_jmp;
};

static void wrap_png_encoder_error_handler(png_structp png_ptr, png_const_charp msg)
{
    struct wrap_png_encoder_state * state = (struct wrap_png_encoder_state *)png_get_error_ptr(png_ptr);

    state->error_handler(png_ptr, state->custom_state, msg);

    /* Return control to the setjmp point */
    longjmp(state->error_handler_jmp, 1);
}

static void wrap_png_encoder_custom_write_data(png_structp png_ptr, png_bytep buffer, png_size_t buffer_length)
{
    struct wrap_png_encoder_state * state = (struct wrap_png_encoder_state *)png_get_io_ptr(png_ptr);

    if (state == NULL || state->custom_state == NULL) {
        png_error(png_ptr, "PNG Write Error - state or custom_state null");
    }

    if (state->write_function == NULL) {
        png_error(png_ptr, "PNG Write Error - write_function null");
    }

    if (!state->write_function(png_ptr, state->custom_state, buffer, buffer_length)){
        png_error(png_ptr, "Write error in write_function callback");
    }
}

static bool wrap_png_encoder_state_init(struct wrap_png_encoder_state * state, void * custom_state,
                                 wrap_png_error_handler error_handler, wrap_png_custom_write_function write_function)
{
    memset(state, 0, sizeof(struct wrap_png_encoder_state));
    state->custom_state = custom_state;
    state->write_function = write_function;
    state->error_handler = error_handler;
    return true;
}

static void png_flush_nullop(png_structp png_ptr) {}

bool wrap_png_encoder_write_png(void * custom_state,
                                wrap_png_error_handler error_handler,
                                wrap_png_custom_write_function write_function,
                                uint8_t * * row_pointers,
                                size_t w,
                                size_t h,
                                bool disable_png_alpha,
                                int zlib_compression_level,
                                flow_pixel_format pixel_format){



    struct wrap_png_encoder_state state;
    wrap_png_encoder_state_init(&state,custom_state,error_handler,write_function);

    if (pixel_format != flow_bgra32 && pixel_format != flow_bgr24 && pixel_format != flow_bgr32) {
        state.error_handler(NULL, state.custom_state, "Unsupported pixel_format passed to wrap_png_encoder_write_png");
        return false;
    }

    png_structp png_ptr = png_create_write_struct(PNG_LIBPNG_VER_STRING, &state, wrap_png_encoder_error_handler,
                                                  NULL); // makepng_error, makepng_warning);
    png_infop info_ptr = NULL;
    if (png_ptr == NULL){
        state.error_handler(png_ptr, state.custom_state, "OOM in wrap_png_encoder_write_png: png_create_write_struct failed. Out of memory.");
        return false;
    }

    if (setjmp(state.error_handler_jmp)) {
        return false;
    }
    if (zlib_compression_level >= -1 && zlib_compression_level <= 9)
    {
        png_set_compression_level(png_ptr, zlib_compression_level);
        png_set_text_compression_level(png_ptr, zlib_compression_level);
    }else{
        png_set_compression_level(png_ptr,  PNG_Z_DEFAULT_COMPRESSION);
        png_set_text_compression_level(png_ptr, PNG_Z_DEFAULT_COMPRESSION);
    }

    png_set_write_fn(png_ptr, &state, wrap_png_encoder_custom_write_data, png_flush_nullop);

    info_ptr = png_create_info_struct(png_ptr);
    if (info_ptr == NULL){
        png_destroy_write_struct(&png_ptr, &info_ptr);
        state.error_handler(png_ptr, state.custom_state, "OOM in wrap_png_encoder_write_png: png_create_info_struct failed. Out of memory.");
        return false;
    }

    png_set_rows(png_ptr, info_ptr, row_pointers);

    int color_type;
    int transform;
    if ((pixel_format == flow_bgra32 && disable_png_alpha) || pixel_format == flow_bgr32) {
        color_type = PNG_COLOR_TYPE_RGB;
        transform = PNG_TRANSFORM_BGR | PNG_TRANSFORM_STRIP_FILLER_AFTER;
    } else if (pixel_format == flow_bgr24) {
        color_type = PNG_COLOR_TYPE_RGB;
        transform = PNG_TRANSFORM_BGR;
    } else if (pixel_format == flow_bgra32) {
        color_type = PNG_COLOR_TYPE_RGB_ALPHA;
        transform = PNG_TRANSFORM_BGR;
    } else {
        png_destroy_write_struct(&png_ptr, &info_ptr);
        state.error_handler(png_ptr, state.custom_state, "Invalid pixel_format argument passed to wrap_png_encoder_write_png.");
        return false;
    }

    png_set_IHDR(png_ptr, info_ptr, (png_uint_32)w, (png_uint_32)h, 8, color_type, PNG_INTERLACE_NONE,
                 PNG_COMPRESSION_TYPE_BASE, PNG_FILTER_TYPE_BASE);

    png_set_sRGB_gAMA_and_cHRM(png_ptr, info_ptr, PNG_sRGB_INTENT_PERCEPTUAL);

    if ( disable_png_alpha) {
        // png_set_filler(png_ptr, (png_uint_32)0, PNG_FILLER_AFTER);
    }

    png_write_png(png_ptr, info_ptr, transform, NULL);

    png_destroy_write_struct(&png_ptr, &info_ptr);
    return true;
}
