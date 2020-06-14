use crate::Context;
use crate::flow::definitions::*;
use crate::internal_prelude::works_everywhere::*;
use petgraph::dot::Dot;
use std::process::Command;
use super::visualize::{notify_graph_changed, GraphRecordingUpdate, GraphRecordingInfo};
use petgraph::EdgeDirection;
use rustc_serialize::base64;
use rustc_serialize::base64::ToBase64;
use imageflow_helpers::timeywimey::precise_time_ns;

pub struct Engine<'a> {
    c: &'a Context,
    job: &'a mut Context,
    g: Graph,
    more_frames: bool,
}

impl<'a> Engine<'a> {

    pub fn create(context: &'a mut Context, g: Graph) -> Engine<'a> {
        let split_context_2 = unsafe{ &mut *(context as *mut Context) };

        Engine {
            c: context,
            job: split_context_2,
            g,
            more_frames: false,
        }
    }

    pub fn ctx(&self) -> OpCtx{
        OpCtx{
            c: self.c,
            graph: &self.g,
            job: self.job,
        }
    }

    fn flow_c(&self) -> *mut crate::ffi::ImageflowContext{
        self.c.flow_c()
    }

    pub fn validate_graph(&self) -> Result<()> {
        for node_index in (0..self.g.node_count()).map(NodeIndex::new) {
            let n = self.g.node_weight(node_index).unwrap();

            let (req_edges_in, req_edges_out) = n.def.edges_required(&n.params).unwrap();

            let outbound_count = self.g
                .graph()
                .edges_directed(node_index, EdgeDirection::Outgoing).count();

            let input_count = self.g
                .graph()
                .edges_directed(node_index, EdgeDirection::Incoming).filter(|&e| e.weight() == &EdgeKind::Input).count();

            let canvas_count = self.g
                .graph()
                .edges_directed(node_index, EdgeDirection::Incoming).filter(|&e| e.weight() == &EdgeKind::Canvas).count();


            let inputs_failed = match req_edges_in {
                EdgesIn::NoInput if input_count > 0 || canvas_count > 0 => true,
                EdgesIn::Arbitrary { canvases, inputs, .. } if input_count != inputs as usize || canvas_count != canvases as usize => true,
                EdgesIn::OneInput if input_count != 1 && canvas_count != 0 => true,
                EdgesIn::OneInputOneCanvas if input_count != 1 && canvas_count != 1 => true,
                EdgesIn::OneOptionalInput if canvas_count != 0 && (input_count != 0 && input_count != 1) => true,
                _ =>                false
            };

            let result = if let Err(e) = n.def.validate_params(&n.params) {
                Err(e)
            } else if inputs_failed {
                Err(nerror!(crate::ErrorKind::InvalidNodeConnections, "Node type {} requires {:?}, but had {} inputs, {} canvases.", n.def.name(), req_edges_in, input_count, canvas_count))
            } else if req_edges_out != EdgesOut::Any && outbound_count > 0 {
                Err(nerror!(crate::ErrorKind::InvalidNodeConnections, "Node type {} prohibits child nodes, but had {} outbound edges.", n.def.name(), outbound_count))
            } else{
                Ok(())
            };
            if let Err(e) = result{
                return Err(e.with_ctx(&self.ctx(),node_index))
            }
        }
        Ok(())
    }

    pub fn execute_many(&mut self) -> Result<s::BuildPerformance> {
        let graph_copy = self.g.clone();
        let mut vec = Vec::with_capacity(1);
        loop {
            let (more, p) = self.execute()?;
            vec.push(p);
            if !more {
                return Ok(s::BuildPerformance{ frames: vec });
            } else {
                //TODO: free unused bitmaps from self.g
                self.g = graph_copy.clone();
            }
        }
    }


    /// This function is a legacy mess. The work duplication isn't terrible, but a cleaner
    /// implementation is definitely possible.
    ///
    /// We also don't yet implement graph-comprehensive optimizations, like reducing a no-op
    /// to copying encoded bytes.
    fn execute(&mut self) -> Result<(bool, s::FramePerformance)> {

        let start = precise_time_ns();
        self.more_frames = false;
        self.validate_graph()?;
        self.notify_graph_changed()?;

        self.request_decoder_commands()?;

        let mut passes = 0;
        loop {
            if self.graph_fully_executed() {
                break;
            }

            if passes >= self.job.max_calc_flatten_execute_passes {
                {
                    self.notify_graph_complete()?;
                }
                eprintln!("{:#?}", self.g);
                panic!("Maximum graph passes exceeded");
                //            error_msg!(c, FlowStatusCode::MaximumGraphPassesExceeded);
                //            return false;
            }
            self.request_decoder_commands()?;

            self.populate_dimensions_where_certain()?;
            self.notify_graph_changed()?;

            self.graph_pre_optimize_flatten()?;
            self.notify_graph_changed()?;

            self.request_decoder_commands()?;

            self.graph_pre_optimize_flatten()?;
            self.notify_graph_changed()?;

            self.request_decoder_commands()?;


            self.populate_dimensions_where_certain()?;
            self.notify_graph_changed()?;

            // graph_optimize()?;
            self.notify_graph_changed()?;

            self.populate_dimensions_where_certain()?;
            self.notify_graph_changed()?;

            self.populate_dimensions_where_certain()?;
            self.notify_graph_changed()?;

            self.validate_graph()?;

            self.graph_execute()?;
            passes += 1;

            self.notify_graph_changed()?;
        }

        self.notify_graph_complete()?;



        let mut perf : Vec<s::NodePerf> = self.g.node_weights_mut().map(|n| s::NodePerf{ wall_microseconds: ( n.cost.wall_ns as f64 / 1000f64).round() as u64, name: n.def.name().to_owned()}).collect();
        perf.sort_by_key(|p| p.wall_microseconds);
        perf.reverse();


        let total_node_ns = self.g.node_weights_mut().map(|n|  n.cost.wall_ns).sum::<u64>();
        let total_ns = precise_time_ns() - start;
        let wall_microseconds = (total_ns as f64 / 1000f64).round() as u64;
        let overhead_microseconds = ((total_ns as i64 - total_node_ns as i64) as f64 / 1000f64).round() as i64;

        Ok((self.more_frames, s::FramePerformance{nodes: perf, wall_microseconds, overhead_microseconds}))
    }


    pub fn request_decoder_commands(&mut self) -> Result<()> {
        self.notify_graph_changed()?;

        for index in 0..self.g.node_count() {

            let result = {
                let n = self.g
                    .node_weight(NodeIndex::new(index))
                    .unwrap();
                n.def.tell_decoder(&n.params)
            };

            let result_value = {
                let ctx = self.op_ctx_mut();
                result.map_err( |e| e.at(here!()).with_ctx_mut(&ctx,NodeIndex::new(index)))?
            };

            if let Some((io_id, commands)) = result_value {
                for c in commands {
                    self.job.tell_decoder(io_id, c.to_owned()).unwrap();
                }
            }
        }

        Ok(())
    }

    pub fn invalidate_all_graph_estimates(&mut self) -> Result<()>{

        for index in 0..self.g.node_count() {
            self.g.node_weight_mut(NodeIndex::new(index))
                    .unwrap().frame_est = FrameEstimate::None;
        }
        Ok(())
    }


    fn assign_stable_ids(&mut self) -> Result<()> {
        // Assign stable IDs;
        for index in 0..self.g.node_count() {
            let weight = self.g.node_weight_mut(NodeIndex::new(index)).unwrap();
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
        let update = notify_graph_changed(&mut self.g, &info)?;
        if let Some(GraphRecordingUpdate { next_graph_version }) = update {
            self.job.next_graph_version = next_graph_version;
        }
        Ok(())
    }

    fn notify_graph_complete(&mut self) -> Result<()> {
        if self.job.next_graph_version > 0 && self.job.graph_recording.record_graph_versions.unwrap_or(false) {
            let prev_filename =
                format!("job_{}_graph_version_{}.dot",
                        self.job.debug_job_id,
                        self.job.next_graph_version - 1);

            super::visualize::render_dotfile_to_png(&prev_filename);
        }
        Ok(())
    }


    pub fn estimate_node(&mut self, node_id: NodeIndex) -> Result<FrameEstimate> {
        let now = precise_time_ns();
        let mut ctx = self.op_ctx_mut();

        // Invoke estimation
        // If not implemented, estimation is impossible
        let result = match ctx.weight(node_id).def.estimate(&mut ctx, node_id){
            Err(FlowError {kind: ErrorKind::MethodNotImplemented, ..}) => {
                Ok(FrameEstimate::Impossible)
            }
            other => other
        }.map_err( |e| e.at(here!()).with_ctx_mut(&ctx,node_id));

        if let Ok(v) = result {
            ctx.weight_mut(node_id).frame_est = v;
        }

        ctx.weight_mut(node_id).cost.wall_ns += precise_time_ns() - now;
        result
    }

    pub fn estimate_node_recursive(&mut self, node_id: NodeIndex, recurse_limit: i32) -> Result<FrameEstimate> {
        if recurse_limit < 0 {
            panic!("Hit node estimation recursion limit");
        }

        // If we're already done, no need
        if let FrameEstimate::Some(info) = self.g.node_weight(node_id).unwrap().frame_est {
            return Ok(FrameEstimate::Some(info));
        }

        // Otherwise let's try again
        let inputs_good = inputs_estimated(&self.g, node_id);
        if !inputs_good {
            // TODO: support UpperBound eventually; for now, use Impossible until all nodes implement
            let give_up = inputs_estimates(&self.g, node_id).iter().any(|est| match *est {
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
                    .iter(&self.g)
                    .map(|(edge_ix, ix)| ix)
                    .collect::<Vec<NodeIndex>>();

                // println!("Estimating recursively {:?}", input_indexes);
                for ix in input_indexes {

                    let _ = self.estimate_node_recursive(ix, recurse_limit -1)?;
                }
            }

            if give_up || !inputs_estimated(&self.g, node_id) {
                self.g.node_weight_mut(node_id).unwrap().frame_est = FrameEstimate::Impossible;
                return Ok(FrameEstimate::Impossible);
            }
        }
        // Should be good on inputs here
        match self.estimate_node(node_id)  {
            Ok(FrameEstimate::None) => {
                panic!("Node estimation misbehaved on {}. Cannot leave FrameEstimate::None, must chose an alternative",
                       self.g.node_weight(node_id).unwrap().def.name());
            },
            Ok(FrameEstimate::InvalidateGraph) => {
                self.invalidate_all_graph_estimates()?;

                // Restore this one nodes' estimate
                self.g.node_weight_mut(node_id).unwrap().frame_est = FrameEstimate::InvalidateGraph;

                self.estimate_node_recursive(node_id, recurse_limit -1)
            },
            other => other
        }
    }

    pub fn populate_dimensions_where_certain(&mut self) -> Result<()> {

        for ix in 0..self.g.node_count() {
            // If any node returns FrameEstimate::Impossible, we might as well move on to execution pass.
            let _ = self.estimate_node_recursive(NodeIndex::new(ix), 100)?;
        }

        Ok(())
    }

    // invoke_estimated_or_non_estimable_nodes
    fn graph_pre_optimize_flatten(&mut self) -> Result<()> {


        // Just find all nodes that offer the given function and whose parents are completed
        // Try to estimate if not already complete
        // TODO: support other values for FrameEstimate
        // TODO: Compare Node value; should differ afterwards
        loop {
            let mut next = None;
            for ix in 0..(self.g.node_count()) {
                let nix = NodeIndex::new(ix);
                let def = self.g
                    .node_weight(nix)
                    .unwrap()
                    .def;
                if def.can_expand() && self.parents_complete(nix) {
                    if let FrameEstimate::Some(_) = self.g
                        .node_weight(nix)
                        .unwrap()
                        .frame_est {} else {
                        //Try estimation one last time if it didn't happen yet
                        let _ = self.estimate_node_recursive(nix, 100).map_err(|e| e.at(here!()))?;
                    }
                    next = Some((nix, def));
                    break;
                }
            }
            match next {
                None => return Ok(()),
                Some((next_ix, def)) => {
                    let more_frames = {
                        let mut ctx = self.op_ctx_mut();
                        def.expand(&mut ctx, next_ix).map_err(|e| e.with_ctx_mut(&ctx, next_ix).at(here!()))?;
                        ctx.more_frames.get()
                    };
                    self.more_frames = self.more_frames || more_frames;
                }
            }

        }
    }



    fn parents_complete(&self, ix: NodeIndex) -> bool{
        self.g
            .parents(ix)
            .iter(&self.g)
            .all(|(ex, parent_ix)| {
                self.g.node_weight(parent_ix).unwrap().result != NodeResult::None
            })
    }
    fn parents_estimated(&self, ix: NodeIndex) -> bool{
        self.g
            .parents(ix)
            .iter(&self.g)
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
                let index = NodeIndex::new(ix);
                let def = self.g.node_weight(index).unwrap().def;
                if def.can_execute() {
                    if self.g.node_weight(index).unwrap().result ==
                        NodeResult::None && self.parents_complete(index) {

                        if  self.g
                            .node_weight(index)
                            .unwrap()
                            .frame_est.is_none(){

                            let _ = self.estimate_node_recursive(index,100).map_err(|e| e.at(here!()))?;
                        }
                        next = Some((index, def));
                        break;
                    }

                } else if !def.can_expand(){
                    return Err(nerror!(crate::ErrorKind::MethodNotImplemented, "Nodes must can_execute() or can_expand(). {:?} does neither", def).into());
                }
            }
            match next {
                None => return Ok(()),
                Some((next_ix, def)) => {
                    let more_frames = {
                        let now = precise_time_ns();
                        let mut ctx = self.op_ctx_mut();
                        let result = def.execute(&mut ctx, next_ix).map_err(|e| e.with_ctx_mut(&ctx, next_ix).at(here!()))?;

                        if result == NodeResult::None {
                            return Err(nerror!(crate::ErrorKind::InvalidOperation, "Node {} execution returned {:?}", def.name(), result).into());
                        }else{
                            // Force update the estimate to match reality
                            if let NodeResult::Frame(bit) = result{
                                if !bit.is_null() {
                                    unsafe {
                                        ctx.weight_mut(next_ix).frame_est = FrameEstimate::Some((*bit).frame_info());
                                    }
                                }
                            }
                            ctx.weight_mut(next_ix).result = result;
                        }
                        ctx.weight_mut(next_ix).cost.wall_ns += precise_time_ns() - now;
                        ctx.more_frames.get()
                    };

                    self.more_frames = self.more_frames || more_frames;

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
                                if !crate::ffi::flow_bitmap_bgra_save_png(self.c.flow_c(),
                                                                     ptr,
                                                                     path_cstr.as_ptr()) {
                                    println!("Failed to save frame {} (from node {})",
                                             path_copy,
                                             next_ix.index());
                                    cerror!(self.c).panic();
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
            graph: &mut self.g,
            job: self.job,
            more_frames: Cell::new(false),
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

    pub fn last_graph(&self) -> &Graph{
        &self.g
    }


    pub fn collect_encode_results(&self) -> Vec<s::EncodeResult>{
        let mut encodes = Vec::new();
        for node in self.g.raw_nodes() {
            if let crate::flow::definitions::NodeResult::Encoded(ref r) = node.weight.result {
                encodes.push((*r).clone());
            }
        }
        encodes
    }
    pub fn collect_augmented_encode_results(&self, io: &[s::IoObject]) -> Vec<s::EncodeResult>{
        self.collect_encode_results().into_iter().map(|r: s::EncodeResult|{
            if r.bytes == s::ResultBytes::Elsewhere {
                let obj: &s::IoObject = io.iter().find(|obj| obj.io_id == r.io_id).unwrap();//There's gotta be one
                let bytes = match obj.io {
                    s::IoEnum::Filename(ref str) => s::ResultBytes::PhysicalFile(str.to_owned()),
                    s::IoEnum::OutputBase64 => {
                        let slice = self.c.get_output_buffer_slice(r.io_id).map_err(|e| e.at(here!())).unwrap();
                        s::ResultBytes::Base64(slice.to_base64(base64::Config{char_set: base64::CharacterSet::Standard, line_length: None, newline: base64::Newline::LF, pad: true}))
                    },
                    _ => s::ResultBytes::Elsewhere
                };
                s::EncodeResult{
                    bytes,
                    .. r
                }
            }else{
                r
            }

        }).collect::<Vec<s::EncodeResult>>()
    }
}
impl<'a> OpCtxMut<'a> {
    pub fn graph_to_str(&mut self) -> Result<String> {
        let mut vec = Vec::new();
        super::visualize::print_graph(&mut vec, self.graph, None).unwrap();
        Ok(String::from_utf8(vec).unwrap())
    }
}



use daggy::walker::Walker;



pub fn flow_node_has_dimensions(g: &Graph, node_id: NodeIndex) -> bool {
    g.node_weight(node_id)
        .map(|node| match node.frame_est {
            FrameEstimate::Some(_) => true,
            _ => false,
        })
        .unwrap_or(false)
}

pub fn inputs_estimated(g: &Graph, node_id: NodeIndex) -> bool {
    inputs_estimates(g, node_id).iter().all(|est| match *est {
        FrameEstimate::Some(_) => true,
        _ => false,
    })
}

// -> impl Iterator<Item = FrameEstimate> caused compiler panic

pub fn inputs_estimates(g: &Graph, node_id: NodeIndex) -> Vec<FrameEstimate> {
    g.parents(node_id)
        .iter(g)
        .filter_map(|(_, node_index)| g.node_weight(node_index).map(|w| w.frame_est))
        .collect()
}
