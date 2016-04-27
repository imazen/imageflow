#include "../nodes.h"

inline void * FrameNode_get_node_info_pointer(struct flow_graph * g, int32_t node_id);

inline void * FrameNode_get_node_info_pointer(struct flow_graph * g, int32_t node_id)
{
    return &(g->info_bytes[g->nodes[node_id].info_byte_index]);
}

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
