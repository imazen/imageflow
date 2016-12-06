use daggy::{Dag, EdgeIndex, NodeIndex};
use ffi::{ImageflowContext, BitmapBgra};
use libc::{int32_t, size_t};
use petgraph::EdgeDirection;
mod rotate_flip_transpose;
mod clone_crop_fill_expand;
mod scale_render;
mod create_canvas;
mod codecs_and_pointer;
mod constrain;

mod internal_prelude {
    pub use ::ffi;
    pub use ffi::{ImageflowContext};
    pub use ffi::BitmapBgra;
    pub use flow::definitions::*;
    pub use ::internal_prelude::works_everywhere::*;
    pub use petgraph::EdgeDirection;
    pub use super::*;
    pub use ::{Context, Job};
}

extern crate imageflow_types as s;
pub use self::clone_crop_fill_expand::CLONE;
pub use self::clone_crop_fill_expand::COPY_RECT;
pub use self::clone_crop_fill_expand::CROP;
pub use self::clone_crop_fill_expand::CROP_MUTATE;
pub use self::clone_crop_fill_expand::EXPAND_CANVAS;
pub use self::clone_crop_fill_expand::FILL_RECT;
pub use self::codecs_and_pointer::BITMAP_BGRA_POINTER;
pub use self::codecs_and_pointer::DECODER;
pub use self::codecs_and_pointer::ENCODE;
pub use self::codecs_and_pointer::PRIMITIVE_DECODER;
pub use self::create_canvas::CREATE_CANVAS;
pub use self::rotate_flip_transpose::APPLY_ORIENTATION;
pub use self::rotate_flip_transpose::FLIP_H;
pub use self::rotate_flip_transpose::FLIP_H_PRIMITIVE;
pub use self::rotate_flip_transpose::FLIP_V;
pub use self::rotate_flip_transpose::FLIP_V_PRIMITIVE;
pub use self::rotate_flip_transpose::NO_OP;
pub use self::rotate_flip_transpose::ROTATE_180;
pub use self::rotate_flip_transpose::ROTATE_270;
pub use self::rotate_flip_transpose::ROTATE_90;
pub use self::rotate_flip_transpose::TRANSPOSE;
pub use self::scale_render::SCALE;
pub use self::scale_render::SCALE_1D;
pub use self::scale_render::SCALE_1D_TO_CANVAS_1D;
pub use self::constrain::CONSTRAIN;
use super::definitions::*;

pub struct NodeDefHelpers {}
impl NodeDefHelpers {
    fn copy_frame_est_from_first_input(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
        ctx.copy_frame_est_from_first_input(ix);
    }
    fn copy_frame_est_from_first_canvas(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
        ctx.copy_frame_est_from_first_canvas(ix);
    }
    fn rotate_frame_info(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
        ctx.rotate_frame_est_from_first_input(ix);
    }
    fn flatten_flip_v(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
        // ctx.graph.node_weight_mut(ix).unwrap()
    }

    fn delete_node_and_snap_together(ctx: &mut OpCtxMut, ix: NodeIndex<u32>) {
        ctx.delete_node_and_snap_together(ix);
    }
}

impl<'c> OpCtxMut<'c> {
    pub fn first_parent_of_kind(&self,
                                    of_node: NodeIndex<u32>,
                                    filter_by_kind: EdgeKind)
                                    -> Option<NodeIndex<u32>> {
        self.graph
            .graph()
            .edges_directed(of_node, EdgeDirection::Incoming)
            .filter(|&(node, &kind)| kind == filter_by_kind)
            .map(|(node, kind)| node)
            .nth(0)
    }

    pub fn flow_c(&self) -> *mut ::ffi::ImageflowContext{
        self.c.flow_c()
    }

    pub fn first_parent_input(&self, of_node: NodeIndex<u32>) -> Option<NodeIndex<u32>> {
        self.first_parent_of_kind(of_node, EdgeKind::Input)
    }
    pub fn first_parent_canvas(&self, of_node: NodeIndex<u32>) -> Option<NodeIndex<u32>> {
        self.first_parent_of_kind(of_node, EdgeKind::Canvas)
    }

    pub fn first_parent_input_weight(&self, of_node: NodeIndex<u32>) -> Option<Node> {
        self.first_parent_input(of_node).map(|ix| self.graph.node_weight(ix).unwrap().clone())
    }


    pub fn first_parent_frame_info_some(&self,
                                            of_node: NodeIndex<u32>)
                                            -> Option<FrameInfo> {
        self.first_parent_input(of_node).and_then(|ix| {
            self.graph.node_weight(ix).and_then(|w| {
                match w.frame_est {
                    FrameEstimate::Some(ref frame_info) => Some(*frame_info),
                    _ => None,
                }
            })
        })
    }

    pub fn get_json_params(&self, ix: NodeIndex<u32>) -> Option<s::Node> {
        self.graph.node_weight(ix).and_then(|w| {
            match w.params {
                NodeParams::Json(ref node) => Some(node.clone()),
                _ => None,
            }
        })
    }

    pub fn first_parent_canvas_weight(&self, of_node: NodeIndex<u32>) -> Option<&Node> {
        self.first_parent_canvas(of_node).map(|ix| self.graph.node_weight(ix).unwrap())
    }

    pub fn first_parent_result_frame(&self,
                                             of_node: NodeIndex<u32>,
                                             kind: EdgeKind)
                                             -> Option<*mut BitmapBgra> {
        self.first_parent_of_kind(of_node, kind)
            .and_then(|ix| self.graph.node_weight(ix))
            .and_then(|w| match w.result {
                NodeResult::Frame(ptr) => Some(ptr),
                _ => None,
            })
    }



    pub fn first_parent_input_weight_mut(&mut self,
                                             of_node: NodeIndex<u32>)
                                             -> Option<&mut Node> {
        self.first_parent_input(of_node).map(move |ix| self.graph.node_weight_mut(ix).unwrap())
    }

    pub fn has_other_children(&self,
                                  of_node: NodeIndex<u32>,
                                  except_child: NodeIndex<u32>)
                                  -> bool {
        self.graph
            .graph()
            .neighbors_directed(of_node, EdgeDirection::Outgoing)
            .any(|n| n != except_child)
    }

    pub fn weight(&self, ix: NodeIndex<u32>) -> &Node {
        self.graph.node_weight(ix).unwrap()
    }

    pub fn weight_mut(&mut self, node_to_update: NodeIndex<u32>) -> &mut Node {
        self.graph.node_weight_mut(node_to_update).unwrap()
    }
    pub fn copy_frame_est_from_first_input(&mut self, node_to_update: NodeIndex<u32>) {
        if let Some(input_ix) = self.first_parent_input(node_to_update)
        {
            if self.graph.node_weight(input_ix).unwrap().frame_est == FrameEstimate::None {
                panic!("Parent frame {} is not estimated", input_ix.index());
            }
            self.graph.node_weight_mut(node_to_update).unwrap().frame_est =
                self.graph.node_weight(input_ix).unwrap().frame_est;
        }
    }
    pub fn copy_frame_est_from_first_canvas(&mut self, node_to_update: NodeIndex<u32>) {
        if let  Some(input_ix) = self.first_parent_canvas(node_to_update) {
            self.graph.node_weight_mut(node_to_update).unwrap().frame_est =
                self.graph.node_weight(input_ix).unwrap().frame_est;
        }
    }
    pub fn assert_ok(&self) {
        self.panic_time()
    }


    pub fn panic_time(&self) {
        if let Some(e) = self.c.c_error(){
            e.panic_time();
        }
    }

    pub fn rotate_frame_est_from_first_input(&mut self,
                                                     node_to_update: NodeIndex<u32>) {
        // TODO: select by EdgeKind=Input
        let input = self.graph
            .graph()
            .neighbors_directed(node_to_update, EdgeDirection::Incoming)
            .nth(0);
        if let Some(input_ix) = input {
            let input_est = self.graph.node_weight(input_ix).unwrap().frame_est;
            let mut w = self.graph.node_weight_mut(node_to_update).unwrap();
            w.frame_est = match input_est {
                FrameEstimate::Some(info) => {
                    FrameEstimate::Some(FrameInfo {
                        w: info.h,
                        h: info.w,
                        ..info
                    })
                }
                FrameEstimate::UpperBound(info) => {
                    FrameEstimate::UpperBound(FrameInfo {
                        w: info.h,
                        h: info.w,
                        ..info
                    })
                }
                other => other,
            };
        }
    }

    pub fn copy_edges_to(&mut self,
                             from_node: NodeIndex<u32>,
                             to_node: NodeIndex<u32>,
                             direction: EdgeDirection) {
        let edges = self.graph
            .graph()
            .edges_directed(from_node, direction)
            .map(|(a, b)| (a, *b))
            .collect::<Vec<_>>();

        for (other_node, weight) in edges {
            match direction {
                EdgeDirection::Incoming => {
                    self.graph.add_edge(other_node, to_node, weight).unwrap()
                }
                EdgeDirection::Outgoing => {
                    self.graph.add_edge(to_node, other_node, weight).unwrap()
                }
            };
        }
    }
    pub fn delete_child_edges_for(&mut self, from_node: NodeIndex<u32>) {
        loop {
            if self.graph
                .raw_edges()
                .iter()
                .position(|e| e.source() == from_node)
                .and_then(|ix| self.graph.remove_edge(EdgeIndex::new(ix)))
                .is_none(){
                break;
            }
        }
    }

    pub fn delete_node_and_snap_together(&mut self, node_to_delete: NodeIndex<u32>) {
        // Prefer EdgeKind=Input
        let input = self.graph
            .graph()
            .neighbors_directed(node_to_delete, EdgeDirection::Incoming)
            .nth(0);
        match input {
            None => {}
            Some(from_node) => {
                self.copy_edges_to(node_to_delete, from_node, EdgeDirection::Outgoing);
                self.graph.remove_node(node_to_delete).unwrap();
            }
        };
    }

    // Links nodes with Input edges
    pub fn replace_node(&mut self, index: NodeIndex<u32>, with_list: Vec<Node>) {
        let mut with = with_list.clone();
        match with.len() {
            0 => self.delete_node_and_snap_together(index),
            n => {
                with.reverse();
                let mut last_ix = self.graph.add_node(with.pop().unwrap());
                self.copy_edges_to(index, last_ix, EdgeDirection::Incoming);
                while !with.is_empty() {
                    let new_ix = self.graph.add_node(with.pop().unwrap());
                    self.graph.add_edge(last_ix, new_ix, EdgeKind::Input).unwrap();
                    last_ix = new_ix;
                }
                self.copy_edges_to(index, last_ix, EdgeDirection::Outgoing);
                self.graph.remove_node(index).unwrap();
            }
        }
    }

    pub fn replace_node_with_existing(&mut self,
                                          index: NodeIndex<u32>,
                                          with_index: NodeIndex<u32>) {
        self.copy_edges_to(index, with_index, EdgeDirection::Incoming);
        self.copy_edges_to(index, with_index, EdgeDirection::Outgoing);
        self.graph.remove_node(index).unwrap();
    }
}
