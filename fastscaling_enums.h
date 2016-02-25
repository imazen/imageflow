
typedef enum _StatusCode {
  No_Error = 0,
  Out_of_memory = 1,
  Invalid_BitmapBgra_dimensions,
  Invalid_BitmapFloat_dimensions,
  Unsupported_pixel_format,
  Invalid_internal_state,
  Transpose_not_permitted_in_place,
  Invalid_interpolation_filter,
  Invalid_argument,
  Null_argument,
  Interpolation_details_missing,
  Node_already_deleted,
  Edge_already_deleted,
  Graph_could_not_be_completed,
  Not_implemented,
  Invalid_inputs_to_node,
  Graph_not_flattened,
  Failed_to_open_file,
  Graph_could_not_be_executed,
  Png_decoding_failed,
  Png_encoding_failed,
  Graph_is_cyclic,
} StatusCode;

typedef enum _InterpolationFilter {
  Filter_RobidouxFast = 1,
  Filter_Robidoux = 2,
  Filter_RobidouxSharp = 3,
  Filter_Ginseng,
  Filter_GinsengSharp,
  Filter_Lanczos,
  Filter_LanczosSharp,
  Filter_Lanczos2,
  Filter_Lanczos2Sharp,
  Filter_CubicFast,
  Filter_Cubic,
  Filter_CubicSharp,
  Filter_CatmullRom,
  Filter_Mitchell,

  Filter_CubicBSpline,
  Filter_Hermite,
  Filter_Jinc,
  Filter_RawLanczos3,
  Filter_RawLanczos3Sharp,
  Filter_RawLanczos2,
  Filter_RawLanczos2Sharp,
  Filter_Triangle,
  Filter_Linear,
  Filter_Box,
  Filter_CatmullRomFast,
  Filter_CatmullRomFastSharp,

  Filter_Fastest,

  Filter_MitchellFast
} InterpolationFilter;

typedef enum _ProfilingEntryFlags {
  Profiling_start = 2,
  Profiling_start_allow_recursion = 6,
  Profiling_stop = 8,
  Profiling_stop_assert_started = 24,
  Profiling_stop_children = 56
} ProfilingEntryFlags;

typedef enum _BitmapPixelFormat {
  Bgr24 = 3,
  Bgra32 = 4,
  Gray8 = 1
} BitmapPixelFormat;

typedef enum _BitmapCompositingMode {
  Replace_self = 0,
  Blend_with_self = 1,
  Blend_with_matte = 2
} BitmapCompositingMode;

typedef enum _WorkingFloatspace {
  Floatspace_srgb = 0,
  Floatspace_as_is = 0,
  Floatspace_linear = 1,
  Floatspace_gamma = 2
} WorkingFloatspace;
