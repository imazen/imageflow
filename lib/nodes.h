#pragma once

#include "imageflow.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef bool (*flow_nodedef_fn_stringify)(flow_c * c, struct flow_graph * g, int32_t node_id, char * buffer,
                                          size_t buffer_size);

typedef bool (*flow_nodedef_fn_infobyte_count)(flow_c * c, struct flow_graph * g, int32_t node_id,
                                               int32_t * infobytes_count_out);

typedef bool (*flow_nodedef_fn_populate_dimensions)(flow_c * c, struct flow_graph * g, int32_t node_id,
                                                    bool force_estimate);

typedef bool (*flow_nodedef_fn_flatten)(flow_c * c, struct flow_graph ** graph_ref, int32_t node_id);

typedef bool (*flow_nodedef_fn_flatten_shorthand)(flow_c * c, struct flow_graph ** graph_ref, int32_t node_id,
                                                  struct flow_node * node, struct flow_node * input_node,
                                                  int32_t * first_replacement_node, int32_t * last_replacement_node);

typedef bool (*flow_nodedef_fn_execute)(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id);

typedef bool (*flow_nodedef_fn_estimate_cost)(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id,
                                              size_t * bytes_required, size_t * cpu_cost);

struct flow_node_definition {
    flow_ntype type;
    int32_t input_count;
    bool prohibit_output_edges;
    int32_t canvas_count;
    const char * type_name;

    flow_nodedef_fn_stringify stringify;
    flow_nodedef_fn_infobyte_count count_infobytes;
    int32_t nodeinfo_bytes_fixed;
    flow_nodedef_fn_populate_dimensions populate_dimensions;
    flow_nodedef_fn_flatten pre_optimize_flatten_complex;
    flow_nodedef_fn_flatten_shorthand pre_optimize_flatten;
    flow_nodedef_fn_flatten post_optimize_flatten_complex;
    flow_nodedef_fn_flatten_shorthand post_optimize_flatten;
    flow_nodedef_fn_execute execute;
    flow_nodedef_fn_estimate_cost estimate_cost;
};

struct flow_node_definition * flow_nodedef_get(flow_c * c, flow_ntype type);

//!Throws an error and returns null if node_id does not represent a valid, non-null node
struct flow_node * flow_node_get(flow_c * c, struct flow_graph * g, int32_t node_id);

//!Throws an error if node_id does not represent a valid, non-null node, or if there are no infobytes, or the infobyte
// size does not match sizeof_infobytes_struct
void * flow_node_get_infobytes_pointer(flow_c * c, struct flow_graph * g, int32_t node_id,
                                       size_t sizeof_infobytes_struct);

bool flow_node_stringify(flow_c * c, struct flow_graph * g, int32_t node_id, char * buffer, size_t buffer_size);

int32_t flow_node_fixed_infobyte_count(flow_c * c, flow_ntype type);
bool flow_node_infobyte_count(flow_c * c, struct flow_graph * g, int32_t node_id, int32_t * infobytes_count_out);
bool flow_node_populate_dimensions(flow_c * c, struct flow_graph * g, int32_t node_id, bool force_estimate);
bool flow_node_pre_optimize_flatten(flow_c * c, struct flow_graph ** graph_ref, int32_t node_id);
bool flow_node_execute(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id);
bool flow_node_estimate_execution_cost(flow_c * c, struct flow_graph * g, int32_t node_id, size_t * bytes_required,
                                       size_t * cpu_cost);
bool flow_node_validate_edges(flow_c * c, struct flow_graph * g, int32_t node_id);
bool flow_node_update_state(flow_c * c, struct flow_graph * g, int32_t node_id);

#ifdef __cplusplus
}
#endif
