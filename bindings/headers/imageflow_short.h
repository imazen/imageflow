

typedef enum imageflow_lifetime {

	imageflow_lifetime_outlives_function_call = 0,

	imageflow_lifetime_outlives_context = 1,
} imageflow_lifetime;

bool imageflow_abi_compatible(uint32_t imageflow_abi_ver_major, uint32_t imageflow_abi_ver_minor);

uint32_t imageflow_abi_version_major(void);

uint32_t imageflow_abi_version_minor(void);

void* imageflow_context_create(uint32_t imageflow_abi_ver_major, uint32_t imageflow_abi_ver_minor);

bool imageflow_context_begin_terminate(void* context);

void imageflow_context_destroy(void* context);

bool imageflow_context_has_error(void* context);

bool imageflow_context_error_recoverable(void* context);

bool imageflow_context_error_try_clear(void* context);

int32_t imageflow_context_error_code(void* context);

int32_t imageflow_context_error_as_exit_code(void* context);

int32_t imageflow_context_error_as_http_code(void* context);

bool imageflow_context_error_write_to_buffer(void* context, char* buffer, size_t buffer_length, size_t* bytes_written);

bool imageflow_context_print_and_exit_if_error(void* context);

bool imageflow_json_response_read(void* context, void const* response_in, int64_t* status_as_http_code_out, uint8_t const** buffer_utf8_no_nulls_out, size_t* buffer_size_out);

bool imageflow_json_response_destroy(void* context, void* response);

void const* imageflow_context_send_json(void* context, char const* method, uint8_t const* json_buffer, size_t json_buffer_size);

bool imageflow_context_add_input_buffer(void* context, int32_t io_id, uint8_t const* buffer, size_t buffer_byte_count, imageflow_lifetime lifetime);

bool imageflow_context_add_output_buffer(void* context, int32_t io_id);

bool imageflow_context_get_output_buffer_by_id(void* context, int32_t io_id, uint8_t const** result_buffer, size_t* result_buffer_length);

void* imageflow_context_memory_allocate(void* context, size_t bytes, char const* filename, int32_t line);

bool imageflow_context_memory_free(void* context, void* pointer, char const* filename, int32_t line);

