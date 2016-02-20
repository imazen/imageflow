#include "graph.h"

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

void flow_graph_destroy(Context *c, struct flow_graph *g){
    CONTEXT_free(c,g);
}



int32_t flow_node_create_generic(Context *c, struct flow_graph ** graph_ref, int32_t prev_node, flow_ntype type){
    if (graph_ref == NULL || (*graph_ref) == NULL){
        CONTEXT_error(c, Null_argument);
        return -20;
    }
    int32_t nodeinfo_size = flow_node_fixed_infobyte_count(c, type);
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
    g->nodes[id].executed = false;
    g->nodes[id].result_bitmap = NULL;
    g->nodes[id].ticks_elapsed = 0;

    g->next_info_byte += g->nodes[id].info_bytes;
    g->next_node_id += 1;
    g->node_count += 1;
    if (prev_node >= 0){
        //TODO - call create_edge??
        g->edge_count += 1;
        g->next_edge_id += 1;
        g->edges[edge_id].from = prev_node;
        g->edges[edge_id].to = id;
        g->edges[edge_id].type = flow_edgetype_input;
        g->edges[edge_id].info_byte_index = -1;
        g->edges[edge_id].info_bytes = 0;
        g->edges[edge_id].from_width = -1;
        g->edges[edge_id].from_height = -1;
        g->edges[edge_id].from_alpha_meaningful = false;
        g->edges[edge_id].from_format = Bgra32;
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
int32_t flow_node_create_primitive_flip_vertical(Context *c, struct flow_graph **g, int32_t prev_node){
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_primitive_Flip_Vertical);
    if (id < 0){
        CONTEXT_add_to_callstack(c);
        return id;
    }
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


int32_t flow_node_create_render_to_canvas_1d(Context *c, struct flow_graph **g, int32_t prev_node,
                                             bool transpose_on_write,
                                             uint32_t canvas_x,
                                             uint32_t canvas_y,
                                             int32_t scale_to_width,
                                             WorkingFloatspace scale_and_filter_in_colorspace,
                                             float sharpen_percent,
                                             flow_compositing_mode compositing_mode,
                                             uint8_t *matte_color[4],
                                             struct flow_scanlines_filter * filter_list,
                                             InterpolationFilter interpolation_filter) {
    int32_t id = flow_node_create_generic(c, g, prev_node, flow_ntype_primitive_RenderToCanvas1D);
    if (id < 0){
        CONTEXT_add_to_callstack(c);
        return id;
    }
    struct flow_nodeinfo_render_to_canvas_1d * info = (struct flow_nodeinfo_render_to_canvas_1d *) FrameNode_get_node_info_pointer(*g, id);
    info->transpose_on_write = transpose_on_write;

    info->scale_to_width = scale_to_width;
    info->interpolation_filter = interpolation_filter;
    info->scale_in_colorspace = scale_and_filter_in_colorspace;
    info->sharpen_percent_goal = sharpen_percent;
    info->compositing_mode = compositing_mode;
    info->filter_list = filter_list;
    info->canvas_x = canvas_x;
    info->canvas_y = canvas_y;
    memcpy(&info->matte_color, matte_color, 4);
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

int32_t flow_edge_create(Context *c, struct flow_graph **g, int32_t from, int32_t to, flow_edge_type type) {
    if ((*g)->next_edge_id >= (*g)->max_edges){
        if (!flow_graph_replace_if_too_small(c, g, 0,1,0)){
            CONTEXT_add_to_callstack(c); //OOM
            return -2;
        }
    }

    struct flow_edge * e = &(*g)->edges[(*g)->next_edge_id];
    e->type = type;
    e->from = from;
    e->to   = to;
    e->from_height = -1;
    e->from_width = -1;
    e->from_alpha_meaningful = false;
    e->from_format = Bgra32;
    e->info_bytes = 0;
    e->info_byte_index = -1;
    (*g)->edge_count++;
    (*g)->next_edge_id++;
    return (*g)->next_edge_id -1;
}
int32_t flow_edge_duplicate(Context *c, struct flow_graph **g, int32_t edge_id){
    struct flow_edge * old = &(*g)->edges[edge_id];
    int32_t new_id = flow_edge_create(c,g,old->from, old->to, old->type);
    if (new_id < 0){
        CONTEXT_add_to_callstack(c);
        return -1;
    }
    struct flow_edge * e = &(*g)->edges[new_id];
    e->from_format = old->from_format;
    e->from_width = old->from_width;
    e->from_height = old->from_height;
    e->from_alpha_meaningful = old->from_alpha_meaningful;

    if (old->info_byte_index >= 0 && old->info_bytes > 0){
        e->info_bytes = old->info_bytes;
        e->info_byte_index = flow_graph_copy_info_bytes_to(c, *g, g, old->info_byte_index, old->info_bytes);
        if (e->info_byte_index < 0){
            CONTEXT_add_to_callstack(c);
            return e->info_byte_index;
        }
    }
    return new_id;
}

bool flow_graph_duplicate_edges_to_another_node(Context *c,  struct flow_graph ** g, int32_t from_node, int32_t to_node, bool copy_inbound, bool copy_outbound){
    int32_t i = -1;
    struct flow_edge * current_edge;
    for (i = 0; i < (*g)->next_edge_id; i++){
        current_edge = &(*g)->edges[i];
        if (current_edge->type != flow_edgetype_null){
            if ((copy_outbound && current_edge->from == from_node) || (copy_inbound && current_edge->to == from_node)) {
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



static bool flow_graph_walk_recursive(Context *c, struct flow_job * job, struct flow_graph **graph_ref, int32_t node_id, bool * quit, flow_graph_visitor node_visitor,flow_graph_visitor edge_visitor, void * custom_data){
    bool skip_outbound_paths = false;
    if (node_visitor != NULL){
        if (!node_visitor(c,job,graph_ref,node_id, quit, &skip_outbound_paths, custom_data)){
            CONTEXT_error_return(c);
        }
    }
    if (skip_outbound_paths || *quit) {
        return true;
    }

    struct flow_edge * edge;
    int32_t edge_ix;
    for (edge_ix = 0; edge_ix < (*graph_ref)->next_edge_id; edge_ix++){
        edge = &(*graph_ref)->edges[edge_ix];
        if (edge->type != flow_edgetype_null && edge->from == node_id){

            skip_outbound_paths = false;
            if (edge_visitor != NULL) {
                if (!edge_visitor(c,job,graph_ref,edge_ix, quit, &skip_outbound_paths, custom_data)){
                    CONTEXT_error_return(c);
                }
            }
            if (*quit) {
                return true;
            }
            if (!skip_outbound_paths) {
                //Recurse
                if (!flow_graph_walk_recursive(c, job, graph_ref, edge->to, quit, node_visitor, edge_visitor,
                                               custom_data)) {
                    CONTEXT_error_return(c);
                }
            }
            if (*quit){
                return true;
            }
        }
    }
    return true;
}


bool flow_graph_walk(Context *c, struct flow_job * job, struct flow_graph **graph_ref, flow_graph_visitor node_visitor,  flow_graph_visitor edge_visitor, void * custom_data ){
    //TODO: would be good to verify graph is acyclic.

    bool quit = false;
    //We start by finding nodes with no inbound edges, then working in a direction.
    struct flow_edge * edge;
    int32_t node_ix;
    int32_t edge_ix;
    int32_t inbound_edge_count = 0;
    for (node_ix = 0; node_ix < (*graph_ref)->next_node_id; node_ix++){
        if ((*graph_ref)->nodes[node_ix].type != flow_ntype_Null){
            //Now count inbound edges
            inbound_edge_count = 0;
            for (edge_ix = 0; edge_ix < (*graph_ref)->next_edge_id; edge_ix++){
                edge = &(*graph_ref)->edges[edge_ix];
                if (edge->type != flow_edgetype_null && edge->to == node_ix){
                    inbound_edge_count++;
                }
            }
            //if zero, we have a winner
            if (inbound_edge_count == 0){
                if (!flow_graph_walk_recursive(c,job, graph_ref, node_ix, &quit, node_visitor, edge_visitor, custom_data )){
                    CONTEXT_error_return(c);
                }
                if (quit){
                    return true;
                }
            }
        }
    }
    return true;
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



int32_t flow_graph_get_first_inbound_edge_of_type(Context *c, struct flow_graph *g, int32_t node_id,
                                                         flow_edge_type type) {
    struct flow_edge * edge;
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++){
        edge = &g->edges[i];
        if (edge->type == type){
            if (edge->to == node_id) {
               return i;
            }
        }
    }
    return -404;
}


int32_t flow_graph_get_inbound_edge_count_of_type(Context *c, struct flow_graph *g, int32_t node_id,
                                                  flow_edge_type type) {
    struct flow_edge * edge;
    int32_t i;
    int32_t count = 0;
    for (i = 0; i < g->next_edge_id; i++){
        edge = &g->edges[i];
        if (edge->type == type){
            if (edge->to == node_id) {
                count++;
            }
        }
    }
    return count;
}

void flow_graph_print_to(Context *c, struct flow_graph *g, FILE * stream){
    fprintf(stream, "%d nodes (%d/%d), %d edges (%d/%d), %d infobytes (%d/%d)\n", g->node_count, g->next_node_id, g->max_nodes,
            g->edge_count, g->next_edge_id, g->max_edges,
            g->next_info_byte - g->deleted_bytes, g->next_info_byte, g->max_info_bytes);

    flow_graph_print_edges_to(c,g,stream);
    flow_graph_print_nodes_to(c,g,stream);
}

static const char * get_format_name(BitmapPixelFormat f, bool alpha_meaningful){
    switch(f){
        case Bgr24: return "Bgr24";
        case Bgra32: return alpha_meaningful ? "Bgra32" : "Bgr32";
        default: return "unknown format";
    }
}

bool flow_graph_print_to_dot(Context *c, struct flow_graph *g, FILE * stream, const char * image_node_filename_prefix){
    fprintf(stream, "digraph g {\n");
    fprintf(stream, "  node [shape=box, fontsize=20, fontcolor=\"#5AFA0A\" fontname=\"sans-serif bold\"]\n  size=\"12,18\"\n");
    fprintf(stream, "  edge [fontsize=20, fontname=\"sans-serif\"]\n");


    char node_label_buffer[1024];
    struct flow_edge * edge;
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++){
        edge = &g->edges[i];
        char dimensions[64];
        if (edge->from_width < 0 && edge->from_height < 0){
            snprintf(dimensions, 63, "?x?");
        }else{
            snprintf(dimensions, 63, "%dx%d %s", edge->from_width, edge->from_height,
                     get_format_name(edge->from_format, edge->from_alpha_meaningful));
        }

        if (edge->type != flow_edgetype_null){
            fprintf(stream, "  n%d -> n%d [label=\"e%d: %s%s\"]\n",  edge->from, edge->to, i, dimensions,
                    edge->type == flow_edgetype_canvas ? " canvas" : "");
        }
    }

    uint64_t total_ticks =0;

    struct flow_node * n;;
    for (i = 0; i < g->next_node_id; i++){
        n = &g->nodes[i];


        if (n->type != flow_ntype_Null){
            flow_node_stringify(c,g,i,node_label_buffer, 1023);
            //fprintf(stream, "  n%d [image=\"./node_frames/%s%d.png\", label=\"n%d: %s\"]\n", i, image_node_filename_prefix, i, i, node_label_buffer); //Todo, add completion info.

            total_ticks += n->ticks_elapsed;
            float ms = n->ticks_elapsed * 1000.0 / (float)get_profiler_ticks_per_second();

            if (n->result_bitmap != NULL && image_node_filename_prefix != NULL){
                fprintf(stream, "  n%d [image=\"%s%d.png\", label=\"n%d: %s\n%.2fms\"]\n", i, image_node_filename_prefix, i, i, node_label_buffer,ms);
            } else{
                fprintf(stream, "  n%d [label=\"n%d: %s\n%.2fms\"]\n", i, i, node_label_buffer,ms); //Todo, add completion info.

            }
        }
    }

    float total_ms =total_ticks * 1000.0 / (float)get_profiler_ticks_per_second();


    //Print graph info last so it displays right or last
    fprintf(stream, " graphinfo [label=\"");
    fprintf(stream, "%d nodes (%d/%d)\n %d edges (%d/%d)\n %d infobytes (%d/%d)\nExecution time: %.2fms", g->node_count, g->next_node_id, g->max_nodes,
            g->edge_count, g->next_edge_id, g->max_edges,
            g->next_info_byte - g->deleted_bytes, g->next_info_byte, g->max_info_bytes, total_ms);
    fprintf(stream, "\"]\n");

    fprintf(stream, "}\n");
    return true;
}
