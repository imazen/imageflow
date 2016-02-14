#include "fastscaling_private.h"
#include "../imageflow.h"
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>

static size_t FrameGraph_size(uint32_t max_edges, uint32_t max_nodes, uint32_t max_info_bytes){
    return sizeof(struct FrameGraph) + sizeof(struct FrameEdge) * max_edges + sizeof(struct FrameNode) * max_nodes + max_info_bytes;
}

struct FrameGraph * FrameGraph_create(Context * c, uint32_t max_edges, uint32_t max_nodes, uint32_t max_info_bytes){
    size_t total_bytes = FrameGraph_size(max_edges, max_nodes, max_info_bytes);
    struct FrameGraph * g = (struct FrameGraph *)CONTEXT_malloc(c,total_bytes);

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

    g->edges =  (struct FrameEdge *) (((size_t) g) + sizeof(struct FrameGraph));
    g->nodes =  (struct FrameNode *) (((size_t) g->edges) + sizeof(struct FrameEdge) * max_edges);
    g->info_bytes =  (uint8_t *) (((size_t) g->nodes) + sizeof(struct FrameNode) * max_nodes);
    if ((size_t)g->info_bytes >= total_bytes - max_info_bytes){
        //PANIC, size was wrong.
    }


    return g;
}


void FrameGraph_destroy(Context * c, struct FrameGraph * g){
    CONTEXT_free(c,g);
}

static int32_t FrameNode_get_info_bytes_size(FrameNodeType type){
    switch(type){
        case Primitive_CreateCanvas: return sizeof(struct FrameNode_CreateCanvas);
        case Filter_Scale: return sizeof(struct FrameNode_Scale);
        default:
            exit(9);
    }
}

static int32_t FrameNode_create_generic(Context * c, struct FrameGraph * g, int32_t prev_node, FrameNodeType type){
    int32_t id = g->next_node_id;

    int32_t edge_id = g->next_edge_id;

    g->nodes[id].type = type;
    g->nodes[id].info_byte_index = g->next_info_byte;
    g->nodes[id].info_bytes = FrameNode_get_info_bytes_size(type);

    g->next_info_byte += g->nodes[id].info_bytes;
    g->next_node_id += 1;
    g->node_count += 1;
    if (prev_node >= 0){
        g->edge_count += 1;
        g->next_edge_id += 1;
        g->edges[edge_id].from = prev_node;
        g->edges[edge_id].to = id;
        g->edges[edge_id].type = FrameEdge_input;
        g->edges[edge_id].info_byte_index = -1;
        g->edges[edge_id].info_bytes = 0;
    }
    return id;
}
static void * FrameNode_get_node_info_pointer(struct FrameGraph * g, int32_t node_id){
    return &(g->info_bytes[g->nodes[node_id].info_byte_index]);
}
int32_t FrameNode_create_canvas(Context * c, struct FrameGraph * g, int32_t prev_node, BitmapPixelFormat format, size_t width, size_t height, uint32_t bgcolor){
    int32_t id = FrameNode_create_generic(c,g,prev_node,Primitive_CreateCanvas);
    struct FrameNode_CreateCanvas * info = (struct FrameNode_CreateCanvas *) FrameNode_get_node_info_pointer(g,id);
    info->format = format;
    info->width = width;
    info->height = height;
    info->bgcolor = bgcolor;
    return id;
}
int32_t FrameNode_create_scale(Context * c, struct FrameGraph * g, int32_t prev_node, size_t width, size_t height){
    int32_t id = FrameNode_create_generic(c,g,prev_node,Primitive_CreateCanvas);
    struct FrameNode_Scale * info = (struct FrameNode_Scale *) FrameNode_get_node_info_pointer(g,id);
    info->width = width;
    info->height = height;
    return id;
}
