use daggy::{Dag, EdgeIndex, NodeIndex, Walker};
use ffi::{ImageflowContext, BitmapBgra};
use libc::{int32_t, size_t};
use petgraph::EdgeDirection;
use petgraph::visit::EdgeRef;
use std::error::Error;
use std::fmt;

mod rotate_flip_transpose;
mod clone_crop_fill_expand;
mod scale_render;
mod create_canvas;
mod codecs_and_pointer;
mod constrain;
mod white_balance;
mod color;

mod internal_prelude {
    pub use ::ffi;
    pub use ffi::{ImageflowContext};
    pub use ffi::BitmapBgra;
    pub use flow::definitions::*;
    pub use ::internal_prelude::works_everywhere::*;
    pub use petgraph::EdgeDirection;
    pub use super::*;
    #[macro_use]
    pub use super::super::*;
    pub use ::{Context, Job, Result, FlowError};
}
use ::{Context, Job, Result, FlowError};
extern crate imageflow_types as s;
pub use self::clone_crop_fill_expand::CLONE;
pub use self::clone_crop_fill_expand::COPY_RECT;
pub use self::clone_crop_fill_expand::CROP;
pub use self::clone_crop_fill_expand::CROP_WHITESPACE;
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
pub use self::constrain::COMMAND_STRING;
pub use self::white_balance::WHITE_BALANCE_SRGB_MUTATE;
pub use self::white_balance::WHITE_BALANCE_SRGB;
pub use self::color::COLOR_MATRIX_SRGB_MUTATE;
pub use self::color::COLOR_MATRIX_SRGB;
pub use self::color::COLOR_FILTER_SRGB;

#[macro_use]
use super::definitions::*;

#[test]
fn test_err() {

    let e = nerror!(::ErrorKind::BitmapPointerNull);
    assert_eq!(e.kind, ::ErrorKind::BitmapPointerNull);
    assert!(format!("{}",&e).starts_with("InternalError: BitmapPointerNull at"));
    let e = nerror!(::ErrorKind::BitmapPointerNull, "hi");
    assert!(format!("{}",&e).starts_with("InternalError: BitmapPointerNull: hi at"));
    let e = nerror!(::ErrorKind::BitmapPointerNull, "hi {}", 1);
    assert!(format!("{}",&e).starts_with("InternalError: BitmapPointerNull: hi 1 at"));
}

pub struct NodeDefHelpers {}
impl NodeDefHelpers {
    fn copy_frame_est_from_first_input(ctx: &mut OpCtxMut, ix: NodeIndex) {
        ctx.copy_frame_est_from_first_input(ix);
    }
    fn copy_frame_est_from_first_canvas(ctx: &mut OpCtxMut, ix: NodeIndex) {
        ctx.copy_frame_est_from_first_canvas(ix);
    }
    fn rotate_frame_info(ctx: &mut OpCtxMut, ix: NodeIndex) {
        ctx.rotate_frame_est_from_first_input(ix);
    }

    fn delete_node_and_snap_together(ctx: &mut OpCtxMut, ix: NodeIndex) {
        ctx.delete_node_and_snap_together(ix);
    }
}

impl<'c> OpCtxMut<'c> {
    pub fn first_parent_of_kind(&self,
                                    of_node: NodeIndex,
                                    filter_by_kind: EdgeKind)
                                    -> Option<NodeIndex> {
        self.graph
            .graph()
            .edges_directed(of_node, EdgeDirection::Incoming)
            .filter(|&e| e.weight() == &filter_by_kind)
            .map(|e| e.source())
            .nth(0)
    }

    pub fn first_parent_of_kind_required(&self,
                                of_node: NodeIndex,
                                filter_by_kind: EdgeKind)
                                -> Result<NodeIndex> {
        if let Some(ix) = self.first_parent_of_kind(of_node, filter_by_kind){
            Ok(ix)
        }else {
            Err(nerror!(::ErrorKind::InvalidOperation, "Parent {:?} node not found", filter_by_kind).with_ctx_mut(self, of_node))
        }
    }

    pub fn flow_c(&self) -> *mut ::ffi::ImageflowContext{
        self.c.flow_c()
    }

    pub fn first_parent_input(&self, of_node: NodeIndex) -> Option<NodeIndex> {
        self.first_parent_of_kind(of_node, EdgeKind::Input)
    }
    pub fn first_parent_canvas(&self, of_node: NodeIndex) -> Option<NodeIndex> {
        self.first_parent_of_kind(of_node, EdgeKind::Canvas)
    }

    pub fn first_parent_input_weight(&self, of_node: NodeIndex) -> Option<Node> {
        self.first_parent_input(of_node).map(|ix| self.graph.node_weight(ix).unwrap().clone())
    }


    pub fn first_parent_frame_info_some(&self,
                                            of_node: NodeIndex)
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

    pub fn get_json_params(&self, ix: NodeIndex) -> Option<s::Node> {
        self.graph.node_weight(ix).and_then(|w| {
            match w.params {
                NodeParams::Json(ref node) => Some(node.clone()),
                _ => None,
            }
        })
    }

    pub fn first_parent_canvas_weight(&self, of_node: NodeIndex) -> Option<&Node> {
        self.first_parent_canvas(of_node).map(|ix| self.graph.node_weight(ix).unwrap())
    }

    pub fn first_parent_result_frame(&self,
                                             of_node: NodeIndex,
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
                                             of_node: NodeIndex)
                                             -> Option<&mut Node> {
        self.first_parent_input(of_node).map(move |ix| self.graph.node_weight_mut(ix).unwrap())
    }

    pub fn has_other_children(&self,
                                  of_node: NodeIndex,
                                  except_child: NodeIndex)
                                  -> bool {
        self.graph
            .graph()
            .neighbors_directed(of_node, EdgeDirection::Outgoing)
            .any(|n| n != except_child)
    }

    pub fn weight(&self, ix: NodeIndex) -> &Node {
        self.graph.node_weight(ix).unwrap()
    }

    pub fn weight_mut(&mut self, node_to_update: NodeIndex) -> &mut Node {
        self.graph.node_weight_mut(node_to_update).unwrap()
    }
    pub fn copy_frame_est_from_first_input(&mut self, node_to_update: NodeIndex) {
        self.graph.node_weight_mut(node_to_update).unwrap().frame_est = self.frame_est_from(node_to_update, EdgeKind::Input).unwrap();
    }


    pub fn frame_info_from(&self, ix: NodeIndex, filter_by_kind: EdgeKind) -> Result<FrameInfo> {
        let parent = self.first_parent_of_kind_required(ix, filter_by_kind)?;
        let est = self.graph.node_weight(parent).expect(loc!("first_parent_of_kind_required provided invalid node index")).frame_est;
        if let  FrameEstimate::Some(info) = est{
            Ok(info)
        } else {
            Err(nerror!(::ErrorKind::InvalidOperation, "Parent {:?} node lacks FrameEstimate::Some (required for expand/execute). Value is {:?}", filter_by_kind, est).with_ctx_mut(self, ix))
        }
    }
    pub fn frame_est_from(&self, ix: NodeIndex, filter_by_kind: EdgeKind) -> Result<FrameEstimate> {
        let parent = self.first_parent_of_kind_required(ix, filter_by_kind)?;

        let est = self.graph.node_weight(parent).expect(loc!("first_parent_of_kind_required provided invalid node index")).frame_est;
        if est == FrameEstimate::None {
            Err(nerror!(::ErrorKind::InvalidOperation, "Parent {:?} node lacks FrameEstimate. Value is {:?}", filter_by_kind, est).with_ctx_mut(self, ix))
        } else {
            Ok(est)
        }
    }


    pub fn bitmap_bgra_from(&mut self, ix: NodeIndex, filter_by_kind: EdgeKind) -> Result<*mut BitmapBgra> {
        let parent = self.first_parent_of_kind_required(ix, filter_by_kind)?;

        let result = &self.graph.node_weight(parent).expect(loc!("first_parent_of_kind_required provided invalid node index")).result;
        if let &NodeResult::Frame(bitmap) = result {
            if bitmap.is_null() {
                Err(nerror!(::ErrorKind::BitmapPointerNull, "Parent {:?} node has NodeResult::Frame(null)", filter_by_kind).with_ctx_mut(self, ix))
            } else {
                Ok(bitmap)
            }
        }else{
            Err(nerror!(::ErrorKind::InvalidOperation, "Parent {:?} node lacks NodeResult::Frame(bitmap). Value is {:?}", filter_by_kind, result).with_ctx_mut(self, ix))
        }
    }
    pub fn consume_parent_result(&mut self, ix: NodeIndex, filter_by_kind: EdgeKind) -> Result<()> {
        let parent = self.first_parent_of_kind_required(ix, filter_by_kind)?;

        let result = {
            let weight = self.graph.node_weight(parent).expect(loc!("first_parent_of_kind_required provided invalid node index"));
            if let NodeResult::Frame(bitmap) = weight.result {
                Ok(())
            } else if let NodeResult::Consumed = weight.result {
                Err(nerror!(::ErrorKind::InvalidOperation, "Parent {:?} node's result has already been consumed", filter_by_kind).with_ctx_mut(self, ix))
            } else {
                Err(nerror!(::ErrorKind::InvalidOperation, "Parent {:?} node's result cannot be consumed. Value is {:?}", filter_by_kind, weight.result).with_ctx_mut(self, ix))
            }
        };
        if result.is_ok(){
            self.graph.node_weight_mut(parent).expect(loc!()).result = NodeResult::Consumed;
        }
        result
    }

    pub fn copy_frame_est_from_first_canvas(&mut self, node_to_update: NodeIndex) {
        if let  Some(input_ix) = self.first_parent_canvas(node_to_update) {
            self.graph.node_weight_mut(node_to_update).unwrap().frame_est =
                self.graph.node_weight(input_ix).unwrap().frame_est;
        }
    }
    pub fn frame_est_from_first_canvas(&mut self, node_to_update: NodeIndex) -> Option<FrameEstimate>{
        self.first_parent_canvas(node_to_update).map(|ix| self.graph.node_weight(ix).unwrap().frame_est)
    }

    pub fn assert_ok(&self) {
        if self.c.c_error().has_error(){
            cerror!(self.c).panic();
        }
    }



    pub fn rotate_frame_est_from_first_input(&mut self,
                                                     node_to_update: NodeIndex) {
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
                             from_node: NodeIndex,
                             to_node: NodeIndex,
                             direction: EdgeDirection) {
        let edges = self.graph
            .graph()
            .edges_directed(from_node, direction)
            .map(|e| {
                match direction {
                    EdgeDirection::Incoming => {
                        (e.source(), to_node, *e.weight())
                    }
                    EdgeDirection::Outgoing => {
                        (to_node, e.target(), *e.weight())
                    }
                }
            })
            .collect::<Vec<_>>();

        for (a,b, weight) in edges {
            let _ = self.graph.add_edge(a, b, weight).unwrap();
        }
    }
    pub fn delete_child_edges_for(&mut self, from_node: NodeIndex) {
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

    pub fn delete_node_and_snap_together(&mut self, node_to_delete: NodeIndex) {
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
    pub fn replace_node(&mut self, index: NodeIndex, with_list: Vec<Node>) {
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
                                          index: NodeIndex,
                                          with_index: NodeIndex) {
        self.copy_edges_to(index, with_index, EdgeDirection::Incoming);
        self.copy_edges_to(index, with_index, EdgeDirection::Outgoing);
        self.graph.remove_node(index).unwrap();
    }

    pub fn get_decoder_io_ids_and_indexes(&self,
                              ancestors_of_node: NodeIndex)
                              -> Vec<(i32,NodeIndex)> {
        self.graph.parents(ancestors_of_node).iter(self.graph).map(|(_, ix)| match self.weight(ix).params{
            NodeParams::Json(s::Node::Decode { io_id, ..}) => Some((io_id,ix)), _ => None
        } ).filter(|v| v.is_some()).map(|v| v.unwrap()).collect::<>()
    }
    pub fn get_decoder_io_ids(&self,
                                          ancestors_of_node: NodeIndex)
                                          -> Vec<i32> {
        self.get_decoder_io_ids_and_indexes(ancestors_of_node).into_iter().map(|(a,b)| a).collect::<>()
    }
    pub fn get_image_info_list(&mut self,
                            ancestors_of_node: NodeIndex) -> Vec<Result<s::ImageInfo>>{
        self.get_decoder_io_ids(ancestors_of_node).into_iter().map(|io_id| self.job.get_image_info(io_id)).collect::<>()
    }

}
