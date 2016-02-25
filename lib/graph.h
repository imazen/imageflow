#pragma once

#include "fastscaling_private.h"
#include "../imageflow.h"
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include "math_functions.h"

typedef bool (*flow_nodedef_fn_stringify)(Context* c, struct flow_graph* g, int32_t node_id, char* buffer,
                                          size_t buffer_size);

typedef bool (*flow_nodedef_fn_infobyte_count)(Context* c, struct flow_graph* g, int32_t node_id,
                                               int32_t* infobytes_count_out);

typedef bool (*flow_nodedef_fn_populate_dimensions)(Context* c, struct flow_graph* g, int32_t node_id,
                                                    int32_t outbound_edge_id, bool force_estimate);

typedef bool (*flow_nodedef_fn_flatten)(Context* c, struct flow_graph** graph_ref, int32_t node_id);

typedef bool (*flow_nodedef_fn_flatten_shorthand)(Context* c, struct flow_graph** graph_ref, int32_t node_id,
                                                  struct flow_node* node, struct flow_edge* input_edge,
                                                  int32_t* first_replacement_node, int32_t* last_replacement_node);

typedef bool (*flow_nodedef_fn_execute)(Context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id);

typedef bool (*flow_nodedef_fn_estimate_cost)(Context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id,
                                              size_t* bytes_required, size_t* cpu_cost);

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

struct flow_node_definition* flow_nodedef_get(Context* c, flow_ntype type);

bool flow_node_stringify(Context* c, struct flow_graph* g, int32_t node_id, char* buffer, size_t buffer_size);
int32_t flow_node_fixed_infobyte_count(Context* c, flow_ntype type);
bool flow_node_infobyte_count(Context* c, struct flow_graph* g, int32_t node_id, int32_t* infobytes_count_out);
bool flow_node_populate_dimensions_to_edge(Context* c, struct flow_graph* g, int32_t node_id, int32_t outbound_edge_id,
                                           bool force_estimate);
bool flow_node_pre_optimize_flatten(Context* c, struct flow_graph** graph_ref, int32_t node_id);
bool flow_node_execute(Context* c, struct flow_job* job, struct flow_graph* g, int32_t node_id);
bool flow_node_estimate_execution_cost(Context* c, struct flow_graph* g, int32_t node_id, size_t* bytes_required,
                                       size_t* cpu_cost);
bool flow_node_validate_inputs(Context* c, struct flow_graph* g, int32_t node_id);
bool flow_node_update_state(Context* c, struct flow_graph* g, int32_t node_id);

bool flow_graph_walk_dependency_wise(Context* c, struct flow_job* job, struct flow_graph** graph_ref,
                                     flow_graph_visitor node_visitor, flow_graph_visitor edge_visitor,
                                     void* custom_data);

#define FLOW_GET_INFOBYTES(g, node_id, type, varname)                                                                  \
    struct type* varname = (struct type*)&g->info_bytes[g->nodes[node_id].info_byte_index];
