#include <glenn/png/png.h>
#include "job.h"
#include "png.h"

struct flow_job * flow_job_create(Context *c){

    struct flow_job * job = (struct flow_job *) CONTEXT_malloc(c, sizeof(struct flow_job));

    job->record_graph_versions = true;
    job->next_graph_version = 0;
    job->next_resource_id = 0x800;
    job->resources_head = NULL;
    job->max_calc_flatten_execute_passes = 5;
    return job;
}

void flow_job_destroy(Context *c, struct flow_job * job){
    //TODO: Free all the resources, and their codecs
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
    r->codec_state = NULL;
    r->codec_type = flow_job_codec_type_null;
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
    int32_t id = flow_job_add_resource(c, job, dir, graph_placeholder_id, flow_job_resource_type_bitmap_bgra, NULL);
    if (id >= 0) {
        flow_job_get_resource(c,job,id)->codec_type = flow_job_codec_type_bitmap_bgra_pointer;
    }
    return id;
}


BitmapBgra * flow_job_get_bitmap_bgra(Context *c, struct flow_job * job, int32_t resource_id){
    struct flow_job_resource_item * r = flow_job_get_resource(c,job,resource_id);
    if (r == NULL || r->data == NULL) return NULL;
    return (BitmapBgra *)r->data;
}

struct flow_job_resource_buffer * flow_job_get_buffer(Context *c, struct flow_job * job, int32_t resource_id){
    struct flow_job_resource_item * r = flow_job_get_resource(c,job,resource_id);
    if (r == NULL || r->data == NULL) return NULL;
    return (struct flow_job_resource_buffer *)r->data;
}



int32_t flow_job_add_buffer(Context *c, struct flow_job * job, FLOW_DIRECTION dir, int32_t graph_placeholder_id, void * buffer, size_t buffer_size, bool owned_by_job){
    struct flow_job_resource_buffer * resource = (struct flow_job_resource_buffer *) CONTEXT_calloc(c, 1, sizeof(struct flow_job_resource_buffer));

    resource->buffer = buffer;
    resource->buffer_size = buffer_size;
    resource->owned_by_job = owned_by_job;
    resource->codec_state = NULL;

    int32_t id = flow_job_add_resource(c, job, dir, graph_placeholder_id, flow_job_resource_type_buffer, resource);
    return id;
}


bool flow_job_execute(Context *c, struct flow_job * job,struct flow_graph **graph_ref){
    if (!flow_job_notify_graph_changed(c,job, *graph_ref)){
        CONTEXT_error_return(c);
    }
    int32_t passes = 0;
    while (!flow_job_graph_fully_executed(c, job, *graph_ref)) {
        if (passes >= job->max_calc_flatten_execute_passes){
            CONTEXT_error(c,Graph_could_not_be_executed);
            return false;
        }
        if (!flow_job_populate_dimensions_where_certain(c,job,graph_ref)){
            CONTEXT_error_return(c);
        }
        if (!flow_graph_flatten_where_certain(c,graph_ref)){
            CONTEXT_error_return(c);
        }
        if (!flow_job_notify_graph_changed(c,job, *graph_ref)){
            CONTEXT_error_return(c);
        }

        if (!flow_job_execute_where_certain(c,job,graph_ref)){
            CONTEXT_error_return(c);
        }
        passes++;
    }
    return true;
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
        snprintf(prev_filename, 254,"graph_version_%d.dot", job->next_graph_version-2);
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
