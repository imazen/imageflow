use ::{Job,Context};
use ::flow::definitions::*;
use ::ffi::CodecInstance;
use ::internal_prelude::works_everywhere::*;
use petgraph::dot::Dot;
use std::process::Command;
use ::rustc_serialize::base64::ToBase64;
use super::visualize::{notify_graph_changed, GraphRecordingUpdate, GraphRecordingInfo};
use petgraph::EdgeDirection;

pub struct Engine<'a, 'b> where 'a: 'b {
    c: &'a Context,
    job: &'a mut Job,
    g: &'b mut Graph,
}

impl<'a, 'b> Engine<'a, 'b> where 'a: 'b {


    pub fn create(context: &'a Context, job: &'a mut Job, g: &'b mut Graph) -> Engine<'a, 'b> {
        Engine {
            c: context,
            job: job,
            g: g,
        }
    }

    fn flow_c(&self) -> *mut ::ffi::ImageflowContext{
        self.c.flow_c()
    }

    pub fn validate_graph(&self) -> Result<()> {
        for index in 0..self.g.node_count() {
            let node_index = NodeIndex::new(index);
            let node_def: &NodeDefinition = self.g
                .node_weight(node_index)
                .unwrap()
                .def;

            let outbound_count = self.g
                .graph()
                .edges_directed(node_index, EdgeDirection::Outgoing).count();

            let input_count = self.g
                .graph()
                .edges_directed(node_index, EdgeDirection::Incoming).filter(|&e| e.weight() == &EdgeKind::Input).count();

            let canvas_count = self.g
                .graph()
                .edges_directed(node_index, EdgeDirection::Incoming).filter(|&e| e.weight() == &EdgeKind::Canvas).count();


            let inputs_failed = match node_def.inbound_edges {
                EdgesIn::NoInput if input_count > 0 || canvas_count > 0 => true,
                EdgesIn::Aribtary { canvases, inputs, .. } if input_count != inputs as usize || canvas_count != canvases as usize => true,
                EdgesIn::OneInput if input_count != 1 && canvas_count != 0 => true,
                EdgesIn::OneInputOneCanvas if input_count != 1 && canvas_count != 1 => true,
                EdgesIn::OneOptionalInput if canvas_count != 0 && (input_count != 0 && input_count != 1) => true,
                _ =>                false
            };
            if inputs_failed {
                let message = format!("Node type {} requires {:?}, but had {} inputs, {} canvases.", node_def.name, node_def.inbound_edges, input_count, canvas_count);
                return Err(FlowError::InvalidConnectionsToNode {
                    value: self.g.node_weight(node_index).unwrap().params.clone(),
                    index: index, message: message
                });
            }
            if !node_def.outbound_edges && outbound_count > 0 {
                let message = format!("Node type {} prohibits child nodes, but had {} outbound edges.", node_def.name, outbound_count);
                return Err(FlowError::InvalidConnectionsToNode {
                    value: self.g.node_weight(node_index).unwrap().params.clone(),
                    index: index, message: message
                });
            }
        }
        Ok(())
    }

    pub fn execute(&mut self) -> Result<()> {
        self.validate_graph()?;
        self.notify_graph_changed()?;

        self.link_codecs(false)?;

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

            if passes >= self.job.max_calc_flatten_execute_passes {
                {
                    self.notify_graph_complete()?;
                }
                panic!("Maximum graph passes exceeded");
                //            error_msg!(c, FlowStatusCode::MaximumGraphPassesExceeded);
                //            return false;
            }
            self.link_codecs(true)?;

            self.populate_dimensions_where_certain()?;
            self.notify_graph_changed()?;

            self.graph_pre_optimize_flatten()?;
            self.notify_graph_changed()?;

            self.link_codecs(true)?;

            self.graph_pre_optimize_flatten()?;
            self.notify_graph_changed()?;

            self.link_codecs(true)?;


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

        if self.job.next_graph_version > 0 && self.job.graph_recording.render_last_graph.unwrap_or(false) {
            self.notify_graph_complete()?;
        }

        Ok(())
    }

    pub fn link_codecs(&mut self, link_only_null_custom_state_nodes: bool) -> Result<()> {
        self.notify_graph_changed()?;

        for index in 0..self.g.node_count() {
            if let Some(func) = self.g
                .node_weight(NodeIndex::new(index))
                .unwrap()
                .def
                .fn_link_state_to_this_io_id {


                let io_id;
                {
                    let mut ctx = self.op_ctx_mut();
                    io_id = func(&mut ctx, NodeIndex::new(index));
                }
                if let Some(io_id) = io_id {
                    let weight = self.g.node_weight(NodeIndex::new(index)).unwrap();
                    // Now, try to send decoder its commands
                    // let ref mut weight = ctx.weight_mut(ix);

                    if let NodeParams::Json(s::Node::Decode { io_id, ref commands }) = weight.params {
                        if let Some(ref list) = *commands {
                            for c in list.iter() {
                                self.job.tell_decoder(io_id, c.to_owned()).unwrap();
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }


    fn assign_stable_ids(&mut self) -> Result<()> {
        // Assign stable IDs;
        for index in 0..self.g.node_count() {
            let mut weight = self.g.node_weight_mut(NodeIndex::new(index)).unwrap();
            if weight.stable_id < 0 {
                weight.stable_id = self.job.next_stable_node_id;
                self.job.next_stable_node_id += 1;
            }
        }
        Ok(())
    }


    fn notify_graph_changed(&mut self) -> Result<()> {
        self.assign_stable_ids()?;

        let info = GraphRecordingInfo {
            debug_job_id: self.job.debug_job_id,
            record_graph_versions: self.job.graph_recording.record_graph_versions.unwrap_or(false),
            current_graph_version: self.job.next_graph_version,
            render_graph_versions: self.job.graph_recording.record_graph_versions.unwrap_or(false),
            maximum_graph_versions: 100,
        };
        let update = notify_graph_changed(self.g, info)?;
        if let Some(GraphRecordingUpdate { next_graph_version }) = update {
            self.job.next_graph_version = next_graph_version;
        }
        Ok(())
    }

    fn notify_graph_complete(&mut self) -> Result<()> {
        let prev_filename =
            format!("job_{}_graph_version_{}.dot",
                    self.job.debug_job_id,
                    self.job.next_graph_version - 1);

        super::visualize::render_dotfile_to_png(&prev_filename);
        Ok(())
    }


    pub fn estimate_node(&mut self, node_id: NodeIndex<u32>) -> FrameEstimate {
        let now = time::precise_time_ns();
        let mut ctx = OpCtxMut{
            c: self.c,
            graph: self.g,
            job: self.job,
        };
        // Invoke estimation
        match ctx.weight(node_id).def.fn_estimate {
            Some(f) => {
                f(&mut ctx, node_id);
                ctx.weight_mut(node_id).cost.wall_ns += (time::precise_time_ns() - now) as u32;
            }
            None => {
                ctx.weight_mut(node_id).frame_est = FrameEstimate::Impossible;
            }
        }
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
                FrameEstimate::Impossible |
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
    fn invoke_estimated_or_non_estimable_nodes<F>(&mut self, f: F) -> Result<()>
    where F: Fn(&NodeDefinition) -> Option<fn(&mut OpCtxMut, NodeIndex<u32>)> {


        // Just find all nodes that offer the given function and whose parents are completed
        // Try to estimate if not already complete
        // TODO: support other values for FrameEstimate
        // TODO: Compare Node value; should differ afterwards
        loop {
            let mut next = None;
            for ix in 0..(self.g.node_count()) {
                let nix = NodeIndex::new(ix);
                if let Some(func) = f(self.g
                    .node_weight(nix)
                    .unwrap()
                    .def){

                    if self.parents_complete(nix) {
                        if let FrameEstimate::Some(_) = self.g
                            .node_weight(nix)
                            .unwrap()
                            .frame_est {} else {
                            //Try estimation one last time if it didn't happen yet
                            let _ = self.estimate_node(nix);
                        }
                        next = Some((nix, func));
                        break;
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


    pub fn graph_pre_optimize_flatten(&mut self) -> Result<()> {
        self.invoke_estimated_or_non_estimable_nodes(|d| d.fn_flatten_pre_optimize )
    }

    pub fn graph_post_optimize_flatten(&mut self) -> Result<()> {
        self.invoke_estimated_or_non_estimable_nodes(|d| d.fn_flatten_post_optimize )
    }

    fn parents_complete(&self, ix: NodeIndex) -> bool{
        self.g
            .parents(ix)
            .iter(self.g)
            .all(|(ex, parent_ix)| {
                self.g.node_weight(parent_ix).unwrap().result != NodeResult::None
            })
    }
    fn parents_estimated(&self, ix: NodeIndex) -> bool{
        self.g
            .parents(ix)
            .iter(self.g)
            .all(|(ex, parent_ix)| {
                if let FrameEstimate::Some(_) = self.g.node_weight(parent_ix).unwrap().frame_est
                    { true } else { false }
            })
    }




    pub fn graph_execute(&mut self) -> Result<()> {
        // Find nodes with fn_execute, which also have been estimated, and whose parents are complete
        // AND who are not already complete
        loop {
            let mut next = None;
            for ix in 0..(self.g.node_count()) {
                if let Some(func) = self.g.node_weight(NodeIndex::new(ix)).unwrap().def.fn_execute {
                    if self.g.node_weight(NodeIndex::new(ix)).unwrap().result ==
                        NodeResult::None && self.parents_complete(NodeIndex::new(ix)) {
                        if let FrameEstimate::Some(_) = self.g
                            .node_weight(NodeIndex::new(ix))
                            .unwrap()
                            .frame_est {
                        }else{
                            //Try estimation one last time if it didn't happen yet
                            let _ = self.estimate_node(NodeIndex::new(ix));
                        }
                        next = Some((NodeIndex::new(ix), func));
                        break;
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
                        unsafe {
                            if self.job.graph_recording.record_frame_images.unwrap_or(false) {
                                if let NodeResult::Frame(ptr) = self.g
                                    .node_weight(next_ix)
                                    .unwrap()
                                    .result {
                                    let path = format!("node_frames/job_{}_node_{}.png",
                                                self.job.debug_job_id,
                                                self.g.node_weight(next_ix).unwrap().stable_id);
                                    let path_copy = path.clone();
                                    let path_cstr = std::ffi::CString::new(path).unwrap();
                                    let _ = std::fs::create_dir("node_frames");
                                    if !::ffi::flow_bitmap_bgra_save_png(self.c.flow_c(),
                                                                         ptr,
                                                                         path_cstr.as_ptr()) {

                                        println!("Failed to save frame {} (from node {})",
                                                 path_copy,
                                                 next_ix.index());
                                        self.c.c_error().unwrap().panic_time();

                                    }
                                }
                            }
                        }
                    }
                }
            }

        }
    }
    fn op_ctx_mut(&mut self) -> OpCtxMut{
        OpCtxMut {
            c: self.c,
            graph: self.g,
            job: self.job,
        }
    }

    fn graph_fully_executed(&self) -> bool {
        for node in self.g.raw_nodes() {
            if node.weight.result == NodeResult::None {
                return false;
            }
        }
        true
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
