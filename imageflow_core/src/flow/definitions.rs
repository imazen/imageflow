use ffi::{ImageflowContext, BitmapBgra};
pub use ::ffi::EdgeKind;
pub use ::ffi::PixelFormat;
use ::{Context,Job};
use flow::nodes;
use ::internal_prelude::works_everywhere::*;
use std::any::Any;
use flow::nodes::*;

// full path
//macro_rules! function {
//    () => {{
//        fn f() {}
//        fn type_name_of<T>(_: T) -> &'static str {
//            extern crate core;
//            unsafe { core::intrinsics::type_name::<T>() }
//        }
//        let name = type_name_of(f);
//        &name[6..name.len() - 4]
//    }}
//}

macro_rules! here {
    () => (
        CodeLocation{ line: line!(), column: column!(), file: file!(), module: module_path!()}
    );
}

macro_rules! loc {
    () => (
        concat!(file!(), ":", line!(), ":", column!(), " in ", module_path!())
    );
    ($msg:expr) => (
        concat!($msg, " at\n", file!(), ":", line!(), ":", column!(), " in ", module_path!())
    );
}

macro_rules! nerror {
    ($kind:expr) => (
        NodeError{
            kind: $kind,
            message: format!("NodeError {:?}", $kind),
            at: ::smallvec::SmallVec::new(),
            node: None
        }.at(here!())
    );
    ($kind:expr, $fmt:expr) => (
        NodeError{
            kind: $kind,
            message:  format!(concat!("NodeError {:?}: ",$fmt ), $kind,),
            at: ::smallvec::SmallVec::new(),
            node: None
        }.at(here!())
    );
    ($kind:expr, $fmt:expr, $($arg:tt)*) => (
        NodeError{
            kind: $kind,
            message:  format!(concat!("NodeError {:?}: ", $fmt), $kind, $($arg)*),
            at: ::smallvec::SmallVec::new(),
            node: None
        }.at(here!())
    );
}

macro_rules! unimpl {
    () => (
        NodeError{
            kind: ErrorKind::MethodNotImplemented,
            message: String::new(),
            at: ::smallvec::SmallVec::new(),
            node: None
        }.at(here!())
    );
}

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

impl<'a> From<&'a OpCtxMut<'a>> for OpCtx<'a> {
    fn from(ctx: &'a OpCtxMut<'a>) -> Self {
        OpCtx{
            c: ctx.c,
            job: ctx.job,
            graph: ctx.graph
        }
    }
}


pub type OptionalNodeFnMut = Option<fn(&mut OpCtxMut, NodeIndex)>;

pub type OptionalNodeFnMutReturnOptI32 = Option<fn(&mut OpCtxMut, NodeIndex) -> Option<i32>>;

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




#[derive(Debug,  Clone, PartialEq)]
pub enum ErrorKind{
    NodeParamsMismatch,
    BitmapPointerNull,
    InvalidCoordinates,
    InvalidNodeParams,
    MethodNotImplemented,
    ValidationNotImplemented,
    InvalidNodeConnections,
    InvalidOperation,
    InvalidState,
    CError(FlowErr)

}
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct CodeLocation{
    pub line: u32,
    pub column: u32,
    pub file: &'static str,
    pub module: &'static str
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeDebugInfo{
    pub stable_id: i32,
    pub params: NodeParams,
    pub index: NodeIndex
}
impl NodeDebugInfo {
    fn from_ctx(ctx: &OpCtx, ix: NodeIndex) -> Option<NodeDebugInfo> {
        ctx.graph.node_weight(ix).map(|w|
            NodeDebugInfo{
                stable_id: w.stable_id,
                params: w.params.clone(),
                index: ix
            }
        )
    }
    fn from_ctx_mut(ctx: &OpCtxMut, ix: NodeIndex) -> Option<NodeDebugInfo> {
        ctx.graph.node_weight(ix).map(|w|
            NodeDebugInfo{
                stable_id: w.stable_id,
                params: w.params.clone(),
                index: ix
            }
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeError{
    pub kind: ErrorKind,
    pub message: String,
    pub at: ::smallvec::SmallVec<[CodeLocation;4]>,
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
    fn add_node_info(mut self, info: Option<NodeDebugInfo>) -> NodeError{
        self.node = info;
        self
    }
    pub fn with_ctx(self, ctx: &OpCtx, ix: NodeIndex ) -> NodeError {
        self.add_node_info(NodeDebugInfo::from_ctx(ctx, ix))
    }
    pub fn with_ctx_mut(self, ctx: &OpCtxMut, ix: NodeIndex ) -> NodeError {
        self.add_node_info(NodeDebugInfo::from_ctx_mut(ctx, ix))
    }

    pub fn at(mut self, c: CodeLocation ) -> NodeError {
        self.at.push(c);
        self
    }
}

impl fmt::Display for NodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.message.is_empty() {
            write!(f, "Error {:?}: at\n", self.kind)?;
        }else{
            write!(f, "{} at\n", self.message)?;
        }
        let url = if::imageflow_types::build_env_info::BUILT_ON_CI{
            let repo = ::imageflow_types::build_env_info::BUILD_ENV_INFO.get("CI_REPO").unwrap_or(&Some("imazen/imageflow")).unwrap_or("imazen/imageflow");
            let commit =  ::imageflow_types::build_env_info::GIT_COMMIT;
            Some(format!("https://github.com/{}/blob/{}/", repo, commit))
        }else { None };

        for recorded_frame in &self.at{
            write!(f, "{}:{}:{} in {}\n", recorded_frame.file, recorded_frame.line, recorded_frame.column, recorded_frame.module)?;

            if let Some(ref url) = url{
                write!(f, "{}{}#L{}\n",url, recorded_frame.file, recorded_frame.line)?;
            }
        }
        if let Some(ref n) = self.node{
            write!(f, "Active node:\n{:#?}\n", n)?;
        }
        Ok(())
    }
}

pub type NResult<T> = ::std::result::Result<T, NodeError>;

// alternate traits for common classes of nodes
pub trait NodeDefOneInputExpand {
    fn fqn(&self) -> &'static str;
    fn validate_params(&self, p: &NodeParams) -> NResult<()>{
        Ok(())
    }
    fn estimate(&self, p: &NodeParams, input: FrameEstimate) -> NResult<FrameEstimate>{
        Ok(input)
    }
    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, params: NodeParams, parent: FrameInfo) -> NResult<()>;
}
pub trait NodeDefOneInputOneCanvas{
    fn fqn(&self) -> &'static str;
    fn validate_params(&self, p: &NodeParams) -> NResult<()>;
    fn render(&self, c: &Context, canvas: &mut BitmapBgra, input: &mut BitmapBgra,  p: &NodeParams) -> NResult<()>;
}
pub trait NodeDefMutateBitmap{
    fn fqn(&self) -> &'static str;
    fn validate_params(&self, p: &NodeParams) -> NResult<()>{
        Ok(())
    }
    fn mutate(&self, c: &Context, bitmap: &mut BitmapBgra,  p: &NodeParams) -> NResult<()>;
}


// Rust prevents us from autoimplementing these conversion because it fears trait conflicts between the three.... gah
//
//impl<T> NodeDef for T where T: NodeDefMutateBitmap + ::std::fmt::Debug {
//    fn as_one_mutate_bitmap(&self) -> Option<&NodeDefMutateBitmap>{
//        Some(self)
//    }
//}
//impl<T> NodeDef for T where T: NodeDefOneInputOneCanvas + ::std::fmt::Debug {
//    fn as_one_input_one_canvas(&self) -> Option<&NodeDefOneInputOneCanvas>{
//        Some(self)
//    }
//
//}
//impl<T> NodeDef for T where T: NodeDefOneInput + ::std::fmt::Debug {
//    fn as_one_input(&self) -> Option<&NodeDefOneInput>{
//        Some(self)
//    }
//
//}

pub trait NodeDef: ::std::fmt::Debug{

    fn as_one_input_expand(&self) -> Option<&NodeDefOneInputExpand>{
        None
    }
    fn as_one_input_one_canvas(&self) -> Option<&NodeDefOneInputOneCanvas>{
        None
    }
    fn as_one_mutate_bitmap(&self) -> Option<&NodeDefMutateBitmap>{
        None
    }


    fn fqn(&self) -> &'static str{
        let convenience_fqn = self.as_one_input_expand().map(|n| n.fqn())
            .or(self.as_one_input_one_canvas().map(|n| n.fqn()))
            .or(self.as_one_mutate_bitmap().map(|n| n.fqn()));
        unimplemented!();
    }
    fn name(&self) -> &'static str{
        self.fqn().split_terminator('.').next_back().expect("Node fn fqn() was empty. Value is required.")
    }
    // There is "immediate" tell decoder and "during estimate" tell decoder. This is the former.
    fn tell_decoder(&self, p: &NodeParams) -> Option<(i32, Vec<s::DecoderCommand>)> {
        None
    }

    /// Edges will be validated before calling estimation or execution or flattening
    fn edges_required(&self, p: &NodeParams) -> NResult<(EdgesIn, EdgesOut)>{
        if self.as_one_input_expand().is_some(){
            Ok((EdgesIn::OneInput, EdgesOut::Any))
        } else if self.as_one_input_one_canvas().is_some(){
            Ok((EdgesIn::OneInputOneCanvas, EdgesOut::Any))
        } else if self.as_one_mutate_bitmap().is_some(){
            Ok((EdgesIn::OneInput, EdgesOut::Any))
        } else{
            Err(unimpl!())
        }
    }

    fn validate_params(&self, p: &NodeParams) -> NResult<()>{
        if let Some(n) = self.as_one_input_one_canvas(){
            n.validate_params(p).map_err(|e| e.at(here!()))
        } else if let Some(n) = self.as_one_mutate_bitmap(){
            n.validate_params(p).map_err(|e| e.at(here!()))
        } else if let Some(n) = self.as_one_input_expand(){
            n.validate_params(p).map_err(|e| e.at(here!()))
        } else{
            Err(unimpl!())
        }
    }

    fn estimate(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> NResult<FrameEstimate>{
        if let Some(n) = self.as_one_input_expand(){
            let input = ctx.frame_est_from(ix, EdgeKind::Input).map_err(|e| e.at(here!()))?;
            let params = &ctx.weight(ix).params;
            n.estimate(params, input).map_err(|e| e.at(here!()))
        } else if self.as_one_input_one_canvas().is_some(){
            ctx.frame_est_from(ix, EdgeKind::Canvas).map_err(|e| e.at(here!()))
        } else if self.as_one_mutate_bitmap().is_some(){
            ctx.frame_est_from(ix, EdgeKind::Input).map_err(|e| e.at(here!()))
        } else{
            Err(unimpl!())
        }
    }

    fn can_expand(&self) -> bool{
        self.as_one_input_expand().is_some()
    }

    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> NResult<()>{
        if let Some(n) = self.as_one_input_expand(){
            let parent = ctx.frame_info_from(ix, EdgeKind::Input)?;
            let params = ctx.weight(ix).params.clone();
            n.expand(ctx, ix, params, parent)
                .map_err(|e| e.at(here!()))
        }else {
            Err(unimpl!())
        }
    }

    fn can_execute(&self) -> bool {
        self.as_one_input_one_canvas().is_some() || self.as_one_mutate_bitmap().is_some()
    }

    fn execute(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> NResult<NodeResult>{
        if let Some(n) = self.as_one_input_one_canvas(){
            let input = ctx.bitmap_bgra_from(ix, EdgeKind::Input).map_err(|e| e.at(here!()))?;
            let canvas = ctx.bitmap_bgra_from(ix, EdgeKind::Canvas).map_err(|e| e.at(here!()))?;

            ctx.consume_parent_result(ix, EdgeKind::Canvas)?;

            n.render(ctx.c, unsafe { &mut *canvas }, unsafe { &mut *input }, &ctx.weight(ix).params).map_err(|e| e.at(here!()).with_ctx_mut(ctx,ix))?;

            Ok(NodeResult::Frame(canvas))

        } else if let Some(n) = self.as_one_mutate_bitmap(){
            let input = ctx.bitmap_bgra_from(ix, EdgeKind::Input).map_err(|e| e.at(here!()).with_ctx_mut(ctx,ix))?;
            ctx.consume_parent_result(ix, EdgeKind::Input)?;

            n.mutate(ctx.c, unsafe { &mut *input }, &ctx.weight(ix).params).map_err(|e| e.at(here!()))?;

            Ok(NodeResult::Frame(input))
        }else {
            Err(unimpl!())
        }
    }


    fn graphviz_node_label(&self, n: &Node, f: &mut std::io::Write) -> std::io::Result<()>{
        write!(f, "{}", self.name())
    }
}


#[derive(Debug,Clone)]
pub struct MutProtect<T> where T: NodeDef + 'static{
    pub node: &'static T,
    pub fqn: &'static str
}
impl<T> MutProtect<T> where T: NodeDef {
    pub fn new(with: &'static T, fqn: & 'static str) -> MutProtect<T>{
        MutProtect {
            node: with,
            fqn: fqn
        }
    }
}
impl<T> NodeDef for MutProtect<T> where T: NodeDef{
    fn as_one_input_expand(&self) -> Option<&NodeDefOneInputExpand>{
        Some(self)
    }
    fn validate_params(&self, p: &NodeParams) -> NResult<()>{
        self.node.validate_params(p).map_err(|e| e.at(here!()))
    }

    fn estimate(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> NResult<FrameEstimate>{
        self.node.estimate(ctx, ix).map_err(|e| e.at(here!()))
    }
}

impl<T> NodeDefOneInputExpand for MutProtect<T> where T: NodeDef{
    fn fqn(&self) -> &'static str{
        self.fqn
    }
    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, params: NodeParams, parent: FrameInfo) -> NResult<()>{
        let mut new_nodes = Vec::with_capacity(2);
        if ctx.has_other_children(ctx.first_parent_input(ix).unwrap(), ix) {
            new_nodes.push(Node::n(self.node, NodeParams::None));
        }
        new_nodes.push(Node::n(&*self.node, ctx.weight(ix).params.clone()));
        ctx.replace_node(ix, new_nodes);
        Ok(())
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
            Ok(ctx.weight(ix).frame_est)
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
            Ok(ctx.weight(ix).result.clone())
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

    /// Maps both UpperBound and Some
    pub fn map_frame<F>(self, f: F) -> NResult<FrameEstimate> where F: Fn(FrameInfo) -> NResult<FrameInfo> {
        match self {
            FrameEstimate::Some(info) =>
                Ok(FrameEstimate::Some(f(info)?)),
            FrameEstimate::UpperBound(info) =>
                Ok(FrameEstimate::UpperBound(f(info)?)),
            other => Ok(other)
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
            s::Node::Crop { .. } => Node::n(&nodes::CROP, NodeParams::Json(node)),
            s::Node::CropWhitespace { .. } => Node::new(&nodes::CROP_WHITESPACE, NodeParams::Json(node)),
            s::Node::Decode { .. } => Node::new(&nodes::DECODER, NodeParams::Json(node)),
            s::Node::FlowBitmapBgraPtr { .. } => {
                Node::new(&nodes::BITMAP_BGRA_POINTER, NodeParams::Json(node))
            }
            s::Node::CommandString{ .. } => Node::new(&nodes::COMMAND_STRING, NodeParams::Json(node)),
            s::Node::FlipV => Node::n(&nodes::FLIP_V, NodeParams::Json(node)),
            s::Node::FlipH => Node::n(&nodes::FLIP_H, NodeParams::Json(node)),
            s::Node::Rotate90 => Node::new(&nodes::ROTATE_90, NodeParams::Json(node)),
            s::Node::Rotate180 => Node::new(&nodes::ROTATE_180, NodeParams::Json(node)),
            s::Node::Rotate270 => Node::new(&nodes::ROTATE_270, NodeParams::Json(node)),
            s::Node::ApplyOrientation { .. } => {
                Node::n(&nodes::APPLY_ORIENTATION, NodeParams::Json(node))
            }
            s::Node::Transpose => Node::new(&nodes::TRANSPOSE, NodeParams::Json(node)),
            s::Node::Resample1D { .. } => Node::new(&nodes::SCALE_1D, NodeParams::Json(node)),
            s::Node::Encode { .. } => Node::new(&nodes::ENCODE, NodeParams::Json(node)),
            s::Node::CreateCanvas { .. } => {
                Node::n(&nodes::CREATE_CANVAS, NodeParams::Json(node))
            }
            s::Node::CopyRectToCanvas { .. } => {
                Node::n(&nodes::COPY_RECT, NodeParams::Json(node))
            }
            s::Node::FillRect { .. } => Node::n(&nodes::FILL_RECT, NodeParams::Json(node)),
            s::Node::Resample2D { .. } => Node::new(&nodes::SCALE, NodeParams::Json(node)),
            s::Node::Constrain (_) => Node::new(&nodes::CONSTRAIN, NodeParams::Json(node)),
            s::Node::ExpandCanvas { .. } => {
                Node::n(&nodes::EXPAND_CANVAS, NodeParams::Json(node))
            },
            s::Node::WhiteBalanceHistogramAreaThresholdSrgb { ..} => {
                Node::n(&nodes::WHITE_BALANCE_SRGB, NodeParams::Json(node))
            },
            s::Node::ColorMatrixSrgb { ..} => {
                Node::n(&nodes::COLOR_MATRIX_SRGB, NodeParams::Json(node))
            },
            s::Node::ColorFilterSrgb { ..} => {
                Node::n(&nodes::COLOR_FILTER_SRGB, NodeParams::Json(node))
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

    pub fn n(def: &'static NodeDef, params: NodeParams) -> Node {
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
