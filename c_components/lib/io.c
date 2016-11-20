#include "imageflow_private.h"

// struct flow_io * flow_io_create_for_file(flow_context * c, flow_io_mode mode, const char *filename, void * owner);

////////////////////////////////////////////////////////////////////////
// flow_io_create_for_output_buffer section

struct flow_io_obuf {
    uint8_t * buffer;
    int64_t cursor;
    size_t uncleared_memory_begins;
    size_t length;
};

bool flow_io_get_output_buffer(flow_c * c, struct flow_io * io, uint8_t ** out_pointer_to_buffer, size_t * out_length)
{
    struct flow_io_obuf * state = (struct flow_io_obuf *)io->user_data;
    *out_pointer_to_buffer = state->buffer;
    *out_length = state->uncleared_memory_begins;
    return true;
}

bool flow_io_write_output_buffer_to_file(flow_c * c, struct flow_io * io, const char * file_path)
{
    struct flow_io_obuf * state = (struct flow_io_obuf *)io->user_data;
    FILE * fh = fopen(file_path, "wb");
    if (fh != NULL) {
        int64_t expected_items_written = state->uncleared_memory_begins;
        int64_t items_written = fwrite(state->buffer, 1, expected_items_written, fh);
        if (items_written != expected_items_written) {
            FLOW_error_msg(c, flow_status_IO_error,
                           "Failed to write buffer to file %s. fwrite returned %i instead of %i. errno=%i", file_path,
                           items_written, expected_items_written, errno);
            fclose(fh);
            return false;
        }
    } else {
        FLOW_error_msg(c, flow_status_IO_error, "Failed to open file %s for binary writing", file_path);
        return false;
    }
    if (fclose(fh) != 0) {
        FLOW_error_msg(c, flow_status_IO_error, "An error ocurred while closing file %s.", file_path);
        return false;
    }
    return true;
}
static bool flow_io_obuf_dispose(flow_c * c, void * io)
{
    // nada, we're using ownership :)
    return true;
}
static bool flow_io_obuf_seek(flow_c * c, struct flow_io * io, int64_t position)
{
    struct flow_io_obuf * state = (struct flow_io_obuf *)io->user_data;
    if (position < 0 || position > (int64_t)state->uncleared_memory_begins) {
        FLOW_error_msg(c, flow_status_IO_error,
                       "Codec tried to seek to position %" PRId64 " - valid values are between 0 and "
                       "%" PRId64 " inclusive (You cannot seek past the written area of an output "
                       "buffer).",
                       position, state->length);
        return false;
    }
    state->cursor = position;
    return true;
}
static int64_t flow_io_obuf_position(flow_c * c, struct flow_io * io)
{
    return ((struct flow_io_obuf *)io->user_data)->cursor;
}
// Returns the number of bytes read into the buffer. Failure to read 'count' bytes could mean EOF or failure. Check
// context status. Pass NULL to buffer if you want to skip 'count' many bytes, seeking ahead.
static int64_t flow_io_obuf_read(flow_c * c, struct flow_io * io, uint8_t * buffer, size_t count)
{
    struct flow_io_obuf * state = (struct flow_io_obuf *)io->user_data;
    int64_t allowed_count = state->uncleared_memory_begins - state->cursor;
    if (allowed_count < 0)
        allowed_count = 0;
    if (allowed_count > (int64_t)count)
        allowed_count = count;

    if (buffer == NULL) {
        if (!flow_io_obuf_seek(c, io, state->cursor + allowed_count)) {
            FLOW_add_to_callstack(c);
            return 0;
        }
        return allowed_count;
    } else {
        if (allowed_count > 0) {
            memcpy(buffer, &state->buffer[state->cursor], allowed_count);
            state->cursor += allowed_count;
        }
        return allowed_count;
    }
}
// Returns the number of bytes written. If it doesn't equal 'count', there was an error. Check context status
static int64_t flow_io_obuf_write(flow_c * c, struct flow_io * io, const uint8_t * buffer, size_t count)
{
    struct flow_io_obuf * state = (struct flow_io_obuf *)io->user_data;
    if (buffer == NULL) {
        FLOW_error_msg(c, flow_status_Null_argument, "Buffer pointer was null");
        return 0;
    }
    if (state->length - state->cursor < count) {
        // We need to expand the buffer at least enough to accomodate this write
        size_t new_size = state->cursor + count;
        // Increase the buffer size by a minimum of 50% each time
        if (new_size < (state->length * 3 / 2))
            new_size = state->length * 3 / 2 + 1;
        // And always a minimum of 4kb
        if (new_size < 4096)
            new_size = 4096;

        // Normally this would be a memory leak on failure, but our objtracker will dispose of the old buffer if
        // non-null
        state->buffer = (uint8_t *)FLOW_realloc(c, state->buffer, new_size);

        if (state->buffer == NULL) {
            FLOW_error_msg(c, flow_status_Out_of_memory, "Failed to allocate %" PRIu64 " bytes for output buffer",
                           new_size);
            return 0;
        } else {
            state->length = new_size;
        }
    }
    memcpy(&state->buffer[state->cursor], buffer, count);
    state->cursor += count;
    if (state->uncleared_memory_begins < (size_t)state->cursor) {
        state->uncleared_memory_begins = state->cursor;
    }
    io->optional_file_length = state->uncleared_memory_begins;
    return count;
}

struct flow_io * flow_io_create_for_output_buffer(flow_c * c, void * owner)
{
    struct flow_io * io = (struct flow_io *)FLOW_malloc_owned(c, sizeof(struct flow_io), owner);
    if (io == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        return NULL;
    }
    io->user_data = FLOW_malloc_owned(c, sizeof(struct flow_io_obuf), io);
    if (io->user_data == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        FLOW_destroy(c, io);
        return NULL;
    }

    io->context = c;
    io->dispose_func = flow_io_obuf_dispose;
    io->write_func = flow_io_obuf_write;
    io->read_func = flow_io_obuf_read;
    io->seek_function = flow_io_obuf_seek;
    io->position_func = flow_io_obuf_position;
    io->mode = flow_io_mode_read_write_seekable;

    struct flow_io_obuf * info = (struct flow_io_obuf *)io->user_data;
    info->buffer = NULL;
    info->length = 0;
    info->uncleared_memory_begins = 0;
    info->cursor = 0;
    return io;
}

////////////////////////////////////////////////////////////////////////
// flow_io_fro_memory section

struct flow_io_memory {
    uint8_t * memory;
    size_t length;
    flow_destructor_function free;
    int64_t cursor;
};

// Return false if something goes wrong.
static bool flow_io_memory_dispose(flow_c * c, void * io)
{
    struct flow_io_memory * mem_struct = (struct flow_io_memory *)((struct flow_io *)io)->user_data;
    if (mem_struct == NULL)
        return false;
    bool success = true;
    if (mem_struct->free != NULL) {
        success = mem_struct->free(c, mem_struct->memory);
        mem_struct->length = 0;
        mem_struct->memory = NULL;
        mem_struct->cursor = 0;
    }
    return success;
}

static bool flow_io_memory_seek(flow_c * c, struct flow_io * io, int64_t position)
{
    struct flow_io_memory * state = (struct flow_io_memory *)io->user_data;
    if (position < 0 || position > (int64_t)state->length) {
        FLOW_error_msg(c, flow_status_IO_error,
                       "Codec tried to seek to position %" PRId64 " - valid values are between 0 and "
                       "%" PRId64 " inclusive (fixed-size memory buffer).",
                       position, state->length);
        return false;
    }
    state->cursor = position;
    return true;
}

// Returns negative on failure - check context for more detail. Returns the current position in the stream when
// successful
static int64_t flow_io_memory_position(flow_c * c, struct flow_io * io)
{
    return ((struct flow_io_memory *)io->user_data)->cursor;
}
// Returns the number of bytes read into the buffer. Failure to read 'count' bytes could mean EOF or failure. Check
// context status. Pass NULL to buffer if you want to skip 'count' many bytes, seeking ahead.
static int64_t flow_io_memory_read(flow_c * c, struct flow_io * io, uint8_t * buffer, size_t count)
{
    struct flow_io_memory * state = (struct flow_io_memory *)io->user_data;
    int64_t allowed_count = state->length - state->cursor;
    if (allowed_count < 0)
        allowed_count = 0;
    if (allowed_count > (int64_t)count)
        allowed_count = count;

    if (buffer == NULL) {
        if (!flow_io_memory_seek(c, io, state->cursor + allowed_count)) {
            FLOW_add_to_callstack(c);
            return 0;
        }
        return allowed_count;
    } else {
        memcpy(buffer, &state->memory[state->cursor], allowed_count);
        state->cursor += allowed_count;
        return allowed_count;
    }
}
// Returns the number of bytes written. If it doesn't equal 'count', there was an error. Check context status
static int64_t flow_io_memory_write(flow_c * c, struct flow_io * io, const uint8_t * buffer, size_t count)
{
    struct flow_io_memory * state = (struct flow_io_memory *)io->user_data;
    if (buffer == NULL) {
        FLOW_error_msg(c, flow_status_Null_argument, "Buffer pointer was null");
        return 0;
    }
    if (state->length - state->cursor > count) {
        FLOW_error_msg(c, flow_status_IO_error, "Tried to write past end of memory buffer");
        return 0;
    }
    memcpy(&state->memory[state->cursor], buffer, count);
    state->cursor += count;
    return count;
}

struct flow_io * flow_io_create_from_memory(flow_c * c, flow_io_mode mode, uint8_t * memory, size_t length,
                                            void * owner, flow_destructor_function memory_free)
{
    struct flow_io * io = (struct flow_io *)FLOW_malloc_owned(c, sizeof(struct flow_io), owner);
    if (io == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        return NULL;
    }
    io->user_data = FLOW_malloc_owned(c, sizeof(struct flow_io_memory), io);
    if (io->user_data == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        FLOW_destroy(c, io);
        return NULL;
    }

    io->context = c;
    io->dispose_func = flow_io_memory_dispose;
    io->write_func = flow_io_memory_write;
    io->read_func = flow_io_memory_read;
    io->seek_function = flow_io_memory_seek;
    io->position_func = flow_io_memory_position;
    io->mode = mode;
    io->optional_file_length = length;

    struct flow_io_memory * mem_struct = (struct flow_io_memory *)io->user_data;
    mem_struct->memory = memory;
    mem_struct->length = length;
    mem_struct->free = memory_free;
    mem_struct->cursor = 0;
    return io;
}

////////////////////////////////////////////////////////////////////////
// flow_io_from_file_ptr section

struct flow_io_fileptr {
    FILE * fp;
    int64_t cursor;
};

// Return false if something goes wrong.
static bool flow_io_fileptr_dispose(flow_c * c, void * io)
{
    struct flow_io_fileptr * state = (struct flow_io_fileptr *)((struct flow_io *)io)->user_data;
    if (state == NULL)
        return false;
    bool success = true;
    if (state->fp != NULL) {
        int result = fflush(state->fp);
        if (result != 0) {
            int error = ferror(state->fp);

            FLOW_error_msg(c, flow_status_IO_error,
                           "Codec tried to dispose flow_io, but fflush failed with ferror %d. ", error);
            success = false;
        }
    }
    return success;
}

static bool flow_io_fileptr_seek(flow_c * c, struct flow_io * io, int64_t position)
{
    struct flow_io_fileptr * state = (struct flow_io_fileptr *)io->user_data;
    if (position < 0) {
        FLOW_error_msg(c, flow_status_IO_error, "Codec tried to seek to %" PRId64 " - valid values are 0 and above",
                       position);
        return false;
    }
    int64_t result = fseek(state->fp, position, SEEK_SET);
    if (result != 0) {
        int error = ferror(state->fp);

        FLOW_error_msg(c, flow_status_IO_error,
                       "Codec tried to seek to %" PRId64 " - fseek failed with %d, ferror=%d, errno=%d", position,
                       result, error, errno);
        return false;
    }
    state->cursor = position;
    return true;
}

// Returns negative on failure - check context for more detail. Returns the current position in the stream when
// successful
static int64_t flow_io_fileptr_position(flow_c * c, struct flow_io * io)
{
    struct flow_io_fileptr * state = (struct flow_io_fileptr *)io->user_data;
    int64_t result = ftell(state->fp);
    if (result == -1L) {
        FLOW_error_msg(c, flow_status_IO_error,
                       "Codec tried to access the current position, but ftell failed with errno=%d", errno);
        return result;
    }
    return result; //((struct flow_io_fileptr *)io->user_data)->cursor;
}
// Returns the number of bytes read into the buffer. Failure to read 'count' bytes could mean EOF or failure. Check
// context status. Pass NULL to buffer if you want to skip 'count' many bytes, seeking ahead.
static int64_t flow_io_fileptr_read(flow_c * c, struct flow_io * io, uint8_t * buffer, size_t count)
{
    struct flow_io_fileptr * state = (struct flow_io_fileptr *)io->user_data;
    if (buffer == NULL) {
        if (!flow_io_fileptr_seek(c, io, state->cursor + count)) {
            FLOW_add_to_callstack(c);
            return 0;
        }
        return count;
    } else {
        int64_t read_bytes = fread(buffer, 1, count, state->fp);
        if (read_bytes != (int64_t)count) {
            int error = ferror(state->fp);
            if (error != 0) {
                FLOW_error_msg(c, flow_status_IO_error,
                               "Codec tried to read %" PRIu64 " bytes, but fread failed with ferror %d. %" PRIu64
                               " bytes read successfully",
                               count, error, read_bytes);
            }
        }
        state->cursor += read_bytes;
        return read_bytes;
    }
}
// Returns the number of bytes written. If it doesn't equal 'count', there was an error. Check context status
static int64_t flow_io_fileptr_write(flow_c * c, struct flow_io * io, const uint8_t * buffer, size_t count)
{
    struct flow_io_fileptr * state = (struct flow_io_fileptr *)io->user_data;
    if (buffer == NULL) {
        FLOW_error_msg(c, flow_status_Null_argument, "Buffer pointer was null");
        return 0;
    }
    size_t bytes_written = fwrite(buffer, 1, count, state->fp);
    if (bytes_written != count) {
        int error = ferror(state->fp);

        FLOW_error_msg(c, flow_status_IO_error, "Codec tried to write %" PRIu64
                                                " bytes, but write failed with ferror %d. %" PRIu64 " bytes written",
                       count, error, bytes_written);
    }
    state->cursor += bytes_written;
    return bytes_written;
}

struct flow_io * flow_io_create_from_file_pointer(flow_c * c, flow_io_mode mode, FILE * file_pointer,
                                                  int64_t optional_file_length, void * owner)
{
    struct flow_io * io = (struct flow_io *)FLOW_malloc_owned(c, sizeof(struct flow_io), owner);
    if (io == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        return NULL;
    }
    io->user_data = FLOW_malloc_owned(c, sizeof(struct flow_io_fileptr), io);
    if (io->user_data == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        FLOW_destroy(c, io);
        return NULL;
    }

    if (!flow_set_destructor(c, io, flow_io_fileptr_dispose)) {
        FLOW_destroy(c, io);
        return NULL;
    }

    io->context = c;
    io->dispose_func = flow_io_fileptr_dispose;
    io->write_func = flow_io_fileptr_write;
    io->read_func = flow_io_fileptr_read;
    io->seek_function = flow_io_fileptr_seek;
    io->position_func = flow_io_fileptr_position;
    io->mode = mode;
    io->optional_file_length = optional_file_length;

    struct flow_io_fileptr * state = (struct flow_io_fileptr *)io->user_data;
    state->fp = file_pointer;
    state->cursor = 0;
    return io;
}

////////////////////////////////////////////////////////////////////////
// flow_io_filename section

struct flow_io_filename {
    struct flow_io_fileptr ifp;
    const char * name;
};

// Return false if something goes wrong.
static bool flow_io_filename_dispose(flow_c * c, void * io)
{
    struct flow_io_filename * state = (struct flow_io_filename *)((struct flow_io *)io)->user_data;
    if (state == NULL)
        return false;
    bool success = flow_io_fileptr_dispose(c, io);
    if (state->ifp.fp != NULL) {
        int result = fclose(state->ifp.fp);
        state->ifp.fp = NULL;
        if (result != 0) {
            FLOW_error_msg(c, flow_status_IO_error,
                           "Codec tried to dispose flow_io, but fclose failed with error %d, errno=%d ", result, errno);
            success = false;
        }
    }
    return success;
}

struct flow_io * flow_io_create_for_file(flow_c * c, flow_io_mode mode, const char * filename, void * owner)
{
    struct flow_io * io = (struct flow_io *)FLOW_malloc_owned(c, sizeof(struct flow_io), owner);
    if (io == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        return NULL;
    }
    io->user_data = FLOW_malloc_owned(c, sizeof(struct flow_io_filename), io);
    if (io->user_data == NULL) {
        FLOW_error(c, flow_status_Out_of_memory);
        FLOW_destroy(c, io);
        return NULL;
    }
    if (!flow_set_destructor(c, io, flow_io_filename_dispose)) {
        FLOW_destroy(c, io);
        return NULL;
    }

    io->context = c;
    io->dispose_func = flow_io_filename_dispose;
    io->write_func = flow_io_fileptr_write;
    io->read_func = flow_io_fileptr_read;
    io->seek_function = flow_io_fileptr_seek;
    io->position_func = flow_io_fileptr_position;
    io->mode = mode;
    io->optional_file_length = -1;

    char * file_mode = "rb";
    if ((mode & flow_io_mode_write_sequential) == flow_io_mode_write_sequential)
        file_mode = "wb";
    if (mode == flow_io_mode_read_write_seekable)
        file_mode = "rb+";

    struct flow_io_filename * state = (struct flow_io_filename *)io->user_data;
    state->name = filename; // [TODO] this string's lifetime will not outlive the function call. Danger
    state->ifp.cursor = 0;
    state->ifp.fp = fopen(filename, file_mode);
    if (state->ifp.fp == NULL) {
        FLOW_destroy(c, io->user_data);
        FLOW_destroy(c, io);
        FLOW_error_msg(c, flow_status_IO_error,
                       "Codec tried to open file %s, but fopen(filename, \"%s\") failed with errno=%d ", filename,
                       file_mode, errno);
        return NULL;
    }
    return io;
}
