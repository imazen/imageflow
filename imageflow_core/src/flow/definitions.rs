use ffi::{ImageflowContext, BitmapBgra};
pub use ::ffi::EdgeKind;
pub use ::ffi::PixelFormat;
use ::{Context,Job};
use flow::nodes;
use ::internal_prelude::works_everywhere::*;

pub type Graph = Dag<Node, EdgeKind>;


#[repr(C)]
#[derive(Copy,Clone,Debug,PartialEq)]
pub enum NodeStage {
    Blank = 0,
    InputDimensionsKnown = 1,
    // FIXME: we shouldn't reuse the value
    // ReadyForPreOptimizeFlatten = 1,
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
pub enum EdgesIn {
    NoInput,
    OneInput,
    OneOptionalInput,
    OneInputOneCanvas,
    Aribtary {
        inputs: i32,
        canvases: i32,
        infos: i32,
    },
}


pub struct OpCtx<'a> {
    pub c: &'a Context,
    pub job: &'a Job,
    pub graph: &'a Graph,
}

pub struct OpCtxMut<'a> {
    pub c: &'a Context,
    pub job: &'a mut Job,
    pub graph: &'a mut Graph,
}

pub type OptionalNodeFnMut = Option<fn(&mut OpCtxMut, NodeIndex<u32>)>;

pub type OptionalNodeFnMutReturnOptI32 = Option<fn(&mut OpCtxMut, NodeIndex<u32>) -> Option<i32>>;
pub type OptionalNodeGraphviz = Option<fn(&mut OpCtxMut, NodeIndex<u32>, &Node, &mut fmt::Formatter)
-> fmt::Result>;


// #[derive(Clone,Debug,PartialEq, Default)]
pub struct NodeDefinition {
    // When comparing equality, we just check 'id' (for now)
    pub fqn: &'static str,
    pub inbound_edges: EdgesIn,
    pub outbound_edges: bool,
    pub name: &'static str,
    pub description: &'static str,

    pub fn_link_state_to_this_io_id: OptionalNodeFnMutReturnOptI32,
    pub fn_graphviz_text: OptionalNodeGraphviz,
    pub fn_estimate: OptionalNodeFnMut,
    pub fn_flatten_pre_optimize: OptionalNodeFnMut,
    pub fn_flatten_post_optimize: OptionalNodeFnMut,
    pub fn_execute: OptionalNodeFnMut,
    pub fn_cleanup: OptionalNodeFnMut,
}

#[derive(Copy, Clone,Debug,PartialEq)]
pub struct FrameInfo {
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
    Some(FrameInfo),
}

#[derive(Clone,Debug,PartialEq)]
pub struct CostInfo {
    pub wall_ns: u32, // Estimated wall ticks to execute
    pub cpu_ticks: Option<u32>, // Estimate overall CPU ticks (larger, if multi-threaded)
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
pub enum NodeResult {
    None, // No result yet
    Consumed, /* Ownership has been transferred to another node for exclusive mutation. If another node tries to access, a panic will occur. Don't consume without verifying no other nodes want access. */
    Frame(*mut BitmapBgra), // Should this be boxed?
    Encoded(s::EncodeResult),
}
#[derive(Clone,Debug,PartialEq)]
pub enum NodeParamsInternal {
    Render1D {
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
pub enum NodeParams {
    None,
    Json(s::Node),
    Internal(NodeParamsInternal),
}

#[derive(Clone,Debug,PartialEq)]
pub struct Node {
    pub def: &'static NodeDefinition,
    pub stage: NodeStage, // TODO: delete
    pub params: NodeParams,
    pub frame_est: FrameEstimate,
    pub cost_est: CostEstimate,
    pub cost: CostInfo,
    pub result: NodeResult,
    pub stable_id: i32,
    pub custom_state: *mut u8, // For simple metadata, we might just use JSON?
}


impl From<s::Node> for Node {
    fn from(node: s::Node) -> Node {
        match node {
            s::Node::Crop { .. } => Node::new(&nodes::CROP, NodeParams::Json(node)),
            s::Node::Decode { .. } => Node::new(&nodes::DECODER, NodeParams::Json(node)),
            s::Node::FlowBitmapBgraPtr { .. } => {
                Node::new(&nodes::BITMAP_BGRA_POINTER, NodeParams::Json(node))
            }
            s::Node::FlipV => Node::new(&nodes::FLIP_V, NodeParams::Json(node)),
            s::Node::FlipH => Node::new(&nodes::FLIP_H, NodeParams::Json(node)),
            s::Node::Rotate90 => Node::new(&nodes::ROTATE_90, NodeParams::Json(node)),
            s::Node::Rotate180 => Node::new(&nodes::ROTATE_180, NodeParams::Json(node)),
            s::Node::Rotate270 => Node::new(&nodes::ROTATE_270, NodeParams::Json(node)),
            s::Node::ApplyOrientation { .. } => {
                Node::new(&nodes::APPLY_ORIENTATION, NodeParams::Json(node))
            }
            s::Node::Transpose => Node::new(&nodes::TRANSPOSE, NodeParams::Json(node)),
            s::Node::Resample1D { .. } => Node::new(&nodes::SCALE_1D, NodeParams::Json(node)),
            s::Node::Encode { .. } => Node::new(&nodes::ENCODE, NodeParams::Json(node)),
            s::Node::CreateCanvas { .. } => {
                Node::new(&nodes::CREATE_CANVAS, NodeParams::Json(node))
            }
            s::Node::CopyRectToCanvas { .. } => {
                Node::new(&nodes::COPY_RECT, NodeParams::Json(node))
            }
            s::Node::FillRect { .. } => Node::new(&nodes::FILL_RECT, NodeParams::Json(node)),
            s::Node::Resample2D { .. } => Node::new(&nodes::SCALE, NodeParams::Json(node)),
            s::Node::Constrain (_) => Node::new(&nodes::CONSTRAIN, NodeParams::Json(node)),
            s::Node::ExpandCanvas { .. } => {
                Node::new(&nodes::EXPAND_CANVAS, NodeParams::Json(node))
            }
        }
    }
}

impl Node {
    pub fn new(def: &'static NodeDefinition, params: NodeParams) -> Node {
        Node {
            def: def,
            custom_state: ::std::ptr::null_mut(),
            frame_est: FrameEstimate::None,
            cost_est: CostEstimate::None,
            cost: CostInfo {
                cpu_ticks: None,
                wall_ns: 0,
                heap_bytes: 0,
                peak_temp_bytes: 0,
            },
            stable_id: -1,
            params: params,
            stage: NodeStage::Blank,
            result: NodeResult::None,
        }
    }




    pub fn graphviz_node_label(&self, f: &mut std::io::Write) -> std::io::Result<()> {
        // name { est f1, f2, ex } none
        let a = if self.def.fn_estimate.is_some() {
            "est "
        } else {
            ""
        };
        let b = if self.def.fn_flatten_pre_optimize.is_some() {
            "f1 "
        } else {
            ""
        };
        let c = if self.def.fn_flatten_post_optimize.is_some() {
            "f2 "
        } else {
            ""
        };
        let d = if self.def.fn_execute.is_some() {
            "exec "
        } else {
            ""
        };

        let e = match self.result {
            NodeResult::None => "none",
            NodeResult::Consumed => "reused",
            NodeResult::Frame(_) => "done",
            NodeResult::Encoded(_) => "encoded",
        };
        try!(write!(f, "{}{{{}{}{}{}}} {}", self.def.name, a, b, c, d, e));
        Ok(())
    }
}


impl PartialEq for NodeDefinition {
    fn eq(&self, other: &NodeDefinition) -> bool {
        self.fqn == other.fqn && self.fqn != ""
    }
}

impl fmt::Debug for NodeDefinition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NodeDefinition {{ fqn: '{}' }}", self.fqn)
    }
}

impl Default for NodeDefinition {
    fn default() -> NodeDefinition {
        NodeDefinition {
            fqn: "",
            inbound_edges: EdgesIn::OneInput,
            outbound_edges: true,
            name: "(null)",
            description: "",
            fn_graphviz_text: None,
            fn_flatten_post_optimize: None,
            fn_execute: None,
            fn_cleanup: None,
            fn_estimate: None,
            fn_flatten_pre_optimize: None,
            fn_link_state_to_this_io_id: None,
        }
    }
}
