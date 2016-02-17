#include "fastscaling_private.h"
#include "../imageflow.h"
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>



typedef enum flow_job_resource_type{
    flow_job_resource_bitmap_bgra = 1,

} flow_job_resource_type;

struct flow_job_resource_item{
    struct flow_job_resource_item * next;
    int32_t id;
    int32_t graph_placeholder_id;
    FLOW_DIRECTION direction;
    flow_job_resource_type type;
    void * data;
};

struct flow_job_state {
    bool * node_completed;
    BitmapBgra * * node_bitmap;
};

struct flow_job {
    bool record_graph_versions;
    int32_t next_graph_version;
    int32_t next_resource_id;
    struct flow_job_resource_item * resources_head;
    struct flow_job_resource_item * resources_tail;
    //struct flow_job_state job_state;
};


bool flow_job_create_state(Context *c, struct flow_job * job, struct flow_graph * g);
