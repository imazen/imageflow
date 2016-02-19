#pragma once

#include "../imageflow.h"
#include "fastscaling_private.h"
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include "png.h"


struct flow_job_codec_magic_bytes {
    flow_job_codec_type codec_type;
    size_t byte_count;
    uint8_t * bytes;
};

struct decoder_frame_info{
    int32_t w;
    int32_t h;

};


struct flow_job_resource_buffer{
    void * buffer;
    size_t buffer_size;
    bool owned_by_job;
    void * codec_state;
};


struct flow_job {
    bool record_graph_versions;
    int32_t next_graph_version;
    int32_t next_resource_id;
    int32_t max_calc_flatten_execute_passes;
    struct flow_job_resource_item * resources_head;
    struct flow_job_resource_item * resources_tail;
};



typedef void * (*codec_aquire_on_buffer_fn)(Context *c, struct flow_job * job, struct flow_job_resource_buffer * buffer);

typedef bool (*codec_get_frame_info_fn)(Context *c, struct flow_job * job, void * codec_state, struct decoder_frame_info * decoder_frame_info_ref);

typedef bool (*codec_read_frame_fn)(Context *c, struct flow_job * job, void * codec_state, BitmapBgra * canvas);

typedef bool (*codec_dispose_fn)(Context *c, struct flow_job * job, void * codec_state);

typedef bool (*codec_stringify_fn)(Context *c,  struct flow_job * job, void * codec_state, char * buffer, size_t buffer_size);


struct flow_job_codec_definition{
    flow_job_codec_type type;
    codec_aquire_on_buffer_fn aquire_on_buffer;
    codec_get_frame_info_fn  get_frame_info;
    codec_read_frame_fn read_frame;
    codec_dispose_fn  dispose;
    codec_stringify_fn  stringify;
    const char * name;

};


//typedef unsigned long png_uint_32;

struct flow_job_codec_definition * flow_job_get_codec_definition(Context *c, flow_job_codec_type type);
flow_job_codec_type flow_job_codec_select(Context *c, struct flow_job * job,  uint8_t * data, size_t data_bytes);

void * flow_job_acquire_decoder_over_buffer(Context *c, struct flow_job *job,
                                                   struct flow_job_resource_buffer *buffer, flow_job_codec_type type);


bool flow_job_decoder_get_frame_info(Context *c, struct flow_job * job, void * codec_state, flow_job_codec_type type, struct decoder_frame_info * decoder_frame_info_ref);

bool flow_job_decoder_read_frame(Context *c, struct flow_job * job, void * codec_state,  flow_job_codec_type type, BitmapBgra * canvas);

struct flow_job_resource_item{
    struct flow_job_resource_item * next;
    int32_t id;
    int32_t graph_placeholder_id;
    FLOW_DIRECTION direction;
    flow_job_resource_type type;
    flow_job_codec_type codec_type;
    void * codec_state;
    void * data;
};
