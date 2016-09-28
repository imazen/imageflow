#include "imageflow_private.h"
// Responsible for writing frames to disk as rendering happens
// Responsible for writing new versions of the graph to disk as it mutates and node states change


static bool files_identical(flow_c * c, const char * path1, const char * path2, bool * identical)
{
    FILE * fp1 = fopen(path1, "r");
    if (fp1 == NULL) {
        FLOW_error_msg(c, flow_status_IO_error, "Failed to open file A for comparison (%s).", path1);
        return false;
    }
    FILE * fp2 = fopen(path2, "r");
    if (fp2 == NULL) {
        FLOW_error_msg(c, flow_status_IO_error, "Failed to open file B for comparison (%s).", path2);
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
#define FLOW_MAX_GRAPH_VERSIONS 100

bool flow_job_notify_node_complete(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id)
{
    struct flow_node * n = &g->nodes[node_id];
    if (n->result_bitmap != NULL && job->record_frame_images == true) {
        char path[1024];
        flow_snprintf(path, 1023, "node_frames/job_%d_node_%d.png", job->debug_job_id, node_id);
        if (!flow_bitmap_bgra_save_png(c, n->result_bitmap, path)) {
            FLOW_error_return(c);
        }
    }
    return true;
}

bool flow_job_notify_graph_changed(flow_c * c, struct flow_job * job, struct flow_graph * g)
{
    if (job == NULL || !job->record_graph_versions || job->next_graph_version > FLOW_MAX_GRAPH_VERSIONS)
        return true;

    char filename[255];
    char image_prefix[255];
    char prev_filename[255];

    if (job->next_graph_version == 0) {
        // Delete existing graphs
        int32_t i = 0;
        for (i = 0; i <= FLOW_MAX_GRAPH_VERSIONS; i++) {
            flow_snprintf(filename, 254, "job_%d_graph_version_%d.dot", job->debug_job_id, i);
            remove(filename);
            flow_snprintf(filename, 254, "job_%d_graph_version_%d.dot.png", job->debug_job_id, i);
            remove(filename);
            flow_snprintf(filename, 254, "job_%d_graph_version_%d.dot.svg", job->debug_job_id, i);
            remove(filename);
            int32_t node_ix = 0;
            for (node_ix = 0; node_ix < 42; node_ix++) {
                flow_snprintf(filename, 254, "./node_frames/job_%d_node_%d.png", job->debug_job_id, node_ix);
                remove(filename);
            }
        }
    }

    int32_t prev_graph_version = job->next_graph_version - 1;
    int32_t current_graph_version = job->next_graph_version;
    job->next_graph_version++;

    flow_snprintf(filename, 254, "job_%d_graph_version_%d.dot", job->debug_job_id, current_graph_version);

    flow_snprintf(image_prefix, 254, "./node_frames/job_%d_node_", job->debug_job_id);

    FILE * f = fopen(filename, "w");
    if (f == NULL) {
        FLOW_error_msg(c, flow_status_IO_error, "Failed to open %s for graph dotfile export.", filename);
        return false;
    }
    if (!flow_graph_print_to_dot(c, g, f, image_prefix)) {
        fclose(f);
        FLOW_error_return(c);
    } else {
        fclose(f);
    }
    // Compare
    if (job->next_graph_version > 1) {
        flow_snprintf(prev_filename, 254, "job_%d_graph_version_%d.dot", job->debug_job_id, prev_graph_version);
        bool identical = false;
        if (!files_identical(c, prev_filename, filename, &identical)) {
            FLOW_error_return(c);
        }
        if (identical) {
            job->next_graph_version--; // Next time we will overwrite the duplicate graph. The last two graphs may
            // remain dupes.
            remove(filename);
        } else if (job->render_graph_versions) {
            flow_job_render_graph_to_png(c, job, g, prev_graph_version);
        }
    }

    return true;
}

bool flow_job_render_graph_to_png(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t graph_version)
{
    char filename[255];
    flow_snprintf(filename, 254, "job_%d_graph_version_%d.dot", job->debug_job_id, graph_version);

    char dotfile_command[2048];
    flow_snprintf(dotfile_command, 2048, "dot -Tpng -Gsize=11,16\\! -Gdpi=150  -O %s", filename);
    int32_t ignore = system(dotfile_command);
    ignore++;
    return true;
}
