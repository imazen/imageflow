use libc::{c_void,c_float,int32_t,int64_t,size_t,uint32_t};

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

/*
 typedef bool (*flow_nodedef_fn_stringify)(flow_c * c, struct flow_graph * g, int32_t node_id, char * buffer,
                                          size_t buffer_size);

typedef bool (*flow_nodedef_fn_infobyte_count)(flow_c * c, struct flow_graph * g, int32_t node_id,
                                               int32_t * infobytes_count_out);

typedef bool (*flow_nodedef_fn_populate_dimensions)(flow_c * c, struct flow_graph * g, int32_t node_id,
                                                    bool force_estimate);

typedef bool (*flow_nodedef_fn_flatten)(flow_c * c, struct flow_graph ** graph_ref, int32_t node_id);

typedef bool (*flow_nodedef_fn_flatten_shorthand)(flow_c * c, struct flow_graph ** graph_ref, int32_t node_id,
                                                  struct flow_node * node, struct flow_node * input_node,
                                                  int32_t * first_replacement_node, int32_t * last_replacement_node);

typedef bool (*flow_nodedef_fn_execute)(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id);

typedef bool (*flow_nodedef_fn_estimate_cost)(flow_c * c, struct flow_job * job, struct flow_graph * g, int32_t node_id,
                                              size_t * bytes_required, size_t * cpu_cost);
*/

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct NodeDefinition {
    pub node_type: NodeType,
    pub input_count: int32_t,
    pub prohibit_output_edges: bool,
    pub canvas_count: int32_t,
    pub type_name: String,

    /*FIXME: put those in a trait
     flow_nodedef_fn_stringify stringify;
     flow_nodedef_fn_infobyte_count count_infobytes;
     int32_t nodeinfo_bytes_fixed;
     flow_nodedef_fn_populate_dimensions populate_dimensions;
     flow_nodedef_fn_flatten pre_optimize_flatten_complex;
     flow_nodedef_fn_flatten_shorthand pre_optimize_flatten;
     flow_nodedef_fn_flatten post_optimize_flatten_complex;
     flow_nodedef_fn_flatten_shorthand post_optimize_flatten;
     flow_nodedef_fn_execute execute;
     flow_nodedef_fn_estimate_cost estimate_cost;
     */
}

/// floating-point bitmap, typically linear RGBA, premultiplied
#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct BitmapFloat {
    /// buffer width in pixels
    w: uint32_t,
    /// buffer height in pixels
    h: uint32_t,
    /// The number of floats per pixel
    channels: uint32_t,
    /// The pixel data
    pixels: *mut c_float,
    /// If true, don't dispose the buffer with the struct
    pixels_borrowed: bool,
    /// The number of floats in the buffer
    float_count: uint32_t,
    /// The number of floats betwen (0,0) and (0,1)
    float_stride: uint32_t,

    /// If true, alpha has been premultiplied
    alpha_premultiplied: bool,
    /// If true, the alpha channel holds meaningful data
    alpha_meaningful: bool,
}

/** flow_context: Heap Manager **/

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct Heap {
    placeholder: u8,
    /*FIXME: fill in the rest
    flow_heap_calloc_function _calloc;
    flow_heap_malloc_function _malloc;
    flow_heap_realloc_function _realloc;
    flow_heap_free_function _free;
    flow_heap_terminate_function _context_terminate;
    void * _private_state;
*/
}

//struct flow_objtracking_info;
//void flow_context_objtracking_initialize(struct flow_objtracking_info * heap_tracking);
//void flow_context_objtracking_terminate(flow_c * c);

/** flow_context: struct flow_error_info **/

/*
struct flow_error_callstack_line {
    const char * file;
    int line;
    const char * function_name;
};
*/

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct ErrorInfo {
    placeholder: u8,
    /*FIXME: replace
    flow_status_code reason;
    struct flow_error_callstack_line callstack[14];
    int callstack_count;
    int callstack_capacity;
    bool locked;
    char message[FLOW_ERROR_MESSAGE_SIZE + 1];
*/
}

/*
#ifdef EXPOSE_SIGMOID
// flow_context: Colorspace
struct flow_SigmoidInfo {
    float constant;
    float x_coeff;
    float x_offset;
    float y_offset;
    float y_coeff;
};
#endif
*/

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct ColorspaceInfo {
    placeholder: u8,
    /*FIXME: replace
    float byte_to_float[256]; // Converts 0..255 -> 0..1, but knowing that 0.255 has sRGB gamma.
    flow_working_floatspace floatspace;
    bool apply_srgb;
    bool apply_gamma;
    float gamma;
    float gamma_inverse;
#ifdef EXPOSE_SIGMOID
    struct flow_SigmoidInfo sigmoid;
    bool apply_sigmoid;
#endif
*/
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct HeapObjectRecord {
    placeholder: u8,
    /*FIXME: fill in the rest
    void * ptr;
    size_t bytes;
    void * owner;
    flow_destructor_function destructor;
    bool destructor_called;
    const char * allocated_by;
    int allocated_by_line;
    bool is_owner;
*/
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct ObjTrackingInfo {
    pub allocs: HeapObjectRecord,
    pub next_free_slot: size_t,
    pub total_slots: size_t,
    pub bytes_allocated_net: size_t,
    pub bytes_allocated_gross: size_t,
    pub allocations_net: size_t,
    pub allocations_gross: size_t,
    pub bytes_free: size_t,
    pub allocations_net_peak: size_t,
    pub bytes_allocations_net_peak: size_t,
}


#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct CodecDefinition {
    placeholder: u8,
    //FIXME: replace
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct ContextCodecSet {
    //FIXME: replace with a Vec?
    codecs: *mut CodecDefinition,
    codecs_count: size_t,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct ContextNodeSet {
    //FIXME: replace with a Vec?
    codecs: *mut NodeDefinition,
    codecs_count: size_t,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct ProfilingLog {
    placeholder: u8,
    //FIXME: replace
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct Context {
    pub error: ErrorInfo,
    pub underlying_heap: Heap,
    pub log: ProfilingLog,
    pub colorspace: ColorspaceInfo,
    pub object_tracking: ObjTrackingInfo,
    pub codec_set: *mut ContextCodecSet,
    pub node_set:  *mut ContextNodeSet,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct NodeInfoIndex {
    pub index: int32_t,
}

#[repr(C)]
#[derive(Copy,Clone,Debug,PartialEq)]
pub enum CodecType {
  Null       = 0,
  DecodePng  = 1,
  EncodePng  = 2,
  DecodeJpeg = 3,
  EncodeJpeg = 4,
  DecodeGif  = 5,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
struct NodeInfoEncoderPlaceholder {
    index: NodeInfoIndex,
    codec_type: CodecType,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
struct NodeInfoCreateCanvas {
    format: PixelFormat,
    width: size_t,
    height: size_t,
    bgcolor: uint32_t,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
struct NodeInfoCrop {
    x1: uint32_t,
    x2: uint32_t,
    y1: uint32_t,
    y2: uint32_t,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
struct NodeInfoCopyRectToCanvas {
    x: uint32_t,
    y: uint32_t,
    from_x: uint32_t,
    from_y: uint32_t,
    width: uint32_t,
    height: uint32_t,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
struct NodeInfoExpandCanvas {
    left: uint32_t,
    top: uint32_t,
    right: uint32_t,
    bottom: uint32_t,
    canvas_color_srgb: uint32_t,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
struct NodeInfoFillRect {
    x1: uint32_t,
    y1: uint32_t,
    x2: uint32_t,
    y2: uint32_t,
    color_srgb: uint32_t,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
struct NodeInfoSize {
    width: int32_t,
    height: int32_t,
}

#[repr(C)]
#[derive(Copy,Clone,Debug,PartialEq)]
enum InterpolationFilter {
    RobidouxFast = 1,
    Robidoux = 2,
    RobidouxSharp = 3,
    Ginseng = 4,
    GinsengSharp = 5,
    Lanczos = 6,
    LanczosSharp = 7,
    Lanczos2 = 8,
    Lanczos2Sharp = 9,
    CubicFast = 10,
    Cubic = 11,
    CubicSharp = 12,
    CatmullRom = 13,
    Mitchell = 14,

    CubicBSpline = 15,
    Hermite = 16,
    Jinc = 17,
    RawLanczos3 = 18,
    RawLanczos3Sharp = 19,
    RawLanczos2 = 20,
    RawLanczos2Sharp = 21,
    Triangle = 22,
    Linear = 23,
    Box = 24,
    CatmullRomFast = 25,
    CatmullRomFastSharp = 26,

    Fastest = 27,

    MitchellFast = 28,

    NCubic = 29,

    NCubicSharp = 30
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
struct NodeInfoScale {
    width: int32_t,
    height: int32_t,
    downscale_filter: InterpolationFilter,
    upscale_filter:   InterpolationFilter,
    flags: size_t,
    sharpen: c_float,
}

#[repr(C)]
#[derive(Copy,Clone,Debug,PartialEq)]
enum ScaleFlags {
    None = 0,
    UseScale2d = 1,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct NodeInfoBitmapBgraPointer {
    ptr: *mut *mut BitmapBGRA,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct DecoderDownscaleHints {
    downscale_if_wider_than: int64_t,
    or_if_taller_than: int64_t,
    downscaled_min_width: int64_t,
    downscaled_min_height: int64_t,
    scale_luma_spatially: bool,
    gamma_correct_for_srgb_during_spatial_luma_scaling: bool
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct EncoderHints {
    jpeg_encode_quality: int32_t,
    disable_png_alpha: bool,
}

// If you want to know what kind of I/O structure is inside user_data, compare the read_func/write_func function
// pointers. No need for another human-assigned set of custom structure identifiers.
#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
struct FlowIO {
  placeholder: u8,
/*
    flow_c * context;
    flow_io_mode mode; // Call nothing, dereference nothing, if this is 0
    flow_io_read_function read_func; // Optional for write modes
    flow_io_write_function write_func; // Optional for read modes
    flow_io_position_function position_func; // Optional for sequential modes
    flow_io_seek_function seek_function; // Optional for sequential modes
    flow_destructor_function dispose_func; // Optional.
    void * user_data;
    int64_t optional_file_length; // Whoever sets up this structure can populate this value - or set it to -1 - as they
    // wish. useful for resource estimation.
*/
}

#[repr(C)]
#[derive(Copy,Clone,Debug,PartialEq)]
pub enum FlowDirection {
  Output = 8,
  Input = 4,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct CodecInstance {
    graph_placeholder_id: int32_t,
    codec_id: int64_t,
    codec_state: *mut c_void,
    io: *mut FlowIO,
    next: *mut CodecInstance,
    direction: FlowDirection,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct NodeInfoCodec {
    placeholder_id: int32_t,
    codec: *mut CodecInstance,
    // For encoders
    desired_encoder_id: int64_t,
    // For decoders
    downscale_hints: DecoderDownscaleHints,
    encoder_hints:   EncoderHints,
}
