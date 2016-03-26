#pragma once

#include "imageflow_private.h"
#include "png.h"
#include "lcms2.h"
#include "codecs.h"
#include "nodes.h"

#ifdef __cplusplus
extern "C" {
#endif


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


bool flow_job_render_graph_to_png(flow_context* c, struct flow_job* job, struct flow_graph* g, int32_t graph_version);
bool flow_job_notify_node_complete(flow_context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id);
bool flow_job_initialize_input_resource(flow_context* c, struct flow_job* job, struct flow_job_resource_item* item);

#ifdef __cplusplus
}
#endif
