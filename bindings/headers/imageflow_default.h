
#ifndef generated_imageflow_h
#define generated_imageflow_h

// Incremented for breaking changes
#define IMAGEFLOW_ABI_VER_MAJOR 3

// Incremented for non-breaking additions
#define IMAGEFLOW_ABI_VER_MINOR 1


struct imageflow_context;
struct imageflow_json_response;
struct imageflow_job;
struct imageflow_job_io;
        

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

//
// How long the provided pointer/buffer will remain valid.
// Callers must prevent the memory from being freed or moved until this contract expires.
//
typedef enum imageflow_lifetime {
  // Pointer will outlive function call. If the host language has a garbage collector, call the appropriate method to ensure the object pointed to will not be collected or moved until the call returns. You may think host languages do this automatically in their FFI system. Most do not.
  imageflow_lifetime_lifetime_outlives_function_call = 0,
  // Pointer will outlive context. If the host language has a GC, ensure that you are using a data type guaranteed to neither be moved or collected automatically.
  imageflow_lifetime_lifetime_outlives_context = 1,
} imageflow_lifetime;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

//
// Call this method before doing anything else to ensure that your header or FFI bindings are compatible
// with the libimageflow that is currently loaded.
//
// Provide the values `IMAGEFLOW_ABI_VER_MAJOR` and `IMAGEFLOW_ABI_VER_MINOR` to this function.
//
// False means that
//
bool imageflow_abi_compatible(uint32_t imageflow_abi_ver_major,
                              uint32_t imageflow_abi_ver_minor);

uint32_t imageflow_abi_version_major(void);

uint32_t imageflow_abi_version_minor(void);

// Adds an input buffer to the job context.
//
// The buffer lifetime semantics depend on the `lifetime` parameter:
//
// * `imageflow_lifetime::OutlivesFunctionCall` - Imageflow copies the buffer immediately.
//   You may free the buffer as soon as this function returns.
//
// * `imageflow_lifetime::OutlivesContext` - Imageflow borrows the buffer (zero-copy).
//   **CRITICAL:** The buffer MUST remain valid and unmodified until the context is destroyed.
//   Do NOT free, move, or modify the buffer while the context exists.
//   In GC languages, pin the buffer to prevent garbage collection or movement.
//
// Returns false if the operation fails. Check error state with `imageflow_context_has_error()`.
//
// # Safety
//
// * `context` must be a valid pointer from `imageflow_context_create`
// * `context` must not be NULL (will abort process)
// * `buffer` must be a valid pointer to at least `buffer_byte_count` readable bytes
// * `buffer` must not be NULL
// * If `lifetime` is `OutlivesContext`, buffer must remain valid until context destruction
// * `io_id` must be unique (not previously used for this context)
// * `buffer_byte_count` must not have the most significant bit set (max 2^31 or 2^63)
//
// # Panics
//
// Internal panics are caught and converted to errors. Returns false on panic.
//
// # Thread Safety
//
// Safe to call from multiple threads on the same context (acquires write lock).
bool imageflow_context_add_input_buffer(struct imageflow_context *context,
                                        int32_t io_id,
                                        const uint8_t *buffer,
                                        size_t buffer_byte_count,
                                        enum imageflow_lifetime lifetime);

// Adds an output buffer to the job context.
//
// Imageflow will allocate and manage a growable output buffer internally.
// After processing, retrieve the buffer contents with `imageflow_context_get_output_buffer_by_id()`.
//
// The output buffer is freed automatically when the context is destroyed.
//
// Returns false if allocation failed or the context is in an error state.
//
// # Safety
//
// * `context` must be a valid pointer from `imageflow_context_create`
// * `context` must not be NULL (will abort process)
// * `io_id` must be unique (not previously used for this context)
//
// # Panics
//
// Internal panics are caught and converted to errors. Returns false on panic.
//
// # Thread Safety
//
// Safe to call from multiple threads on the same context (acquires write lock).
bool imageflow_context_add_output_buffer(struct imageflow_context *context,
                                         int32_t io_id);

// Begins the process of destroying the context, yet leaves error information intact
// so that any errors in the tear-down process can be
// debugged with `imageflow_context_error_write_to_buffer`.
//
// Returns true if no errors occurred. Returns false if there were tear-down issues.
//
// # Safety
//
// * `context` must be a valid pointer returned from `imageflow_context_create`
// * `context` must not have been previously destroyed
// * No other threads may be accessing this context during or after this call
// * After calling this function, only error-retrieval functions and `imageflow_context_destroy`
//   may be called on this context
//
// # Panics
//
// Cannot panic - designed to be panic-safe for cleanup paths.
bool imageflow_context_begin_terminate(struct imageflow_context *context);

// Creates and returns an imageflow context.
// An imageflow context is required for all other imageflow API calls.
//
// An imageflow context tracks
// * error state
// * error messages
// * stack traces for errors (in C land, at least)
// * context-managed memory allocations
// * performance profiling information
//
// **As of ABI 3.1, contexts ARE thread-safe!** A single context may be accessed from multiple
// threads concurrently. Operations will serialize internally to prevent data races.
// For best performance with multiple threads, use separate contexts per thread.
//
// Returns a null pointer if allocation fails or the provided interface version is incompatible.
//
// # Panics
//
// Cannot panic - all panics during creation are caught and result in returning null.
//
// # Thread Safety
//
// This function is safe to call from multiple threads simultaneously. Each call creates an
// independent context.
struct imageflow_context *imageflow_context_create(uint32_t imageflow_abi_ver_major,
                                            uint32_t imageflow_abi_ver_minor);

// Destroys the imageflow context and frees the context object.
// Only use this with contexts created using `imageflow_context_create`
//
// All memory associated with the context is freed, including:
// * The context itself
// * All buffers created by imageflow for this context
// * All struct imageflow_json_response objects not explicitly freed
// * All internal allocations
//
// # Safety
//
// * `context` must be either NULL (no-op) or a valid pointer from `imageflow_context_create`
// * `context` must not have been previously destroyed (double-free)
// * No other threads may be accessing this context during or after this call
// * After this call, the pointer is invalid - do not use it (use-after-free)
// * All pointers to data owned by this context become invalid (output buffers, struct imageflow_json_response, etc.)
//
// # Panics
//
// Cannot panic - destruction is panic-safe and will attempt cleanup even in degraded states.
//
// # Thread Safety
//
// This function is NOT safe to call while other threads are using the context.
// Ensure all threads have finished using the context before destroying it.
void imageflow_context_destroy(struct imageflow_context *context);

// Converts the error (or lack thereof) into an unix process exit code
//
// ## Values
//
// * 0 - No error
// * 64 - Invalid usage (graph invalid, node argument invalid, action not supported)
// * 65 - Invalid Json, Image malformed, Image type not supported
// * 66 - Primary or secondary file or resource not found.
// * 69 - Upstream server errored or timed out
// * 70 - Possible bug: internal error, custom error, unknown error, or no graph solution found
// * 71 - Out Of Memory condition (malloc/calloc/realloc failed).
// * 74 - I/O Error
// * 77 - Action forbidden under imageflow security policy
// * 402 - License error
// * 401 - Imageflow server authorization required
// # Safety
//
// * `context` must be a valid pointer from `imageflow_context_create`
// * `context` must not be NULL (will abort process)
// * `context` must not have been destroyed
int32_t imageflow_context_error_as_exit_code(struct imageflow_context *context);

// Converts the error (or lack thereof) into an equivalent http status code
//
// ## Values
//
// * 200 - No error
// * 400 - Bad argument/node parameters/graph/json/image/image type
// * 401 - Authorization to imageflow server required
// * 402 - License error
// * 403 - Action forbidden under imageflow security policy
// * 404 - Primary resource/file not found
// * 500 - Secondary resource/file not found, IO error, no solution error, unknown error, custom error, internal error
// * 502 - Upstream server error
// * 503 - Out Of Memory condition (malloc/calloc/realloc failed).
// * 504 - Upstream timeout
// # Safety
//
// * `context` must be a valid pointer from `imageflow_context_create`
// * `context` must not be NULL (will abort process)
// * `context` must not have been destroyed
int32_t imageflow_context_error_as_http_code(struct imageflow_context *context);

// Returns the numeric code associated with the error category. 0 means no error.
//
// These will be stabilized after 1.0, once error categories have passed rigorous real-world testing
// `imageflow_context_error_as_exit_code` and `imageflow_context_error_as_http_status` are suggested in the meantime.
// Unstable, do not use.
//
// # Safety
//
// * `context` must be a valid pointer from `imageflow_context_create`
// * `context` must not be NULL (will abort process)
// * `context` must not have been destroyed
int32_t imageflow_context_error_code(struct imageflow_context *context);

// Returns true if the context is "ok" or in an error state that is recoverable.
//
// A recoverable error is one that can be cleared with `imageflow_context_error_try_clear()`.
// Panics and some critical errors are NOT recoverable.
//
// # Safety
//
// * `context` must be a valid pointer from `imageflow_context_create`
// * `context` must not be NULL (will abort process)
// * `context` must not have been destroyed
//
// # Thread Safety
//
// Safe to call from multiple threads on the same context (acquires read lock).
bool imageflow_context_error_recoverable(struct imageflow_context *context);

// Attempts to clear a recoverable error from the context.
//
// Returns true if the error was cleared (or if there was no error).
// Returns false if the error is not recoverable (panic, critical error).
//
// You MUST check `imageflow_context_error_recoverable()` before calling this.
//
// # Safety
//
// * `context` must be a valid pointer from `imageflow_context_create`
// * `context` must not be NULL (will abort process)
// * `context` must not have been destroyed
//
// # Thread Safety
//
// Safe to call from multiple threads on the same context (acquires write lock).
bool imageflow_context_error_try_clear(struct imageflow_context *context);

// Writes error messages (and stack frames) to the provided buffer in UTF-8 format.
//
// The output is null-terminated. The number of bytes written (excluding null terminator)
// is written to `bytes_written` if it's not NULL.
//
// ## Return Value
//
// * Returns **true** if all error data was written successfully
// * Returns **false** if the buffer was too small and output was truncated
// * Returns **false** if buffer is NULL
//
// When truncated, the buffer will contain "\n[truncated]\n" at the end (before the null terminator).
//
// ## Usage
//
// ```c
// char error_buffer[1024];
// size_t bytes_written;
// if (!imageflow_context_error_write_to_buffer(ctx, error_buffer, sizeof(error_buffer), &bytes_written)) {
//     // Buffer was too small, error message truncated
// }
// printf("Error: %s\n", error_buffer);
// ```
//
// # Safety
//
// * `context` must be a valid pointer from `imageflow_context_create`
// * `context` must not be NULL (will abort process)
// * `buffer` must be either NULL or a valid pointer to at least `buffer_length` writable bytes
// * `buffer_length` must accurately reflect the buffer size - incorrect size causes buffer overflow
// * `buffer_length` must not have the most significant bit set (max 2^31 or 2^63)
// * `bytes_written`, if not NULL, must be a valid pointer to write a size_t
//
// # Thread Safety
//
// Safe to call from multiple threads on the same context (acquires read lock on error state).
bool imageflow_context_error_write_to_buffer(struct imageflow_context *context,
                                             char *buffer,
                                             size_t buffer_length,
                                             size_t *bytes_written);

// Provides access to the underlying buffer for the given output io_id.
//
// This function writes the buffer pointer and length to the provided output parameters.
// The buffer pointer remains valid until the context is destroyed.
//
// **Important:** The returned buffer is read-only and owned by the context.
// Do NOT modify or free the buffer.
//
// Returns false and sets an error if:
// * The io_id is invalid or not an output buffer
// * The result pointers are null
// * The context is in an error state
//
// # Safety
//
// * `context` must be a valid pointer from `imageflow_context_create`
// * `context` must not be NULL (will abort process)
// * `result_buffer` must be a valid pointer to write a `*const u8` to (not NULL)
// * `result_buffer_length` must be a valid pointer to write a `size_t` to (not NULL)
// * The returned buffer pointer is only valid until context destruction
// * The returned buffer must NOT be modified or freed by the caller
//
// # Panics
//
// Internal panics are caught and converted to errors. Returns false on panic.
//
// # Thread Safety
//
// Safe to call from multiple threads on the same context (acquires write lock).
// Note: The buffer pointer becomes shared - ensure no thread is writing to outputs while reading.
bool imageflow_context_get_output_buffer_by_id(struct imageflow_context *context,
                                               int32_t io_id,
                                               const uint8_t **result_buffer,
                                               size_t *result_buffer_length);

// Returns true if the context is in an error state.
//
// When true, you should retrieve error details and handle the error before attempting more operations.
// Some operations will fail or return incorrect results if the context is in an error state.
//
// # Safety
//
// * `context` must be a valid pointer from `imageflow_context_create`
// * `context` must not be NULL (will abort process)
// * `context` must not have been destroyed
//
// # Thread Safety
//
// Safe to call from multiple threads on the same context (acquires read lock).
bool imageflow_context_has_error(struct imageflow_context *context);

// Allocates zeroed memory that will be freed automatically with the context.
//
// The allocated memory is zeroed and aligned to 16 bytes.
// The memory will be automatically freed when the context is destroyed.
//
// You may free the memory early using `imageflow_context_memory_free()`.
//
// * `filename`/`line` are optional debug parameters. Pass NULL/-1 to skip.
// * If provided, `filename` should be a null-terminated UTF-8 string with static lifetime.
//
// Returns NULL on allocation failure or if the context is in an error state.
//
// # Safety
//
// * `context` must be a valid pointer from `imageflow_context_create`
// * `context` must not be NULL (will abort process)
// * `bytes` must not have the most significant bit set (max 2^31 or 2^63)
// * `filename`, if not NULL, must be a valid null-terminated string with static lifetime
// * The returned pointer is valid until freed or until context destruction
//
// # Panics
//
// Cannot panic - allocation failures result in returning NULL with error set.
//
// # Thread Safety
//
// Safe to call from multiple threads on the same context (uses separate allocation lock).
void *imageflow_context_memory_allocate(struct imageflow_context *context,
                                        size_t bytes,
                                        const char *filename,
                                        int32_t line);

// Frees memory allocated with `imageflow_context_memory_allocate` early.
//
// This is optional - all context-allocated memory is automatically freed when the context is destroyed.
// Use this to reduce memory usage during long-running operations.
//
// * `filename`/`line` are reserved for future debugging. Pass NULL/-1.
//
// Returns true on success or if `pointer` is NULL.
// Returns false if the pointer was not found in the allocation list.
//
// # Safety
//
// * `context` must be a valid pointer from `imageflow_context_create`
// * `context` must not be NULL (will abort process)
// * `pointer` must be either NULL or a pointer returned from `imageflow_context_memory_allocate` on this context
// * `pointer` must not have been previously freed (double-free)
// * After this call, `pointer` is invalid - do not use it (use-after-free)
//
// # Panics
//
// Cannot panic - safe to use in cleanup paths even if context is in error state.
//
// # Thread Safety
//
// Safe to call from multiple threads on the same context (uses separate allocation lock).
bool imageflow_context_memory_free(struct imageflow_context *context,
                                   void *pointer,
                                   const char *_filename,
                                   int32_t _line);

// Prints the error to stderr and exits the process if an error has been raised on the context.
//
// **WARNING: THIS FUNCTION CALLS exit() AND TERMINATES THE PROCESS!**
//
// If an error is present:
// * Prints error details to stderr
// * Calls `exit()` with an appropriate exit code (see `imageflow_context_error_as_exit_code`)
// * **DOES NOT RETURN**
//
// If no error is present:
// * Returns false
// * Does not exit
//
// **DO NOT USE IN SERVICES, LIBRARIES, OR LONG-RUNNING PROCESSES!**
// This is only appropriate for command-line utilities where process termination is acceptable.
//
// # Safety
//
// * `context` must be a valid pointer from `imageflow_context_create`
// * `context` must not be NULL (will abort process)
//
// # Thread Safety
//
// If called while other threads are running, those threads will be terminated without cleanup.
// Only use this in single-threaded command-line programs.
bool imageflow_context_print_and_exit_if_error(struct imageflow_context *context);

// Requests cancellation of any running or future operations on this context.
//
// This sets an atomic cancellation flag that operations check periodically.
// When an operation detects the cancellation flag, it will abort and return an error.
//
// **Important:** Cancellation is not guaranteed to be immediate. Some operations may complete
// before detecting the flag. Once cancellation is requested, the context enters an error state.
//
// Error details: `ErrorKind::OperationCancelled`, error code 21, HTTP 499, exit code 130
//
// No further operations can be attempted after cancellation - the context is in an errored state.
//
// # Safety
//
// * `context` must be a valid pointer from `imageflow_context_create`
// * `context` must not be NULL (will abort process)
//
// # Panics
//
// Cannot panic.
//
// # Thread Safety
//
// **This is the only function specifically designed to be called from another thread while an
// operation is running.** Use this to implement timeouts or user cancellation.
void imageflow_context_request_cancellation(struct imageflow_context *context);

// Sends a JSON message to the imageflow context using the specified endpoint method.
//
// This is the primary API for invoking imageflow operations.
//
// ## Endpoints
//
// * `v1/build` - Build and execute an image processing job
// * `v1/execute` - Execute a pre-configured operation graph
// * `v1/get_version_info` - Get version and build information
//
// For the latest endpoints, see:
// `https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/context_json_api.txt`
//
// ## Parameters
//
// * `method` - UTF-8 null-terminated string specifying the endpoint (e.g., "v1/build")
// * `json_buffer` - UTF-8 encoded JSON (NOT null-terminated)
// * `json_buffer_size` - Length of json_buffer in bytes
//
// ## Return Value
//
// Returns a pointer to a `struct imageflow_json_response` on success or error. The response contains:
// * HTTP-style status code (200 for success, 4xx/5xx for errors)
// * UTF-8 JSON response buffer
//
// Returns NULL only if:
// * A panic occurred and could not be converted to a JSON error
// * Invalid arguments were provided
//
// **Always check `imageflow_context_has_error()` after calling this function.**
//
// The returned struct imageflow_json_response is owned by the context and remains valid until:
// * You call `imageflow_json_response_destroy()`, OR
// * You destroy the context
//
// ## Memory imageflow_lifetime
//
// * `method` and `json_buffer` are only borrowed during this function call
// * You remain responsible for freeing them if dynamically allocated
// * Static strings are ideal for `method`
//
// # Safety
//
// * `context` must be a valid pointer from `imageflow_context_create`
// * `context` must not be NULL (will abort process)
// * `method` must be a valid null-terminated UTF-8 string
// * `method` must not be NULL
// * `json_buffer` must be a valid pointer to at least `json_buffer_size` readable bytes
// * `json_buffer` must not be NULL
// * `json_buffer_size` must not have the most significant bit set (max 2^31 or 2^63)
// * `json_buffer` and `method` must remain valid for the duration of this call
//
// # Panics
//
// Internal panics are caught and converted to error responses. Check the response status code.
//
// # Thread Safety
//
// Safe to call from multiple threads on the same context (acquires write lock).
// Operations will serialize - only one operation executes at a time per context.
const struct imageflow_json_response *imageflow_context_send_json(struct imageflow_context *context,
                                                const char *method,
                                                const uint8_t *json_buffer,
                                                size_t json_buffer_size);

// Frees a struct imageflow_json_response object early.
//
// This is optional - struct imageflow_json_response objects are automatically freed when the context is destroyed.
// Use this to reduce memory usage if you're done with a response before destroying the context.
//
// Returns true if successful or if `response` is NULL.
// Returns false if the pointer was not found in the allocation list.
//
// # Safety
//
// * `context` must be a valid pointer from `imageflow_context_create`
// * `context` must not be NULL (will abort process)
// * `context` must be the same context that created the response
// * `response` must be either NULL or a valid pointer from `imageflow_context_send_json` on this context
// * `response` must not have been previously freed (double-free)
// * After this call, `response` and its buffer are invalid - do not use them (use-after-free)
//
// # Panics
//
// Cannot panic - safe to use in cleanup paths.
//
// # Thread Safety
//
// Safe to call from multiple threads on the same context, but ensure no other thread is reading
// this specific response object.
bool imageflow_json_response_destroy(struct imageflow_context *context,
                                     struct imageflow_json_response *response);

// Reads fields from a struct imageflow_json_response and writes them to the provided output parameters.
//
// The buffer pointer will be a UTF-8 byte array (NOT null-terminated).
//
// **Important:** The buffer pointer is only valid until:
// * The struct imageflow_json_response is freed with `imageflow_json_response_destroy`, OR
// * The context is destroyed
//
// Any of the output parameters may be NULL if you don't need that field.
//
// Returns false if `response_in` is NULL.
//
// # Safety
//
// * `context` must be a valid pointer from `imageflow_context_create`
// * `context` must not be NULL (will abort process)
// * `response_in` must be either NULL or a valid pointer from `imageflow_context_send_json`
// * `status_as_http_code_out`, if not NULL, must be a valid pointer to write an i64
// * `buffer_utf8_no_nulls_out`, if not NULL, must be a valid pointer to write a *const u8
// * `buffer_size_out`, if not NULL, must be a valid pointer to write a size_t
// * The returned buffer pointer is only valid until response/context destruction
//
// # Thread Safety
//
// The response object is not protected by locks. Do not read a response while another thread
// might be freeing it or destroying the context.
bool imageflow_json_response_read(struct imageflow_context *context,
                                  const struct imageflow_json_response *response_in,
                                  int64_t *status_as_http_code_out,
                                  const uint8_t **buffer_utf8_no_nulls_out,
                                  size_t *buffer_size_out);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus


#endif // generated_imageflow_h
