#include "fastscaling_private.h"
#include "../imageflow.h"
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>


typedef enum flow_job_resource_type{
    flow_job_resource_bitmap_bgra = 1,

} flow_job_resource_type;

struct flow_job_resource_item{
    struct flow_job_resource_item * next;
    int32_t id;
    int32_t graph_placeholder_id;
    FLOW_DIRECTION direction;
    flow_job_resource_type type;
    void * data;

};

struct flow_job_state {
    bool * node_completed;
    BitmapBgra * * node_bitmap;
};

struct flow_job {

    int32_t next_resource_id;
    struct flow_job_resource_item * resources_head;
    struct flow_job_resource_item * resources_tail;
    struct flow_job_state job_state;
};

struct flow_job * flow_job_create(Context *c){

    struct flow_job * job = (struct flow_job *) CONTEXT_malloc(c, sizeof(struct flow_job));

    job->next_resource_id = 0x800;
    job->resources_head = NULL;
    job->job_state.node_completed = NULL;
    job->job_state.node_bitmap = NULL;
    return job;
}

static void flow_job_state_destroy(Context *c, struct flow_job * job) {
    if (job->job_state.node_bitmap != NULL) {
        //TOOD: dispose of the owned bitmaps;
        CONTEXT_free(c, job->job_state.node_bitmap);
        job->job_state.node_bitmap = NULL;
    }
    if (job->job_state.node_completed != NULL) {
        CONTEXT_free(c, job->job_state.node_completed);
        job->job_state.node_completed = NULL;
    }
}


static bool flow_job_create_state(Context *c, struct flow_job * job, struct flow_graph * g) {
    flow_job_state_destroy(c,job);
    job->job_state.node_completed = (bool *) CONTEXT_malloc(c, sizeof(bool) * g->max_nodes);
    if (job->job_state.node_completed == NULL) {
        CONTEXT_error(c, Out_of_memory);
        return false;
    }

    job->job_state.node_bitmap = (BitmapBgra **) CONTEXT_malloc(c, sizeof(BitmapBgra *) * g->max_nodes);
    if (job->job_state.node_bitmap == NULL) {
        CONTEXT_error(c, Out_of_memory);
        return false;
    }
    for (int i = 0; i < g->max_nodes; i++) {
        job->job_state.node_bitmap[i] = NULL;
        job->job_state.node_completed[i] = false;
    }
    return true;
}

void flow_job_destroy(Context *c, struct flow_job * job){
    //TODO: Free all the resources
    flow_job_state_destroy(c,job);
    CONTEXT_free(c, job);
}

static int32_t create_node_for_resource(Context *c, struct flow_job_resource_item * item, struct flow_graph ** g){
    int32_t node_id = -1;
    if (item->type == flow_job_resource_bitmap_bgra){
        node_id = flow_node_create_resource_bitmap_bgra(c,g, -1, (BitmapBgra **)&item->data);
    }else{
        CONTEXT_error(c,Invalid_internal_state);
        return -404;
    }
    if (node_id < 0){
        CONTEXT_add_to_callstack(c);
    }
    return node_id;
}

//
//static bool flow_job_replace_placeholder_in_graph(Context *c, struct flow_job * job, struct flow_graph ** g, int32_t node_to_replace_ix, int32_t graph_placeholder_id){
//    struct flow_job_resource_item * current = job->resources_head;
//    int32_t current_node = -1;
//    while (current != NULL){
//        if (current->graph_placeholder_id == graph_placeholder_id){
//            //Add node
//            current_node = create_node_for_resource(c,current,g);
//            if (current_node < 0){
//                CONTEXT_error_return(c);
//            }
//
//            if (!flow_graph_duplicate_edges_to_another_node(c,g, node_to_replace_ix, current_node)){
//                CONTEXT_error_return(c);
//            }
//
//        }
//        current = current->next;
//    }
//    if (current_node == -1){
//        CONTEXT_error(c, Invalid_internal_state);
//        //TODO: Failure - no matching placeholders found
//        return false;
//    }else{
//        if (!flow_node_delete(c, *g, node_to_replace_ix)){
//            CONTEXT_error_return(c);
//        }
//    }
//    return true;
//}

//When placeholder_id == -1, the first resource placeholder is returned.
static int32_t flow_job_find_first_node_with_placeholder_id(Context *c, struct flow_graph * g, int32_t placeholder_id){
    if (g == NULL){
        CONTEXT_error(c, Null_argument);
        return -1;
    }
    int32_t i;
    for(i = 0; i < g->next_node_id; i ++){
        if (g->nodes[i].type == flow_ntype_Resource_Placeholder){
            if (placeholder_id == -1){
                return i;
            }else{
                uint8_t * info_bytes = &g->info_bytes[g->nodes[i].info_byte_index];
                struct flow_nodeinfo_index * info = (struct flow_nodeinfo_index *) info_bytes;
                if (info->index == placeholder_id){
                    return i;
                }
            }
        }
    }
    return -404;
}


static struct flow_graph * flow_job_complete_graph_with_reuse(Context *c, struct flow_job * job, struct flow_graph * from, bool may_reuse_graph){
    if (from == NULL){
        CONTEXT_error(c, Null_argument);
        return NULL;
    }
    struct flow_graph * g = may_reuse_graph ? from : flow_graph_memcpy(c,from);
    if (g == NULL){
        CONTEXT_add_to_callstack(c);
        return NULL;
    }

    struct flow_job_resource_item * current = job->resources_head;
    int32_t next_match;
    int32_t replacement_node_id;
    int32_t match_count;
    while (current != NULL){
        match_count = 0;
        do {
            //flow_graph_print_to(c,g,stderr);

            next_match = flow_job_find_first_node_with_placeholder_id(c,g, current->graph_placeholder_id);
            if (next_match >= 0) {
                replacement_node_id = create_node_for_resource(c, current, &g);
                if (replacement_node_id < 0) {
                    CONTEXT_error_return(c);
                }
                //flow_graph_print_to(c, g, stderr);
                if (!flow_graph_duplicate_edges_to_another_node(c, &g, next_match, replacement_node_id)) {
                    CONTEXT_error_return(c);
                }
                //flow_graph_print_to(c, g, stderr);
                if (!flow_node_delete(c, g, next_match)) {
                    CONTEXT_error_return(c);
                }
                //flow_graph_print_to(c, g, stderr);
                match_count++;
            }
        }while(next_match >= 0);

        if (match_count == 0){
            //No matching nodes exist in the graph
            //This is a warning situation
        }

        current = current->next;
    }
    if (flow_job_find_first_node_with_placeholder_id(c,g, -1) > -1){
        //Leftover nodes with no placeholder - this is an error
        CONTEXT_error(c, Graph_could_not_be_completed);
        return NULL;
    }
    return g;
}
struct flow_graph * flow_job_complete_graph(Context *c, struct flow_job * job, struct flow_graph * from){
    struct flow_graph * g = flow_job_complete_graph_with_reuse(c, job, from, false);
    if (g == NULL){
        CONTEXT_add_to_callstack(c);
    }
    return g;
}



static bool flow_job_node_is_completed(Context * c, struct flow_job * job,  struct flow_graph * g, int32_t node_id) {
    return job->job_state.node_completed[node_id];
}
static bool flow_job_node_is_ready(Context * c, struct flow_job * job,  struct flow_graph * g, int32_t node_id ){
    if (flow_job_node_is_completed(c,job,g,node_id)){
        return false; //Completed already
    }
    struct flow_edge * edge;
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++){
        edge = &g->edges[i];
        if (edge->type != flow_edgetype_null){
            if (edge->to == node_id) {
                if (!flow_job_node_is_completed(c,job,g, edge->from)) {
                    return false;
                    //One of our inputs in incomplete;
                }

            }
        }
    }
    return true;
}

static int32_t flow_graph_get_first_inbound_node_of_type(Context *c, struct flow_graph *g, int32_t node_id, flow_edge_type type) {
    struct flow_edge * edge;
    int32_t i;
    for (i = 0; i < g->next_edge_id; i++){
        edge = &g->edges[i];
        if (edge->type == type){
            if (edge->to == node_id) {
                return edge->from;
            }
        }
    }
    return -404;
}


static int32_t flow_graph_get_inbound_node_count_of_type(Context *c, struct flow_graph *g, int32_t node_id, flow_edge_type type) {
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



static bool flow_job_complete_node(Context *c, struct flow_job * job, struct flow_graph * g, int32_t node_id ) {

    struct flow_node * n = &g->nodes[node_id];
    uint8_t * bytes = &g->info_bytes[n->info_byte_index];

    BitmapBgra * b = NULL;
    if (n->type == flow_ntype_Create_Canvas) {
        if (flow_graph_get_inbound_node_count_of_type(c,g,node_id, flow_edgetype_input) +
            flow_graph_get_inbound_node_count_of_type(c,g,node_id, flow_edgetype_canvas) != 0){
            CONTEXT_error(c, Invalid_inputs_to_node);
            return false;
        }
        struct flow_nodeinfo_createcanvas *info = (struct flow_nodeinfo_createcanvas *) bytes;
        b = BitmapBgra_create(c, info->width, info->height, true, info->format);
        if (b == NULL){
            CONTEXT_error_return(c);
        }
        //b->matte_color = info->bgcolor;
    }else if(n->type == flow_ntype_primitive_bitmap_bgra_pointer){
        if (flow_graph_get_inbound_node_count_of_type(c,g,node_id, flow_edgetype_input) != 1 &&
            flow_graph_get_inbound_node_count_of_type(c,g,node_id, flow_edgetype_canvas) != 0){
            CONTEXT_error(c, Invalid_inputs_to_node);
            return false;
        }
        struct flow_nodeinfo_resource_bitmap_bgra *info = (struct flow_nodeinfo_resource_bitmap_bgra *) bytes;

        int32_t input_node_id = flow_graph_get_first_inbound_node_of_type(c, g, node_id, flow_edgetype_input);
        //get input
        b = job->job_state.node_bitmap[input_node_id];
        //Update pointer
        *info->ref = b;
    }else{
        CONTEXT_error(c,Not_implemented);
        return false;
    }


    job->job_state.node_completed[node_id] = true;
    job->job_state.node_bitmap[node_id] = b;
    return true;
}
//jobs can run multiple graphs
//so execution has state
//should this execution state be in the job?

//static void visit_dfs(Context * c, struct flow_graph * g, bool visited[], int32_t node_id){
//    visited[node_id] = true;
//
//    struct flow_edge * edge;
//    int32_t i;
//    for (i = 0; i < g->next_edge_id; i++){
//        edge = &g->edges[i];
//        if (edge->type != flow_edgetype_null){
//            int next_node_id = -1;
//            if (edge->from == node_id)
//                next_node_id = edge->to;
//            if (edge->to     == node_id)
//                next_node_id == edge->from;
//            if (next_node_id > -1){
//                if (!visited[next_node_id]){
//                    visit_dfs(c,g,visited,next_node_id);
//                }
//            }
//        }
//        }
//    }
//}


static int32_t flow_job_get_next_unfinished_node(Context * c, struct flow_job * job,  struct flow_graph * g) {

    int32_t i;
    for(i = 0; i < g->next_node_id; i ++){
        if (g->nodes[i].type != flow_ntype_Null){
            if (flow_job_node_is_ready(c,job,g,i)){
                return i;
            }
        }
    }
    return -404;
}
//if no hits, search forward

bool flow_job_execute_graph(Context *c, struct flow_job * job, struct flow_graph * g){

    //Resets and creates state tracking for this graph
    flow_job_create_state(c,job, g);
    int32_t next_task = -1;
    while((next_task = flow_job_get_next_unfinished_node(c,job,g)) > -1){
        if (!flow_job_complete_node(c, job, g, next_task)){
            CONTEXT_error_return(c);
        }
    }
    return true;
}


struct flow_graph * flow_graph_flatten(Context *c, struct flow_graph * graph, bool free_previous_graph){
    struct flow_graph * new_graph = flow_graph_memcpy(c,graph);
    if (new_graph == NULL) {
        CONTEXT_add_to_callstack(c);
    }
    if (free_previous_graph){
        flow_graph_destroy(c,graph);
    }
    return new_graph;
}

static int32_t flow_job_add_resource(Context *c, struct flow_job * job, FLOW_DIRECTION dir, int32_t graph_placeholder_id, flow_job_resource_type type, void * data){
    struct flow_job_resource_item * r = (struct flow_job_resource_item *) CONTEXT_malloc(c,sizeof(struct flow_job_resource_item));
    if (r == NULL){
        CONTEXT_error(c, Out_of_memory);
        return -2;
    }
    r->next = NULL;
    r->id = job->next_resource_id;
    job->next_resource_id++;
    r->graph_placeholder_id = graph_placeholder_id;
    r->data = data;
    r->direction = dir;
    r->type = type;
    if (job->resources_head == NULL){
        job->resources_head = r;
        job->resources_tail = r;
    }else{
        job->resources_tail->next = r;
        job->resources_tail = r;
    }
    return r->id;
}

static struct flow_job_resource_item * flow_job_get_resource(Context *c, struct flow_job * job, int32_t resource_id){
    struct flow_job_resource_item * current = job->resources_head;
    while (current != NULL){
        if (current->id == resource_id){
            return current;
        }
        current = current->next;
    }
    return NULL;
}
int32_t flow_job_add_bitmap_bgra(Context *c, struct flow_job * job, FLOW_DIRECTION dir, int32_t graph_placeholder_id){
    int32_t id = flow_job_add_resource(c,job,dir,graph_placeholder_id, flow_job_resource_bitmap_bgra, NULL);
    return id;
}


BitmapBgra * flow_job_get_bitmap_bgra(Context *c, struct flow_job * job, int32_t resource_id){
    struct flow_job_resource_item * r = flow_job_get_resource(c,job,resource_id);
    if (r == NULL || r->data == NULL) return NULL;
    return (BitmapBgra *)r->data;
}


