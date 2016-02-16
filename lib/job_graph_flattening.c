#include "job.h"

static bool flow_graph_flatten_node_scale(Context * c, struct flow_graph **g, int32_t node_id, int32_t * first_replacement_node, int32_t * last_replacement_node){
    struct flow_nodeinfo_size * size = (struct flow_nodeinfo_size *) &(*g)->info_bytes[(*g)->nodes[node_id].info_bytes];

    int32_t input_id = flow_graph_get_first_inbound_edge_of_type(c,*g,node_id, flow_edgetype_input);
    struct flow_edge * input_edge = &(*g)->edges[input_id];


    //create canvas for render1d
    int32_t canvas_a = flow_node_create_canvas(c,g,-1,input_edge->from_format,size->width, input_edge->from_height,0);
    if (canvas_a < 0){
        CONTEXT_error_return(c);
    }
    int32_t canvas_b = flow_node_create_canvas(c,g,-1,input_edge->from_format,size->width,size->height,0);
    if (canvas_b < 0){
        CONTEXT_error_return(c);
    }

    WorkingFloatspace floatspace = Floatspace_linear;
    flow_compositing_mode mode = flow_compositing_mode_overwrite;
    InterpolationFilter filter = Filter_Robidoux;
    uint8_t *matte_color[4];
    float sharpen_percent =0;

    *first_replacement_node = flow_node_create_render_to_canvas_1d(c,g,-1,true,0,0,size->width, floatspace, sharpen_percent, mode, matte_color, NULL, filter);
    if (*first_replacement_node < 0){
        CONTEXT_error_return(c);
    }
    if (flow_edge_create(c,g, canvas_a, *first_replacement_node, flow_edgetype_canvas) < 0){
        CONTEXT_error_return(c);
    }

    *last_replacement_node = flow_node_create_render_to_canvas_1d(c,g, *first_replacement_node,true, 0,0, size->height, floatspace, sharpen_percent, mode, matte_color, NULL, filter);
    if (*last_replacement_node < 0){
        CONTEXT_error_return(c);
    }
    if (flow_edge_create(c,g, canvas_b, *last_replacement_node, flow_edgetype_canvas) < 0){
        CONTEXT_error_return(c);
    }
    return true;
}
static bool flow_graph_flatten_node(Context * c, struct flow_graph **g, int32_t node_id){

    int32_t first_replacement_node = -1;
    int32_t last_replacement_node = -1;

    bool result = false;
    //Create a separate set of unconnected nodes to replace the target
    switch((*g)->nodes[node_id].type){
        case flow_ntype_Scale:
            result = flow_graph_flatten_node_scale(c, g, node_id, &first_replacement_node,&last_replacement_node);
            break;
        default:
            CONTEXT_error(c,Not_implemented);
            return false;
    }
    if (!result){
        CONTEXT_error_return(c);
    }

    //Clone edges
    if (!flow_graph_duplicate_edges_to_another_node(c,g,node_id, first_replacement_node, true, false)){
        CONTEXT_error_return(c);
    }
    if (!flow_graph_duplicate_edges_to_another_node(c,g,node_id, last_replacement_node, false, true)){
        CONTEXT_error_return(c);
    }

    //Delete the original
    if (!flow_node_delete(c,*g, node_id)){
        CONTEXT_error_return(c);
    }
    return true;
}


static bool node_visitor_flatten(Context *c, struct flow_job *job, struct flow_graph **graph_ref,
                                                      int32_t node_id, bool *quit, bool *skip_outbound_paths,
                                                      void *custom_data){

    struct flow_node * node =&(*graph_ref)->nodes[node_id];


    //If input nodes are populated
    if (flow_node_input_edges_have_dimensions(c,*graph_ref,node_id)){
        if (node->type >= flow_ntype_non_primitive_nodes_begin ) {
            if (!flow_graph_flatten_node(c, graph_ref, node_id)) {
                CONTEXT_error_return(c);
            }
            *quit = true;
            *((bool *)custom_data) = true;
        }
    }else{
        //we can't flatten past missing dimensions
        *skip_outbound_paths = true;
    }
    return true;
}

bool flow_graph_flatten_where_certain(Context *c, struct flow_graph ** graph_ref){
    if (*graph_ref == NULL){
        CONTEXT_error(c,Null_argument);
        return false;
    }
    bool re_walk = false;
    do {
        if (!flow_graph_walk(c, NULL, graph_ref, node_visitor_flatten, NULL, &re_walk)) {
            CONTEXT_error_return(c);
        }
    }while(re_walk);
    return true;
}
