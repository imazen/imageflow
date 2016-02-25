#pragma once

#include "../imageflow.h"
#include "fastscaling_private.h"
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <errno.h>
#include "png.h"

struct flow_job_codec_magic_bytes {
    flow_job_codec_type codec_type;
    size_t byte_count;
    uint8_t* bytes;
};

struct decoder_frame_info {
    int32_t w;
    int32_t h;
};

struct flow_job {
    int32_t debug_job_id;
    int32_t next_graph_version;
    int32_t next_resource_id;
    int32_t max_calc_flatten_execute_passes;
    struct flow_job_resource_item* resources_head;
    struct flow_job_resource_item* resources_tail;
    bool record_graph_versions;
    bool record_frame_images;
    bool render_graph_versions;
    bool render_animated_graph;
    bool render_last_graph;
};

void flow_utils_ensure_directory_exists(const char* dir_path);

bool flow_job_render_graph_to_png(Context* c, struct flow_job* job, struct flow_graph* g, int32_t graph_version);
bool flow_job_notify_node_complete(Context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id);

typedef void* (*codec_aquire_on_buffer_fn)(Context* c, struct flow_job* job, struct flow_job_resource_buffer* buffer);

typedef bool (*codec_get_frame_info_fn)(Context* c, struct flow_job* job, void* codec_state,
                                        struct decoder_frame_info* decoder_frame_info_ref);

typedef bool (*codec_read_frame_fn)(Context* c, struct flow_job* job, void* codec_state, BitmapBgra* canvas);

typedef bool (*codec_write_frame_fn)(Context* c, struct flow_job* job, void* codec_state, BitmapBgra* frame);

typedef bool (*codec_dispose_fn)(Context* c, struct flow_job* job, void* codec_state);

typedef bool (*codec_stringify_fn)(Context* c, struct flow_job* job, void* codec_state, char* buffer,
                                   size_t buffer_size);

struct flow_job_codec_definition {
    flow_job_codec_type type;
    codec_aquire_on_buffer_fn aquire_on_buffer;
    codec_get_frame_info_fn get_frame_info;
    codec_read_frame_fn read_frame;
    codec_write_frame_fn write_frame;
    codec_dispose_fn dispose;
    codec_stringify_fn stringify;
    const char* name;
};

// typedef unsigned long png_uint_32;

struct flow_job_codec_definition* flow_job_get_codec_definition(Context* c, flow_job_codec_type type);
flow_job_codec_type flow_job_codec_select(Context* c, struct flow_job* job, uint8_t* data, size_t data_bytes);

void* flow_job_acquire_decoder_over_buffer(Context* c, struct flow_job* job, struct flow_job_resource_buffer* buffer,
                                           flow_job_codec_type type);

bool flow_job_decoder_get_frame_info(Context* c, struct flow_job* job, void* codec_state, flow_job_codec_type type,
                                     struct decoder_frame_info* decoder_frame_info_ref);

bool flow_job_decoder_read_frame(Context* c, struct flow_job* job, void* codec_state, flow_job_codec_type type,
                                 BitmapBgra* canvas);

struct flow_job_resource_item {
    struct flow_job_resource_item* next;
    int32_t id;
    int32_t graph_placeholder_id;
    FLOW_DIRECTION direction;
    flow_job_resource_type type;
    flow_job_codec_type codec_type;
    void* codec_state;
    void* data;
};
