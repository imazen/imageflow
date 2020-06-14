use daggy::{Dag, EdgeIndex, NodeIndex, Walker};
use crate::ffi::{ImageflowContext, BitmapBgra};
use libc::size_t;
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
mod watermark;
mod enable_transparency;
//mod detection;

mod internal_prelude {
    pub use crate::ffi;
    pub use crate::ffi::{ImageflowContext};
    pub use crate::ffi::BitmapBgra;
    pub use crate::flow::definitions::*;
    pub use crate::internal_prelude::works_everywhere::*;
    pub use petgraph::EdgeDirection;
    pub use super::*;
    pub use super::super::*;
    pub use crate::{Context, Result, FlowError};
}
use crate::{Context, Result, FlowError};
extern crate imageflow_types as s;
pub use self::clone_crop_fill_expand::CLONE;
pub use self::clone_crop_fill_expand::COPY_RECT;
pub use self::clone_crop_fill_expand::CROP;
pub use self::clone_crop_fill_expand::CROP_WHITESPACE;
pub use self::clone_crop_fill_expand::CROP_MUTATE;
pub use self::clone_crop_fill_expand::EXPAND_CANVAS;
pub use self::clone_crop_fill_expand::REGION_PERCENT;
pub use self::clone_crop_fill_expand::REGION;
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
pub use self::scale_render::DRAW_IMAGE_EXACT;
//pub use self::scale_render::SCALE_1D;
//pub use self::scale_render::SCALE_1D_TO_CANVAS_1D;
pub use self::constrain::CONSTRAIN;
pub use self::constrain::COMMAND_STRING;
pub use self::white_balance::WHITE_BALANCE_SRGB_MUTATE;
pub use self::white_balance::WHITE_BALANCE_SRGB;
pub use self::color::COLOR_MATRIX_SRGB_MUTATE;
pub use self::color::COLOR_MATRIX_SRGB;
pub use self::color::COLOR_FILTER_SRGB;
pub use self::watermark::WATERMARK;
pub use self::enable_transparency::ENABLE_TRANSPARENCY;

//pub use self::detection::CROP_FACES;

use super::definitions::*;

#[test]
fn test_err() {

    let e = nerror!(crate::ErrorKind::BitmapPointerNull);
    assert_eq!(e.kind, crate::ErrorKind::BitmapPointerNull);
    assert!(format!("{}",&e).starts_with("InternalError: BitmapPointerNull at"));
    let e = nerror!(crate::ErrorKind::BitmapPointerNull, "hi");
    assert!(format!("{}",&e).starts_with("InternalError: BitmapPointerNull: hi at"));
    let e = nerror!(crate::ErrorKind::BitmapPointerNull, "hi {}", 1);
    assert!(format!("{}",&e).starts_with("InternalError: BitmapPointerNull: hi 1 at"));
}
impl<'c> OpCtxMut<'c> {

    pub fn set_more_frames(&self, value: bool){
        self.more_frames.set(value);
    }
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
            Err(nerror!(crate::ErrorKind::InvalidOperation, "Parent {:?} node not found", filter_by_kind).with_ctx_mut(self, of_node))
        }
    }

    pub fn flow_c(&self) -> *mut crate::ffi::ImageflowContext{
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

    pub fn visit_ancestors<F>(&self, ancestors_of_node: NodeIndex, f: &mut F) where F: FnMut(NodeIndex) -> (){
        for (_,ix) in self.graph.parents(ancestors_of_node).iter(self.graph){
            f(ix);
            self.visit_ancestors(ix, f);
        }
    }

    pub fn get_decoder_io_ids_and_indexes(&self,
                                          ancestors_of_node: NodeIndex)
                                          -> Vec<(i32,NodeIndex)> {
        let mut vec = Vec::new();
//        eprintln!("Searching graph for ancestors of {:?}", ancestors_of_node);
        self.visit_ancestors(ancestors_of_node, &mut |ix| {
//            eprintln!("{:?}", ix);
            if let  NodeParams::Json(s::Node::Decode { io_id, ..}) =  self.weight(ix).params{
                vec.push((io_id, ix));
            }
        });

        vec.sort_by_key(|&(io_id, _)| io_id);
        vec
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

    pub fn frame_info_from(&self, ix: NodeIndex, filter_by_kind: EdgeKind) -> Result<FrameInfo> {
        let parent = self.first_parent_of_kind_required(ix, filter_by_kind)?;
        let est = self.graph.node_weight(parent).expect(loc!("first_parent_of_kind_required provided invalid node index")).frame_est;
        if let  FrameEstimate::Some(info) = est{
            Ok(info)
        } else {
            Err(nerror!(crate::ErrorKind::InvalidOperation, "Parent {:?} node lacks FrameEstimate::Some (required for expand/execute). Value is {:?}", filter_by_kind, est).with_ctx_mut(self, ix))
        }
    }
    pub fn frame_est_from(&self, ix: NodeIndex, filter_by_kind: EdgeKind) -> Result<FrameEstimate> {
        let parent = self.first_parent_of_kind_required(ix, filter_by_kind)?;

        let est = self.graph.node_weight(parent).expect(loc!("first_parent_of_kind_required provided invalid node index")).frame_est;
        if est == FrameEstimate::None {
            Err(nerror!(crate::ErrorKind::InvalidOperation, "Parent {:?} node lacks FrameEstimate. Value is {:?}", filter_by_kind, est).with_ctx_mut(self, ix))
        } else {
            Ok(est)
        }
    }


    pub fn bitmap_bgra_from(&mut self, ix: NodeIndex, filter_by_kind: EdgeKind) -> Result<*mut BitmapBgra> {
        let parent = self.first_parent_of_kind_required(ix, filter_by_kind)?;

        let result = &self.graph.node_weight(parent).expect(loc!("first_parent_of_kind_required provided invalid node index")).result;
        if let NodeResult::Frame(bitmap) = *result {
            if bitmap.is_null() {
                Err(nerror!(crate::ErrorKind::BitmapPointerNull, "Parent {:?} node has NodeResult::Frame(null)", filter_by_kind).with_ctx_mut(self, ix))
            } else {
                Ok(bitmap)
            }
        }else{
            Err(nerror!(crate::ErrorKind::InvalidOperation, "Parent {:?} node lacks NodeResult::Frame(bitmap). Value is {:?}", filter_by_kind, result).with_ctx_mut(self, ix))
        }
    }
    pub fn consume_parent_result(&mut self, ix: NodeIndex, filter_by_kind: EdgeKind) -> Result<()> {
        let parent = self.first_parent_of_kind_required(ix, filter_by_kind)?;

        let result = {
            let weight = self.graph.node_weight(parent).expect(loc!("first_parent_of_kind_required provided invalid node index"));
            if let NodeResult::Frame(bitmap) = weight.result {
                Ok(())
            } else if let NodeResult::Consumed = weight.result {
                Err(nerror!(crate::ErrorKind::InvalidOperation, "Parent {:?} node's result has already been consumed", filter_by_kind).with_ctx_mut(self, ix))
            } else {
                Err(nerror!(crate::ErrorKind::InvalidOperation, "Parent {:?} node's result cannot be consumed. Value is {:?}", filter_by_kind, weight.result).with_ctx_mut(self, ix))
            }
        };
        if result.is_ok(){
            self.graph.node_weight_mut(parent).expect(loc!()).result = NodeResult::Consumed;
        }
        result
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

    /// Adds the given nodes in a chain, then returns the first and last node index
    pub fn add_nodes(&mut self, list: Vec<Node>) -> Option<(NodeIndex,NodeIndex)>{
        let mut with = list.clone();
        match with.len() {
            0 => None,
            n => {
                with.reverse();
                let mut last_ix = self.graph.add_node(with.pop().unwrap());
                let first_ix = last_ix.clone();
                while !with.is_empty() {
                    let new_ix = self.graph.add_node(with.pop().unwrap());
                    self.graph.add_edge(last_ix, new_ix, EdgeKind::Input).unwrap();
                    last_ix = new_ix;
                }
                Some((first_ix, last_ix))
            }
        }
    }
    // Links nodes with Input edges
    pub fn replace_node(&mut self, index: NodeIndex, with_list: Vec<Node>) {
        if let Some((first_ix,last_ix)) = self.add_nodes(with_list){
            self.copy_edges_to(index, first_ix, EdgeDirection::Incoming);
            self.copy_edges_to(index, last_ix, EdgeDirection::Outgoing);
            self.graph.remove_node(index).unwrap();
        }else{
            self.delete_node_and_snap_together(index)
        }
    }

    pub fn replace_node_with_existing(&mut self,
                                          index: NodeIndex,
                                          with_index: NodeIndex) {
        self.copy_edges_to(index, with_index, EdgeDirection::Incoming);
        self.copy_edges_to(index, with_index, EdgeDirection::Outgoing);
        self.graph.remove_node(index).unwrap();
    }






}
