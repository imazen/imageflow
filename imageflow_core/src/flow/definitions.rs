use libc::{c_void,c_float,int32_t,int64_t,size_t,uint32_t};
extern crate imageflow_serde as s;
use daggy::{Dag,EdgeIndex,NodeIndex};
use super::graph::Graph;
use ffi::*;
use std::fmt;

#[repr(C)]
#[derive(Copy,Clone,Debug,PartialEq)]
pub enum NodeStage {
    Blank = 0,
    InputDimensionsKnown = 1,
    //FIXME: we shouldn't reuse the value
    //ReadyForPreOptimizeFlatten = 1,
    PreOptimizeFlattened = 2,
    ReadyForOptimize = 3,
    Optimized = 4,
    ReadyForPostOptimizeFlatten = 7,
    PostOptimizeFlattened = 8,
    InputsExecuted = 16,
    ReadyForExecution = 31,
    Executed = 32,
    Done = 63,
}

#[derive(Clone,Debug,PartialEq)]
pub enum EdgesIn{
    NoInput,
    OneInput,
    OneOptionalInput,
    OneInputOneCanvas,
    Aribtary{
        inputs: i32,
        canvases: i32,
        infos: i32
    }
}


pub struct OpCtx<'a>{
    pub c: *mut Context,
    pub job: *const Job,
    pub graph: &'a Graph
}

pub struct OpCtxMut<'a> {
    pub c: *mut Context,
    pub job: *mut Job,
    pub graph: &'a mut Graph
}

pub type OptionalNodeFnMut = Option<fn(&mut OpCtxMut, NodeIndex<u32>)>;

//#[derive(Clone,Debug,PartialEq, Default)]
pub struct NodeDefinition {
    //When comparing equality, we just check 'id' (for now)
    pub id: NodeType,
    pub inbound_edges: EdgesIn,
    pub outbound_edges: bool,
    pub name: &'static str,
    pub description: &'static str,

    pub fn_graphviz_text: Option<fn(&mut OpCtxMut, NodeIndex<u32>,  &Node, &mut fmt::Formatter) -> fmt::Result>,
    pub fn_estimate: OptionalNodeFnMut,
    pub fn_flatten_pre_optimize: OptionalNodeFnMut,
    pub fn_flatten_post_optimize: OptionalNodeFnMut,
    pub fn_execute: OptionalNodeFnMut,
    pub fn_cleanup: OptionalNodeFnMut,
}

#[derive(Copy, Clone,Debug,PartialEq)]
pub struct FrameInfo{
    pub w: i32,
    pub h: i32,
    pub fmt: PixelFormat,
    pub alpha_meaningful: bool,
}

#[derive(Copy, Clone,Debug,PartialEq)]
pub enum FrameEstimate {
    None,
    Impossible,
    UpperBound(FrameInfo),
    Some(FrameInfo)
}

#[derive(Clone,Debug,PartialEq)]
pub struct CostInfo{
    pub wall_ticks: u32, //Estimated wall ticks to execute
    pub cpu_ticks: Option<u32>, //Estimate overall CPU ticks (larger, if multi-threaded)
    pub heap_bytes: u32,
    pub peak_temp_bytes: u32,
}

#[derive(Clone,Debug,PartialEq)]
pub enum CostEstimate {
    None,
    Impossible,
    UpperBound(CostInfo),
    Some(CostInfo),
    NotImplemented,
}
#[derive(Clone,Debug,PartialEq)]
pub enum NodeResult{
    None, // No result yet
    Consumed, //Ownership has been transferred to another node for exclusive mutation. If another node tries to access, a panic will occur. Don't consume without verifying no other nodes want access.
    Frame(*mut BitmapBgra), //Should this be boxed?
}
#[derive(Clone,Debug,PartialEq)]
pub enum NodeParamsInternal{
    Render1D{
        scale_to_width: usize,
        canvas_x: usize,
        canvas_y: usize,
        filter: Option<s::Filter>,
        sharpen_percent_goal: Option<f32>,
        transpose_on_write: bool,
        matte_color: Option<s::Color>,
        compositing_mode: ::ffi::BitmapCompositingMode,
    },
}
#[derive(Clone,Debug,PartialEq)]
pub enum NodeParams{
    None,
    Json(s::Node),
    Internal(NodeParamsInternal)
}

#[derive(Clone,Debug,PartialEq)]
pub struct Node {
    pub def: &'static NodeDefinition,
    pub stage: NodeStage,
    pub params: NodeParams,
    pub frame_est: FrameEstimate,
    pub cost_est: CostEstimate,
    pub cost: CostInfo,
    pub result: NodeResult,
    pub custom_state: *mut u8, //For simple metadata, we might just use JSON?
}

impl Node{
   pub fn new(def: &'static NodeDefinition, params: NodeParams) -> Node{
        Node{
            def: def,
            custom_state: ::std::ptr::null_mut(),
            frame_est: FrameEstimate::None,
            cost_est: CostEstimate::None,
            cost: CostInfo {
                cpu_ticks: None,
                wall_ticks: 0,
                heap_bytes: 0,
                peak_temp_bytes: 0
            },
            params: params,
            stage: NodeStage::Blank,
            result: NodeResult::None
        }
    }
}


impl PartialEq for NodeDefinition {
    fn eq(&self, other: &NodeDefinition) -> bool {
        self.id == other.id
    }
}

impl fmt::Debug for NodeDefinition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NodeDefinition {{ name: '{}', id: {} }}", self.name, self.id as i32)
    }
}

impl Default for NodeDefinition {
    fn default() -> NodeDefinition {
        NodeDefinition {
            id: NodeType::Null,
            inbound_edges: EdgesIn::OneInput,
            outbound_edges: true,
            name: "(null)",
            description: "",
            fn_graphviz_text: None,
            fn_flatten_post_optimize: None,
            fn_execute: None,
            fn_cleanup: None,
            fn_estimate: None,
            fn_flatten_pre_optimize: None
        }
    }
}


