#pragma once

#include "imageflow.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef bool (*flow_nodedef_fn_stringify)(flow_context* c, struct flow_graph* g, int32_t node_id, char* buffer,
                                          size_t buffer_size);

typedef bool (*flow_nodedef_fn_infobyte_count)(flow_context* c, struct flow_graph* g, int32_t node_id,
                                               int32_t* infobytes_count_out);

typedef bool (*flow_nodedef_fn_populate_dimensions)(flow_context* c, struct flow_graph* g, int32_t node_id,
                                                    int32_t outbound_edge_id, bool force_estimate);

typedef bool (*flow_nodedef_fn_flatten)(flow_context* c, struct flow_graph** graph_ref, int32_t node_id);

typedef bool (*flow_nodedef_fn_flatten_shorthand)(flow_context* c, struct flow_graph** graph_ref, int32_t node_id,
                                                  struct flow_node* node, struct flow_edge* input_edge,
                                                  int32_t* first_replacement_node, int32_t* last_replacement_node);

typedef bool (*flow_nodedef_fn_execute)(flow_context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id);

typedef bool (*flow_nodedef_fn_estimate_cost)(flow_context* c, struct flow_job* job, struct flow_graph* g,
                                              int32_t node_id, size_t* bytes_required, size_t* cpu_cost);

struct flow_node_definition {
    flow_ntype type;
    int32_t input_count;
    int32_t canvas_count;
    const char* type_name;

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

struct flow_node_definition* flow_nodedef_get(flow_context* c, flow_ntype type);

#define FLOW_GET_INFOBYTES(g, node_id, type, varname)                                                                  \
    struct type* varname = (struct type*)&g->info_bytes[g->nodes[node_id].info_byte_index];

bool flow_node_stringify(flow_context* c, struct flow_graph* g, int32_t node_id, char* buffer, size_t buffer_size);
int32_t flow_node_fixed_infobyte_count(flow_context* c, flow_ntype type);
bool flow_node_infobyte_count(flow_context* c, struct flow_graph* g, int32_t node_id, int32_t* infobytes_count_out);
bool flow_node_populate_dimensions_to_edge(flow_context* c, struct flow_graph* g, int32_t node_id,
                                           int32_t outbound_edge_id, bool force_estimate);
bool flow_node_pre_optimize_flatten(flow_context* c, struct flow_graph** graph_ref, int32_t node_id);
bool flow_node_execute(flow_context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id);
bool flow_node_estimate_execution_cost(flow_context* c, struct flow_graph* g, int32_t node_id, size_t* bytes_required,
                                       size_t* cpu_cost);
bool flow_node_validate_inputs(flow_context* c, struct flow_graph* g, int32_t node_id);
bool flow_node_update_state(flow_context* c, struct flow_graph* g, int32_t node_id);

#ifdef __cplusplus
}
#endif
