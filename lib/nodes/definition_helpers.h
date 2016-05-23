#include "../nodes.h"

bool stringify_state(char * buffer, size_t buffer_isze, struct flow_node * n);

bool set_node_optimized_and_update_state(flow_c * c, struct flow_graph * g, int32_t node_id);

char * stringify_colorspace(flow_working_floatspace space);
char * stringify_filter(flow_interpolation_filter filter);

int32_t create_primitve_render_to_canvas_1d_node(flow_c * c, struct flow_graph ** g, int32_t last, int32_t to_width,
                                                 bool transpose, flow_interpolation_filter filter);

bool dimensions_of_canvas(flow_c * c, struct flow_graph * g, int32_t node_id, bool force_estimate);

bool dimensions_mimic_input(flow_c * c, struct flow_graph * g, int32_t node_id, bool force_estimate);

bool node_has_other_dependents(flow_c * c, struct flow_graph * g, int32_t node_id, int32_t excluding_dependent_node_id,
                               bool * has_other_dependents);

bool flatten_delete_node(flow_c * c, struct flow_graph ** graph_ref, int32_t node_id);

FLOW_EXPORT void * FrameNode_get_node_info_pointer(struct flow_graph * g, int32_t node_id);

#define FLOW_GET_INFOBYTES(g, node_id, type, varname)                                                                  \
    struct type * varname = (struct type *)&g->info_bytes[g->nodes[node_id].info_byte_index];

#define FLOW_GET_INPUT_EDGE(g, node_id)                                                                                \
    int32_t input_edge_id = flow_graph_get_first_inbound_edge_of_type(c, g, node_id, flow_edgetype_input);             \
    if (input_edge_id < 0) {                                                                                           \
        FLOW_error(c, flow_status_Invalid_inputs_to_node);                                                             \
        return false;                                                                                                  \
    }                                                                                                                  \
    struct flow_edge * input_edge = &g->edges[input_edge_id];

#define FLOW_GET_INPUT_NODE(g, node_id)                                                                                \
    int32_t input_edge_id = flow_graph_get_first_inbound_edge_of_type(c, g, node_id, flow_edgetype_input);             \
    if (input_edge_id < 0) {                                                                                           \
        FLOW_error(c, flow_status_Invalid_inputs_to_node);                                                             \
        return false;                                                                                                  \
    }                                                                                                                  \
    struct flow_node * input_node = &g->nodes[g->edges[input_edge_id].from];

#define FLOW_GET_CANVAS_EDGE(g, node_id)                                                                               \
    int32_t canvas_edge_id = flow_graph_get_first_inbound_edge_of_type(c, g, node_id, flow_edgetype_canvas);           \
    if (canvas_edge_id < 0) {                                                                                          \
        FLOW_error(c, flow_status_Invalid_inputs_to_node);                                                             \
        return false;                                                                                                  \
    }                                                                                                                  \
    struct flow_edge * canvas_edge = &g->edges[canvas_edge_id];

#define FLOW_GET_CANVAS_NODE(g, node_id)                                                                               \
    int32_t canvas_edge_id = flow_graph_get_first_inbound_edge_of_type(c, g, node_id, flow_edgetype_canvas);           \
    if (canvas_edge_id < 0) {                                                                                          \
        FLOW_error(c, flow_status_Invalid_inputs_to_node);                                                             \
        return false;                                                                                                  \
    }                                                                                                                  \
    struct flow_node * canvas_node = &g->nodes[g->edges[canvas_edge_id].from];
