#include "fastscaling_private.h"
#include "../imageflow.h"
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>

static size_t flow_graph_size_for(uint32_t max_edges, uint32_t max_nodes, uint32_t max_info_bytes){
    return sizeof(struct flow_graph) + sizeof(struct flow_edge) * max_edges + sizeof(struct flow_node) * max_nodes + max_info_bytes;
}

struct flow_graph *flow_graph_create(Context *c, uint32_t max_edges, uint32_t max_nodes, uint32_t max_info_bytes){
    size_t total_bytes = flow_graph_size_for(max_edges, max_nodes, max_info_bytes);
    struct flow_graph * g = (struct flow_graph *)CONTEXT_malloc(c, total_bytes);

    g->memory_layout_version = 1;

    g->deleted_bytes = 0;
    g->max_info_bytes = max_info_bytes;
    g->next_info_byte = 0;

    g->edge_count = 0;
    g->max_edges = max_edges;
    g->next_edge_id = 0;

    g->node_count = 0;
    g->max_nodes = max_nodes;
    g->next_node_id =0;

    g->edges =  (struct flow_edge *) (((size_t) g) + sizeof(struct flow_graph));
    g->nodes =  (struct flow_node *) (((size_t) g->edges) + sizeof(struct flow_edge) * max_edges);
    g->info_bytes =  (uint8_t *) (((size_t) g->nodes) + sizeof(struct flow_node) * max_nodes);
    if ((size_t)g->info_bytes >= total_bytes - max_info_bytes){
        //PANIC, size was wrong.
    }


    return g;
}


void flow_graph_destroy(Context *c, struct flow_graph *g){
    CONTEXT_free(c,g);
}

static int32_t flow_nodeinfo_size(flow_ntype type){
    switch(type){
        case flow_ntype_Create_Canvas: return sizeof(struct flow_nodeinfo_createcanvas);
        case flow_ntype_Scale: return sizeof(struct flow_nodeinfo_scale);
        default:
            exit(9);
    }
}

static int32_t FrameNode_create_generic(Context * c, struct flow_graph * g, int32_t prev_node, flow_ntype type){
    int32_t id = g->next_node_id;

    int32_t edge_id = g->next_edge_id;

    g->nodes[id].type = type;
    g->nodes[id].info_byte_index = g->next_info_byte;
    g->nodes[id].info_bytes = flow_nodeinfo_size(type);

    g->next_info_byte += g->nodes[id].info_bytes;
    g->next_node_id += 1;
    g->node_count += 1;
    if (prev_node >= 0){
        g->edge_count += 1;
        g->next_edge_id += 1;
        g->edges[edge_id].from = prev_node;
        g->edges[edge_id].to = id;
        g->edges[edge_id].type = flow_edgetype_input;
        g->edges[edge_id].info_byte_index = -1;
        g->edges[edge_id].info_bytes = 0;
    }
    return id;
}
static void * FrameNode_get_node_info_pointer(struct flow_graph * g, int32_t node_id){
    return &(g->info_bytes[g->nodes[node_id].info_byte_index]);
}
int32_t flow_node_create_canvas(Context *c, struct flow_graph *g, int32_t prev_node, BitmapPixelFormat format,
                                size_t width, size_t height, uint32_t bgcolor){
    int32_t id = FrameNode_create_generic(c,g,prev_node,flow_ntype_Create_Canvas);
    struct flow_nodeinfo_createcanvas * info = (struct flow_nodeinfo_createcanvas *) FrameNode_get_node_info_pointer(g, id);
    info->format = format;
    info->width = width;
    info->height = height;
    info->bgcolor = bgcolor;
    return id;
}
int32_t flow_node_create_scale(Context *c, struct flow_graph *g, int32_t prev_node, size_t width, size_t height){
    int32_t id = FrameNode_create_generic(c,g,prev_node,flow_ntype_Scale);
    struct flow_nodeinfo_scale * info = (struct flow_nodeinfo_scale *) FrameNode_get_node_info_pointer(g, id);
    info->width = width;
    info->height = height;
    return id;
}
