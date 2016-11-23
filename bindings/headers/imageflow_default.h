
#ifndef cheddar_generated_imageflow_default_h
#define cheddar_generated_imageflow_default_h


#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <stdbool.h>


struct imageflow_context;
struct imageflow_json_response;
struct imageflow_job;
struct imageflow_job_io;
        

///
/// What is possible with the IO object
typedef enum imageflow_io_mode {
	imageflow_io_mode_none = 0,
	imageflow_io_mode_read_sequential = 1,
	imageflow_io_mode_write_sequential = 2,
	imageflow_io_mode_read_seekable = 5,
	imageflow_io_mode_write_seekable = 6,
	imageflow_io_mode_read_write_seekable = 15,
} imageflow_io_mode;

///
/// Input or output?
typedef enum imageflow_direction {
	imageflow_direction_out = 8,
	imageflow_direction_in = 4,
} imageflow_direction;

///
/// When a resource should be closed/freed/cleaned up
///
typedef enum imageflow_cleanup_with {
	/// When the context is destroyed
	imageflow_cleanup_with_context = 0,
	/// When the first job that the item is associated with is destroyed. (Not yet implemented)
	imageflow_cleanup_with_first_job = 1,
} imageflow_cleanup_with;

///
/// How long the provided pointer/buffer will remain valid.
/// Callers must prevent the memory from being freed or moved until this contract expires.
///
typedef enum imageflow_lifetime {
	/// Pointer will outlive function call. (I.e, in .NET, the memory has been pinned through the end of the call, perhaps via the 'fixed' keyword)
	imageflow_lifetime_outlives_function_call = 0,
	/// Pointer will outlive context (Usually a GCHandle is required to pin an object for a longer time in C#)
	imageflow_lifetime_outlives_context = 1,
} imageflow_lifetime;

/// Creates and returns an imageflow context.
/// An imageflow context is required for all other imageflow API calls.
///
/// An imageflow context tracks
/// * error state
/// * error messages
/// * stack traces for errors (in C land, at least)
/// * context-managed memory allocations
/// * performance profiling information
///
/// **Contexts are not thread-safe!** Once you create a context, *you* are responsible for ensuring that it is never involved in two overlapping API calls.
///
/// Returns a null pointer if allocation fails.
struct imageflow_context* imageflow_context_create(void);

/// Begins the process of destroying the context, yet leaves error information intact
/// so that any errors in the tear-down process can be
/// debugged with imageflow_context_error_and_stacktrace.
///
/// Returns true if no errors occurred. Returns false if there were tear-down issues.
///
/// *Behavior is undefined if context is a null or invalid ptr.*
bool imageflow_context_begin_terminate(struct imageflow_context* context);

/// Destroys the imageflow context and frees the context object.
/// Only use this with contexts created using imageflow_context_create
///
/// Behavior is undefined if context is a null or invalid ptr; may segfault on free(NULL);
void imageflow_context_destroy(struct imageflow_context* context);

/// Returns true if the context is in an error state. You must immediately deal with the error,
/// as subsequent API calls will fail or cause undefined behavior until the error state is cleared
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
bool imageflow_context_has_error(struct imageflow_context* context);

/// Clear the error state. This assumes that you know which API call failed and the problem has
/// been resolved. Don't use this unless you're sure you've accounted for all possible
/// inconsistent state (and fully understand the code paths that led to the error).
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
void imageflow_context_clear_error(struct imageflow_context* context);

/// Prints the error messages and stacktrace to the given buffer
/// Happy(ish) path: Returns the length of the error message written to the buffer.
/// Sad path: Returns -1 if buffer_length was too small or buffer was nullptr.
/// full_file_path, if true, will display the directory associated with the files in each stack frame.
///
/// Please be accurate with the buffer length, or a buffer overflow will occur.
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
int64_t imageflow_context_error_and_stacktrace(struct imageflow_context* context, uint8_t* buffer, size_t buffer_length, bool full_file_path);

/// Returns the numeric code associated with the error.
///
/// ## Error codes
///
/// * 0 - No error condition.
/// * 10 - Out Of Memory condition (malloc/calloc/realloc failed).
/// * 20 - I/O error
/// * 30 - Invalid internal state (assertion failed; you found a bug).
/// * 40 - Error: Not implemented. (Feature not implemented).
/// * 50 - Invalid argument provided
/// * 51 - Null argument provided
/// * 52 - Invalid dimensions
/// * 53 - Unsupported pixel format
/// * 54 - Item does not exist
/// * 60 - Image decoding failed
/// * 61 - Image encoding failed
/// * 70 - Graph invalid
/// * 71 - Graph is cyclic
/// * 72 - Invalid inputs to node
/// * 73 - Maximum graph passes exceeded
/// * 1024 - Other error; something else happened
/// * 1025 through 2147483647 are reserved for user-defined errors
///
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
int32_t imageflow_context_error_code(struct imageflow_context* context);

/// Prints the error to stderr and exits the process if an error has been raised on the context.
/// If no error is present, the function returns false.
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
///
/// THIS PRINTS DIRECTLY TO STDERR! Do not use in any kind of service! Command-line usage only!
bool imageflow_context_print_and_exit_if_error(struct imageflow_context* context);

///
/// Raises an error on the context.
///
/// Returns `true` on success, `false`  if an error was already present.
///
/// Designed to be safe(ish) for use in out-of-memory scenarios; no additional allocations are made.
///
/// See `imageflow_context_error_code` for a list of error codes.
///
///
/// # Expectations
///
/// * All strings must be null-terminated, C-style, valid UTF-8.
/// * The lifetime of `message` is expected to exceed the duration of this function call.
/// * The lifetime of `file` and `function_name` (if provided), is expected to match or exceed the lifetime of `context`.
/// * You may provide a null value for `filename` or `function_name`, but for the love of puppies,
/// don't provide a dangling or invalid pointer, that will segfault... a long time later.
///
/// # Caveats
///
/// * You cannot raise a second error until the first has been cleared with
///  `imageflow_context_clear_error`. You'll be ignored, as will future
///   `imageflow_add_to_callstack` invocations.
/// * Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
/// * Behavior is undefined if `message` is an invalid ptr; immediate segfault likely.
/// * If you provide an error code of zero (why?!), a different error code will be provided.
bool imageflow_context_raise_error(struct imageflow_context* context, int32_t error_code, char const* message, char const* file, int32_t line, char const* function_name);

///
/// Adds the given filename, line number, and function name to the call stack.
///
/// Returns `true` if add was successful.
///
/// # Will fail and return false if...
///
/// * You haven't previously called `imageflow_context_raise_error`
/// * You tried to raise a second error without clearing the first one. Call will be ignored.
/// * You've exceeded the capacity of the call stack (which, at one point, was 14). But this
///   category of failure is acceptable.
///
///
/// # Expectations
///
/// * An error has been raised.
/// * You may provide a null value for `filename` or `function_name`, but for the love of puppies,
/// don't provide a dangling or invalid pointer, that will segfault... a long time later.
/// * The lifetime of `file` and `function_name` (if provided), is expected to match
///   or exceed the lifetime of `context`.
/// * All strings must be null-terminated, C-style, valid UTF-8.
///
/// # Caveats
///
/// * Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
///
bool imageflow_context_add_to_callstack(struct imageflow_context* context, char const* filename, int32_t line, char const* function_name);

///
/// Writes fields from the given imageflow_json_response to the locations referenced.
///
bool imageflow_json_response_read(struct imageflow_context* context, struct imageflow_json_response const* response_in, int64_t* status_code_out, uint8_t const** buffer_utf8_no_nulls_out, uintptr_t* buffer_size_out);

/// Frees memory associated with the given object (and owned objects) after
/// running any owned or attached destructors. Returns false if something went wrong during tear-down.
///
/// Returns true if the object to destroy is a null pointer, or if tear-down was successful.
///
/// Behavior is undefined if the pointer is dangling or not a valid memory reference.
/// Although certain implementations catch
/// some kinds of invalid pointers, a segfault is likely in future revisions).
///
/// Behavior is undefined if the context provided does not match the context with which the
/// object was created.
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
///
bool imageflow_json_response_destroy(struct imageflow_context* context, struct imageflow_json_response* response);

///
/// Sends a JSON message to the imageflow_context
///
/// The context is provided `method`, which determines which code path will be used to
/// process the provided JSON data and compose a response.
///
/// * `method` and `json_buffer` are only borrowed for the duration of the function call. You are
///    responsible for their cleanup (if necessary - static strings are handy for things like
///    `method`).
///
/// The function will return NULL if a JSON response could not be allocated (or if some other
/// bug occurred). If a null pointer is returned, consult the standard error methods of `context`
/// for more detail.
///
/// The response can be cleaned up with `imageflow_json_response_destroy`
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
struct imageflow_json_response const* imageflow_context_send_json(struct imageflow_context* context, int8_t const* method, uint8_t const* json_buffer, size_t json_buffer_size);

///
/// Sends a JSON message to the imageflow_job
///
/// The recipient is provided `method`, which determines which code path will be used to
/// process the provided JSON data and compose a response.
///
/// * `method` and `json_buffer` are only borrowed for the duration of the function call. You are
///    responsible for their cleanup (if necessary - static strings are handy for things like
///    `method`).
///
/// The function will return NULL if a JSON response could not be allocated (or if some other
/// bug occurred). If a null pointer is returned, consult the standard error methods of `context`
/// for more detail.
///
/// The response can be cleaned up with `imageflow_json_response_destroy`
///
/// Behavior is undefined if `context` is a null or invalid ptr; segfault likely.
struct imageflow_json_response const* imageflow_job_send_json(struct imageflow_context* context, struct imageflow_job* job, int8_t const* method, uint8_t const* json_buffer, size_t json_buffer_size);

///
/// Creates an imageflow_io object to wrap a filename.
///
/// If the filename is fopen compatible, you're probably OK.
///
/// As always, `mode` is not enforced except for the file open flags.
///
struct imageflow_job_io* imageflow_io_create_for_file(struct imageflow_context* context, imageflow_io_mode mode, char const* filename, imageflow_cleanup_with cleanup);

///
/// Creates an imageflow_io structure for reading from the provided buffer.
/// You are ALWAYS responsible for freeing the memory provided in accordance with the imageflow_lifetime value.
/// If you specify OutlivesFunctionCall, then the buffer will be copied.
///
///
struct imageflow_job_io* imageflow_io_create_from_buffer(struct imageflow_context* context, uint8_t const* buffer, size_t buffer_byte_count, imageflow_lifetime lifetime, imageflow_cleanup_with cleanup);

///
/// Creates an imageflow_io structure for writing to an expanding memory buffer.
///
/// Reads/seeks, are, in theory, supported, but unless you've written, there will be nothing to read.
///
/// The I/O structure and buffer will be freed with the context.
///
///
/// Returns null if allocation failed; check the context for error details.
struct imageflow_job_io* imageflow_io_create_for_output_buffer(struct imageflow_context* context);

///
/// Provides access to the underlying buffer for the given imageflow_io object.
///
/// Ensure your length variable always holds 64-bits.
///
bool imageflow_io_get_output_buffer(struct imageflow_context* context, struct imageflow_job_io* io, uint8_t const** result_buffer, uintptr_t* result_buffer_length);

///
/// Provides access to the underlying buffer for the given imageflow_io object.
///
/// Ensure your length variable always holds 64-bits
///
bool imageflow_job_get_output_buffer_by_id(struct imageflow_context* context, struct imageflow_job* job, int32_t io_id, uint8_t const** result_buffer, uintptr_t* result_buffer_length);

///
/// Creates an imageflow_job, which permits the association of imageflow_io instances with
/// numeric identifiers and provides a 'sub-context' for job execution
///
struct imageflow_job* imageflow_job_create(struct imageflow_context* context);

///
/// Looks up the imageflow_io pointer from the provided placeholder_id
///
struct imageflow_job_io* imageflow_job_get_io(struct imageflow_context* context, struct imageflow_job* job, int32_t placeholder_id);

///
/// Associates the imageflow_io object with the job and the assigned placeholder_id.
///
/// The placeholder_id will correspond with io_id in the graph
///
/// direction is in or out.
bool imageflow_job_add_io(struct imageflow_context* context, struct imageflow_job* job, struct imageflow_job_io* io, int32_t placeholder_id, imageflow_direction direction);

///
/// Destroys the provided imageflow_job
///
bool imageflow_job_destroy(struct imageflow_context* context, struct imageflow_job* job);

///
/// Allocates zeroed memory that will be freed with the context.
/// filename/line may be used for debugging purposes. They are optional. Provide null/-1 to skip.
///
/// Returns null(0) on failure.
///
void* imageflow_context_memory_allocate(struct imageflow_context* context, uintptr_t bytes, char const* filename, int32_t line);

///
/// Frees memory allocated with imageflow_context_memory_allocate early.
/// filename/line may be used for debugging purposes. They are optional. Provide null/-1 to skip.
///
/// Returns false on failure.
///
bool imageflow_context_memory_free(struct imageflow_context* context, void* pointer, char const* filename, int32_t line);



#ifdef __cplusplus
}
#endif


#endif
