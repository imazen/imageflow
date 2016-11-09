use ffi::*;
use libc::{self, int32_t, c_void};
use petgraph::dot::Dot;
use petgraph::graph::{NodeIndex, EdgeIndex};
use std;
use std::ffi::CStr;
use std::fmt;
use std::fs::File;
use std::io;
use std::io::prelude::*;
// use std::fs::PathExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::ptr;
use std::string;
use time;

pub mod definitions;
pub mod nodes;
use self::definitions::*;

pub mod graph {
    pub use ::flow::definitions::Graph;
    pub fn create(max_edges: u32,
                  max_nodes: u32)
                  -> Graph {
        Graph::with_capacity(max_nodes as usize, max_edges as usize)
    }

}

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

impl ::JobPtr {
    pub fn execute(&self, graph_ref: &mut Graph) -> bool {
        let c = self.context_ptr();
        let job = self.as_ptr();

        job_notify_graph_changed(c, job, graph_ref);

        if !self.link_codecs(graph_ref) {
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
                unsafe {
                    let prev_filename = format!("job_{}_graph_version_{}.dot", (*job).debug_job_id, (*job).next_graph_version - 1);
                    render_dotfile_to_png(&prev_filename);
                }


                panic!("Maximum graph passes exceeded");
                //            error_msg!(c, FlowStatusCode::MaximumGraphPassesExceeded);
                //            return false;
            }
            if !job_populate_dimensions_where_certain(c, job, graph_ref) {
                error_return!(c);
            }
            job_notify_graph_changed(c, job, graph_ref);

            if !graph_pre_optimize_flatten(c, job, graph_ref) {
                error_return!(c);
            }
            job_notify_graph_changed(c, job, graph_ref);

            if !job_populate_dimensions_where_certain(c, job, graph_ref) {
                error_return!(c);
            }
            job_notify_graph_changed(c, job, graph_ref);

            if !graph_optimize(c, job, graph_ref) {
                error_return!(c);
            }
            job_notify_graph_changed(c, job, graph_ref);

            if !job_populate_dimensions_where_certain(c, job, graph_ref) {
                error_return!(c);
            }
            job_notify_graph_changed(c, job, graph_ref);

            if !graph_post_optimize_flatten(c, job, graph_ref) {
                error_return!(c);
            }
            job_notify_graph_changed(c, job, graph_ref);

            if !job_populate_dimensions_where_certain(c, job, graph_ref) {
                error_return!(c);
            }
            job_notify_graph_changed(c, job, graph_ref);

            if !graph_execute(c, job, graph_ref) {
                error_return!(c);
            }
            passes += 1;

            job_notify_graph_changed(c, job, graph_ref);
        }
        unsafe {
            if (*job).next_graph_version > 0 && (*job).render_last_graph {
                let prev_filename = format!("job_{}_graph_version_{}.dot", (*job).debug_job_id, (*job).next_graph_version - 1);

                render_dotfile_to_png(&prev_filename);
            }
        }
        true
    }

    pub fn link_codecs(&self, g: &mut Graph) -> bool {
        let c = self.context_ptr();
        let job = self.as_ptr();
        job_notify_graph_changed(c, job, g);

        // Assign stable IDs;
        for index in 0..g.node_count() {
            if let Some(func) = g.node_weight(NodeIndex::new(index))
                .unwrap()
                .def
                .fn_link_state_to_this_io_id {
                let placeholder_id;
                {
                    let mut ctx = OpCtxMut {
                        c: c,
                        graph: g,
                        job: job,
                    };
                    placeholder_id = func(&mut ctx, NodeIndex::new(index));
                }
                if let Some(io_id) = placeholder_id {
                    let codec_instance =
                    unsafe { ::ffi::flow_job_get_codec_instance(c, job, io_id) as *mut u8 };
                    if codec_instance == ptr::null_mut() { panic!("") }

                    g.node_weight_mut(NodeIndex::new(index)).unwrap().custom_state = codec_instance;
                }
            }
        }

        // FIXME
        // struct flow_graph * g = *graph_ref;
        // let mut i: int32_t = 0;
        // for (i = 0; i < g->next_node_id; i++) {
        // if (g->nodes[i].type == flow_ntype_decoder || g->nodes[i].type == flow_ntype_encoder) {
        // uint8_t * info_bytes = &g->info_bytes[g->nodes[i].info_byte_index];
        // struct flow_nodeinfo_codec * info = (struct flow_nodeinfo_codec *)info_bytes;
        // if (info->codec == NULL) {
        // info->codec = flow_job_get_codec_instance(c, job, info->placeholder_id);
        //
        // if (info->codec == NULL)
        // FLOW_error_msg(c, flow_status_Graph_invalid,
        // "No matching codec or io found for placeholder id %d (node #%d).",
        // info->placeholder_id, i);
        // }
        // }
        // }
        //

        true
    }
}

const FLOW_MAX_GRAPH_VERSIONS: i32 = 100;

fn remove_file_if_exists(path: &str) -> io::Result<()> {
    let result = std::fs::remove_file(path);
    if result.as_ref().err().and_then(|e| Some(e.kind() == io::ErrorKind::NotFound)) == Some(true) {
        return Ok(());
    }
    result
}

fn job_delete_graphviz(c: *mut Context, job: *mut Job) -> io::Result<()> {
    let job_id = unsafe { (*job).debug_job_id };
    let safety_limit = 8000;

    // Keep deleting until we run out of files or hit a safety limit
    let mut node_index = 0;
    loop {
        let next = format!("./node_frames/job_{}_node_{}.png", job_id, node_index);
        if !Path::new(&next).exists() || node_index > safety_limit {
            break;
        } else {
            node_index += 1;
            try!(remove_file_if_exists(&next));
        }
    }
    let mut version_index = 0;
    loop {
        let next = format!("job_{}_graph_version_{}.dot", job_id, version_index);
        let next_png = format!("job_{}_graph_version_{}.dot.png", job_id, version_index);
        let next_svg = format!("job_{}_graph_version_{}.dot.svg", job_id, version_index);
        if !Path::new(&next).exists() || version_index > safety_limit {
            break;
        } else {
            version_index += 1;
            try!(remove_file_if_exists(&next));
            try!(remove_file_if_exists(&next_png));
            try!(remove_file_if_exists(&next_svg));
        }
    }
    Ok(())
}

fn assign_stable_ids(job: *mut Job, graph_ref: &mut Graph) {
    // Assign stable IDs;
    for index in 0..graph_ref.node_count() {
        let mut weight = graph_ref.node_weight_mut(NodeIndex::new(index)).unwrap();
        if weight.stable_id < 0 {
            unsafe {
                weight.stable_id = (*job).next_stable_node_id;
                (*job).next_stable_node_id += 1;
            }
        }
    }
}




fn job_notify_graph_changed(c: *mut Context, job: *mut Job, graph_ref: &mut Graph) {

    assign_stable_ids(job, graph_ref);


    // Write out graphviz files
    let debug_job_id = unsafe { (*job).debug_job_id };
    let current_graph_version: i32 = unsafe { (*job).next_graph_version };
    let prev_graph_version = current_graph_version - 1;
    let record_graph_versions = unsafe { (*job).record_graph_versions };

    if record_graph_versions {
        // println!("record_graph_versions=true, current_graph_version={}", current_graph_version);
    }
    if job == ptr::null_mut() || !record_graph_versions ||
       current_graph_version > FLOW_MAX_GRAPH_VERSIONS {

        return;
    }

    if current_graph_version == 0 {
        job_delete_graphviz(c, job).unwrap();
    }


    // increment graph version
    unsafe { (*job).next_graph_version += 1 };

    let frame_prefix = format!("./node_frames/job_{}_node_", debug_job_id);

    let current_filename =
        format!("job_{}_graph_version_{}.dot", debug_job_id, current_graph_version);
    {
        let mut f = File::create(&current_filename).unwrap();
        print_graph(&mut f, graph_ref, Some(&frame_prefix)).unwrap();
        println!("Writing file {}", &current_filename);
    }
    if prev_graph_version >= 0 {
        let prev_filename =
            format!("job_{}_graph_version_{}.dot", debug_job_id, prev_graph_version);
        match files_identical(&current_filename, &prev_filename).expect(&format!("Comparison err'd for {} and {}", &current_filename, &prev_filename)){
            true => {
                unsafe {
                    // Next time we will overwrite the duplicate graph. The last two graphs may
                    // remaine dupes
                    (*job).next_graph_version -= 1;
                    std::fs::remove_file(&current_filename).unwrap();
                }
            },
            false => {
                if unsafe{ (*job).render_graph_versions} {
                    render_dotfile_to_png(&prev_filename)
                }
            }
        };
    }
}

impl<'a> OpCtxMut<'a> {
    // TODO: Should return Result<String,??>
    pub fn graph_to_str(&mut self) -> String {
        let mut vec = Vec::new();
        print_graph(&mut vec, self.graph, None).unwrap();
        String::from_utf8(vec).unwrap()
    }
}

fn files_identical(filename_a: &str, filename_b: &str) -> std::io::Result<bool> {
    let mut a = try!(File::open(filename_a));
    let mut a_str = Vec::new();
    try!(a.read_to_end(&mut a_str));
    let mut b = try!(File::open(filename_b));
    let mut b_str = Vec::new();
    try!(b.read_to_end(&mut b_str));

    Ok(a_str == b_str)
}

use daggy::walker::Walker;
pub fn job_graph_fully_executed(c: *mut Context, job: *mut Job, graph_ref: &mut Graph) -> bool {
    for node in graph_ref.raw_nodes() {
        if node.weight.result == NodeResult::None {
            return false;
        }
    }
    return true;
}


pub fn flow_node_has_dimensions(g: &Graph, node_id: NodeIndex<u32>) -> bool {
    g.node_weight(node_id)
        .map(|node| match node.frame_est {
            FrameEstimate::Some(_) => true,
            _ => false,
        })
        .unwrap_or(false)
}

pub fn inputs_estimated(g: &Graph, node_id: NodeIndex<u32>) -> bool {
    inputs_estimates(g, node_id).iter().all(|est| match *est {
        FrameEstimate::Some(_) => true,
        _ => false,
    })
}

// -> impl Iterator<Item = FrameEstimate> caused compiler panic

pub fn inputs_estimates(g: &Graph, node_id: NodeIndex<u32>) -> Vec<FrameEstimate> {
    g.parents(node_id)
        .iter(g)
        .filter_map(|(_, node_index)| g.node_weight(node_index).map(|w| w.frame_est))
        .collect()
}

pub fn estimate_node(c: *mut Context,
                     job: *mut Job,
                     g: &mut Graph,
                     node_id: NodeIndex<u32>)
                     -> FrameEstimate {
    let now = time::precise_time_ns();
    let mut ctx = OpCtxMut {
        c: c,
        graph: g,
        job: job,
    };
    // Invoke estimation
    ctx.weight(node_id).def.fn_estimate.unwrap()(&mut ctx, node_id);
    ctx.weight_mut(node_id).cost.wall_ns + (time::precise_time_ns() - now) as u32;

    ctx.weight(node_id).frame_est
}

pub fn estimate_node_recursive(c: *mut Context,
                               job: *mut Job,
                               g: &mut Graph,
                               node_id: NodeIndex<u32>)
                               -> FrameEstimate {
    // If we're already done, no need
    if let FrameEstimate::Some(info) = g.node_weight(node_id).unwrap().frame_est {
        return FrameEstimate::Some(info);
    }

    // Otherwise let's try again
    let inputs_good = inputs_estimated(g, node_id);
    if !inputs_good {
        // TODO: support UpperBound eventually; for now, use Impossible until all nodes implement
        let give_up = inputs_estimates(g, node_id).iter().any(|est| match *est {
            FrameEstimate::Impossible => true,
            FrameEstimate::UpperBound(_) => true,
            _ => false,
        });

        // If it's possible, let's try to estimate parent nodes
        // This is problematic if we want a single call to 'fix' all Impossible nodes.
        // For nodes already populated by Impossible/UpperBound, they will have to be called directly.
        // We won't retry them recursively
        if !give_up {

            let input_indexes =
                g.parents(node_id).iter(g).map(|(edge_ix, ix)| ix).collect::<Vec<NodeIndex<u32>>>();

            println!("Estimating recursively {:?}", input_indexes);
            for ix in input_indexes {

                estimate_node_recursive(c, job, g, ix);
            }
        }

        if give_up || !inputs_estimated(g, node_id) {
            g.node_weight_mut(node_id).unwrap().frame_est = FrameEstimate::Impossible;
            return FrameEstimate::Impossible;
        }
    }
    // Should be good on inputs here
    if estimate_node(c, job, g, node_id) == FrameEstimate::None {
        panic!("Node estimation misbehaved on {}. Cannot leave FrameEstimate::None, must chose an alternative", g.node_weight(node_id).unwrap().def.name);
    }
    g.node_weight(node_id).unwrap().frame_est
}

pub fn job_populate_dimensions_where_certain(c: *mut Context,
                                             job: *mut Job,
                                             g: &mut Graph)
                                             -> bool {

    for ix in 0..g.node_count() {
        // If any node returns FrameEstimate::Impossible, we might as well move on to execution pass.
        estimate_node_recursive(c, job, g, NodeIndex::new(ix));
    }

    return true;
}


pub fn graph_pre_optimize_flatten(c: *mut Context, job: *mut Job, g: &mut Graph) -> bool {
    // Just find all nodes that offer fn_flatten_pre_optimize and have been estimated.
    // Oops, we also need to insure inputs have been estimated
    // TODO: Compare Node value; should differ afterwards
    loop {
        let mut next = None;
        for ix in 0..(g.node_count()) {
            if let Some(func) = g.node_weight(NodeIndex::new(ix))
                .unwrap()
                .def
                .fn_flatten_pre_optimize {
                if let FrameEstimate::Some(_) = g.node_weight(NodeIndex::new(ix))
                    .unwrap()
                    .frame_est {
                    if g.parents(NodeIndex::new(ix))
                        .iter(g)
                        .all(|(ex, ix)| g.node_weight(ix).unwrap().result != NodeResult::None) {
                        next = Some((NodeIndex::new(ix), func));
                        break;
                    }
                }
            }
        }
        match next {
            None => return true,
            Some((next_ix, next_func)) => {
                let mut ctx = OpCtxMut {
                    c: c,
                    graph: g,
                    job: job,
                };
                next_func(&mut ctx, next_ix);

            }
        }

    }
}

pub fn graph_optimize(c: *mut Context, job: *mut Job, graph_ref: &mut Graph) -> bool {
    // FIXME: is it still needed?
    // if unsafe { (*graph_ref).is_null()} {
    // error_msg!(c, FlowStatusCode::NullArgument);
    // return false;
    // }
    //
    // FIXME
    // bool re_walk;
    // do {
    // re_walk = false;
    // if (!flow_graph_walk(c, job, graph_ref, node_visitor_optimize, NULL, &re_walk)) {
    // FLOW_error_return(c);
    // }
    // } while (re_walk);
    //
    return true;
}



pub fn graph_post_optimize_flatten(c: *mut Context, job: *mut Job, g: &mut Graph) -> bool {
    // Just find all nodes that offer fn_flatten_pre_optimize and have been estimated.
    // TODO: Compare Node value; should differ afterwards
    loop {
        let mut next = None;
        for ix in 0..(g.node_count()) {
            if let Some(func) = g.node_weight(NodeIndex::new(ix))
                .unwrap()
                .def
                .fn_flatten_post_optimize {
                if let FrameEstimate::Some(_) = g.node_weight(NodeIndex::new(ix))
                    .unwrap()
                    .frame_est {
                    if g.parents(NodeIndex::new(ix))
                        .iter(g)
                        .all(|(ex, ix)| g.node_weight(ix).unwrap().result != NodeResult::None) {
                        next = Some((NodeIndex::new(ix), func));
                        break;
                    }
                }
            }
        }
        match next {
            None => return true,
            Some((next_ix, next_func)) => {
                let mut ctx = OpCtxMut {
                    c: c,
                    graph: g,
                    job: job,
                };
                next_func(&mut ctx, next_ix);

            }
        }

    }
}



pub fn graph_execute(c: *mut Context, job: *mut Job, g: &mut Graph) -> bool {
    // Find nodes with fn_execute, which also have been estimated, and whose parents are complete
    // AND who are not already complete
    loop {
        let mut next = None;
        for ix in 0..(g.node_count()) {
            if let Some(func) = g.node_weight(NodeIndex::new(ix)).unwrap().def.fn_execute {
                if let FrameEstimate::Some(_) = g.node_weight(NodeIndex::new(ix))
                    .unwrap()
                    .frame_est {
                    if g.node_weight(NodeIndex::new(ix)).unwrap().result == NodeResult::None {
                        if g.parents(NodeIndex::new(ix))
                            .iter(g)
                            .all(|(ex, ix)| g.node_weight(ix).unwrap().result != NodeResult::None) {
                            next = Some((NodeIndex::new(ix), func));
                            break;
                        }
                    }
                }
            }
        }
        match next {
            None => return true,
            Some((next_ix, next_func)) => {
                {
                    let mut ctx = OpCtxMut {
                        c: c,
                        graph: g,
                        job: job,
                    };
                    next_func(&mut ctx, next_ix);
                }
                if g.node_weight(next_ix).unwrap().result == NodeResult::None {
                    panic!("fn_execute of {} failed to save a result", g.node_weight(next_ix).unwrap().def.name);
                } else {
                    unsafe {
                        if (*job).record_frame_images {
                            if let NodeResult::Frame(ptr) = g.node_weight(next_ix).unwrap().result {
                                let path = format!("node_frames/job_{}_node_{}.png", (*job).debug_job_id, g.node_weight(next_ix).unwrap().stable_id);
                                let path_copy = path.clone();
                                let path_cstr = std::ffi::CString::new(path).unwrap();
                                let _ = std::fs::create_dir("node_frames");
                                if !::ffi::flow_bitmap_bgra_save_png(c, ptr, path_cstr.as_ptr()) {
                                    println!("Failed to save frame {} (from node {})", path_copy, next_ix.index());
                                    ::ContextPtr::from_ptr(c).assert_ok(None);

                                }
                            }
                        }
                    }
                }
            }
        }

    }
}

pub fn render_dotfile_to_png(dotfile_path: &str) {
    Command::new("dot")
        .arg("-Tpng")
        .arg("-Gsize=11,16\\!")
        .arg("-Gdpi=150")
        .arg("-O")
        .arg(dotfile_path)
        .spawn()
        .expect("dot command failed");
}
// pub fn job_render_graph_to_png(c: *mut Context, job: *mut Job, g: &mut Graph, graph_version: int32_t) -> bool
// {
//    let filename = format!("job_{}_graph_version_{}.dot", unsafe { (*job).debug_job_id }, graph_version);
//    let mut file = File::create(&filename).unwrap();
//    let _ = file.write_fmt(format_args!("{:?}", Dot::new(g.graph())));
//
//    return true;
// }

pub fn node_visitor_optimize(c: *mut Context,
                             job: *mut Job,
                             graph_ref: &mut Graph,
                             node_id: NodeIndex<u32>,
                             quit: *mut bool,
                             skip_outbound_paths: *mut bool,
                             custom_data: *mut c_void)
                             -> bool {
    graph_ref.node_weight_mut(node_id)
        .map(|node| {
            // Implement optimizations
            if node.stage == NodeStage::ReadyForOptimize {
                // FIXME: should we implement AND on NodeStage? Yes
                // node.stage |= NodeStage::Optimized;
                node.stage = NodeStage::Optimized;
            }
            true
        })
        .unwrap_or(false)
}


static INDENT: &'static str = "    ";

fn get_pixel_format_name_for(bitmap: *const BitmapBgra) -> &'static str {
    unsafe { get_pixel_format_name((*bitmap).fmt, (*bitmap).alpha_meaningful) }
}

fn get_pixel_format_name(fmt: PixelFormat, alpha_meaningful: bool) -> &'static str {
    match fmt {
        PixelFormat::BGR24 => "bgra24",
        PixelFormat::Gray8 => "gray8",
        PixelFormat::BGRA32 if alpha_meaningful => "bgra32",
        PixelFormat::BGRA32 => "bgr32",
        // _ => "?"
    }
}

pub fn print_graph(f: &mut std::io::Write,
                   g: &Graph,
                   node_frame_filename_prefix: Option<&str>)
                   -> std::io::Result<()> {
    try!(writeln!(f, "digraph g {{\n"));
    try!(writeln!(f, "{}node [shape=box, fontsize=20, fontcolor=\"#5AFA0A\" fontname=\"sans-serif bold\"]\n  size=\"12,18\"\n", INDENT));
    try!(writeln!(f, "{}edge [fontsize=20, fontname=\"sans-serif\"]\n", INDENT));


    // output all edges
    for (i, edge) in g.raw_edges().iter().enumerate() {
        try!(write!(f, "{}n{} -> n{}",
                    INDENT,
                    edge.source().index(),
                    edge.target().index()));

        let weight = g.node_weight(edge.source()).unwrap();

        let dimensions = match weight.result {
            NodeResult::Frame(ptr) => {
                unsafe { format!("frame {}x{} {}", (*ptr).w, (*ptr).h, get_pixel_format_name_for(ptr)) }
            }
            _ => {
                match weight.frame_est {
                    FrameEstimate::None => "?x?".to_owned(),
                    FrameEstimate::Some(info) => format!("est {}x{} {}", info.w, info.h, get_pixel_format_name(info.fmt, info.alpha_meaningful)),
                    _ => "!x!".to_owned(),
                }
            }
        };
        try!(write!(f, " [label=\"e{}: {}{}\"]\n", i, dimensions, match g.edge_weight(EdgeIndex::new(i)).unwrap() {
            &EdgeKind::Canvas => " canvas",
            _ => ""
        }));
    }

    let mut total_ns: u64 = 0;

    // output all labels
    for index in g.graph().node_indices() {
        let weight: &Node = g.node_weight(index).unwrap();
        total_ns += weight.cost.wall_ns as u64;
        let ms = weight.cost.wall_ns as f64 / 1000f64;

        try!(write!(f, "{}n{} [", INDENT, index.index()));

        if let Some(prefix) = node_frame_filename_prefix {
            try!(write!(f, "image=\"{}{}.png\", ", prefix, weight.stable_id));
        }
        try!(write!(f, "label=\"n{}: ",  index.index()));
        try!(weight.graphviz_node_label(f));
        try!(write!(f, "\n{:.5}ms\"]\n", ms));
    }
    let total_ms = (total_ns as f64) / 1000.0f64;
    try!(writeln!(f, "{}graphinfo [label=\"{} nodes\n{} edges\nExecution time: {:.3}ms\"]\n",
                  INDENT, g.node_count(), g.edge_count(), total_ms));
    try!(writeln!(f, "}}"));
    Ok(())
}
