#include "job.h"

struct flow_job * flow_job_create(Context *c){

    struct flow_job * job = (struct flow_job *) CONTEXT_malloc(c, sizeof(struct flow_job));

    job->record_graph_versions = true;
    job->next_graph_version = 0;
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


bool flow_job_create_state(Context *c, struct flow_job * job, struct flow_graph * g) {
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

static bool files_identical(Context *c, const char * path1, const char* path2, bool * identical){
    FILE * fp1 = fopen(path1, "r");
    if (fp1 == NULL){
        CONTEXT_error(c, Failed_to_open_file); 
        return false;
    }
    FILE * fp2 = fopen(path2, "r");
    if (fp2 == NULL){
        CONTEXT_error(c, Failed_to_open_file); 
        fclose(fp1);
        return false;
    }
    int ch1 = getc(fp1);
    int ch2 = getc(fp2);

    while ((ch1 != EOF) && (ch2 != EOF) && (ch1 == ch2)) {
        ch1 = getc(fp1);
        ch2 = getc(fp2);
    }

    *identical = (ch1 == ch2);
    fclose(fp1);
    fclose(fp2);
    return true;
}
#define FLOW_MAX_GRAPH_VERSIONS 50

bool flow_job_notify_graph_changed(Context *c, struct flow_job *job, struct flow_graph * g){
    if (job == NULL || !job->record_graph_versions || job->next_graph_version > FLOW_MAX_GRAPH_VERSIONS) return true;

    char filename[255];
    char prev_filename[255];

    if (job->next_graph_version == 0){
        //Delete existing graphs
        int32_t i =0; 
        for (i = 0; i <= FLOW_MAX_GRAPH_VERSIONS; i++){
            snprintf(filename, 254,"graph_version_%d.dot", i);
            remove(filename);
        }
    }

    snprintf(filename, 254,"graph_version_%d.dot", job->next_graph_version++);

    FILE * f = fopen(filename,"w");
    if (f == NULL){
        CONTEXT_error(c, Failed_to_open_file); 
        return false;
    }
    if (!flow_graph_print_to_dot(c,g,f)){
        fclose(f);
        CONTEXT_error_return(c);
    }else {
        fclose(f);
    }
    //Compare
    if (job->next_graph_version > 1){
        snprintf(prev_filename, 254,"graph_version_%d.dot", job->next_graph_version-1);
        bool identical = false;
        if (!files_identical(c, prev_filename, filename, &identical)){
            CONTEXT_error_return(c);
        }
        if (identical){
            job->next_graph_version--; //Next time we will overwrite the duplicate graph. The last two graphs may remain dupes.
        }
    } 

    return true;
}
