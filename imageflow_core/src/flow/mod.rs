use ffi::*;
use libc::{self, int32_t};
use std::ffi::CStr;

macro_rules! error_return (
    ($context:expr) => (
        unsafe {
            flow_context_add_to_callstack($context, concat!(file!(), "\0").as_ptr() as *const libc::c_char,
                line!() as i32, concat!(module_path!(), "\0").as_ptr() as *const libc::c_char);
        }
    );
);

macro_rules! error_msg (
    ($context:expr, $status: expr) => (
        unsafe {
            let c = CStr::from_ptr($crate::ffi::flow_context_set_error_get_message_buffer($context, $status as i32,
                concat!(file!(), "\0").as_ptr() as *const libc::c_char,
                line!() as i32, concat!(module_path!(), "\0").as_ptr() as *const libc::c_char));
            println!("{:?}", c);
        }
    );
    ($context:expr, $status: expr, $format:expr, $($args:expr),*) => (
        let c = CStr::from_ptr($crate::ffi::flow_context_set_error_get_message_buffer($context, $status as i32,
            concat!(file!(), "\0").as_ptr() as *const libc::c_char,
            line!() as i32, concat!(module_path!(), "\0").as_ptr() as *const libc::c_char));
        let formatted = fmt::format(format_args!(concat!($format, "\0"),$($args),*));
        println!("{:?} {}", c, formatted);
    );
);

pub fn job_execute(c: *mut Context, job: *mut Job, graph_ref: *mut *mut Graph) -> bool {
  if !flow_job_notify_graph_changed(c, job, graph_ref) {
        error_return!(c);
    }
    if !job_link_codecs(c, job, graph_ref) {
        error_return!(c);
    }
/* FIXME
    // States for a node
    // New
    // OutboundDimensionsKnown
    // Flattened
    // Optimized
    // LockedForExecution
    // Executed
    let mut passes: libc::int32_t = 0;
    while (!flow_job_graph_fully_executed(c, job, *graph_ref)) {
        if (passes >= job->max_calc_flatten_execute_passes) {
            FLOW_error(c, flow_status_Maximum_graph_passes_exceeded);
            return false;
        }
        if (!flow_job_populate_dimensions_where_certain(c, job, graph_ref)) {
            error_return!(c);
        }
        if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
            error_return!(c);
        }
        if (!flow_graph_pre_optimize_flatten(c, graph_ref)) {
            error_return!(c);
        }
        if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
            error_return!(c);
        }
        if (!flow_job_populate_dimensions_where_certain(c, job, graph_ref)) {
            error_return!(c);
        }
        if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
            error_return!(c);
        }
        if (!flow_graph_optimize(c, job, graph_ref)) {
            error_return!(c);
        }
        if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
            error_return!(c);
        }
        if (!flow_job_populate_dimensions_where_certain(c, job, graph_ref)) {
            error_return!(c);
        }
        if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
            error_return!(c);
        }
        if (!flow_graph_post_optimize_flatten(c, job, graph_ref)) {
            error_return!(c);
        }
        if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
            error_return!(c);
        }
        if (!flow_job_populate_dimensions_where_certain(c, job, graph_ref)) {
            error_return!(c);
        }
        if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
            error_return!(c);
        }
        if (!flow_job_execute_where_certain(c, job, graph_ref)) {
            error_return!(c);
        }
        passes++;

        if (!flow_job_notify_graph_changed(c, job, *graph_ref)) {
            error_return!(c);
        }
    }
    if (job->next_graph_version > 0 && job->render_last_graph
        && !flow_job_render_graph_to_png(c, job, *graph_ref, job->next_graph_version - 1)) {
        error_return!(c);
    }
*/
  true
}

pub fn job_link_codecs(c: *mut Context, job: *mut Job, graph_ref: *mut *mut Graph) -> bool {

    if graph_ref.is_null() || unsafe { (*graph_ref).is_null() } {
        error_msg!(c, FlowStatusCode::NullArgument);
        return false;
    }
    if !flow_job_notify_graph_changed(c, job, graph_ref) {
        error_return!(c);
    }

/* FIXME
    struct flow_graph * g = *graph_ref;
    let mut i: int32_t = 0;
    for (i = 0; i < g->next_node_id; i++) {
        if (g->nodes[i].type == flow_ntype_decoder || g->nodes[i].type == flow_ntype_encoder) {
            uint8_t * info_bytes = &g->info_bytes[g->nodes[i].info_byte_index];
            struct flow_nodeinfo_codec * info = (struct flow_nodeinfo_codec *)info_bytes;
            if (info->codec == NULL) {
                info->codec = flow_job_get_codec_instance(c, job, info->placeholder_id);

                if (info->codec == NULL)
                    FLOW_error_msg(c, flow_status_Graph_invalid,
                                   "No matching codec or io found for placeholder id %d (node #%d).",
                                   info->placeholder_id, i);
            }
        }
    }
*/

    return true;
}

fn flow_job_notify_graph_changed(c: *mut Context, job: *mut Job, graph_ref: *mut *mut Graph) -> bool {
/* FIXME
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
*/
    return true;
}

pub fn flow_job_graph_fully_executed(c: *mut Context, job: *mut Job, graph_ref: *mut *mut Graph) -> bool
{
/*FIXME
    int32_t i;
    for (i = 0; i < g->next_node_id; i++) {
        if (g->nodes[i].type != flow_ntype_Null) {
            if (!flow_job_node_is_executed(c, job, g, i)) {
                return false;
            }
        }
    }
*/
    return true;
}

