#include "fastscaling_private.h"
#include "../imageflow.h"
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include "math_functions.h"

static size_t flow_graph_size_for(uint32_t max_edges, uint32_t max_nodes, uint32_t max_info_bytes){
    return sizeof(struct flow_graph) + sizeof(struct flow_edge) * max_edges + sizeof(struct flow_node) * max_nodes + max_info_bytes;
}

struct flow_graph *flow_graph_create(Context *c, uint32_t max_edges, uint32_t max_nodes, uint32_t max_info_bytes, float growth_factor){
    size_t total_bytes = flow_graph_size_for(max_edges, max_nodes, max_info_bytes);
    struct flow_graph * g = (struct flow_graph *)CONTEXT_malloc(c, total_bytes);

    if (g == NULL){
        CONTEXT_error(c,Out_of_memory);
        return NULL;
    }
    g->memory_layout_version = 1;
    g->growth_factor = growth_factor;

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
    if (((size_t)g->info_bytes - (size_t)g) != total_bytes - max_info_bytes){
        //Somehow our math was inconsistent with flow_graph_size_for()
        CONTEXT_error(c, Invalid_internal_state);
        CONTEXT_free(c,g);
        return NULL;
    }
    return g;
}

struct flow_graph *flow_graph_copy_and_resize(Context *c, struct flow_graph * from, uint32_t max_edges, uint32_t max_nodes, uint32_t max_info_bytes){
    if ((int32_t)max_edges < from->next_edge_id || (int32_t)max_nodes < from->next_node_id || (int32_t)max_info_bytes < from->next_info_byte){
        CONTEXT_error(c, Invalid_argument);
        return NULL;
    }
    struct flow_graph * g = flow_graph_create(c,max_nodes,max_edges,max_info_bytes, from->growth_factor);
    g->growth_factor = from->growth_factor;

    g->deleted_bytes = from->deleted_bytes;
    g->max_info_bytes = max_info_bytes;
    g->next_info_byte = from->next_info_byte;
    g->max_nodes = max_nodes;
    g->node_count = from->node_count;
    g->next_node_id = from->next_node_id;
    g->max_edges = max_edges;
    g->edge_count = from->edge_count;
    g->next_edge_id = from->next_edge_id;
    memcpy(g->info_bytes, from->info_bytes, from->next_info_byte);
    memcpy(g->edges, from->edges, from->next_edge_id * sizeof(struct flow_edge));
    memcpy(g->nodes, from->nodes, from->next_node_id * sizeof(struct flow_node));
    return g;
}
struct flow_graph *flow_graph_memcpy(Context *c, struct flow_graph * from){
    struct flow_graph * g = flow_graph_create(c,from->max_edges, from->max_nodes, from->max_info_bytes, from->growth_factor);
    if (g == NULL){
        CONTEXT_add_to_callstack(c);
        return NULL;
    }
    size_t bytes = flow_graph_size_for(from->max_edges, from->max_nodes, from->max_info_bytes);
    memcpy(g, from, bytes);
    return g;
}

void flow_graph_destroy(Context *c, struct flow_graph *g){
    CONTEXT_free(c,g);
}

static int32_t flow_nodeinfo_size(Context *c, flow_ntype type){
    switch(type){
        case flow_ntype_Create_Canvas: return sizeof(struct flow_nodeinfo_createcanvas);
        case flow_ntype_Scale: return sizeof(struct flow_nodeinfo_size);
        case flow_ntype_Resource_Placeholder: return sizeof(struct flow_nodeinfo_index);
        case flow_ntype_primitive_bitmap_bgra_pointer: return sizeof(struct flow_nodeinfo_resource_bitmap_bgra);
        default:

            CONTEXT_error(c, Invalid_argument);
            return -404;
    }
}

static int32_t flow_node_create_generic(Context *c, struct flow_graph ** graph_ref, int32_t prev_node, flow_ntype type){
    if (graph_ref == NULL || (*graph_ref) == NULL){
        CONTEXT_error(c, Null_argument);
        return -20;
    }
    int32_t nodeinfo_size = flow_nodeinfo_size(c, type);
    if (nodeinfo_size < 0){
        CONTEXT_add_to_callstack(c);
        return nodeinfo_size;
    }
    if (!flow_graph_replace_if_too_small(c,graph_ref,1, prev_node >= 0 ? 1 : 0, nodeinfo_size)){
        CONTEXT_add_to_callstack(c);
        return -2;
    }
    struct flow_graph * g = *graph_ref;
    int32_t id = g->next_node_id;

    int32_t edge_id = g->next_edge_id;

    g->nodes[id].type = type;
    g->nodes[id].info_byte_index = g->next_info_byte;
    g->nodes[id].info_bytes = nodeinfo_size;

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
int32_t flow_node_create_canvas(Context *c, struct flow_graph **g, int32_t prev_node, BitmapPixelFormat format,
                                size_t width, size_t height, uint32_t bgcolor){
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Create_Canvas);
    if (id < 0){
        CONTEXT_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_createcanvas * info = (struct flow_nodeinfo_createcanvas *) FrameNode_get_node_info_pointer(*g, id);
    info->format = format;
    info->width = width;
    info->height = height;
    info->bgcolor = bgcolor;
    return id;
}
int32_t flow_node_create_scale(Context *c, struct flow_graph **g, int32_t prev_node, size_t width, size_t height){
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Scale);
    if (id < 0){
        CONTEXT_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_size * info = (struct flow_nodeinfo_size *) FrameNode_get_node_info_pointer(*g, id);
    info->width = width;
    info->height = height;
    return id;
}

int32_t flow_node_create_resource_placeholder(Context *c, struct flow_graph **g, int32_t prev_node,
                                              int32_t output_slot_id){
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_Resource_Placeholder);
    if (id < 0){
        CONTEXT_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_index * info = (struct flow_nodeinfo_index *) FrameNode_get_node_info_pointer(*g, id);
    info->index = output_slot_id;
    return id;
}

int32_t flow_node_create_resource_bitmap_bgra(Context *c, struct flow_graph **g, int32_t prev_node, BitmapBgra ** ref){
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_primitive_bitmap_bgra_pointer);
    if (id < 0){
        CONTEXT_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_resource_bitmap_bgra * info = (struct flow_nodeinfo_resource_bitmap_bgra *) FrameNode_get_node_info_pointer(*g, id);
    info->ref = ref;
    return id;
}

bool flow_edge_delete(Context *c, struct flow_graph *g, int32_t edge_id){
    if (edge_id < 0 || edge_id >= g->next_edge_id){
        CONTEXT_error(c, Invalid_argument);
        return false;
    }
    struct flow_edge * e = &g->edges[edge_id];
    if (e->type == flow_edgetype_null){
        CONTEXT_error(c, Edge_already_deleted);
        return false;
    }else{
        g->deleted_bytes += e->info_bytes;
        g->edge_count--;

        e->type = flow_edgetype_null;
        e->info_byte_index = -1;
        e->info_bytes = 0;
        e->from = -1;
        e->to = -1;
        return true;
    }

}

bool flow_edge_delete_all_connected_to_node(Context *c, struct flow_graph *g, int32_t node_id){
    struct flow_edge * current_edge;
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++){
        current_edge = &g->edges[i];
        if (current_edge->type != flow_edgetype_null){
            if (current_edge->from == node_id || current_edge->to == node_id) {
                if (!flow_edge_delete(c,g,i)){
                    CONTEXT_error_return(c);
                }
            }
        }
    }
    return true;
}

bool flow_graph_replace_if_too_small(Context *c,  struct flow_graph ** g, uint32_t free_nodes_required, uint32_t free_edges_required, uint32_t free_bytes_required){
    float growth_factor = (float)fmax((*g)->growth_factor,1.0f);
    if (    (int32_t)free_nodes_required > (*g)->max_nodes - (*g)->next_node_id ||
            (int32_t)free_edges_required > (*g)->max_edges - (*g)->next_edge_id ||
            (int32_t)free_bytes_required > (*g)->max_info_bytes - (*g)->next_info_byte){
        int32_t min_nodes = max((*g)->max_nodes, (*g)->next_node_id + free_nodes_required);
        int32_t min_edges = max((*g)->max_edges, (*g)->next_edge_id + free_edges_required);
        int32_t min_bytes = max((*g)->max_info_bytes, (*g)->next_info_byte + free_bytes_required);
        struct flow_graph * new_graph = flow_graph_copy_and_resize(c, (*g),  (uint32_t)(growth_factor * (float)min_nodes),  (uint32_t)(growth_factor * (float)min_edges),  (uint32_t)(growth_factor * (float)min_bytes));
        if (new_graph == NULL){
            CONTEXT_error_return(c);
        }
        struct flow_graph *old = *g;
        *g = new_graph; //Swap the pointer out
        flow_graph_destroy(c,old); //Delete the old graph

    }
    return true;
}

int32_t flow_graph_copy_info_bytes_to(Context *c, struct flow_graph *from, struct flow_graph **to, int32_t byte_index,
                                       int32_t byte_count){
    if (byte_index < 0 || byte_count == 0){
     return -1;
    }
    int32_t new_index = (*to)->next_info_byte;
    if ((*to)->max_info_bytes <= new_index + byte_count){
        if (!flow_graph_replace_if_too_small(c, to, 0,0,byte_count)){
            CONTEXT_add_to_callstack(c); //OOM
            return -2;
        }
    }
    memcpy(&(*to)->info_bytes[new_index],&from->info_bytes[byte_index], byte_count);
    (*to)->next_info_byte += byte_count;
    return new_index;
}

int32_t flow_edge_duplicate(Context *c, struct flow_graph **g, int32_t edge_id){
    if ((*g)->next_edge_id >= (*g)->max_edges){
        if (!flow_graph_replace_if_too_small(c, g, 0,1,0)){
            CONTEXT_add_to_callstack(c); //OOM
            return -2;
        }
    }
    struct flow_edge * old = &(*g)->edges[edge_id];
    struct flow_edge * e = &(*g)->edges[(*g)->next_edge_id];
    e->type = old->type;
    e->from = old->from;
    e->to   = old->to;
    if (old->info_byte_index >= 0 && old->info_bytes > 0){
        e->info_bytes = old->info_bytes;
        e->info_byte_index = flow_graph_copy_info_bytes_to(c, *g, g, old->info_byte_index, old->info_bytes);
        if (e->info_byte_index < 0){
            CONTEXT_add_to_callstack(c);
            return e->info_byte_index;
        }
    }else{
        e->info_byte_index = -1;
        e->info_bytes = 0;
    }
    (*g)->edge_count++;
    (*g)->next_edge_id++;
    return (*g)->next_edge_id-1;
}

bool flow_graph_duplicate_edges_to_another_node(Context *c,  struct flow_graph ** g, int32_t from_node, int32_t to_node){
    int32_t i = -1;
    struct flow_edge * current_edge;
    for (i = 0; i < (*g)->next_edge_id; i++){
        current_edge = &(*g)->edges[i];
        if (current_edge->type != flow_edgetype_null){
            if (current_edge->from == from_node || current_edge->to == from_node) {
                int32_t new_edge_id = flow_edge_duplicate(c, g, i);
                if (new_edge_id < 0){
                    CONTEXT_add_to_callstack(c);
                    return false;
                }
                struct flow_edge * new_edge = &(*g)->edges[new_edge_id];

                if (new_edge->from == from_node){
                    new_edge->from = to_node;
                }
                if (new_edge->to == from_node){
                    new_edge->to = to_node;
                }
            }
        }
    }
    return true;

}


bool flow_node_delete(Context *c, struct flow_graph *g, int32_t node_id){
    if (node_id < 0 || node_id >= g->next_node_id){
        CONTEXT_error(c, Invalid_argument);
        return false;
    }
    struct flow_node * n = &g->nodes[node_id];
    if (n->type == flow_ntype_Null){
        CONTEXT_error(c, Node_already_deleted);
        return false;
    }else{
        if (!flow_edge_delete_all_connected_to_node(c,g,node_id)){
            CONTEXT_error_return(c);
        }
        n->type = flow_ntype_Null;
        g->deleted_bytes += n->info_bytes;
        n->info_byte_index = -1;
        n->info_bytes = 0;
        g->node_count--;
        return true;
    }

}

static void flow_graph_print_nodes_to(Context *c, struct flow_graph *g, FILE * stream) {
    struct flow_node * n;
    int32_t i;
    for (i = 0; i < g->next_node_id; i++){
        n = &g->nodes[i];

        if (n->type != flow_ntype_Null){
            fprintf(stream, "[%d]: node type %d, %d infobytes\n", i, n->type,n->info_bytes);
        }else{
            fprintf(stream, "(null)\n");
        }
    }
}

static void flow_graph_print_edges_to(Context *c, struct flow_graph *g, FILE * stream) {
    struct flow_edge * edge;
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++){
        edge = &g->edges[i];

        if (edge->type != flow_edgetype_null){
            fprintf(stream, "%d: (%d, %d) type %d, %d infobytes\n", i, edge->from, edge->to, edge->type,edge->info_bytes);
        }else{
            fprintf(stream, "(null)\n");
        }
    }
}


void flow_graph_print_to(Context *c, struct flow_graph *g, FILE * stream){
    fprintf(stream, "Graph nodes: %d, edges: %d, infobytes: %d\n", g->node_count, g->edge_count, g->next_info_byte);
    fprintf(stream, "Space utilization: nodes: %d/%d, edges: %d/%d, infobytes %d/%d\n", g->next_node_id, g->max_nodes, g->next_edge_id, g->max_edges, g->next_info_byte, g->max_info_bytes);

    flow_graph_print_edges_to(c,g,stream);
    flow_graph_print_nodes_to(c,g,stream);
}