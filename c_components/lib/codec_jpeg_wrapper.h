#pragma once
#include "imageflow_private.h"
#include "jpeglib.h"
#include "jerror.h"


#define PUB FLOW_EXPORT

#ifdef __cplusplus
extern "C" {
#endif

typedef  bool (*wrap_jpeg_error_handler) (void * custom_state, j_common_ptr cinfo, struct jpeg_error_mgr * error_mgr, int error_code, char * error_message_buffer, int error_message_buffer_length);


struct wrap_jpeg_error_state;

PUB size_t wrap_jpeg_error_state_bytes(void);

PUB void wrap_jpeg_setup_error_handler(j_decompress_ptr cinfo, struct wrap_jpeg_error_state * state, void * custom_state, wrap_jpeg_error_handler  error_handler);

PUB void * wrap_jpeg_get_custom_state(j_decompress_ptr codec_info);

PUB bool wrap_jpeg_create_decompress(j_decompress_ptr codec_info);
PUB bool wrap_jpeg_read_header(j_decompress_ptr codec_info);

PUB bool wrap_jpeg_save_markers(j_decompress_ptr codec_info,
                            int marker_code,
                            unsigned int length_limit);

PUB bool wrap_jpeg_start_decompress(j_decompress_ptr codec_info);
PUB bool wrap_jpeg_finish_decompress(j_decompress_ptr codec_info);

PUB bool wrap_jpeg_read_scan_lines(j_decompress_ptr codec_info, uint8_t ** scan_lines, uint32_t max_scan_lines, uint32_t * scan_lines_read);

PUB void wrap_jpeg_set_downscale_type(j_decompress_ptr codec_info, bool scale_luma_spatially, bool gamma_correct_for_srgb_during_spatial_luma_scaling);
PUB void wrap_jpeg_set_idct_method_selector(j_decompress_ptr codec_info);

typedef  bool (*wrap_jpeg_source_manager_func) (j_decompress_ptr codec_info, void * custom_state);
typedef  bool (*wrap_jpeg_source_manager_fill_buffer_func) (j_decompress_ptr codec_info, void * custom_state, bool * suspend_io);
typedef  bool (*wrap_jpeg_source_manager_skip_bytes_func) (j_decompress_ptr codec_info, void * custom_state, long byte_count);

struct wrap_jpeg_source_manager{
    struct jpeg_source_mgr shared_mgr;
    wrap_jpeg_source_manager_func init_source_fn;
    wrap_jpeg_source_manager_func term_source_fn;
    wrap_jpeg_source_manager_fill_buffer_func fill_input_buffer_fn;
    wrap_jpeg_source_manager_skip_bytes_func skip_input_data_fn;
    void * custom_state;
};

PUB void wrap_jpeg_setup_source_manager(struct wrap_jpeg_source_manager * manager);


#ifdef __cplusplus
}
#endif


