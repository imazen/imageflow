

typedef signed char  int8_t;
typedef short int  int16_t;
typedef int   int32_t;
# if __WORDSIZE == 64
typedef long int  int64_t;
# else
__extension__
typedef long long int  int64_t;
# endif


/* Unsigned.  */
typedef unsigned char  uint8_t;
typedef unsigned short int uint16_t;
typedef unsigned int  uint32_t;
#if __WORDSIZE == 64
typedef unsigned long int uint64_t;
#else
__extension__
typedef unsigned long long int uint64_t;
#endif



/* Includes the header in the wrapper code */
#include "../imageflow.h"

  flow_ntype_Null = 0
  flow_ntype_primitive_Flip_Vertical_Mutate = 1
  flow_ntype_primitive_Flip_Horizontal_Mutate = 1
  flow_ntype_primitive_bitmap_bgra_pointer = 1
  flow_ntype_primitive_decoder = 1
  flow_ntype_primitive_encoder = 1
  flow_ntype_Transpose = 1
  flow_ntype_Flip_Vertical = 1
  flow_ntype_Flip_Horizontal = 1
  flow_ntype_Render1D = 1
  flow_ntype_Crop = 1
  flow_ntype_Clone = 1
  flow_ntype_decoder = 1
  flow_ntype_encoder = 1
  flow_ntype_Rotate_90 = 1
  flow_ntype_Rotate_180 = 1
  flow_ntype_Rotate_270 = 1
  flow_ntype_Scale = 1
  flow_ntype_Rotate_Flip_Per_Orientation = 1
  flow_ntype_Crop_Percentage = 1
  flow_ntype_Crop_Percentage_Infinite_Canvas = 1
  flow_ntype_Crop_Rectangle = 1
  flow_ntype_Constrain = 1
  flow_ntype_Matte = 1
  flow_ntype_EnlargeCanvas = 1
  flow_ntype_Sharpen = 1
  flow_ntype_Blur = 1
  flow_ntype_Convolve_Custom = 1
  flow_ntype_AdjustContrast = 1
  flow_ntype_AdjustSaturation = 1
  flow_ntype_AdjustBrightness = 1
  flow_ntype_CropWhitespace = 1
  flow_ntype_Opacity = 1
  flow_ntype_Sepia = 1
  flow_ntype_Grayscale = 1
  flow_ntype_DrawImage = 1
  flow_ntype_RemoveNoise = 1
  flow_ntype_ColorMatrixsRGB = 1
  flow_ntype_Resource_Placeholder = 1
  flow_ntype_Encoder_Placeholder = 1
  flow_ntype_primitive_Crop_Mutate_Alias = 2
  flow_ntype__FORCE_ENUM_SIZE_INT32 = 2147483647
  flow_ntype_non_primitive_nodes_begin = 256
  flow_ntype_primitive_CopyRectToCanvas = 3
  flow_ntype_Create_Canvas = 4
  flow_ntype_primitive_RenderToCanvas1D = 5
  flow_ntype_non_optimizable_nodes_begin = 512

  flow_node_state_Blank = 0
  flow_node_state_InputDimensionsKnown = 1
  flow_node_state_ReadyForPreOptimizeFlatten = 1
  flow_node_state_InputsExecuted = 16
  flow_node_state_PreOptimizeFlattened = 2
  flow_node_state_ReadyForOptimize = 3
  flow_node_state_ReadyForExecution = 31
  flow_node_state_Executed = 32
  flow_node_state_Optimized = 4
  flow_node_state_Done = 63
  flow_node_state_ReadyForPostOptimizeFlatten = 7
  flow_node_state_PostOptimizeFlattened = 8

  flow_edgetype_null = 0
  flow_edgetype_input = 1
  flow_edgetype_canvas = 1
  flow_edgetype_info = 1
  flow_edgetype_FORCE_ENUM_SIZE_INT32 = 2147483647

  flow_compositing_mode_overwrite = 0
  flow_compositing_mode_compose = 1
  flow_compositing_mode_blend_with_matte = 1

  flow_job_resource_type_bitmap_bgra = 1
  flow_job_resource_type_buffer = 2

  flow_codec_type_null = 0
  flow_codec_type_bitmap_bgra_pointer = 1
  flow_codec_type_decode_png = 1
  flow_codec_type_encode_png = 1
  flow_codec_type_decode_jpeg = 1
  flow_codec_type_encode_jpeg = 1

  flow_scanlines_filter_Sharpen = 0
  flow_scanlines_filter_Blur = 1
  flow_scanlines_filter_Convolve = 1
  flow_scanlines_filter_ColorMatrix = 1
  flow_scanlines_filter_ToLinear = 1
  flow_scanlines_filter_ToSrgb = 1
  flow_scanlines_filter_Custom = 1
  flow_scanlines_filter__FORCE_ENUM_SIZE_INT32 = 2147483647

  flow_status_No_Error = 0
  flow_status_Out_of_memory = 1
  flow_status_Invalid_dimensions = 1
  flow_status_Invalid_dimensions = 1
  flow_status_Unsupported_pixel_format = 1
  flow_status_Invalid_internal_state = 1
  flow_status_Invalid_argument = 1
  flow_status_Invalid_argument = 1
  flow_status_Invalid_argument = 1
  flow_status_Null_argument = 1
  flow_status_Invalid_argument = 1
  flow_status_Item_does_not_exist = 1
  flow_status_Item_does_not_exist = 1
  flow_status_Graph_invalid = 1
  flow_status_Not_implemented = 1
  flow_status_Invalid_inputs_to_node = 1
  flow_status_Graph_not_flattened = 1
  flow_status_IO_error = 1
  flow_status_Maximum_graph_passes_exceeded = 1
  flow_status_Image_decoding_failed = 1
  flow_status_Image_encoding_failed = 1
  flow_status_Image_decoding_failed = 1
  flow_status_Image_encoding_failed = 1
  flow_status_Graph_is_cyclic = 1

  flow_interpolation_filter_RobidouxFast = 1
  flow_interpolation_filter_Ginseng = 1
  flow_interpolation_filter_GinsengSharp = 1
  flow_interpolation_filter_Lanczos = 1
  flow_interpolation_filter_LanczosSharp = 1
  flow_interpolation_filter_Lanczos2 = 1
  flow_interpolation_filter_Lanczos2Sharp = 1
  flow_interpolation_filter_CubicFast = 1
  flow_interpolation_filter_Cubic = 1
  flow_interpolation_filter_CubicSharp = 1
  flow_interpolation_filter_CatmullRom = 1
  flow_interpolation_filter_Mitchell = 1
  flow_interpolation_filter_CubicBSpline = 1
  flow_interpolation_filter_Hermite = 1
  flow_interpolation_filter_Jinc = 1
  flow_interpolation_filter_RawLanczos3 = 1
  flow_interpolation_filter_RawLanczos3Sharp = 1
  flow_interpolation_filter_RawLanczos2 = 1
  flow_interpolation_filter_RawLanczos2Sharp = 1
  flow_interpolation_filter_Triangle = 1
  flow_interpolation_filter_Linear = 1
  flow_interpolation_filter_Box = 1
  flow_interpolation_filter_CatmullRomFast = 1
  flow_interpolation_filter_CatmullRomFastSharp = 1
  flow_interpolation_filter_Fastest = 1
  flow_interpolation_filter_MitchellFast = 1
  flow_interpolation_filter_Robidoux = 2
  flow_interpolation_filter_RobidouxSharp = 3

  flow_profiling_entry_start = 2
  flow_profiling_entry_stop_assert_started = 24
  flow_profiling_entry_stop_children = 56
  flow_profiling_entry_start_allow_recursion = 6
  flow_profiling_entry_stop = 8

  flow_gray8 = 1
  flow_bgr24 = 3
  flow_bgra32 = 4

  flow_bitmap_compositing_replace_self = 0
  flow_bitmap_compositing_blend_with_self = 1
  flow_bitmap_compositing_blend_with_matte = 2

  flow_working_floatspace_srgb = 0
  flow_working_floatspace_as_is = 0
  flow_working_floatspace_linear = 1
  flow_working_floatspace_gamma = 2

  class FlowProfilingEntry < FFI::Struct
    layout(
           :time, int64_t,
           :name, :pointer,
           :flags, :int
    )
    def name=(str)
      @name = FFI::MemoryPointer.from_string(str)
      self[:name] = @name
    end
    def name
      @name.get_string(0)
    end

  end
  class FlowProfilingLog < FFI::Struct
    layout(
           :log, :pointer,
           :count, uint32_t,
           :capacity, uint32_t,
           :ticks_per_second, int64_t
    )
  end
  attach_function :flow_context_get_profiler_log, [ :pointer ], :pointer
  attach_function :flow_context_create, [  ], :pointer
  attach_function :flow_context_destroy, [ :pointer ], :void
  attach_function :flow_context_free_all_allocations, [ :pointer ], :void
  attach_function :flow_context_print_memory_info, [ :pointer ], :void
  attach_function :flow_context_error_message, [ :pointer, :string, :uint ], :string
  attach_function :flow_context_stacktrace, [ :pointer, :string, :uint ], :string
  attach_function :flow_context_has_error, [ :pointer ], bool
  attach_function :flow_context_error_reason, [ :pointer ], :int
  attach_function :flow_context_free_static_caches, [  ], :void
  attach_function :flow_context_print_and_exit_if_err, [ :pointer ], bool
  attach_function :flow_context_clear_error, [ :pointer ], :void
  attach_function :flow_context_print_error_to, [ :pointer, :pointer ], :void
  class FlowBitmapBgraStruct < FFI::Struct
    layout(
           :w, uint32_t,
           :h, uint32_t,
           :stride, uint32_t,
           :pixels, :pointer,
           :borrowed_pixels, bool,
           :alpha_meaningful, bool,
           :pixels_readonly, bool,
           :stride_readonly, bool,
           :can_reuse_space, bool,
           :fmt, :int,
           :matte_color, [uint8_t, 4],
           :compositing_mode, :int
    )
  end
  attach_function :flow_context_byte_to_floatspace, [ :pointer, uint8_t ], :float
  attach_function :flow_context_floatspace_to_byte, [ :pointer, :float ], uint8_t
  attach_function :flow_context_set_floatspace, [ :pointer, :int, :float, :float, :float ], :void
  callback(:flow_detailed_interpolation_method, [ :pointer, :double ], :double)
  class FlowInterpolationDetailsStruct < FFI::Struct
    layout(
           :window, :double,
           :p1, :double,
           :p2, :double,
           :p3, :double,
           :q1, :double,
           :q2, :double,
           :q3, :double,
           :q4, :double,
           :blur, :double,
           :filter, :flow_detailed_interpolation_method,
           :sharpen_percent_goal, :float
    )
    def filter=(cb)
      @filter = cb
      self[:filter] = @filter
    end
    def filter
      @filter
    end

  end
  class FlowConvolutionKernel < FFI::Struct
    layout(
           :kernel, :pointer,
           :width, uint32_t,
           :radius, uint32_t,
           :threshold_min_change, :float,
           :threshold_max_change, :float,
           :buffer, :pointer
    )
  end
  class FlowRenderDetailsStruct < FFI::Struct
    layout(
           :interpolation, :pointer,
           :minimum_sample_window_to_interposharpen, :float,
           :interpolate_last_percent, :float,
           :havling_acceptable_pixel_loss, :float,
           :halving_divisor, uint32_t,
           :kernel_a, :pointer,
           :kernel_b, :pointer,
           :sharpen_percent_goal, :float,
           :apply_color_matrix, bool,
           :color_matrix_data, [:float, 25],
           :color_matrix, [:pointer, 5],
           :post_transpose, bool,
           :post_flip_x, bool,
           :post_flip_y, bool,
           :enable_profiling, bool
    )
  end
  attach_function :flow_bitmap_bgra_create, [ :pointer, :int, :int, bool, :int ], :pointer
  attach_function :flow_bitmap_bgra_create_header, [ :pointer, :int, :int ], :pointer
  attach_function :flow_bitmap_bgra_destroy, [ :pointer, :pointer ], :void
  attach_function :flow_bitmap_bgra_flip_horizontal, [ :pointer, :pointer ], bool
  attach_function :flow_bitmap_bgra_compare, [ :pointer, :pointer, :pointer, :pointer ], bool
  attach_function :flow_RenderDetails_create, [ :pointer ], :pointer
  attach_function :flow_RenderDetails_create_with, [ :pointer, :int ], :pointer
  attach_function :flow_RenderDetails_render, [ :pointer, :pointer, :pointer, :pointer ], bool
  attach_function :flow_RenderDetails_render_in_place, [ :pointer, :pointer, :pointer ], bool
  attach_function :flow_RenderDetails_destroy, [ :pointer, :pointer ], :void
  attach_function :flow_interpolation_filter_exists, [ :int ], bool
  attach_function :flow_interpolation_details_create, [ :pointer ], :pointer
  attach_function :flow_interpolation_details_create_bicubic_custom, [ :pointer, :double, :double, :double, :double ], :pointer
  attach_function :flow_interpolation_details_create_custom, [ :pointer, :double, :double, :flow_detailed_interpolation_method ], :pointer
  attach_function :flow_interpolation_details_create_from, [ :pointer, :int ], :pointer
  attach_function :flow_interpolation_details_percent_negative_weight, [ :pointer ], :double
  attach_function :flow_interpolation_details_destroy, [ :pointer, :pointer ], :void
  attach_function :flow_pixel_format_bytes_per_pixel, [ :int ], uint32_t
  class FlowInterpolationPixelContributions < FFI::Struct
    layout(
           :Weights, :pointer,
           :Left, :int,
           :Right, :int
    )
  end
  class FlowInterpolationLineContributions < FFI::Struct
    layout(
           :ContribRow, :pointer,
           :WindowSize, uint32_t,
           :LineLength, uint32_t,
           :percent_negative, :double
    )
  end
  attach_function :flow_interpolation_line_contributions_create, [ :pointer, uint32_t, uint32_t, :pointer ], :pointer
  attach_function :flow_interpolation_line_contributions_destroy, [ :pointer, :pointer ], :void
  attach_function :flow_convolution_kernel_create, [ :pointer, uint32_t ], :pointer
  attach_function :flow_convolution_kernel_destroy, [ :pointer, :pointer ], :void
  attach_function :flow_convolution_kernel_create_guassian, [ :pointer, :double, uint32_t ], :pointer
  attach_function :flow_convolution_kernel_sum, [ :pointer ], :double
  attach_function :flow_convolution_kernel_normalize, [ :pointer, :float ], :void
  attach_function :flow_convolution_kernel_create_gaussian_normalized, [ :pointer, :double, uint32_t ], :pointer
  attach_function :flow_convolution_kernel_create_guassian_sharpen, [ :pointer, :double, uint32_t ], :pointer
  attach_function :flow_bitmap_bgra_populate_histogram, [ :pointer, :pointer, :pointer, uint32_t, uint32_t, :pointer ], bool
  class FlowScanlinesFilter < FFI::Struct
    layout(
           :type, :int,
           :next, :pointer
    )
  end
  class FlowEdge < FFI::Struct
    layout(
           :type, :int,
           :from, int32_t,
           :to, int32_t,
           :from_width, int32_t,
           :from_height, int32_t,
           :from_format, :int,
           :from_alpha_meaningful, bool,
           :info_byte_index, int32_t,
           :info_bytes, int32_t
    )
  end
  class FlowNode < FFI::Struct
    layout(
           :type, :int,
           :info_byte_index, int32_t,
           :info_bytes, int32_t,
           :state, :int,
           :result_bitmap, :pointer,
           :ticks_elapsed, uint32_t
    )
  end
  class FlowGraph < FFI::Struct
    layout(
           :memory_layout_version, uint32_t,
           :edges, :pointer,
           :edge_count, int32_t,
           :next_edge_id, int32_t,
           :max_edges, int32_t,
           :nodes, :pointer,
           :node_count, int32_t,
           :next_node_id, int32_t,
           :max_nodes, int32_t,
           :info_bytes, :pointer,
           :max_info_bytes, int32_t,
           :next_info_byte, int32_t,
           :deleted_bytes, int32_t,
           :growth_factor, :float
    )
  end
  attach_function :flow_graph_create, [ :pointer, uint32_t, uint32_t, uint32_t, :float ], :pointer
  attach_function :flow_graph_destroy, [ :pointer, :pointer ], :void
  attach_function :flow_graph_replace_if_too_small, [ :pointer, :pointer, uint32_t, uint32_t, uint32_t ], bool
  attach_function :flow_graph_copy_and_resize, [ :pointer, :pointer, uint32_t, uint32_t, uint32_t ], :pointer
  attach_function :flow_graph_copy_info_bytes_to, [ :pointer, :pointer, :pointer, int32_t, int32_t ], int32_t
  attach_function :flow_edge_duplicate, [ :pointer, :pointer, int32_t ], int32_t
  attach_function :flow_node_create_canvas, [ :pointer, :pointer, int32_t, :int, :uint, :uint, uint32_t ], int32_t
  attach_function :flow_node_create_scale, [ :pointer, :pointer, int32_t, :uint, :uint ], int32_t
  attach_function :flow_node_create_primitive_flip_vertical, [ :pointer, :pointer, int32_t ], int32_t
  attach_function :flow_node_create_primitive_flip_horizontal, [ :pointer, :pointer, int32_t ], int32_t
  attach_function :flow_node_create_clone, [ :pointer, :pointer, int32_t ], int32_t
  attach_function :flow_node_create_transpose, [ :pointer, :pointer, int32_t ], int32_t
  attach_function :flow_node_create_rotate_90, [ :pointer, :pointer, int32_t ], int32_t
  attach_function :flow_node_create_rotate_180, [ :pointer, :pointer, int32_t ], int32_t
  attach_function :flow_node_create_rotate_270, [ :pointer, :pointer, int32_t ], int32_t
  attach_function :flow_node_create_resource_placeholder, [ :pointer, :pointer, int32_t, int32_t ], int32_t
  attach_function :flow_node_create_encoder_placeholder, [ :pointer, :pointer, int32_t, int32_t, :int ], int32_t
  attach_function :flow_node_create_resource_bitmap_bgra, [ :pointer, :pointer, int32_t, :pointer ], int32_t
  attach_function :flow_node_create_primitive_copy_rect_to_canvas, [ :pointer, :pointer, int32_t, uint32_t, uint32_t, uint32_t, uint32_t, uint32_t, uint32_t ], int32_t
  attach_function :flow_node_create_primitive_crop, [ :pointer, :pointer, int32_t, uint32_t, uint32_t, uint32_t, uint32_t ], int32_t
  attach_function :flow_node_create_render_to_canvas_1d, [ :pointer, :pointer, int32_t, bool, uint32_t, uint32_t, int32_t, :int, :float, :int, [:pointer, 4], :pointer, :int ], :pointer
  attach_function :flow_node_delete, [ :pointer, :pointer, int32_t ], bool
  attach_function :flow_edge_delete, [ :pointer, :pointer, int32_t ], bool
  attach_function :flow_edge_delete_all_connected_to_node, [ :pointer, :pointer, int32_t ], bool
  attach_function :flow_graph_get_inbound_edge_count_of_type, [ :pointer, :pointer, int32_t, :int ], int32_t
  attach_function :flow_graph_get_first_inbound_edge_of_type, [ :pointer, :pointer, int32_t, :int ], int32_t
  attach_function :flow_edge_has_dimensions, [ :pointer, :pointer, int32_t ], bool
  attach_function :flow_node_input_edges_have_dimensions, [ :pointer, :pointer, int32_t ], bool
  attach_function :flow_graph_duplicate_edges_to_another_node, [ :pointer, :pointer, int32_t, int32_t, bool, bool ], bool
  attach_function :flow_edge_create, [ :pointer, :pointer, int32_t, int32_t, :int ], int32_t
  callback(:flow_graph_visitor, [ :pointer, :pointer, :pointer, int32_t, :pointer, :pointer, :pointer ], bool)
  attach_function :flow_graph_walk, [ :pointer, :pointer, :pointer, :flow_graph_visitor, :flow_graph_visitor, :pointer ], bool
  class FlowNodeinfoIndex < FFI::Struct
    layout(
           :index, int32_t
    )
  end
  class FlowNodeinfoEncoderPlaceholder < FFI::Struct
    layout(
           :index, FlowNodeinfoIndex,
           :codec_type, :int
    )
  end
  class FlowNodeinfoCreatecanvas < FFI::Struct
    layout(
           :format, :int,
           :width, :uint,
           :height, :uint,
           :bgcolor, uint32_t
    )
  end
  class FlowNodeinfoCrop < FFI::Struct
    layout(
           :x1, uint32_t,
           :x2, uint32_t,
           :y1, uint32_t,
           :y2, uint32_t
    )
  end
  class FlowNodeinfoCopyRectToCanvas < FFI::Struct
    layout(
           :x, uint32_t,
           :y, uint32_t,
           :from_x, uint32_t,
           :from_y, uint32_t,
           :width, uint32_t,
           :height, uint32_t
    )
  end
  class FlowNodeinfoSize < FFI::Struct
    layout(
           :width, :uint,
           :height, :uint
    )
  end
  class FlowNodeinfoResourceBitmapBgra < FFI::Struct
    layout(
           :ref, :pointer
    )
  end
  class FlowNodeinfoCodec < FFI::Struct
    layout(
           :codec_state, :pointer,
           :type, :int
    )
  end
  class FlowNodeinfoRenderToCanvas1d < FFI::Struct
    layout(
           :interpolation_filter, :int,
           :scale_to_width, int32_t,
           :canvas_x, uint32_t,
           :canvas_y, uint32_t,
           :transpose_on_write, bool,
           :scale_in_colorspace, :int,
           :sharpen_percent_goal, :float,
           :compositing_mode, :int,
           :matte_color, [uint8_t, 4],
           :filter_list, :pointer
    )
  end
  attach_function :flow_node_execute_render_to_canvas_1d, [ :pointer, :pointer, :pointer, :pointer, :pointer ], bool
  attach_function :flow_node_create_render1d, [ :pointer, :pointer, int32_t, bool, int32_t, :int, :float, :pointer, :int ], int32_t
  FLOW_INPUT = 4
  FLOW_OUTPUT = 8

  attach_function :flow_job_create, [ :pointer ], :pointer
  attach_function :flow_job_destroy, [ :pointer, :pointer ], :void
  attach_function :flow_job_configure_recording, [ :pointer, :pointer, bool, bool, bool, bool, bool ], bool
  attach_function :flow_job_insert_resources_into_graph, [ :pointer, :pointer, :pointer ], bool
  attach_function :flow_job_populate_dimensions_where_certain, [ :pointer, :pointer, :pointer ], bool
  attach_function :flow_job_force_populate_dimensions, [ :pointer, :pointer, :pointer ], bool
  attach_function :flow_job_execute_where_certain, [ :pointer, :pointer, :pointer ], bool
  attach_function :flow_job_graph_fully_executed, [ :pointer, :pointer, :pointer ], bool
  attach_function :flow_job_notify_graph_changed, [ :pointer, :pointer, :pointer ], bool
  attach_function :flow_job_execute, [ :pointer, :pointer, :pointer ], bool
  attach_function :flow_graph_post_optimize_flatten, [ :pointer, :pointer, :pointer ], bool
  attach_function :flow_graph_optimize, [ :pointer, :pointer, :pointer ], bool
  attach_function :flow_graph_pre_optimize_flatten, [ :pointer, :pointer ], bool
  attach_function :flow_graph_get_edge_count, [ :pointer, :pointer, int32_t, bool, :int, bool, bool ], int32_t
  attach_function :flow_graph_validate, [ :pointer, :pointer ], bool
  attach_function :flow_job_add_bitmap_bgra, [ :pointer, :pointer, :int, int32_t, :pointer ], int32_t
  attach_function :flow_job_add_buffer, [ :pointer, :pointer, :int, int32_t, :pointer, :uint, bool ], int32_t
  attach_function :flow_node_create_generic, [ :pointer, :pointer, int32_t, :int ], int32_t
  attach_function :flow_graph_print_to_dot, [ :pointer, :pointer, :pointer, :string ], bool
  attach_function :flow_job_get_bitmap_bgra, [ :pointer, :pointer, int32_t ], :pointer
  attach_function :flow_job_get_buffer, [ :pointer, :pointer, int32_t ], :pointer
  attach_function :flow_graph_print_to, [ :pointer, :pointer, :pointer ], :void
  class FlowJobResourceBuffer < FFI::Struct
    layout(
           :buffer, :pointer,
           :buffer_size, :uint,
           :owned_by_job, bool,
           :codec_state, :pointer
    )
  end
  attach_function :flow_bitmap_bgra_write_png, [ :pointer, :pointer, :pointer, :pointer ], bool
  attach_function :flow_node_post_optimize_flatten, [ :pointer, :pointer, int32_t ], bool
