

typedef enum imageflow_lifetime {
  imageflow_lifetime_lifetime_outlives_function_call = 0,
  imageflow_lifetime_lifetime_outlives_context = 1,
} imageflow_lifetime;

bool imageflow_abi_compatible(uint32_t imageflow_abi_ver_major, uint32_t imageflow_abi_ver_minor);

uint32_t imageflow_abi_version_major(void);

uint32_t imageflow_abi_version_minor(void);

bool imageflow_context_add_input_buffer(void *context,
                                        int32_t io_id,
                                        const uint8_t *buffer,
                                        size_t buffer_byte_count,
                                        imageflow_lifetime lifetime);

bool imageflow_context_add_output_buffer(void *context, int32_t io_id);

bool imageflow_context_begin_terminate(void *context);

void *imageflow_context_create(uint32_t imageflow_abi_ver_major,
                                  uint32_t imageflow_abi_ver_minor);

void imageflow_context_destroy(void *context);

int32_t imageflow_context_error_as_exit_code(void *context);

int32_t imageflow_context_error_as_http_code(void *context);

int32_t imageflow_context_error_code(void *context);

bool imageflow_context_error_recoverable(void *context);

bool imageflow_context_error_try_clear(void *context);

bool imageflow_context_error_write_to_buffer(void *context,
                                             char *buffer,
                                             size_t buffer_length,
                                             size_t *bytes_written);

bool imageflow_context_get_output_buffer_by_id(void *context,
                                               int32_t io_id,
                                               const uint8_t **result_buffer,
                                               size_t *result_buffer_length);

bool imageflow_context_has_error(void *context);

void *imageflow_context_memory_allocate(void *context,
                                        size_t bytes,
                                        const char *filename,
                                        int32_t line);

bool imageflow_context_memory_free(void *context,
                                   void *pointer,
                                   const char *filename,
                                   int32_t line);

bool imageflow_context_print_and_exit_if_error(void *context);

const void *imageflow_context_send_json(void *context,
                                                const char *method,
                                                const uint8_t *json_buffer,
                                                size_t json_buffer_size);

bool imageflow_json_response_destroy(void *context, void *response);

bool imageflow_json_response_read(void *context,
                                  const void *response_in,
                                  int64_t *status_as_http_code_out,
                                  const uint8_t **buffer_utf8_no_nulls_out,
                                  size_t *buffer_size_out);

