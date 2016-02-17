#include "fastscaling_private.h"
#include "../imageflow.h"
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include "math_functions.h"



typedef bool (*flow_nodedef_fn_stringify)(Context *c, struct flow_graph *g, int32_t node_id, char * buffer, size_t buffer_size);



typedef bool (*flow_nodedef_fn_infobyte_count)(Context *c, struct flow_graph *g, int32_t node_id, int32_t * infobytes_count_out);

typedef bool (*flow_nodedef_fn_populate_dimensions)(Context *c, struct flow_graph *g, int32_t node_id, int32_t outbound_edge_id);


typedef bool (*flow_nodedef_fn_flatten)(Context *c, struct flow_graph **graph_ref, int32_t node_id);


typedef bool (*flow_nodedef_fn_flatten_shorthand)(Context *c, struct flow_graph **graph_ref, int32_t node_id, struct flow_node * node, struct flow_edge * input_edge, int32_t * first_replacement_node, int32_t * last_replacement_node);

typedef bool (*flow_nodedef_fn_execute)(Context *c, struct flow_job * job, struct flow_graph *g, int32_t node_id);




struct flow_node_definition{
    flow_ntype type;
    int32_t input_count;
    int32_t canvas_count;
    const char * type_name;

    flow_nodedef_fn_stringify stringify;
    flow_nodedef_fn_infobyte_count count_infobytes;
    int32_t nodeinfo_bytes_fixed;
    flow_nodedef_fn_populate_dimensions populate_dimensions;
    flow_nodedef_fn_flatten flatten;
    flow_nodedef_fn_flatten_shorthand flatten_shorthand;
    flow_nodedef_fn_execute execute;

};

struct flow_node_definition * flow_nodedef_get(Context *c, flow_ntype type);

bool flow_node_stringify(Context *c, struct flow_graph *g, int32_t node_id, char * buffer, size_t buffer_size);
int32_t flow_node_fixed_infobyte_count(Context *c, flow_ntype type);
bool flow_node_infobyte_count(Context *c, struct flow_graph *g, int32_t node_id, int32_t * infobytes_count_out);
bool flow_node_populate_dimensions_to_edge(Context *c, struct flow_graph *g, int32_t node_id, int32_t outbound_edge_id);
bool flow_node_flatten(Context *c, struct flow_graph **graph_ref, int32_t node_id);
bool flow_node_execute(Context *c, struct flow_job * job, struct flow_graph *g, int32_t node_id);
bool flow_node_validate_inputs(Context *c, struct flow_graph *g, int32_t node_id);