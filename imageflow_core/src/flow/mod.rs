use ffi::*;
use libc::{self, int32_t, c_void};
use std::ffi::CStr;
use std;
use std::fs::File;
use std::io::Write;
use std::process::Command;
use petgraph::dot::Dot;
use petgraph::graph::{NodeIndex};
use time;

pub mod graph;
pub mod definitions;
pub mod nodes;
use self::graph::Graph;
use self::definitions::*;

#[macro_export]
macro_rules! error_return (
    ($context:expr) => (
        unsafe {
            flow_context_add_to_callstack($context, concat!(file!(), "\0").as_ptr() as *const libc::c_char,
                line!() as i32, concat!(module_path!(), "\0").as_ptr() as *const libc::c_char);
        }
    );
);

#[macro_export]
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

pub fn job_execute(c: *mut Context, job: *mut Job, graph_ref: &mut Graph) -> bool {
  if !job_notify_graph_changed(c, job, graph_ref) {
        error_return!(c);
    }
    if !job_link_codecs(c, job, graph_ref) {
        error_return!(c);
    }
    // States for a node
    // New
    // OutboundDimensionsKnown
    // Flattened
    // Optimized
    // LockedForExecution
    // Executed
    let mut passes: libc::int32_t = 0;
    while !job_graph_fully_executed(c, job, graph_ref) {
        if passes >= unsafe { (*job).max_calc_flatten_execute_passes } {
            error_msg!(c, FlowStatusCode::MaximumGraphPassesExceeded);
            return false;
        }
        if !job_populate_dimensions_where_certain(c, job, graph_ref) {
            error_return!(c);
        }
        if !job_notify_graph_changed(c, job, graph_ref) {
            error_return!(c);
        }
        if !graph_pre_optimize_flatten(c, job, graph_ref) {
            error_return!(c);
        }
        if !job_notify_graph_changed(c, job, graph_ref) {
            error_return!(c);
        }
        if !job_populate_dimensions_where_certain(c, job, graph_ref) {
            error_return!(c);
        }
        if !job_notify_graph_changed(c, job, graph_ref) {
            error_return!(c);
        }
        if !graph_optimize(c, job, graph_ref) {
            error_return!(c);
        }
        if !job_notify_graph_changed(c, job, graph_ref) {
            error_return!(c);
        }
        if !job_populate_dimensions_where_certain(c, job, graph_ref) {
            error_return!(c);
        }
        if !job_notify_graph_changed(c, job, graph_ref) {
            error_return!(c);
        }
        if !graph_post_optimize_flatten(c, job, graph_ref) {
            error_return!(c);
        }
        if !job_notify_graph_changed(c, job, graph_ref) {
            error_return!(c);
        }
        if !job_populate_dimensions_where_certain(c, job, graph_ref) {
            error_return!(c);
        }
        if !job_notify_graph_changed(c, job, graph_ref) {
            error_return!(c);
        }
        if !graph_execute(c, job, graph_ref) {
            error_return!(c);
        }
        passes += 1;

        if !job_notify_graph_changed(c, job, graph_ref) {
            error_return!(c);
        }
    }
    if unsafe { (*job).next_graph_version > 0 && (*job).render_last_graph
        && !job_render_graph_to_png(c, job, graph_ref, (*job).next_graph_version - 1)} {
        error_return!(c);
    }
  true
}

pub fn job_link_codecs(c: *mut Context, job: *mut Job, graph_ref: &mut Graph) -> bool {
    /*FIXME: will it still be needed?
    if graph_ref.is_null() || unsafe { (*graph_ref).is_null() } {
        error_msg!(c, FlowStatusCode::NullArgument);
        return false;
    }
    */
    if !job_notify_graph_changed(c, job, graph_ref) {
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

fn job_notify_graph_changed(c: *mut Context, job: *mut Job, graph_ref: &mut Graph) -> bool {
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

use daggy::walker::Walker;
pub fn job_graph_fully_executed(c: *mut Context, job: *mut Job, graph_ref: &mut Graph) -> bool
{
    for node in graph_ref.raw_nodes() {
        if node.weight.result == NodeResult::None {
            return false
        }
    }
    return true;
}


pub fn flow_node_has_dimensions(g: &Graph, node_id: NodeIndex<u32>) -> bool
{
    g.node_weight(node_id).map(|node| match node.frame_est { FrameEstimate::Some(_) => true, _ => false}).unwrap_or(false)
}

pub fn inputs_estimated(g: &Graph, node_id: NodeIndex<u32>) -> bool
{
    inputs_estimates(g,node_id).iter().any (|est| match *est { FrameEstimate::Some(_) => true, _ => false})
}

//-> impl Iterator<Item = FrameEstimate> caused compiler panic

pub fn inputs_estimates(g: &Graph, node_id: NodeIndex<u32>) -> Vec<FrameEstimate>
{
    g.parents(node_id).iter(g).filter_map(|(_, node_index)| {
        g.node_weight(node_index).map(|w| w.frame_est)
    }).collect()
}

pub fn estimate_node(c: *mut Context, job: *mut Job, g: &mut Graph,
                                             node_id: NodeIndex<u32>) -> FrameEstimate
{
    let now = time::precise_time_ns();
    let mut ctx = OpCtxMut{
        c: c,
        graph: g,
        job: job
    };
    //Invoke estimation
    ctx.weight(node_id).def.fn_estimate.unwrap()(&mut ctx, node_id);
    ctx.weight_mut(node_id).cost.wall_ticks + (time::precise_time_ns() - now) as u32;

    ctx.weight(node_id).frame_est
}

pub fn estimate_node_recursive(c: *mut Context, job: *mut Job, g: &mut Graph,
                                             node_id: NodeIndex<u32>) -> FrameEstimate
{
    //If we're already done, no need
    if let FrameEstimate::Some(info) = g.node_weight(node_id).unwrap().frame_est{
        return FrameEstimate::Some(info);
    }

    //Otherwise let's try again
    let inputs_good = inputs_estimated(g, node_id);
    if !inputs_good{
        //TODO: support UpperBound eventually; for now, use Impossible until all nodes implement
        let give_up = inputs_estimates(g, node_id).iter().any(|est| match *est { FrameEstimate::Impossible => true, FrameEstimate::UpperBound(_) => true, _ => false} );

        //If it's possible, let's try to estimate parent nodes
        //This is problematic if we want a single call to 'fix' all Impossible nodes.
        //For nodes already populated by Impossible/UpperBound, they will have to be called directly.
        //We won't retry them recursively
        if !give_up {

            let input_indexes =  g.parents(node_id).iter(g).map(|(edge_ix, ix)| ix).collect::<Vec<NodeIndex<u32>>>();

            for ix in input_indexes {
                estimate_node_recursive(c, job, g, ix);
            }
        }

        if give_up || !inputs_estimated(g, node_id){
            g.node_weight_mut(node_id).unwrap().frame_est = FrameEstimate::Impossible;
            return FrameEstimate::Impossible;
        }
    }
    //Should be good on inputs here
    if estimate_node(c, job, g, node_id) == FrameEstimate::None {
        panic!("Node estimation misbehaved. Cannot leave FrameEstimate::None, must chose an alternative");
    }
    g.node_weight(node_id).unwrap().frame_est
}

pub fn job_populate_dimensions_where_certain(c:*mut Context, job: *mut Job, g: &mut Graph) -> bool
{

    for ix in 0..g.node_count(){
        //If any node returns FrameEstimate::Impossible, we might as well move on to execution pass.
        estimate_node_recursive(c, job, g, NodeIndex::new(ix));
    }

    return true;
}


pub fn graph_pre_optimize_flatten(c: *mut Context, job: *mut Job, g: &mut Graph) -> bool
{
    //Just find all nodes that offer fn_flatten_pre_optimize and have been estimated.
    // TODO: Compare Node value; should differ afterwards
    loop {
        let mut next = None;
        for ix in 0..(g.node_count()){
            if let Some(func) = g.node_weight(NodeIndex::new(ix)).unwrap().def.fn_flatten_pre_optimize{
                if let FrameEstimate::Some(_) = g.node_weight(NodeIndex::new(ix)).unwrap().frame_est {
                    next = Some((NodeIndex::new(ix), func));
                    break;
                }
            }
        }
        match next {
            None => {return true},
            Some((next_ix, next_func)) => {
                let mut ctx = OpCtxMut{
                    c: c,
                    graph: g,
                    job: job
                };
                next_func(&mut ctx, next_ix);

            }
        }

    }
}

pub fn graph_optimize(c: *mut Context,job: *mut Job, graph_ref: &mut Graph) -> bool
{
    /*FIXME: is it still needed?
    if unsafe { (*graph_ref).is_null()} {
        error_msg!(c, FlowStatusCode::NullArgument);
        return false;
    }
    */
    /*FIXME
    bool re_walk;
    do {
        re_walk = false;
        if (!flow_graph_walk(c, job, graph_ref, node_visitor_optimize, NULL, &re_walk)) {
            FLOW_error_return(c);
        }
    } while (re_walk);
    */
    return true;
}



pub fn graph_post_optimize_flatten(c: *mut Context, job: *mut Job, g: &mut Graph) -> bool
{
    //Just find all nodes that offer fn_flatten_pre_optimize and have been estimated.
    // TODO: Compare Node value; should differ afterwards
    loop {
        let mut next = None;
        for ix in 0..(g.node_count()){
            if let Some(func) = g.node_weight(NodeIndex::new(ix)).unwrap().def.fn_flatten_post_optimize{
                if let FrameEstimate::Some(_) = g.node_weight(NodeIndex::new(ix)).unwrap().frame_est {
                    next = Some((NodeIndex::new(ix), func));
                    break;
                }
            }
        }
        match next {
            None => {return true},
            Some((next_ix, next_func)) => {
                let mut ctx = OpCtxMut{
                    c: c,
                    graph: g,
                    job: job
                };
                next_func(&mut ctx, next_ix);

            }
        }

    }
}



pub fn graph_execute(c: *mut Context, job: *mut Job, g: &mut Graph) -> bool
{
    //Just find all nodes that offer fn_flatten_pre_optimize and have been estimated.
    // TODO: Check result
    //Verify all nodes remaining have fn_execute

    loop {
        let mut next = None;
        for ix in 0..(g.node_count()){
            if let Some(func) = g.node_weight(NodeIndex::new(ix)).unwrap().def.fn_execute{
                if let FrameEstimate::Some(_) = g.node_weight(NodeIndex::new(ix)).unwrap().frame_est {
                    if g.node_weight(NodeIndex::new(ix)).unwrap().result == NodeResult::None {
                        if g.parents(NodeIndex::new(ix)).iter(g).all(|(ex,ix)| g.node_weight(ix).unwrap().result != NodeResult::None){
                            next = Some((NodeIndex::new(ix), func));
                            break;
                        }
                    }
                }
            }
        }
        match next {
            None => {return true},
            Some((next_ix, next_func)) => {
                let mut ctx = OpCtxMut{
                    c: c,
                    graph: g,
                    job: job
                };
                next_func(&mut ctx, next_ix);

            }
        }

    }
}

pub fn job_render_graph_to_png(c: *mut Context, job: *mut Job, g: &mut Graph, graph_version: int32_t) -> bool
{
    let filename = format!("job_{}_graph_version_{}.dot", unsafe { (*job).debug_job_id }, graph_version);
    let mut file = File::create(&filename).unwrap();
    let _ = file.write_fmt(format_args!("{:?}", Dot::new(g.graph())));
    Command::new("dot").arg("-Tpng").arg("-Gsize=11,16\\!").arg("-Gdpi=150").arg("-O").arg(filename)
                       .spawn().expect("dot command failed");
    return true;
}

pub fn node_visitor_optimize(c: *mut Context, job: *mut Job, graph_ref: &mut Graph, node_id: NodeIndex<u32>,
                                  quit:*mut bool, skip_outbound_paths: *mut bool, custom_data: *mut c_void) -> bool
{
    graph_ref.node_weight_mut(node_id).map(|node| {
        // Implement optimizations
        if node.stage == NodeStage::ReadyForOptimize {
            //FIXME: should we implement AND on NodeStage? Yes
            //node.stage |= NodeStage::Optimized;
            node.stage = NodeStage::Optimized;
        }
        true
    }).unwrap_or(false)
}

pub fn flow_job_force_populate_dimensions(c: *mut Context, job: *mut Job, graph_ref: &mut Graph) -> bool
{
    //FIXME: reimplement
    // TODO: would be good to verify graph is acyclic.
    //if (!flow_graph_walk(c, job, graph_ref, node_visitor_dimensions, NULL, (void *)true)) {
    //    FLOW_error_return(c);
    //}
    return true;
}

pub fn flow_node_populate_dimensions(c: *mut Context, g: &mut Graph, node_id: NodeIndex<u32>, force_estimate: bool) -> bool
{
    // FIXME: do we need to validate if daggy ensures the graph is valid?
    /*if (!flow_node_validate_edges(c, g, node_id)) {
        FLOW_error_return(c);
    }
    struct flow_node * node = &g->nodes[node_id];
    struct flow_node_definition * def = flow_nodedef_get(c, node->type);
    if (def == NULL) {
        FLOW_error_return(c);
    }
    if (def->populate_dimensions == NULL) {
        FLOW_error_msg(c, flow_status_Not_implemented, "populate_dimensions is not implemented for node type %s",
                       def->type_name);
        return false;
    } else {
        if (!def->populate_dimensions(c, g, node_id, force_estimate)) {
            FLOW_error_return(c);
        }
    }
    */
    return true;
}

/* FIXME
static bool node_visitor_post_optimize_flatten(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref,
                                               int32_t node_id, bool * quit, bool * skip_outbound_paths,
                                               void * custom_data)
{

    if (!flow_node_update_state(c, *graph_ref, node_id)) {
        FLOW_error_return(c);
    }
    struct flow_node * n = &(*graph_ref)->nodes[node_id];

    // If input nodes are populated
    if (n->state == flow_node_state_ReadyForPostOptimizeFlatten) {
        if (!flow_node_post_optimize_flatten(c, graph_ref, node_id)) {
            FLOW_error_return(c);
        }
        if (!flow_graph_validate(c, *graph_ref)) {
            FLOW_error_return(c);
        }
        *quit = true;
        *((bool *)custom_data) = true;
    } else if ((n->state & flow_node_state_InputDimensionsKnown) == 0) {
        // we can't flatten past missing dimensions
        *skip_outbound_paths = true;
    }
    return true;
}

static bool node_visitor_dimensions(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref, int32_t node_id,
                                    bool * quit, bool * skip_outbound_paths, void * custom_data)
{

    struct flow_node * n = &(*graph_ref)->nodes[node_id];

    int32_t outbound_edges = flow_graph_get_edge_count(c, *graph_ref, node_id, false, flow_edgetype_null, false, true);
    if (outbound_edges == 0) {
        return true; // Endpoint node - no need.
    }
    if (!flow_node_has_dimensions(c, *graph_ref, node_id)) {
        if (!flow_node_update_state(c, *graph_ref, node_id)) {
            FLOW_error_return(c);
        }

        // If input nodes are populated
        if ((n->state & flow_node_state_InputDimensionsKnown) > 0) {
            if (!flow_job_populate_dimensions_for_node(c, job, *graph_ref, node_id, (bool)custom_data)) {
                FLOW_error_return(c);
            }
        }
        if (!flow_node_has_dimensions(c, *graph_ref, node_id)) {
            // We couldn't populate this edge, so we sure can't populate others in this direction.
            // Stop this branch of recursion
            *skip_outbound_paths = true;
        } else {
            flow_job_notify_graph_changed(c, job, *graph_ref);
        }
    }
    return true;
}



//FIXME: can be deleted
static bool flow_job_node_is_executed(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id)
{
    return (g->nodes[node_id].stage & flow_node_state_Executed) > 0;
}
*/


/*FIXME
static bool node_visitor_execute(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref, int32_t node_id,
                                 bool * quit, bool * skip_outbound_paths, void * custom_data)
{

    if (!flow_node_update_state(c, *graph_ref, node_id)) {
        FLOW_error_return(c);
    }
    struct flow_node * n = &(*graph_ref)->nodes[node_id];

    if (!flow_job_node_is_executed(c, job, *graph_ref, node_id) && n->state == flow_node_state_ReadyForExecution) {
        uint64_t now = flow_get_high_precision_ticks();
        if (!flow_node_execute(c, job, *graph_ref, node_id)) {
            FLOW_error_return(c);
        } else {
            (*graph_ref)->nodes[node_id].ticks_elapsed += (int32_t)(flow_get_high_precision_ticks() - now);
            n->state = (flow_node_state)(n->state | flow_node_state_Executed);
            flow_job_notify_node_complete(c, job, *graph_ref, node_id);
        }
    }
    if (!flow_job_node_is_executed(c, job, *graph_ref, node_id)) {
        // If we couldn't complete this node yet, end this branch.
        *skip_outbound_paths = true;
    } else {
        flow_job_notify_graph_changed(c, job, *graph_ref);
    }
    return true;
}

// if no hits, search forward


static bool node_visitor_flatten(flow_c * c, struct flow_job * job, struct flow_graph ** graph_ref, int32_t node_id,
                                 bool * quit, bool * skip_outbound_paths, void * custom_data)
{

    if (!flow_node_update_state(c, *graph_ref, node_id)) {
        FLOW_error_return(c);
    }
    struct flow_node * n = &(*graph_ref)->nodes[node_id];

    // If input nodes are populated
    if (n->state == flow_node_state_ReadyForPreOptimizeFlatten) {
        if (!flow_node_pre_optimize_flatten(c, graph_ref, node_id)) {
            FLOW_error_return(c);
        }
        *quit = true;
        *((bool *)custom_data) = true;
    } else if ((n->state & flow_node_state_InputDimensionsKnown) == 0) {
        // we can't flatten past missing dimensions
        *skip_outbound_paths = true;
    }
    return true;
}

*/
