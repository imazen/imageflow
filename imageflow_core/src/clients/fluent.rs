//!
//! Fluent (chainable) Rust API for building an operation graph in an easier and more readable way.


use crate::internal_prelude::works_everywhere::*;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic;

static NEXT_FLUENT_NODE_ID: AtomicUsize = AtomicUsize::new(0);


pub fn fluently() -> FluentNode {
    FluentNode::empty()
}

#[derive(Clone,Debug)]
pub struct FluentNode {
    input: Option<Box<FluentNode>>,
    canvas: Option<Box<FluentNode>>,
    data: Option<s::Node>,
    uid: u64,
}

impl FluentNode {
    fn next_uid() -> u64 {
        NEXT_FLUENT_NODE_ID.fetch_add(1, atomic::Ordering::SeqCst) as u64
    }
    fn new(node: s::Node,
           input_node: Option<FluentNode>,
           canvas_node: Option<FluentNode>)
           -> FluentNode {
        FluentNode {
            input: input_node.and_then(|v| if v.is_empty() {
                None } else {Some(Box::new(v)) }),
            canvas: canvas_node.and_then(|v| if v.is_empty() {
                None } else {Some(Box::new(v)) }),
            data: Some(node),
            uid: FluentNode::next_uid(),
        }
    }
    pub fn empty() -> FluentNode {
        FluentNode {
            input: None,
            canvas: None,
            data: None,
            uid: FluentNode::next_uid(),
        }
    }


    pub fn is_empty(&self) -> bool {
        self.data.is_none()
    }

    pub fn to(self, v: s::Node) -> FluentNode {
        FluentNode::new(v, Some(self), None)
    }
    pub fn node_with_canvas(self, canvas: FluentNode, v: s::Node) -> FluentNode {
        FluentNode::new(v, Some(self), Some(canvas))
    }
    pub fn branch(&self) -> FluentNode {
        self.clone()
    }
    pub fn builder(self) -> FluentGraphBuilder {
        FluentGraphBuilder::new_with(self)
    }

    /// Injects placeholders
    pub fn into_build_0_1(self) -> s::Build001{
        self.builder().to_framewise().wrap_in_build_0_1()
    }

    pub fn constrain_within(self, w: Option<u32>, h: Option<u32>, resampling_hints: Option<s::ResampleHints>) -> FluentNode{
        self.to(s::Node::Constrain(imageflow_types::Constraint{ mode: s::ConstraintMode::Within , w, h, hints: resampling_hints, gravity: None, canvas_color: None }))
    }

    pub fn canvas_bgra32(self,
                         w: usize,
                         // camelCased: #[serde(rename="fromY")]
                         h: usize,
                         color: s::Color)
                         -> FluentNode {
        self.to(s::Node::CreateCanvas {
            w,
            h,
            format: s::PixelFormat::Bgra32,
            color,
        })
    }



    pub fn create_canvas(self,
                         w: usize,
                         // camelCased: #[serde(rename="fromY")]
                         h: usize,
                         format: s::PixelFormat,
                         color: s::Color)
                         -> FluentNode {
        self.to(s::Node::CreateCanvas {
            w,
            h,
            format,
            color,
        })
    }


    pub fn decode(self, io_id: i32) -> FluentNode {
        self.to(s::Node::Decode {
            io_id,
            commands: None,
        })
    }
    pub fn encode(self, io_id: i32, preset: s::EncoderPreset) -> FluentNode {
        self.to(s::Node::Encode {
            io_id,
            preset
        })
    }



    pub fn flip_vertical(self) -> FluentNode {
        self.to(s::Node::FlipV)
    }

    pub fn flip_horizontal(self) -> FluentNode {
        self.to(s::Node::FlipH)
    }

    pub fn rotate_90(self) -> FluentNode {
        self.to(s::Node::Rotate90)
    }
    pub fn rotate_180(self) -> FluentNode {
        self.to(s::Node::Rotate180)
    }
    pub fn rotate_270(self) -> FluentNode {
        self.to(s::Node::Rotate270)
    }

    pub fn transpose(self) -> FluentNode {
        self.to(s::Node::Transpose)
    }

    #[cfg_attr(feature = "cargo-clippy", allow(too_many_arguments))]
    pub fn copy_rect_from(self,
                          from: FluentNode,
                          from_x: u32,
                          // camelCased: #[serde(rename="fromY")]
                          from_y: u32,
                          width: u32,
                          height: u32,
                          x: u32,
                          y: u32)
                          -> FluentNode {
        from.node_with_canvas(self,
                              s::Node::CopyRectToCanvas {
                           from_x,
                           from_y,
                                  w: width,
                                  h: height,
                           x,
                           y,
                       })
    }
}
impl PartialEq for FluentNode {
    fn eq(&self, other: &FluentNode) -> bool {
        self.uid == other.uid
    }
}


#[derive(Default)]
pub struct FluentGraphBuilder {
    output_nodes: Vec<Box<FluentNode>>,
}

impl FluentGraphBuilder {
    pub fn new() -> FluentGraphBuilder {
        FluentGraphBuilder { output_nodes: vec![] }
    }
    pub fn new_with(n: FluentNode) -> FluentGraphBuilder {
        FluentGraphBuilder { output_nodes: vec![Box::new(n)] }
    }

    pub fn with(self, n: FluentNode) -> FluentGraphBuilder {
        let mut new_vec = self.output_nodes.clone();
        new_vec.push(Box::new(n));
        FluentGraphBuilder { output_nodes: new_vec }
    }

    fn collect_unique(&self) -> Vec<&FluentNode> {
        let mut set = HashSet::new();
        let mut todo = Vec::new();
        let mut unique = Vec::new();
        for end in self.output_nodes.as_slice().iter() {
            todo.push(end);
        }

        loop {
            if todo.is_empty() {
                break;
            }
            let next = todo.pop().unwrap();
            if !set.contains(&next.uid) {
                set.insert(next.uid);
                unique.push(next.as_ref());
                if let Some(ref c) = next.canvas {
                    todo.push(c);
                }
                if let Some(ref c) = next.input {
                    todo.push(c);
                }
            }
        }

        unique
    }

    fn collect_edges(&self, for_nodes: &[&FluentNode]) -> Vec<(u64, u64, s::EdgeKind)> {
        let mut edges = vec![];
        for n in for_nodes {
            if let Some(ref parent) = n.canvas {
                edges.push((parent.uid, n.uid, s::EdgeKind::Canvas));
            }
            if let Some(ref parent) = n.input {
                edges.push((parent.uid, n.uid, s::EdgeKind::Input));
            }
        }
        edges
    }
    fn lowest_uid(for_nodes: &[&FluentNode]) -> Option<u64> {
        for_nodes.iter().map(|n| n.uid).min()
    }
    pub fn to_framewise(&self) -> s::Framewise {
        let mut nodes = self.collect_unique();
        if self.output_nodes.len() == 1 && nodes.as_slice().iter().all(|n| n.canvas == None) {
            nodes.sort_by(|a,b|a.uid.cmp(&b.uid));
            s::Framewise::Steps(nodes.into_iter().map(|b| b.data.clone().unwrap()).collect::<Vec<s::Node>>())
        }else{
            self.to_framewise_graph()
        }
    }
    pub fn to_framewise_graph(&self) -> s::Framewise {
        let nodes = self.collect_unique();
        let lowest_uid = FluentGraphBuilder::lowest_uid(&nodes).unwrap_or(0);
        let edges = self.collect_edges(&nodes);
        let framewise_edges = edges.into_iter()
            .map(|(from, to, kind)| {
                s::Edge {
                    from: (from - lowest_uid) as i32,
                    to: (to - lowest_uid) as i32,
                    kind: kind,
                }
            })
            .collect::<Vec<s::Edge>>();
        let mut framewise_nodes = HashMap::new();
        for n in nodes {
            let _ =
                framewise_nodes.insert((n.uid - lowest_uid).to_string(), n.data.clone().unwrap());
        }
        s::Framewise::Graph(s::Graph {
            edges: framewise_edges,
            nodes: framewise_nodes,
        })
    }

    //    pub fn to_graph(&self) -> ::Graph {
    //        let mut uid_map = HashMap::new();
    //        let from_list = self.collect_unique();
    //
    //        let mut g = ::Graph::with_capacity(from_list.len(), from_list.len() + 8);
    //        for n in from_list.as_slice(){
    //            if let Some(ref data) = n.data {
    //                let ix = g.add_node(::flow::definitions::Node::from(data.clone()));
    //                uid_map.insert(n.uid, ix);
    //            }
    //        }
    //
    //        for n in from_list.as_slice(){
    //            if let Some(ref parent) = n.canvas{
    //                g.add_edge(uid_map[&parent.uid], uid_map[&n.uid],::ffi::EdgeKind::Canvas).unwrap();
    //            }
    //            if let Some(ref parent) = n.input{
    //                g.add_edge(uid_map[&parent.uid], uid_map[&n.uid],::ffi::EdgeKind::Input).unwrap();
    //            }
    //        }
    //        g
    //    }
}


#[test]
fn smoke_test_chaining(){

    let chain = fluently()
        .decode(0)
        .constrain_within(Some(1400), Some(1400), Some(s::ResampleHints::with(Some(s::Filter::CatmullRom), Some(40f32))))
        .flip_horizontal()
        .flip_vertical()
        .transpose()
        .rotate_90()
        .rotate_180()
        .rotate_270().encode(1, s::EncoderPreset::libpng32()).builder().to_framewise();
}


#[test]
fn smoke_test_many_operations(){

    let chain = fluently()
        .to(s::Node::Decode{io_id:0, commands: Some(vec![s::DecoderCommand::JpegDownscaleHints(s::JpegIDCTDownscaleHints{
            gamma_correct_for_srgb_during_spatial_luma_scaling: Some(false),
            scale_luma_spatially: Some(false),
            width: 1600,
            height:1600
        })])})
        .constrain_within(Some(1400), None,None)
        .constrain_within(Some(1400), Some(1400), Some(s::ResampleHints::with(Some(s::Filter::CatmullRom), Some(40f32))))
        .to(s::Node::Resample2D {
            w: 800,
            h: 800,
            hints: Some(s::ResampleHints {
                sharpen_percent: Some(10f32),
                background_color: None,
                resample_when: None,
                down_filter: Some(s::Filter::Robidoux),
                up_filter: Some(s::Filter::Ginseng),
                scaling_colorspace: None,
                sharpen_when: None
            }),
        })
        .to(s::Node::ApplyOrientation{flag: 7}).flip_horizontal().flip_vertical().transpose().rotate_90().rotate_180().rotate_270()
        .to(s::Node::FillRect {
            x1: 0,
            y1: 0,
            x2: 8,
            y2: 8,
            color: s::Color::Transparent,
        }).to(                              s::Node::ExpandCanvas {
        left: 10,
        top: 10,
        right: 10,
        bottom: 10,
        color: s::Color::Srgb(s::ColorSrgb::Hex("FFEECCFF".to_owned())),
    }).to(s::Node::Crop {
        x1: 10,
        y1: 10,
        x2: 650,
        y2: 490,
    }).encode(1, s::EncoderPreset::Libpng{
        depth: Some(s::PngBitDepth::Png24),
        matte: Some(s::Color::Srgb(s::ColorSrgb::Hex("9922FF".to_owned()))),
        zlib_compression: Some(7)
    });

    let framewise = chain.builder().to_framewise();
}
#[test]
fn smoke_test_graph_builder() {


    // let d = fluently().decode(0).flip_horizontal().rotate_90().
    let a = fluently()
        .to(s::Node::CreateCanvas {
            w: 200,
            h: 200,
            format: s::PixelFormat::Bgra32,
            color: s::Color::Black,
        })
        .to(s::Node::FlipV);
    let b = a.branch().to(s::Node::Encode {
        preset: s::EncoderPreset::libjpeg_turbo(),
        io_id: 0,
    });
    let c = a.branch()
        .to(s::Node::Resample2D {
            w: 100,
            h: 100,
            hints: None,
        })
        .to(s::Node::Encode {
            preset: s::EncoderPreset::libpng32(),
            io_id: 1,
        });
    b.builder().with(c).to_framewise();
}
