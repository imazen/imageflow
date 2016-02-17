#include "job.h"

static bool flow_job_calculate_canvas_dimensions(Context * c, struct flow_node * node, uint8_t * nodeinfo , struct flow_edge * output){
    struct flow_nodeinfo_createcanvas * info = (struct flow_nodeinfo_createcanvas *) nodeinfo;
    output->from_format = info->format;
    output->from_width = info->width;
    output->from_height = info->height;
    output->from_alpha_meaningful = false;
    return true;
}
static bool flow_job_calculate_render1d_dimensions(Context * c, struct flow_node * node, uint8_t * nodeinfo, struct flow_edge * input, struct flow_edge * canvas, struct flow_edge * output){
    output->from_format = Bgra32; //TODO: maybe wrong
    output->from_alpha_meaningful = true; //TODO: WRONG! Involve "input" in decision
    output->from_width = canvas->from_width;
    output->from_height = canvas->from_height;
    return true;
}
static bool flow_job_calculate_dimensions(Context * c, struct flow_node * node, uint8_t * nodeinfo, struct flow_edge * input, struct flow_edge * output){
    if (node->type == flow_ntype_Scale) {
        struct flow_nodeinfo_size *size = (struct flow_nodeinfo_size *) nodeinfo;
        output->from_width = size->width;
        output->from_height = size->height;
        output->from_alpha_meaningful = input->from_alpha_meaningful;
        output->from_format = input->from_format;
    }else{
        printf("Dimension calculation not implemented for %d", node->type);
        CONTEXT_error(c, Not_implemented);
        return false;
    }
    return true;
}

static bool flow_job_populate_outbound_dimensions_for_edge(Context *c, struct flow_job * job, struct flow_graph *g, int32_t outbound_edge_id){
    struct flow_edge * edge = &g->edges[outbound_edge_id];

    int32_t node_id = edge->from;
    struct flow_node * node = &g->nodes[node_id];
    uint8_t * nodebytes = NULL;
    if (node->info_bytes > 0) nodebytes = &g->info_bytes[node->info_byte_index];

    int32_t input_edge_count = flow_graph_get_inbound_edge_count_of_type(c,g,node_id, flow_edgetype_input);
    int32_t canvas_edge_count = flow_graph_get_inbound_edge_count_of_type(c,g,node_id, flow_edgetype_canvas);
    if (node->type == flow_ntype_primitive_CreateCanvas) {
        if (input_edge_count != 0 && canvas_edge_count != 0) {
            CONTEXT_error(c, Invalid_inputs_to_node);
            return false;
        }

        if (!flow_job_calculate_canvas_dimensions(c,node,nodebytes, edge)){
            CONTEXT_error_return(c);
        }
        return true;
    }
    int32_t input_edge_id = flow_graph_get_first_inbound_edge_of_type(c,g,node_id, flow_edgetype_input);
    struct flow_edge * input_edge = &g->edges[input_edge_id];
    if (node->type == flow_ntype_primitive_RenderToCanvas1D) {
        if (input_edge_count != 1 && canvas_edge_count != 1){
            CONTEXT_error(c, Invalid_inputs_to_node);
            return false;
        }
        int32_t canvas = flow_graph_get_first_inbound_edge_of_type(c, g, node_id, flow_edgetype_canvas);
        if (!flow_job_calculate_render1d_dimensions(c,node,nodebytes, input_edge, &g->edges[canvas], edge)){
            CONTEXT_error_return(c);
        }
        return true;
    }else{
        if (!flow_job_calculate_dimensions(c,node, nodebytes,input_edge, edge)){
            CONTEXT_error_return(c);
        }
        return true;
    }
    return true;
}

bool flow_edge_has_dimensions(Context *c,struct flow_graph *g, int32_t edge_id){
    struct flow_edge * edge = &g->edges[edge_id];
    return edge->from_width > 0;
}
bool flow_node_input_edges_have_dimensions(Context *c,struct flow_graph *g, int32_t node_id){
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++) {
        if (g->edges[i].type != flow_edgetype_null && g->edges[i].to == node_id) {
            if (!flow_edge_has_dimensions(c, g, i)) {
                return false;
            }
        }
    }
    return true;
}

static bool edge_visitor_populate_outbound_dimensions(Context *c, struct flow_job *job, struct flow_graph **graph_ref,
                                                      int32_t edge_id, bool *quit, bool *skip_outbound_paths,
                                                      void *custom_data){
    //Only populate if empty
    if (!flow_edge_has_dimensions(c,*graph_ref,edge_id)) {
        if (!flow_job_populate_outbound_dimensions_for_edge(c, job, *graph_ref, edge_id)){
            CONTEXT_error_return(c);
        }
        if (!flow_edge_has_dimensions(c,*graph_ref,edge_id)) {
            //We couldn't populate this edge, so we sure can't populate others in this direction.
            // Stop this branch of recursion
            *skip_outbound_paths = true;
        }else{
            flow_job_notify_graph_changed(c,job, *graph_ref);
        }
    }

    return true;
}

bool flow_job_populate_dimensions_where_certain(Context *c, struct flow_job * job, struct flow_graph **graph_ref ){
    //TODO: would be good to verify graph is acyclic.
    if (!flow_graph_walk(c, job, graph_ref, NULL, edge_visitor_populate_outbound_dimensions, NULL)){
        CONTEXT_error_return(c);
    }
    return true;
}
