#include "imageflow_private.h"

// struct flow_io * flow_io_create_for_file(flow_context * c, flow_io_mode mode, const char *filename, void * owner);

////////////////////////////////////////////////////////////////////////
// flow_io_create_for_output_buffer section

struct flow_io_obuf {
    uint8_t* buffer;
    int64_t cursor;
    size_t uncleared_memory_begins;
    size_t length;
};

bool flow_io_get_output_buffer(flow_c* c, struct flow_io* io, uint8_t** out_pointer_to_buffer, size_t* out_length)
{
    struct flow_io_obuf* state = (struct flow_io_obuf*)io->user_data;
    *out_pointer_to_buffer = state->buffer;
    *out_length = state->uncleared_memory_begins;
    return true;
}

bool flow_io_write_output_buffer_to_file(flow_c* c, struct flow_io* io, const char* file_path)
{
    struct flow_io_obuf* state = (struct flow_io_obuf*)io->user_data;
    FILE* fh = fopen(file_path, "wb");
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
static bool flow_io_obuf_dispose(flow_c* c, void* io)
{
    // nada, we're using ownership :)
    return true;
}
static bool flow_io_obuf_seek(flow_c* c, struct flow_io* io, int64_t position)
{
    struct flow_io_obuf* state = (struct flow_io_obuf*)io->user_data;
    if (position < 0 || position > (int64_t)state->uncleared_memory_begins) {
        FLOW_error_msg(c, flow_status_IO_error, "Codec tried to seek to position %il - valid values are between 0 and "
                                                "%il inclusive (You cannot seek past the written area of an output "
                                                "buffer).",
                       position, state->length);
        return false;
    }
    state->cursor = position;
    return true;
}
static int64_t flow_io_obuf_position(flow_c* c, struct flow_io* io)
{
    return ((struct flow_io_obuf*)io->user_data)->cursor;
}
// Returns the number of bytes read into the buffer. Failure to read 'count' bytes could mean EOF or failure. Check
// context status. Pass NULL to buffer if you want to skip 'count' many bytes, seeking ahead.
static int64_t flow_io_obuf_read(flow_c* c, struct flow_io* io, uint8_t* buffer, size_t count)
{
    struct flow_io_obuf* state = (struct flow_io_obuf*)io->user_data;
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
static int64_t flow_io_obuf_write(flow_c* c, struct flow_io* io, const uint8_t* buffer, size_t count)
{
    struct flow_io_obuf* state = (struct flow_io_obuf*)io->user_data;
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
        state->buffer = (uint8_t*)FLOW_realloc(c, state->buffer, new_size);

        if (state->buffer == NULL) {
            FLOW_error_msg(c, flow_status_Out_of_memory, "Failed to allocate %ul bytes for output buffer", new_size);
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

struct flow_io* flow_io_create_for_output_buffer(flow_c* c, void* owner)
{
    struct flow_io* io = (struct flow_io*)FLOW_malloc_owned(c, sizeof(struct flow_io), owner);
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

    struct flow_io_obuf* info = (struct flow_io_obuf*)io->user_data;
    info->buffer = NULL;
    info->length = 0;
    info->uncleared_memory_begins = 0;
    info->cursor = 0;
    return io;
}

////////////////////////////////////////////////////////////////////////
// flow_io_fro_memory section

struct flow_io_memory {
    uint8_t* memory;
    size_t length;
    flow_destructor_function free;
    int64_t cursor;
};

// Return false if something goes wrong.
static bool flow_io_memory_dispose(flow_c* c, void* io)
{
    struct flow_io_memory* mem_struct = (struct flow_io_memory*)((struct flow_io*)io)->user_data;
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

static bool flow_io_memory_seek(flow_c* c, struct flow_io* io, int64_t position)
{
    struct flow_io_memory* state = (struct flow_io_memory*)io->user_data;
    if (position < 0 || position > (int64_t)state->length) {
        FLOW_error_msg(c, flow_status_IO_error, "Codec tried to seek to position %l - valid values are between 0 and "
                                                "%l inclusive (fixed-size memory buffer).",
                       position, state->length);
        return false;
    }
    state->cursor = position;
    return true;
}

// Returns negative on failure - check context for more detail. Returns the current position in the stream when
// successful
static int64_t flow_io_memory_position(flow_c* c, struct flow_io* io)
{
    return ((struct flow_io_memory*)io->user_data)->cursor;
}
// Returns the number of bytes read into the buffer. Failure to read 'count' bytes could mean EOF or failure. Check
// context status. Pass NULL to buffer if you want to skip 'count' many bytes, seeking ahead.
static int64_t flow_io_memory_read(flow_c* c, struct flow_io* io, uint8_t* buffer, size_t count)
{
    struct flow_io_memory* state = (struct flow_io_memory*)io->user_data;
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
static int64_t flow_io_memory_write(flow_c* c, struct flow_io* io, const uint8_t* buffer, size_t count)
{
    struct flow_io_memory* state = (struct flow_io_memory*)io->user_data;
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

struct flow_io* flow_io_create_from_memory(flow_c* c, flow_io_mode mode, uint8_t* memory, size_t length, void* owner,
                                           flow_destructor_function memory_free)
{
    struct flow_io* io = (struct flow_io*)FLOW_malloc_owned(c, sizeof(struct flow_io), owner);
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

    struct flow_io_memory* mem_struct = (struct flow_io_memory*)io->user_data;
    mem_struct->memory = memory;
    mem_struct->length = length;
    mem_struct->free = memory_free;
    mem_struct->cursor = 0;
    return io;
}
