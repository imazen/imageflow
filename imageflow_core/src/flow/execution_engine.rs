use ::JobPtr;
use ::flow::definitions::*;
use ::internal_prelude::works_everywhere::*;
use petgraph::dot::Dot;
use std::process::Command;
use ::rustc_serialize::base64::ToBase64;
use super::visualize::{notify_graph_changed, GraphRecordingUpdate, GraphRecordingInfo};


pub struct Engine<'a, 'b> {
    c: *mut ::ffi::ImageflowContext,
    job_p: *mut ::ffi::ImageflowJob,
    job: &'a mut JobPtr,
    g: &'b mut Graph,
}

impl<'a, 'b> Engine<'a, 'b> {
    pub fn create(job: &'a mut JobPtr, g: &'b mut Graph) -> Engine<'a, 'b> {
        Engine {
            c: job.context_ptr(),
            job_p: job.as_ptr(),
            job: job,
            g: g,
        }
    }


    pub fn execute(&mut self) -> Result<()> {
        let job = self.job_p;
        self.notify_graph_changed()?;

        self.link_codecs()?;

        // States for a node
        // New
        // OutboundDimensionsKnown
        // Flattened
        // Optimized
        // LockedForExecution
        // Executed
        let mut passes = 0;
        loop {
            if self.graph_fully_executed() {
                break;
            }

            if passes >= unsafe { (*job).max_calc_flatten_execute_passes } {
                {
                    self.notify_graph_complete()?;
                }
                panic!("Maximum graph passes exceeded");
                //            error_msg!(c, FlowStatusCode::MaximumGraphPassesExceeded);
                //            return false;
            }
            self.populate_dimensions_where_certain()?;
            self.notify_graph_changed()?;

            self.graph_pre_optimize_flatten()?;
            self.notify_graph_changed()?;

            self.populate_dimensions_where_certain()?;
            self.notify_graph_changed()?;

            // graph_optimize()?;
            self.notify_graph_changed()?;

            self.populate_dimensions_where_certain()?;
            self.notify_graph_changed()?;

            self.graph_post_optimize_flatten()?;
            self.notify_graph_changed()?;

            self.populate_dimensions_where_certain()?;
            self.notify_graph_changed()?;

            self.graph_execute()?;
            passes += 1;

            self.notify_graph_changed()?;
        }
        unsafe {
            if (*job).next_graph_version > 0 && (*job).render_last_graph {
                self.notify_graph_complete()?;
            }
        }
        Ok(())
    }

    pub fn link_codecs(&mut self) -> Result<()> {
        self.notify_graph_changed()?;

        for index in 0..self.g.node_count() {
            if let Some(func) = self.g
                .node_weight(NodeIndex::new(index))
                .unwrap()
                .def
                .fn_link_state_to_this_io_id {
                let placeholder_id;
                {
                    let mut ctx = self.op_ctx_mut();
                    placeholder_id = func(&mut ctx, NodeIndex::new(index));
                }
                if let Some(io_id) = placeholder_id {
                    let codec_instance =
                        unsafe {
                            ::ffi::flow_job_get_codec_instance(self.c, self.job_p, io_id) as *mut u8
                        };
                    if codec_instance == ptr::null_mut() {
                        panic!("")
                    }

                    {
                        self.g.node_weight_mut(NodeIndex::new(index)).unwrap().custom_state =
                            codec_instance;
                    }
                    {
                        let weight = self.g.node_weight(NodeIndex::new(index)).unwrap();
                        // Now, try to send decoder its commands
                        // let ref mut weight = ctx.weight_mut(ix);

                        match weight.params {
                            NodeParams::Json(s::Node::Decode { io_id, ref commands }) => {
                                if let &Some(ref list) = commands {
                                    for c in list.iter() {
                                        self.job.tell_decoder(io_id, c.to_owned()).unwrap();
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        Ok(())
    }


    fn assign_stable_ids(&mut self) -> Result<()> {
        let job = self.job_p;

        // Assign stable IDs;
        for index in 0..self.g.node_count() {
            let mut weight = self.g.node_weight_mut(NodeIndex::new(index)).unwrap();
            if weight.stable_id < 0 {
                unsafe {
                    weight.stable_id = (*job).next_stable_node_id;
                    (*job).next_stable_node_id += 1;
                }
            }
        }
        Ok(())
    }


    fn notify_graph_changed(&mut self) -> Result<()> {
        let job = self.job_p;
        self.assign_stable_ids()?;

        let info = GraphRecordingInfo {
            debug_job_id: unsafe { (*job).debug_job_id },
            record_graph_versions: unsafe { (*job).record_graph_versions },
            current_graph_version: unsafe { (*job).next_graph_version },
            render_graph_versions: unsafe { (*job).render_graph_versions },
            maximum_graph_versions: 100,
        };
        let update = notify_graph_changed(self.g, info)?;
        if let Some(GraphRecordingUpdate { next_graph_version }) = update {
            unsafe {
                (*job).next_graph_version = next_graph_version;
            }
        }
        Ok(())
    }

    fn notify_graph_complete(&mut self) -> Result<()> {
        let job = self.job_p;
        let prev_filename = unsafe {
            format!("job_{}_graph_version_{}.dot",
                    (*job).debug_job_id,
                    (*job).next_graph_version - 1)
        };

        super::visualize::render_dotfile_to_png(&prev_filename);
        Ok(())
    }


    pub fn estimate_node(&mut self, node_id: NodeIndex<u32>) -> FrameEstimate {
        let now = time::precise_time_ns();
        let mut ctx = OpCtxMut {
            c: self.c,
            graph: self.g,
            job: self.job_p,
        };
        // Invoke estimation
        ctx.weight(node_id).def.fn_estimate.unwrap()(&mut ctx, node_id);
        ctx.weight_mut(node_id).cost.wall_ns + (time::precise_time_ns() - now) as u32;

        ctx.weight(node_id).frame_est
    }

    pub fn estimate_node_recursive(&mut self, node_id: NodeIndex<u32>) -> FrameEstimate {
        // If we're already done, no need
        if let FrameEstimate::Some(info) = self.g.node_weight(node_id).unwrap().frame_est {
            return FrameEstimate::Some(info);
        }

        // Otherwise let's try again
        let inputs_good = inputs_estimated(self.g, node_id);
        if !inputs_good {
            // TODO: support UpperBound eventually; for now, use Impossible until all nodes implement
            let give_up = inputs_estimates(self.g, node_id).iter().any(|est| match *est {
                FrameEstimate::Impossible => true,
                FrameEstimate::UpperBound(_) => true,
                _ => false,
            });

            // If it's possible, let's try to estimate parent nodes
            // This is problematic if we want a single call to 'fix' all Impossible nodes.
            // For nodes already populated by Impossible/UpperBound, they will have to be called directly.
            // We won't retry them recursively
            if !give_up {

                let input_indexes = self.g
                    .parents(node_id)
                    .iter(self.g)
                    .map(|(edge_ix, ix)| ix)
                    .collect::<Vec<NodeIndex<u32>>>();

                // println!("Estimating recursively {:?}", input_indexes);
                for ix in input_indexes {

                    self.estimate_node_recursive(ix);
                }
            }

            if give_up || !inputs_estimated(self.g, node_id) {
                self.g.node_weight_mut(node_id).unwrap().frame_est = FrameEstimate::Impossible;
                return FrameEstimate::Impossible;
            }
        }
        // Should be good on inputs here
        if self.estimate_node(node_id) == FrameEstimate::None {
            panic!("Node estimation misbehaved on {}. Cannot leave FrameEstimate::None, must chose an alternative",
                   self.g.node_weight(node_id).unwrap().def.name);
        }
        self.g.node_weight(node_id).unwrap().frame_est
    }

    pub fn populate_dimensions_where_certain(&mut self) -> Result<()> {

        for ix in 0..self.g.node_count() {
            // If any node returns FrameEstimate::Impossible, we might as well move on to execution pass.
            self.estimate_node_recursive(NodeIndex::new(ix));
        }

        Ok(())
    }


    pub fn graph_pre_optimize_flatten(&mut self) -> Result<()> {
        // Just find all nodes that offer fn_flatten_pre_optimize and have been estimated.
        // Oops, we also need to insure inputs have been estimated
        // TODO: Compare Node value; should differ afterwards
        loop {
            let mut next = None;
            for ix in 0..(self.g.node_count()) {
                if let Some(func) = self.g
                    .node_weight(NodeIndex::new(ix))
                    .unwrap()
                    .def
                    .fn_flatten_pre_optimize {
                    if let FrameEstimate::Some(_) = self.g
                        .node_weight(NodeIndex::new(ix))
                        .unwrap()
                        .frame_est {
                        if self.g
                            .parents(NodeIndex::new(ix))
                            .iter(self.g)
                            .all(|(ex, ix)| {
                                self.g.node_weight(ix).unwrap().result != NodeResult::None
                            }) {
                            next = Some((NodeIndex::new(ix), func));
                            break;
                        }
                    }
                }
            }
            match next {
                None => return Ok(()),
                Some((next_ix, next_func)) => {
                    let mut ctx = self.op_ctx_mut();
                    next_func(&mut ctx, next_ix);

                }
            }

        }
    }




    pub fn graph_post_optimize_flatten(&mut self) -> Result<()> {
        // Just find all nodes that offer fn_flatten_pre_optimize and have been estimated.
        // TODO: Compare Node value; should differ afterwards
        loop {
            let mut next = None;
            for ix in 0..(self.g.node_count()) {
                if let Some(func) = self.g
                    .node_weight(NodeIndex::new(ix))
                    .unwrap()
                    .def
                    .fn_flatten_post_optimize {
                    if let FrameEstimate::Some(_) = self.g
                        .node_weight(NodeIndex::new(ix))
                        .unwrap()
                        .frame_est {
                        if self.g
                            .parents(NodeIndex::new(ix))
                            .iter(self.g)
                            .all(|(ex, ix)| {
                                self.g.node_weight(ix).unwrap().result != NodeResult::None
                            }) {
                            next = Some((NodeIndex::new(ix), func));
                            break;
                        }
                    }
                }
            }
            match next {
                None => return Ok(()),
                Some((next_ix, next_func)) => {
                    let mut ctx = self.op_ctx_mut();
                    next_func(&mut ctx, next_ix);

                }
            }

        }
    }



    pub fn graph_execute(&mut self) -> Result<()> {
        // Find nodes with fn_execute, which also have been estimated, and whose parents are complete
        // AND who are not already complete
        loop {
            let mut next = None;
            for ix in 0..(self.g.node_count()) {
                if let Some(func) = self.g.node_weight(NodeIndex::new(ix)).unwrap().def.fn_execute {
                    if let FrameEstimate::Some(_) = self.g
                        .node_weight(NodeIndex::new(ix))
                        .unwrap()
                        .frame_est {
                        if self.g.node_weight(NodeIndex::new(ix)).unwrap().result ==
                           NodeResult::None {
                            if self.g
                                .parents(NodeIndex::new(ix))
                                .iter(self.g)
                                .all(|(ex, ix)| {
                                    self.g.node_weight(ix).unwrap().result != NodeResult::None
                                }) {
                                next = Some((NodeIndex::new(ix), func));
                                break;
                            }
                        }
                    }
                }
            }
            match next {
                None => return Ok(()),
                Some((next_ix, next_func)) => {
                    {
                        let mut ctx = self.op_ctx_mut();
                        next_func(&mut ctx, next_ix);
                    }
                    if self.g.node_weight(next_ix).unwrap().result == NodeResult::None {
                        panic!("fn_execute of {} failed to save a result",
                               self.g.node_weight(next_ix).unwrap().def.name);
                    } else {
                        let job = self.job_p;
                        unsafe {
                            if (*job).record_frame_images {
                                if let NodeResult::Frame(ptr) = self.g
                                    .node_weight(next_ix)
                                    .unwrap()
                                    .result {
                                    let path = format!("node_frames/job_{}_node_{}.png",
                                                (*job).debug_job_id,
                                                self.g.node_weight(next_ix).unwrap().stable_id);
                                    let path_copy = path.clone();
                                    let path_cstr = std::ffi::CString::new(path).unwrap();
                                    let _ = std::fs::create_dir("node_frames");
                                    if !::ffi::flow_bitmap_bgra_save_png(self.c,
                                                                         ptr,
                                                                         path_cstr.as_ptr()) {
                                        println!("Failed to save frame {} (from node {})",
                                                 path_copy,
                                                 next_ix.index());
                                        ::ContextPtr::from_ptr(self.c).assert_ok(None);

                                    }
                                }
                            }
                        }
                    }
                }
            }

        }
    }
    fn op_ctx_mut<'c>(&'c mut self) -> OpCtxMut<'c> {
        OpCtxMut {
            c: self.c,
            graph: self.g,
            job: self.job_p,
        }
    }

    fn graph_fully_executed(&self) -> bool {
        for node in self.g.raw_nodes() {
            if node.weight.result == NodeResult::None {
                return false;
            }
        }
        return true;
    }
}
impl<'a> OpCtxMut<'a> {
    // TODO: Should return Result<String,??>
    pub fn graph_to_str(&mut self) -> Result<String> {
        let mut vec = Vec::new();
        super::visualize::print_graph(&mut vec, self.graph, None).unwrap();
        Ok(String::from_utf8(vec).unwrap())
    }
}



use daggy::walker::Walker;



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
