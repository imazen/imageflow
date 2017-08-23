use ffi::{ImageflowContext, BitmapBgra};
pub use ::ffi::EdgeKind;
pub use ::ffi::PixelFormat;
use ::{Context,Job};
use flow::nodes;
use ::internal_prelude::works_everywhere::*;
use std::any::Any;
use flow::nodes::*;


pub type Graph = Dag<Node, EdgeKind>;


#[derive(Copy, Clone,Debug,PartialEq)]
pub enum EdgesIn {
    NoInput,
    OneInput,
    OneOptionalInput,
    OneInputOneCanvas,
    Arbitrary {
        inputs: i32,
        canvases: i32,
        infos: i32,
    },
}

#[derive(Copy, Clone,Debug,PartialEq)]
pub enum EdgesOut {
    None,
    Any
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

// #[derive(Clone,Debug,PartialEq, Default)]
pub struct NodeDefinition {
    // When comparing equality, we just check 'id' (for now)
    pub fqn: &'static str,
    pub inbound_edges: EdgesIn,
    pub outbound_edges: bool,
    pub name: &'static str,
    pub description: &'static str,

    pub fn_link_state_to_this_io_id: OptionalNodeFnMutReturnOptI32, //default impl
    pub fn_estimate: OptionalNodeFnMut,
    pub fn_flatten_pre_optimize: OptionalNodeFnMut,
    pub fn_flatten_post_optimize: OptionalNodeFnMut, // not used
    pub fn_execute: OptionalNodeFnMut,
}



macro_rules! code_location {
    () => (
        CodeLocation{ line: line!(), column: column!(), file: file!(), module: module_path!()}
    );
}

macro_rules! nerror {
    ($kind:expr) => (
        NodeError{
            kind: $kind,
            message: format!("Error {:?} at\n{}:{}:{} in {}", $kind, file!(), line!(), column!(), module_path!()),
            at: code_location!(),
            node: None
        }
    );
    ($kind:expr, $fmt:expr) => (
        NodeError{
            kind: $kind,
            message:  format!(concat!("Error {:?}: ",$fmt , " at\n{}:{}:{} in {}"), $kind, file!(), line!(), column!(), module_path!()),
            at: code_location!(),
            node: None
        }
    );
    ($kind:expr, $fmt:expr, $($arg:tt)*) => (
        NodeError{
            kind: $kind,
            message:  format!(concat!("Error {:?}: ", $fmt , " at\n{}:{}:{} in {}"), $kind, $($arg)*, file!(), line!(), column!(), module_path!()),
            at: code_location!(),
            node: None
        }
    );
}

macro_rules! unimpl {
    () => (
        NodeError{
            kind: ErrorKind::MethodNotImplemented,
            message: String::new(),
            at: code_location!(),
            node: None
        }
    );
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ErrorKind{
    NodeParamsMismatch,
    BitmapPointerNull,
    InvalidCoordinates,
    InvalidNodeParams,
    MethodNotImplemented,
    ValidationNotImplemented

}
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct CodeLocation{
    pub line: u32,
    pub column: u32,
    pub file: &'static str,
    pub module: &'static str
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct NodeDebugInfo{
    pub stable_id: i32,
}
impl NodeDebugInfo {
    fn from_ctx(ctx: &OpCtx, ix: NodeIndex) -> Option<NodeDebugInfo> {
        ctx.graph.node_weight(ix).map(|w|
            NodeDebugInfo{
                stable_id: w.stable_id
            }
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeError{
    pub kind: ErrorKind,
    pub message: String,
    pub at: CodeLocation,
    pub node: Option<NodeDebugInfo>
}


impl ::std::error::Error for NodeError {
    fn description(&self) -> &str {
        if self.message.is_empty() {
            "Node Error (no message)"
        }else{
            &self.message
        }
    }

}
impl NodeError{
    fn add_node_info(self, info: Option<NodeDebugInfo>) -> NodeError{
        NodeError{
            node: info,
            .. self
        }
    }
}

impl fmt::Display for NodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.message.is_empty() {
            write!(f, "Error {:?}: at\n{}:{}:{} in {}", self.kind, self.at.file, self.at.line, self.at.column, self.at.module)
        }else{
            write!(f, "{}", self.message)
        }
    }
}

pub type NResult<T> = ::std::result::Result<T, NodeError>;


// alternate traits for common classes of nodes
pub trait NodeDefOneInput{

}
pub trait NodeDefOneInputOneCanvas{

}

pub trait NodeDef: ::std::fmt::Debug{

    fn as_one_input(&self) -> Option<&NodeDefOneInput>{
        None
    }
    fn as_one_input_one_canvas(&self) -> Option<&NodeDefOneInputOneCanvas>{
        None
    }

    fn fqn(&self) -> &'static str;
    fn name(&self) -> &'static str{
        self.fqn().split_terminator('.').next_back().expect("Node fn fqn() was empty. Value is required.")
    }
    // There is "immediate" tell decoder and "during estimate" tell decoder. This is the former.
    fn tell_decoder(&self, p: &NodeParams) -> Option<(i32, Vec<s::DecoderCommand>)> {
        None
    }
    /// Edges will be validated before calling estimation or execution or flattening
    fn edges_required(&self, p: &NodeParams) -> NResult<(EdgesIn, EdgesOut)>{
        if self.as_one_input().is_some(){
            Ok((EdgesIn::OneInput, EdgesOut::Any))
        } else if self.as_one_input_one_canvas().is_some(){
            Ok((EdgesIn::OneInputOneCanvas, EdgesOut::Any))
        } else{
            Err(unimpl!())
        }
    }
    fn validate_params(&self, p: &NodeParams) -> NResult<()>{
        if let NodeParams::Json(ref n) = *p{
            self.validate_json(n) //Caller should: .map_err(|e| e.add_node_info(NodeDebugInfo::from_ctx(ctx, ix)))
        }else {
            Err(nerror!(ErrorKind::NodeParamsMismatch))
        }
    }

    fn validate_json(&self, n: &s::Node) -> NResult<()>{
        Err(unimpl!())
    }

    fn estimate(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> NResult<FrameEstimate>{
        Err(unimpl!())
    }

    fn can_expand(&self) -> bool{
        false
    }

    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> NResult<()>{

        Err(unimpl!())
    }

    fn can_execute(&self) -> bool{
        false
    }


    fn execute(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> NResult<NodeResult>{

        Err(unimpl!())
    }

    fn estimate_from_json(&self, n: &s::Node) -> NResult<FrameEstimate>{
        Err(unimpl!())
    }

    fn execute_from_json(&self, n: &s::Node) -> NResult<NodeResult>{
        Err(unimpl!())
    }


    fn graphviz_node_label(&self, n: &Node, f: &mut std::io::Write) -> std::io::Result<()>{
        write!(f, "{}", self.name())
    }
}

impl NodeDefinition{
    pub fn as_node_def(&self) -> &NodeDef{
        self
    }
}

impl NodeDef for NodeDefinition{

    fn fqn(&self) -> &'static str{
        self.fqn
    }
    fn name(&self) -> &'static str{
        self.name
    }
    fn edges_required(&self, p: &NodeParams) -> NResult<(EdgesIn, EdgesOut)>{
        Ok((self.inbound_edges, if self.outbound_edges { EdgesOut::Any } else { EdgesOut::None }))
    }

    fn validate_params(&self, p: &NodeParams) -> NResult<()>{
        Ok(())
    }

    fn estimate(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> NResult<FrameEstimate>{
        if let Some(f) = self.fn_estimate{
            f(ctx, ix);
            Ok(FrameEstimate::None)
        }else{
            Err(unimpl!())
        }
    }

    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> NResult<()>{
        if let Some(f) = self.fn_flatten_pre_optimize{
            f(ctx, ix);
            Ok(())
        }else{
            Err(unimpl!())
        }
    }

    fn can_expand(&self) ->bool{
        self.fn_flatten_pre_optimize.is_some()
    }

    fn can_execute(&self) ->bool{
        self.fn_execute.is_some()
    }

    fn execute(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> NResult<NodeResult>{
        if let Some(f) = self.fn_execute{
            f(ctx, ix);
            Ok(NodeResult::None)
        }else{
            Err(unimpl!())
        }
    }

    fn graphviz_node_label(&self, n: &Node, f: &mut std::io::Write) -> std::io::Result<()>{

        // name { est f1, f2, ex } none
        let a = if self.fn_estimate.is_some() {
            "est "
        } else {
            ""
        };
        let b = if self.fn_flatten_pre_optimize.is_some() {
            "f1 "
        } else {
            ""
        };
        let c = if self.fn_flatten_post_optimize.is_some() {
            "f2 "
        } else {
            ""
        };
        let d = if self.fn_execute.is_some() {
            "exec "
        } else {
            ""
        };

        let e = match n.result {
            NodeResult::None => "none",
            NodeResult::Consumed => "reused",
            NodeResult::Frame(_) => "done",
            NodeResult::Encoded(_) => "encoded",
        };
        write!(f, "{}{{{}{}{}{}}} {}", self.name(), a, b, c, d, e)?;
        Ok(())
    }
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
    Invalidated,
    UpperBound(FrameInfo),
    Some(FrameInfo),
}

impl FrameEstimate{
    pub fn is_none(&self) -> bool{
        self == &FrameEstimate::None
    }
    pub fn is_some(&self) -> bool{
        if let &FrameEstimate::Some(_) = self {
            true
        }else{
            false
        }
    }
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

#[derive(Clone,Debug)]
pub struct Node {
    pub def: &'static NodeDef,
    pub params: NodeParams,
    pub frame_est: FrameEstimate,
    pub cost_est: CostEstimate,
    pub cost: CostInfo,
    pub result: NodeResult,
    pub stable_id: i32,
}

#[test]
fn limit_node_bytes(){
    let size = mem::size_of::<Node>();
    eprintln!("{} bytes.", size);
    assert!(size < 1024);
}

impl From<s::Node> for Node {
    fn from(node: s::Node) -> Node {
        match node {
            s::Node::Crop { .. } => Node::new(&nodes::CROP, NodeParams::Json(node)),
            s::Node::CropWhitespace { .. } => Node::new(&nodes::CROP_WHITESPACE, NodeParams::Json(node)),
            s::Node::Decode { .. } => Node::new(&nodes::DECODER, NodeParams::Json(node)),
            s::Node::FlowBitmapBgraPtr { .. } => {
                Node::new(&nodes::BITMAP_BGRA_POINTER, NodeParams::Json(node))
            }
            s::Node::CommandString{ .. } => Node::new(&nodes::COMMAND_STRING, NodeParams::Json(node)),
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
            },
            s::Node::WhiteBalanceHistogramAreaThresholdSrgb { ..} => {
                Node::new(&nodes::WHITE_BALANCE_SRGB, NodeParams::Json(node))
            },
            s::Node::ColorMatrixSrgb { ..} => {
                Node::new(&nodes::COLOR_MATRIX_SRGB, NodeParams::Json(node))
            },
            s::Node::ColorFilterSrgb { ..} => {
                Node::new(&nodes::COLOR_FILTER_SRGB, NodeParams::Json(node))
            },

        }
    }
}

impl Node {
    pub fn new(def: &'static NodeDefinition, params: NodeParams) -> Node {
        Node {
            def: def,
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
            result: NodeResult::None,
        }
    }




    pub fn graphviz_node_label(&self, f: &mut std::io::Write) -> std::io::Result<()> {
        self.def.graphviz_node_label(self, f)
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
            fn_flatten_post_optimize: None,
            fn_execute: None,
            fn_estimate: None,
            fn_flatten_pre_optimize: None,
            fn_link_state_to_this_io_id: None,
        }
    }
}
