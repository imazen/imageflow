use ffi::*;
use libc::{self, int32_t,uint32_t};
use std::ffi::CStr;
use petgraph;
use daggy::{Dag,EdgeIndex,NodeIndex};

#[repr(C)]
#[derive(Copy,Clone,Debug,PartialEq)]
pub enum NodeType {
    Null = 0,
    primitive_Flip_Vertical_Mutate = 1,
    primitive_Flip_Horizontal_Mutate = 2,
    primitive_Crop_Mutate_Alias = 3,
    primitive_CopyRectToCanvas = 4, // Overwrite only, no compositing
    Create_Canvas = 5,
    primitive_RenderToCanvas1D = 6,
    primitive_Scale2D_RenderToCanvas1D = 7,
    primitive_bitmap_bgra_pointer,
    primitive_decoder,
    primitive_encoder,

    Fill_Rect_Mutate,
    non_primitive_nodes_begin = 256,

    Expand_Canvas,
    Transpose,
    Flip_Vertical,
    Flip_Horizontal,
    Render1D,
    Crop,
    Apply_Orientation,
    non_optimizable_nodes_begin = 512,

    Clone,
    decoder,
    encoder,

    Rotate_90,
    Rotate_180,
    Rotate_270,
    Scale, //(preserve colorspace), interpolation filter
    Noop,

    // Not implemented below here:
    Rotate_Flip_Per_Orientation,
    Crop_Percentage,
    Crop_Percentage_Infinite_Canvas, // canvas_color
    Crop_Rectangle,
    Constrain, //(mode=pad|max|crop|stretch) (width, height) (scale=down|up|both|canvas) (anchor=9 points)
    Matte,
    EnlargeCanvas,
    Sharpen,
    Blur,
    Convolve_Custom,
    AdjustContrast,
    AdjustSaturation,
    AdjustBrightness,
    CropWhitespace, // tolerances and padding
    Opacity,
    Sepia,
    Grayscale, // true|y|ry|ntsc|bt709|flat
    DrawImage,
    RemoveNoise,
    ColorMatrixsRGB,
    _FORCE_ENUM_SIZE_INT32 = 2147483647,
}

#[repr(C)]
#[derive(Copy,Clone,Debug,PartialEq)]
pub enum NodeState {
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

#[repr(C)]
#[derive(Copy,Clone,Debug,PartialEq)]
pub enum PixelFormat {
  Gray8  = 1,
  BGR24  = 3,
  BGRA32 = 4,
}

#[repr(C)]
#[derive(Copy,Clone,Debug,PartialEq)]
pub enum EdgeType {
  Null   = 0,
  Input  = 1,
  Canvas = 2,
  info   = 3,
  FORCE_ENUM_SIZE_INT32 = 2147483647,
}

#[repr(C)]
#[derive(Copy,Clone,Debug,PartialEq)]
pub enum BitmapCompositingMode {
  ReplaceSelf    = 0,
  BlendWithSelf  = 1,
  BlendWithMatte = 2,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct BitmapBGRA {
  /// bitmap width in pixels
  pub w: uint32_t,
  /// bitmap height in pixels
  pub h: uint32_t,
  /// byte length of each row (may include any amount of padding)
  pub stride: uint32_t,
  //FIXME: replace with a vec or slice
  ///pointer to pixel 0,0; should be of length > h * stride
  pub pixels: *mut u8,
  /// If true, we don't dispose of *pixels when we dispose the struct
  pub borrowed_pixels: bool,
  /// If false, we can even ignore the alpha channel on 4bpp
  pub alpha_meaningful: bool,
  /// If false, we can edit pixels without affecting the stride
  pub pixels_readonly: bool,
  ///If false, we can change the stride of the image
  pub stride_readonly: bool,
  /// If true, we can reuse the allocated memory for other purposes
  pub can_reuse_space: bool,
  pub fmt: PixelFormat,
  ///When using compositing mode blend_with_matte, this color will be used. We should probably define this as
  ///always being sRGBA, 4 bytes.
  pub matte_color: [u8;4],

  pub compositing_mode: BitmapCompositingMode,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct Node {
  pub node_type: NodeType,
  pub state:     NodeState,
  pub result_width: int32_t,
  pub result_height: int32_t,
  pub result_format: PixelFormat,
  pub result_alpha_meaningful: bool,
  pub result_bitmap: BitmapBGRA,
  pub ticks_elapsed: uint32_t,

}
pub type Graph = Dag<Node,EdgeKind>;

pub fn print_to_stdout(c: *mut Context, g: &Graph) -> bool {
    true
}

pub fn create(context: *mut Context,
                         max_edges: u32,
                         max_nodes: u32,
                         max_info_bytes: u32,
                         growth_factor: f32)
                         -> Graph {
    Graph::with_capacity(max_nodes as usize, max_edges as usize)
}

pub fn edge_create(c: *mut Context,
                   g: &mut Graph,
                   from: i32,
                   to: i32,
                   kind: EdgeKind) -> i32 {
  //edges are nodeindex, not just u32
  //FIXME: error management. We should return something else than i32
  // we should also get index as U32 instead of i32
  g.add_edge(NodeIndex::new(from as usize), NodeIndex::new(to as usize), kind).unwrap_or(EdgeIndex::new(0usize)).index() as i32
}

pub fn node_create_decoder(c: *mut Context,
                                g: &mut Graph,
                                prev_node: i32,
                                placeholder_id: i32) -> i32 {
    0
}

pub fn node_create_canvas(c: *mut Context,
                                   g: &mut Graph,
                                   prev_node: i32,
                                   format: PixelFormat,
                                   width: usize,
                                   height: usize,
                                   bgcolor: u32)
                                   -> i32 {
    0
}

pub fn node_create_scale(c: *mut Context,
                                  g: &mut Graph,
                                  prev_node: i32,
                                  width: usize,
                                  height: usize,
                                  downscale_filter: i32,
                                  upscale_filter: i32,
                                  flags: usize,
                                  sharpen: f32)
                                  -> i32 {
    0
}

pub fn node_create_expand_canvas(c: *mut Context,
                                          g: &mut Graph,
                                          prev_node: i32,
                                          left: u32,
                                          top: u32,
                                          right: u32,
                                          bottom: u32,
                                          canvas_color_srgb: u32)
                                          -> i32 {
    0
}

pub fn node_create_fill_rect(c: *mut Context,
                                      g: &mut Graph,
                                      prev_node: i32,
                                      x1: u32,
                                      y1: u32,
                                      x2: u32,
                                      y2: u32,
                                      color_srgb: u32)
                                      -> i32 { 0 }

pub fn node_create_bitmap_bgra_reference(c: *mut Context,
                                              g: &mut Graph,
                                              prev_node: i32, reference: *mut *mut FlowBitmapBgra) -> i32 { 0 }
pub fn node_create_rotate_90(c: *mut Context, g: &mut Graph, prev_node: i32) -> i32 { 0 }
pub fn node_create_rotate_180(c: *mut Context, g: &mut Graph, prev_node: i32) -> i32 { 0 }
pub fn node_create_rotate_270(c: *mut Context, g: &mut Graph, prev_node: i32) -> i32 { 0 }

pub fn node_create_transpose(c: *mut Context, g: &mut Graph, prev_node: i32) -> i32 { 0 }

pub fn node_create_primitive_copy_rect_to_canvas(c: *mut Context,
                                                          g: &mut Graph,
                                                          prev_node: i32,
                                                          from_x: u32,
                                                          from_y: u32,
                                                          width: u32,
                                                          height: u32,
                                                          x: u32,
                                                          y: u32)
                                                          -> i32 { 0 }

pub fn node_create_encoder(c: *mut Context,
                                    g: &mut Graph,
                                    prev_node: i32,
                                    placeholder_id: i32,
                                    desired_encoder_id: i64,
                                    hints: *const EncoderHints)
                                    -> i32 { 0 }

pub fn node_create_primitive_flip_vertical(c: *mut Context,
                                                    g: &mut Graph,
                                                    prev_node: i32)
                                                    -> i32 { 0 }

pub fn node_create_primitive_flip_horizontal(c: *mut Context,
                                                      g: &mut Graph,
                                                      prev_node: i32)
                                                      -> i32 { 0 }

pub fn node_create_primitive_crop(c: *mut Context,
                                           g: &mut Graph,
                                           prev_node: i32,
                                           x1: u32,
                                           y1: u32,
                                           x2: u32,
                                           y2: u32)
                                           -> i32 { 0 }

extern "C" {
/*
    fn flow_graph_print_to_stdout(c: *mut Context, g: *const Graph) -> bool;

    fn flow_graph_create(context: *mut Context,
                             max_edges: u32,
                             max_nodes: u32,
                             max_info_bytes: u32,
                             growth_factor: f32)
                             -> *mut Graph;


    fn flow_edge_create(c: *mut Context,
                            g: *mut *mut Graph,
                            from: i32,
                            to: i32,
                            kind: EdgeKind)
                            -> i32;
    fn flow_node_create_decoder(c: *mut Context,
                                    g: *mut *mut Graph,
                                    prev_node: i32,
                                    placeholder_id: i32)
                                    -> i32;
    fn flow_node_create_canvas(c: *mut Context,
                                   g: *mut *mut Graph,
                                   prev_node: i32,
                                   format: PixelFormat,
                                   width: usize,
                                   height: usize,
                                   bgcolor: u32)
                                   -> i32;

    fn flow_node_create_scale(c: *mut Context,
                                  g: *mut *mut Graph,
                                  prev_node: i32,
                                  width: usize,
                                  height: usize,
                                  downscale_filter: i32,
                                  upscale_filter: i32,
                                  flags: usize,
                                  sharpen: f32)
                                  -> i32;

    fn flow_node_create_expand_canvas(c: *mut Context,
                                          g: *mut *mut Graph,
                                          prev_node: i32,
                                          left: u32,
                                          top: u32,
                                          right: u32,
                                          bottom: u32,
                                          canvas_color_srgb: u32)
                                          -> i32;

    fn flow_node_create_fill_rect(c: *mut Context,
                                      g: *mut *mut Graph,
                                      prev_node: i32,
                                      x1: u32,
                                      y1: u32,
                                      x2: u32,
                                      y2: u32,
                                      color_srgb: u32)
                                      -> i32;

pub fn flow_node_create_bitmap_bgra_reference(c: *mut Context,
                                              g: *mut *mut Graph,
                                              prev_node: i32, reference: *mut *mut FlowBitmapBgra) -> i32 { 0 }
    fn flow_node_create_rotate_90(c: *mut Context, g: *mut *mut Graph, prev_node: i32) -> i32;
    fn flow_node_create_rotate_180(c: *mut Context, g: *mut *mut Graph, prev_node: i32) -> i32;
    fn flow_node_create_rotate_270(c: *mut Context, g: *mut *mut Graph, prev_node: i32) -> i32;

    fn flow_node_create_transpose(c: *mut Context, g: *mut *mut Graph, prev_node: i32) -> i32;

    fn flow_node_create_primitive_copy_rect_to_canvas(c: *mut Context,
                                                          g: *mut *mut Graph,
                                                          prev_node: i32,
                                                          from_x: u32,
                                                          from_y: u32,
                                                          width: u32,
                                                          height: u32,
                                                          x: u32,
                                                          y: u32)
                                                          -> i32;

    fn flow_node_create_encoder(c: *mut Context,
                                    g: *mut *mut Graph,
                                    prev_node: i32,
                                    placeholder_id: i32,
                                    desired_encoder_id: i64,
                                    hints: *const EncoderHints)
                                    -> i32;

    fn flow_node_create_primitive_flip_vertical(c: *mut Context,
                                                    g: *mut *mut Graph,
                                                    prev_node: i32)
                                                    -> i32;

    fn flow_node_create_primitive_flip_horizontal(c: *mut Context,
                                                      g: *mut *mut Graph,
                                                      prev_node: i32)
                                                      -> i32;

    fn flow_node_create_primitive_crop(c: *mut Context,
                                           g: *mut *mut Graph,
                                           prev_node: i32,
                                           x1: u32,
                                           y1: u32,
                                           x2: u32,
                                           y2: u32)
                                           -> i32;
*/
}

