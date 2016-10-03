extern crate imageflow_serde as s;
use daggy::{Dag, EdgeIndex, NodeIndex};
use ffi;
use ffi::{Context, Job, NodeType, EdgeKind};
use flow::definitions::*;
use flow::graph::Graph;
use petgraph;

impl<'c> OpCtxMut<'c> {
    pub fn first_parent_input<'a>(&'a self, of_node: NodeIndex<u32>) -> Option<NodeIndex<u32>> {
        self.graph
            .graph()
            .edges_directed(of_node, petgraph::EdgeDirection::Incoming)
            .filter(|&(node, &kind)| kind == EdgeKind::Input)
            .map(|(node, kind)| node)
            .nth(0)
    }
    pub fn first_parent_canvas<'a>(&'a self, of_node: NodeIndex<u32>) -> Option<NodeIndex<u32>> {
        self.graph
            .graph()
            .edges_directed(of_node, petgraph::EdgeDirection::Incoming)
            .filter(|&(node, &kind)| kind == EdgeKind::Canvas)
            .map(|(node, kind)| node)
            .nth(0)
    }

    pub fn first_parent_input_weight<'a>(&'a self, of_node: NodeIndex<u32>) -> Option<Node> {
        self.first_parent_input(of_node).map(|ix| self.graph.node_weight(ix).unwrap().clone())
    }

    pub fn first_parent_canvas_weight<'a>(&'a self, of_node: NodeIndex<u32>) -> Option<&Node> {
        self.first_parent_canvas(of_node).map(|ix| self.graph.node_weight(ix).unwrap())
    }

    pub fn first_parent_input_weight_mut<'a>(&'a mut self,
                                             of_node: NodeIndex<u32>)
                                             -> Option<&mut Node> {
        self.first_parent_input(of_node).map(move |ix| self.graph.node_weight_mut(ix).unwrap())
    }

    pub fn has_other_children<'a>(&'a self,
                                  of_node: NodeIndex<u32>,
                                  except_child: NodeIndex<u32>)
                                  -> bool {
        self.graph
            .graph()
            .neighbors_directed(of_node, petgraph::EdgeDirection::Incoming)
            .any(|n| n != except_child)
    }

    pub fn weight<'a>(&'a mut self, node_to_update: NodeIndex<u32>) -> &'a Node {
        self.graph.node_weight(node_to_update).unwrap()
    }

    pub fn weight_mut<'a>(&'a mut self, node_to_update: NodeIndex<u32>) -> &'a mut Node {
        self.graph.node_weight_mut(node_to_update).unwrap()
    }
    pub fn copy_frame_est_from_first_input<'a>(&'a mut self, node_to_update: NodeIndex<u32>) {
        match self.first_parent_input(node_to_update) {
            Some(input_ix) => {
                self.graph.node_weight_mut(node_to_update).unwrap().frame_est =
                    self.graph.node_weight(input_ix).unwrap().frame_est.clone();
            }
            None => {}
        }
    }
    pub fn copy_frame_est_from_first_canvas<'a>(&'a mut self, node_to_update: NodeIndex<u32>) {
        match self.first_parent_canvas(node_to_update) {
            Some(input_ix) => {
                self.graph.node_weight_mut(node_to_update).unwrap().frame_est =
                    self.graph.node_weight(input_ix).unwrap().frame_est.clone();
            }
            None => {}
        }
    }

    pub fn rotate_frame_est_from_first_input<'a, 'b>(&'a mut self,
                                                     node_to_update: NodeIndex<u32>) {
        // TODO: select by EdgeKind=Input
        let input = self.graph
            .graph()
            .neighbors_directed(node_to_update, petgraph::EdgeDirection::Incoming)
            .nth(0);
        match input {
            Some(input_ix) => {
                let input_est = self.graph.node_weight(input_ix).unwrap().frame_est.clone();
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
            None => {}
        }
    }

    pub fn copy_edges_to<'a>(&'a mut self,
                             from_node: NodeIndex<u32>,
                             to_node: NodeIndex<u32>,
                             direction: petgraph::EdgeDirection) {
        let edges = self.graph
            .graph()
            .edges_directed(from_node, direction)
            .map(|(a, b)| (a, b.clone()))
            .collect::<Vec<_>>();

        for (other_node, weight) in edges {
            match direction {
                petgraph::EdgeDirection::Incoming => {
                    self.graph.add_edge(other_node, to_node, weight.clone()).unwrap()
                }
                petgraph::EdgeDirection::Outgoing => {
                    self.graph.add_edge(to_node, other_node, weight.clone()).unwrap()
                }
            };
        }
    }
    pub fn delete_node_and_snap_together<'a>(&'a mut self, node_to_delete: NodeIndex<u32>) {
        // Prefer EdgeKind=Input
        let input = self.graph
            .graph()
            .neighbors_directed(node_to_delete, petgraph::EdgeDirection::Incoming)
            .nth(0);
        match input {
            None => {}
            Some(from_node) => {
                self.copy_edges_to(node_to_delete, from_node, petgraph::EdgeDirection::Outgoing);
                self.graph.remove_node(node_to_delete).unwrap();
            }
        };
    }

    pub fn replace_node<'a>(&'a mut self, index: NodeIndex<u32>, with_list: Vec<Node>) {
        let mut with = with_list.clone();
        match with.len() {
            0 => self.delete_node_and_snap_together(index),
            n => {
                with.reverse();
                let mut last_ix = self.graph.add_node(with.pop().unwrap());
                self.copy_edges_to(index, last_ix, petgraph::EdgeDirection::Incoming);
                while with.len() > 0 {
                    last_ix = self.graph.add_node(with.pop().unwrap());
                }
                self.copy_edges_to(index, last_ix, petgraph::EdgeDirection::Outgoing);
                self.graph.remove_node(index).unwrap();
            }
        }
    }

    pub fn replace_node_with_existing<'a>(&'a mut self,
                                          index: NodeIndex<u32>,
                                          with_index: NodeIndex<u32>) {
        self.copy_edges_to(index, with_index, petgraph::EdgeDirection::Incoming);
        self.copy_edges_to(index, with_index, petgraph::EdgeDirection::Outgoing);
        self.graph.remove_node(index).unwrap();
    }
}
struct Helpers {}
impl Helpers {
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
lazy_static! {
pub static ref NO_OP: NodeDefinition = NodeDefinition {
        id: NodeType::Noop,
        name: "NoOp",
        description: "Does nothing; pass-through node",
        fn_estimate: Some(Helpers::copy_frame_est_from_first_input),
        fn_flatten_pre_optimize: Some(Helpers::delete_node_and_snap_together),
        .. Default::default()};


pub static ref COPY_RECT: NodeDefinition = NodeDefinition {
        id: NodeType::primitive_CopyRectToCanvas,
        name: "copy_rect",
        inbound_edges: EdgesIn::OneInputOneCanvas,
        description: "Copy Rect",
        fn_estimate:  Some(Helpers::copy_frame_est_from_first_canvas),
        fn_execute: Some({

            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

//              FLOW_GET_INFOBYTES(g, node_id, flow_nodeinfo_copy_rect_to_canvas, info)
//    FLOW_GET_INPUT_EDGE(g, node_id)
//    FLOW_GET_CANVAS_EDGE(g, node_id)
//    struct flow_node * n = &g->nodes[node_id];
//
//    struct flow_bitmap_bgra * input = g->nodes[input_edge->from].result_bitmap;
//    struct flow_bitmap_bgra * canvas = g->nodes[canvas_edge->from].result_bitmap;
//
//    // TODO: implement bounds checks!!!
//    if (input->fmt != canvas->fmt) {
//        FLOW_error(c, flow_status_Invalid_argument);
//        return false;
//    }
//    if (info->x == 0 && info->from_x == 0 && info->from_y == 0 && info->y == 0 && info->width == input->w
//        && info->width == canvas->w && info->height == input->h && info->height == canvas->h
//        && canvas->stride == input->stride) {
//        memcpy(canvas->pixels, input->pixels, input->stride * input->h);
//        canvas->alpha_meaningful = input->alpha_meaningful;
//    } else {
//        int32_t bytes_pp = flow_pixel_format_bytes_per_pixel(input->fmt);
//        for (uint32_t y = 0; y < info->height; y++) {
//            void * from_ptr = input->pixels + (size_t)(input->stride * (info->from_y + y) + bytes_pp * info->from_x);
//            void * to_ptr = canvas->pixels + (size_t)(canvas->stride * (info->y + y) + bytes_pp * info->x);
//            memcpy(to_ptr, from_ptr, info->width * bytes_pp);
//        }
//    }
//    n->result_bitmap = canvas;
//                let ref mut weight = ctx.weight_mut(ix);
//                match weight.params{
//                    NodeParams::Json(s::Node::CreateCanvas{format,w,h,color}) => {
//                        weight.result = NodeResult::Frame(::ffi::flow_bitmap_bgra_create(ctx.c, w as i32, h as i32, true, ffi::PixelFormat::from(format)))
//                    },
//                    _ => { panic!("Node params missing");}
//                }

            }
            f
        }),
        .. Default::default()
    };

pub static ref CREATE_CANVAS: NodeDefinition = NodeDefinition {
        id: NodeType::Create_Canvas,
        name: "create_canvas",
        description: "Create Canvas",
        fn_estimate: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                let ref mut weight = ctx.weight_mut(ix);
                match weight.params{
                    NodeParams::Json(s::Node::CreateCanvas{ ref format, ref  w, ref h, ref color}) => {
                        weight.frame_est = FrameEstimate::Some(FrameInfo{w: *w as i32, h: *h as i32, fmt: ffi::PixelFormat::from(format), alpha_meaningful: true});
                    },
                    _ => { panic!("Node params missing");}
                }
            }
            f
        }),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                let c = ctx.c;
                let ref mut weight = ctx.weight_mut(ix);
                match  weight.params{
// TODO: support color
                    NodeParams::Json(s::Node::CreateCanvas{ ref format, ref  w, ref h, ref color}) => unsafe {
                        weight.result = NodeResult::Frame(::ffi::flow_bitmap_bgra_create(c, *w as i32, *h as i32, true, ffi::PixelFormat::from(format)))
                    },
                    _ => { panic!("Node params missing");}
                }

            }
            f
        }),
        .. Default::default()
    };

pub static ref CLONE: NodeDefinition = NodeDefinition {
        id: NodeType::Clone,
        name: "Clone",
        description: "Clone",
        fn_estimate: Some(Helpers::copy_frame_est_from_first_input),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                match ctx.first_parent_input_weight(ix).unwrap().frame_est{
                    FrameEstimate::Some(FrameInfo{w,h,fmt,alpha_meaningful}) => {
                        let canvas_params = s::Node::CreateCanvas{w: w as usize, h: h as usize, format: s::PixelFormat::from(fmt), color: s::Color::Transparent };
                        let copy_params = s::Node::CopyRectToCanvas{from_x: 0, from_y: 0, x: 0, y: 0, width: w as u32, height: h as u32};
                        let canvas = ctx.graph.add_node(Node::new(&CREATE_CANVAS, NodeParams::Json(canvas_params)));
                        let copy = ctx.graph.add_node(Node::new(&COPY_RECT, NodeParams::Json(copy_params)));
                        ctx.graph.add_edge(canvas, copy, EdgeKind::Canvas).unwrap();
                        ctx.replace_node_with_existing(ix, copy);
                    }
                    _ => {panic!("")}
                }

            }
            f
        }),
        .. Default::default()
    };

// pub static ref RENDER_1D: NodeDefinition = NodeDefinition {
//        id: NodeType::Render1D,
//        name: "render1d",
//        description: "Render1D",
//        fn_estimate: Some({
//            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
//                ctx.replace_node(ix, vec![
//                Node::new(&RENDER1D_TO_CANVAS, NodeParams::None)
//                ]);
//            }
//            f
//        }),
//        fn_flatten_pre_optimize: Some({
//            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
//
//            }
//            f
//        }),
//        .. Default::default()
//    };
   pub static ref FLIP_V_PRIMITIVE: NodeDefinition = NodeDefinition {
        id: NodeType::primitive_Flip_Vertical_Mutate,
        name: "FlipVPrimitive",
        description: "Flip frame vertical",
        fn_estimate: Some(Helpers::copy_frame_est_from_first_input),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                let from_node = ctx.first_parent_input_weight(ix).unwrap().clone();
                match from_node.result {
                    NodeResult::Frame(bitmap) => {
                        unsafe {::ffi::flow_bitmap_bgra_flip_vertical(ctx.c, bitmap); }
                        ctx.weight_mut(ix).result = NodeResult::Frame(bitmap);
                        ctx.first_parent_input_weight_mut(ix).unwrap().result = NodeResult::Consumed;
                    }
                    _ => {panic!{"Previous node not ready"}}
                }
            }
            f
        }),
        .. Default::default()
    };
    pub static ref FLIP_V: NodeDefinition = NodeDefinition {
        id: NodeType::Flip_Vertical,
        name: "FlipV",
        description: "Flip frame vertical",
        fn_estimate: Some(Helpers::copy_frame_est_from_first_input),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                let mut new_nodes = Vec::with_capacity(2);
                if ctx.has_other_children(ctx.first_parent_input(ix).unwrap(), ix) {
                    new_nodes.push(Node::new(&CLONE, NodeParams::None));
                }
                new_nodes.push(Node::new(&FLIP_V_PRIMITIVE, NodeParams::None));
                ctx.replace_node(ix, new_nodes);
            }
            f
        }),
        .. Default::default()
    };
     pub static ref FLIP_H_PRIMITIVE: NodeDefinition = NodeDefinition {
        id: NodeType::primitive_Flip_Horizontal_Mutate,
        name: "FlipHPrimitive",
        description: "Flip frame horizontal",
        fn_estimate: Some(Helpers::copy_frame_est_from_first_input),
        fn_execute: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                let from_node = ctx.first_parent_input_weight(ix).unwrap().clone();
                match from_node.result {
                    NodeResult::Frame(bitmap) => {
                        unsafe {::ffi::flow_bitmap_bgra_flip_horizontal(ctx.c, bitmap); }
                        ctx.weight_mut(ix).result = NodeResult::Frame(bitmap);
                        ctx.first_parent_input_weight_mut(ix).unwrap().result = NodeResult::Consumed;
                    }
                    _ => {panic!{"Previous node not ready"}}
                }
            }
            f
        }),
        .. Default::default()
    };
    pub static ref FLIP_H: NodeDefinition = NodeDefinition {
        id: NodeType::Flip_Horizontal,
        name: "FlipH",
        description: "Flip frame horizontal",
        fn_estimate: Some(Helpers::copy_frame_est_from_first_input),
         fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                let mut new_nodes = Vec::with_capacity(2);
                if ctx.has_other_children(ctx.first_parent_input(ix).unwrap(), ix) {
                    new_nodes.push(Node::new(&CLONE, NodeParams::None));
                }
                new_nodes.push(Node::new(&FLIP_H_PRIMITIVE, NodeParams::None));
                ctx.replace_node(ix, new_nodes);
            }
            f
        }),

        .. Default::default()
    };
    pub static ref ROTATE_90: NodeDefinition = NodeDefinition {
        id: NodeType::Rotate_90,
        name: "Rot90",
        description: "Rotate",
        fn_estimate: Some(Helpers::rotate_frame_info),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                ctx.replace_node(ix, vec![
                    Node::new(&TRANSPOSE, NodeParams::None),
                    Node::new(&FLIP_V, NodeParams::None),
                ]);
            }
            f
        }),
        .. Default::default()
    };
     pub static ref ROTATE_180: NodeDefinition = NodeDefinition {
        id: NodeType::Rotate_180,
        name: "Rot180",
        description: "Rotate",
        fn_estimate: Some(Helpers::copy_frame_est_from_first_input),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                ctx.replace_node(ix, vec![
                    Node::new(&FLIP_V, NodeParams::None),
                    Node::new(&FLIP_H, NodeParams::None),
                ]);
            }
            f
        }),
        .. Default::default()
    };
    pub static ref ROTATE_270: NodeDefinition = NodeDefinition {
        id: NodeType::Rotate_270,
        name: "Rot270",
        description: "Rotate",
        fn_estimate: Some(Helpers::rotate_frame_info),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){
                ctx.replace_node(ix, vec![
                    Node::new(&FLIP_V, NodeParams::None),
                    Node::new(&TRANSPOSE, NodeParams::None),
                ]);
            }
            f
        }),
        .. Default::default()
    };
    pub static ref APPLY_ORIENTATION: NodeDefinition = NodeDefinition {
        id: NodeType::Apply_Orientation,
        name: "Apply orientation",
        description: "Apply orientation",
        fn_estimate: Some(Helpers::copy_frame_est_from_first_input),
        fn_flatten_pre_optimize: Some(Helpers::delete_node_and_snap_together),
        .. Default::default()
    };

    pub static ref TRANSPOSE: NodeDefinition = NodeDefinition {
        id: NodeType::Transpose,
        name: "Transpose",
        description: "Transpose",
        fn_estimate: Some(Helpers::rotate_frame_info),
        fn_flatten_pre_optimize: Some({
            fn f(ctx: &mut OpCtxMut, ix: NodeIndex<u32>){

            }
            f
        }),
        .. Default::default()
    };

    //TODO: Render1D
    //TODO: APPLY_ORIENTATION
    //RENDER2d
    //BitmapBgra
    //Encoder
    //Decoder
    //Crop
    //Fill
    //Expand


}
