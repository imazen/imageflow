module Imageflow
  module Native
    extend FFI::Library
    ffi_lib File.expand_path('../../../../../build/libimageflow.so', __FILE__)

    def self.attach_function (prefixed_name, *vars)
      super prefixed_name.to_s.gsub(/^flow_/, "").to_sym, prefixed_name, *vars
    end

    FLOW_INPUT = 4
    FLOW_OUTPUT = 8
    enum :flow_direction, [
        :flow_input, 4,
        :flow_output, 8
    ]

    enum :flow_ntype, [
        :ntype_Null, 0,
        :ntype_primitive_Flip_Vertical_Mutate, 1,
        :ntype_primitive_Flip_Horizontal_Mutate, 1,
        :ntype_primitive_Crop_Mutate_Alias, 2,
        :ntype_primitive_CopyRectToCanvas, 3,
        :ntype_Create_Canvas, 4,
        :ntype_primitive_RenderToCanvas1D, 5,

        :ntype_primitive_bitmap_bgra_pointer,
        :ntype_primitive_decoder,
        :ntype_primitive_encoder,
        :ntype_primitive_fill_rect,

        :ntype_non_primitive_nodes_begin, 256,

        :ntype_Expand_Canvas,
        :ntype_Transpose,
        :ntype_Flip_Vertical,
        :ntype_Flip_Horizontal,
        :ntype_Render1D,
        :ntype_Crop,
        :ntype_non_optimizable_nodes_begin, 512,

        :ntype_Clone,
        :ntype_decoder,
        :ntype_encoder,

        :ntype_Rotate_90,
        :ntype_Rotate_180,
        :ntype_Rotate_270,
        :ntype_Scale,
        # Not implemented below here:
        :ntype_Rotate_Flip_Per_Orientation,
        :ntype_Crop_Percentage,
        :ntype_Crop_Percentage_Infinite_Canvas,
        :ntype_Crop_Rectangle,
        :ntype_Constrain,
        :ntype_Matte,
        :ntype_EnlargeCanvas,
        :ntype_Sharpen,
        :ntype_Blur,
        :ntype_Convolve_Custom,
        :ntype_AdjustContrast,
        :ntype_AdjustSaturation,
        :ntype_AdjustBrightness,
        :ntype_CropWhitespace,
        :ntype_Opacity,
        :ntype_Sepia,
        :ntype_Grayscale,
        :ntype_DrawImage,
        :ntype_RemoveNoise,
        :ntype_ColorMatrixsRGB,
        :ntype__FORCE_ENUM_SIZE_INT32, 2147483647
    ]
    #
    # typedef enum flow_node_state {
    # 	flow_node_state_Blank = 0,
    # 			flow_node_state_InputDimensionsKnown = 1,
    # 			flow_node_state_ReadyForPreOptimizeFlatten = 1,
    # 			flow_node_state_PreOptimizeFlattened = 2,
    # 			flow_node_state_ReadyForOptimize = 3,
    # 			flow_node_state_Optimized = 4,
    # 			flow_node_state_ReadyForPostOptimizeFlatten = 7,
    # 			flow_node_state_PostOptimizeFlattened = 8,
    # 			flow_node_state_InputsExecuted = 16,
    # 			flow_node_state_ReadyForExecution = 31,
    # 			flow_node_state_Executed = 32,
    # 			flow_node_state_Done = 63
    # } flow_node_state;
    #
    # typedef enum flow_edgetype {
    # 	flow_edgetype_null,
    # 			flow_edgetype_input,
    # 			flow_edgetype_canvas,
    # 			flow_edgetype_info,
    # 			flow_edgetype_FORCE_ENUM_SIZE_INT32 = 2147483647
    # } flow_edgetype;
    #
    # typedef enum flow_compositing_mode {
    # 	flow_compositing_mode_overwrite,
    # 			flow_compositing_mode_compose,
    # 			flow_compositing_mode_blend_with_matte
    # } flow_compositing_mode;
    #
    # struct flow_job;
    #
    # typedef enum flow_job_resource_type {
    # 	flow_job_resource_type_bitmap_bgra = 1,
    # 			flow_job_resource_type_buffer = 2
    #
    # } flow_job_resource_type;
    #

    # typedef enum flow_scanlines_filter_type {
    # 	flow_scanlines_filter_Sharpen, // 3x3, percentage-based
    # 	flow_scanlines_filter_Blur, // 3x box blur to simulate guassian
    # 	flow_scanlines_filter_Convolve, // Apply convolution kernel
    # 	flow_scanlines_filter_ColorMatrix, // Apply color matrix
    # 	flow_scanlines_filter_ToLinear,
    # 			flow_scanlines_filter_ToSrgb,
    # 			flow_scanlines_filter_Custom, // Execute custom callback.,
    # 																											flow_scanlines_filter__FORCE_ENUM_SIZE_INT32 = 2147483647
    # } flow_scanlines_filter_type;

    enum :interpolation_filter, [

        	:filter_RobidouxFast, 1,
    			:filter_Robidoux, 2,
    			:filter_RobidouxSharp, 3,
    			:filter_Ginseng,
    			:filter_GinsengSharp,
    			:filter_Lanczos,
    			:filter_LanczosSharp,
    			:filter_Lanczos2,
    			:filter_Lanczos2Sharp,
    			:filter_CubicFast,
    			:filter_Cubic,
    			:filter_CubicSharp,
    			:filter_CatmullRom,
    			:filter_Mitchell,

    			:filter_CubicBSpline,
    			:filter_Hermite,
    			:filter_Jinc,
    			:filter_RawLanczos3,
    			:filter_RawLanczos3Sharp,
    			:filter_RawLanczos2,
    			:filter_RawLanczos2Sharp,
    			:filter_Triangle,
    			:filter_Linear,
    			:filter_Box,
    			:filter_CatmullRomFast,
    			:filter_CatmullRomFastSharp,

    			:filter_Fastest,

    			:filter_MitchellFast
    ]
    # } flow_interpolation_filter;
    #
    # typedef enum flow_profiling_entry_flags {
    # 	flow_profiling_entry_start = 2,
    # 			flow_profiling_entry_start_allow_recursion = 6,
    # 			flow_profiling_entry_stop = 8,
    # 			flow_profiling_entry_stop_assert_started = 24,
    # 			flow_profiling_entry_stop_children = 56
    # } flow_profiling_entry_flags;
    #
    # typedef enum flow_pixel_format { flow_bgr24 = 3, flow_bgra32 = 4, flow_gray8 = 1 } flow_pixel_format;
    #
    # typedef enum flow_bitmap_compositing_mode {
    # 	flow_bitmap_compositing_replace_self = 0,
    # 			flow_bitmap_compositing_blend_with_self = 1,
    # 			flow_bitmap_compositing_blend_with_matte = 2
    # } flow_bitmap_compositing_mode;
    #
    # typedef enum flow_working_floatspace {
    # 	flow_working_floatspace_srgb = 0,
    # 			flow_working_floatspace_as_is = 0,
    # 			flow_working_floatspace_linear = 1,
    # 			flow_working_floatspace_gamma = 2
    # } flow_working_floatspace;

    enum :flow_status_code, [
        :No_Error, 0,
        :Out_of_memory, 1,
        :Not_implemented,
        :Unsupported_pixel_format,
        :Null_argument,
        :Invalid_argument,
        :Invalid_dimensions,
        :Invalid_internal_state,
        :IO_error,
        :Image_decoding_failed,
        :Image_encoding_failed,
        :Item_does_not_exist,
        :Graph_invalid,
        :Invalid_inputs_to_node,
        :Maximum_graph_passes_exceeded,
        :Graph_is_cyclic,
        :Other_error,
        :___Last_library_error,
        :First_user_defined_error, 1025,
        :Last_user_defined_error, 2147483647,
    ]
    enum :pixel_format, [
        :bgr24, 3,
        :bgra32, 4,
        :gray8, 1
    ]

    enum :flow_io_mode, [
        :mode_null,
        :flow_io_mode_read_sequential, 1,
        :mode_write_sequential, 2,
        :mode_read_seekable, 5, #1 | 4,
        :mode_write_seekable, 6, #2 | 4,
        :mode_read_write_seekable, 15, #1 | 2 | 4 | 8

    ]

    class FlowProfilingEntry < FFI::Struct
      layout(
          :time, :int64,
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
          :count, :uint32,
          :capacity, :uint32,
          :ticks_per_second, :int64
      )
    end
    attach_function :flow_context_get_profiler_log, [:pointer], :pointer
    attach_function :flow_context_create, [], :pointer
    attach_function :flow_context_destroy, [:pointer], :void
    attach_function :flow_context_print_memory_info, [:pointer], :void
    attach_function :flow_context_error_message, [:pointer, :pointer, :uint], :int64
    attach_function :flow_context_stacktrace, [:pointer, :pointer, :uint, :bool], :int64
    attach_function :flow_context_error_and_stacktrace, [:pointer, :pointer, :uint, :bool], :int64
    attach_function :flow_context_has_error, [:pointer], :bool
    attach_function :flow_context_error_reason, [:pointer], :int
    attach_function :flow_context_print_and_exit_if_err, [:pointer], :bool
    attach_function :flow_context_clear_error, [:pointer], :void
    attach_function :flow_context_print_error_to, [:pointer, :pointer], :void

    # PUB bool flow_job_get_decoder_info(flow_context* c, struct flow_job* job, int32_t by_placeholder_id,
    #                                                                                   struct flow_decoder_info* info);



    #PUB struct flow_io* flow_io_create_for_file(flow_context* c, flow_io_mode mode, const char* filename, void* owner);
    #attach_function :flow_io_create_for_file, [:pointer, :flow_io_mode, :string, :pointer], :pointer
    #
    # PUB bool flow_job_initialize_encoder(flow_context* c, struct flow_job* job, int32_t by_placeholder_id,
    #                                                                                     flow_codec_type codec_id);
    #
    # PUB struct flow_codec_instance* flow_job_get_codec_instance(flow_context* c, struct flow_job* job,
    #                                                                                     int32_t by_placeholder_id);

    # PUB struct flow_io* flow_io_create_from_memory(flow_context* c, flow_io_mode mode, uint8_t* memory, size_t length,
    #    void* owner, flow_destructor_function memory_free);
    attach_function :flow_io_create_from_memory, [:pointer, :flow_io_mode, :pointer, :uint64, :pointer, :pointer], :pointer
    #PUB struct flow_io* flow_io_create_for_output_buffer(flow_context* c, void* owner);
    attach_function :flow_io_create_for_output_buffer, [:pointer, :pointer], :pointer


    # // Returns false if the flow_io struct is disposed or not an output buffer type (or for any other error)
    # PUB bool flow_io_get_output_buffer(flow_context* c, struct flow_io* io, uint8_t** out_pointer_to_buffer,
    #                                                            size_t* out_length);
    attach_function :flow_io_get_output_buffer, [:pointer, :pointer, :pointer, :pointer], :bool

    # PUB bool flow_io_write_output_buffer_to_file(flow_context* c, struct flow_io* io, const char* file_path);
    attach_function :flow_io_write_output_buffer_to_file, [:pointer, :pointer, :string], :bool

    #PUB struct flow_io * flow_job_get_io(flow_context* c, struct flow_job * job, int32_t placeholder_id);
    attach_function :flow_job_get_io, [:pointer, :pointer, :int32], :pointer

    #PUB bool flow_job_get_output_buffer_by_placeholder(flow_context* c, struct flow_job * job, int32_t placeholder_id, uint8_t** out_pointer_to_buffer,
    #                                                                                                   size_t* out_length)
    attach_function :flow_job_get_output_buffer, [:pointer, :pointer, :int32,  :pointer, :pointer], :bool
    # PUB bool flow_job_add_io(flow_context* c, struct flow_job* job, struct flow_io* io, int32_t placeholder_id,
    #                                                                                             FLOW_DIRECTION direction);
    attach_function :flow_job_add_io, [:pointer, :pointer, :pointer, :int32, :flow_direction], :bool

    # bool flow_job_set_default_encoder(flow_context* c, struct flow_job* job, int32_t by_placeholder_id,
    #                                                                                  flow_codec_type default_encoder_id);
    attach_function :flow_job_set_default_encoder, [:pointer, :pointer, :int32, :int64], :bool



    class FlowBitmapBgraStruct < FFI::Struct
      layout(
          :w, :uint32,
          :h, :uint32,
          :stride, :uint32,
          :pixels, :pointer,
          :borrowed_pixels, :bool,
          :alpha_meaningful, :bool,
          :pixels_readonly, :bool,
          :stride_readonly, :bool,
          :can_reuse_space, :bool,
          :fmt, :int,
          :matte_color, [:uint8, 4],
          :compositing_mode, :int
      )
    end
    attach_function :flow_context_byte_to_floatspace, [:pointer, :uint8], :float
    attach_function :flow_context_floatspace_to_byte, [:pointer, :float], :uint8
    attach_function :flow_context_set_floatspace, [:pointer, :int, :float, :float, :float], :void
    callback(:flow_detailed_interpolation_method, [:pointer, :double], :double)
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
          :width, :uint32,
          :radius, :uint32,
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
          :halving_divisor, :uint32,
          :kernel_a, :pointer,
          :kernel_b, :pointer,
          :sharpen_percent_goal, :float,
          :apply_color_matrix, :bool,
          :color_matrix_data, [:float, 25],
          :color_matrix, [:pointer, 5],
          :post_transpose, :bool,
          :post_flip_x, :bool,
          :post_flip_y, :bool,
          :enable_profiling, :bool
      )
    end
    attach_function :flow_bitmap_bgra_create, [:pointer, :int, :int, :bool, :int], :pointer
    attach_function :flow_bitmap_bgra_create_header, [:pointer, :int, :int], :pointer
    attach_function :flow_bitmap_bgra_destroy, [:pointer, :pointer], :void
    attach_function :flow_bitmap_bgra_flip_horizontal, [:pointer, :pointer], :bool
    attach_function :flow_bitmap_bgra_compare, [:pointer, :pointer, :pointer, :pointer], :bool
    attach_function :flow_RenderDetails_create, [:pointer], :pointer
    attach_function :flow_RenderDetails_create_with, [:pointer, :int], :pointer
    attach_function :flow_RenderDetails_render, [:pointer, :pointer, :pointer, :pointer], :bool
    attach_function :flow_RenderDetails_render_in_place, [:pointer, :pointer, :pointer], :bool
    attach_function :flow_RenderDetails_destroy, [:pointer, :pointer], :void
    attach_function :flow_interpolation_filter_exists, [:int], :bool
    attach_function :flow_interpolation_details_create, [:pointer], :pointer
    attach_function :flow_interpolation_details_create_bicubic_custom, [:pointer, :double, :double, :double, :double], :pointer
    attach_function :flow_interpolation_details_create_custom, [:pointer, :double, :double, :flow_detailed_interpolation_method], :pointer
    attach_function :flow_interpolation_details_create_from, [:pointer, :int], :pointer
    attach_function :flow_interpolation_details_percent_negative_weight, [:pointer], :double
    attach_function :flow_interpolation_details_destroy, [:pointer, :pointer], :void
    attach_function :flow_pixel_format_bytes_per_pixel, [:int], :uint32
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
          :WindowSize, :uint32,
          :LineLength, :uint32,
          :percent_negative, :double
      )
    end
    attach_function :flow_interpolation_line_contributions_create, [:pointer, :uint32, :uint32, :pointer], :pointer
    attach_function :flow_interpolation_line_contributions_destroy, [:pointer, :pointer], :void
    attach_function :flow_convolution_kernel_create, [:pointer, :uint32], :pointer
    attach_function :flow_convolution_kernel_destroy, [:pointer, :pointer], :void
    attach_function :flow_convolution_kernel_create_guassian, [:pointer, :double, :uint32], :pointer
    attach_function :flow_convolution_kernel_sum, [:pointer], :double
    attach_function :flow_convolution_kernel_normalize, [:pointer, :float], :void
    attach_function :flow_convolution_kernel_create_gaussian_normalized, [:pointer, :double, :uint32], :pointer
    attach_function :flow_convolution_kernel_create_guassian_sharpen, [:pointer, :double, :uint32], :pointer
    attach_function :flow_bitmap_bgra_populate_histogram, [:pointer, :pointer, :pointer, :uint32, :uint32, :pointer], :bool
    class FlowScanlinesFilter < FFI::Struct
      layout(
          :type, :int,
          :next, :pointer
      )
    end
    class FlowEdge < FFI::Struct
      layout(
          :type, :int,
          :from, :int32,
          :to, :int32,
          :from_width, :int32,
          :from_height, :int32,
          :from_format, :int,
          :from_alpha_meaningful, :bool,
          :info_byte_index, :int32,
          :info_bytes, :int32
      )
    end
    class FlowNode < FFI::Struct
      layout(
          :type, :int,
          :info_byte_index, :int32,
          :info_bytes, :int32,
          :state, :int,
          :result_bitmap, :pointer,
          :ticks_elapsed, :uint32
      )
    end
    class FlowGraph < FFI::Struct
      layout(
          :memory_layout_version, :uint32,
          :edges, :pointer,
          :edge_count, :int32,
          :next_edge_id, :int32,
          :max_edges, :int32,
          :nodes, :pointer,
          :node_count, :int32,
          :next_node_id, :int32,
          :max_nodes, :int32,
          :info_bytes, :pointer,
          :max_info_bytes, :int32,
          :next_info_byte, :int32,
          :deleted_bytes, :int32,
          :growth_factor, :float
      )
    end
    attach_function :flow_graph_create, [:pointer, :uint32, :uint32, :uint32, :float], :pointer
    attach_function :flow_graph_destroy, [:pointer, :pointer], :void
    attach_function :flow_graph_replace_if_too_small, [:pointer, :pointer, :uint32, :uint32, :uint32], :bool
    attach_function :flow_graph_copy_and_resize, [:pointer, :pointer, :uint32, :uint32, :uint32], :pointer

    attach_function :flow_graph_copy, [:pointer, :pointer], :pointer
    attach_function :flow_graph_copy_info_bytes_to, [:pointer, :pointer, :pointer, :int32, :int32], :int32
    attach_function :flow_edge_duplicate, [:pointer, :pointer, :int32], :int32
    attach_function :flow_node_create_canvas, [:pointer, :pointer, :int32, :pixel_format, :uint, :uint, :uint32], :int32
    attach_function :flow_node_create_scale, [:pointer, :pointer, :int32, :uint, :uint, :interpolation_filter, :interpolation_filter], :int32
    attach_function :flow_node_create_primitive_flip_vertical, [:pointer, :pointer, :int32], :int32
    attach_function :flow_node_create_primitive_flip_horizontal, [:pointer, :pointer, :int32], :int32
    attach_function :flow_node_create_clone, [:pointer, :pointer, :int32], :int32
    attach_function :flow_node_create_noop, [:pointer, :pointer, :int32], :int32
    attach_function :flow_node_create_expand_canvas, [:pointer, :pointer, :int32, :uint32, :uint32, :uint32, :uint32, :uint32], :int32
    attach_function :flow_node_create_transpose, [:pointer, :pointer, :int32], :int32
    attach_function :flow_node_create_rotate_90, [:pointer, :pointer, :int32], :int32
    attach_function :flow_node_create_rotate_180, [:pointer, :pointer, :int32], :int32
    attach_function :flow_node_create_rotate_270, [:pointer, :pointer, :int32], :int32
    attach_function :flow_node_create_decoder, [:pointer, :pointer, :int32, :int32], :int32
    attach_function :flow_node_create_encoder_placeholder, [:pointer, :pointer, :int32, :int32], :int32

    attach_function :flow_node_set_decoder_downscale_hint, [:pointer, :pointer, :int32, :int64, :int64, :int64, :int64], :bool


    attach_function :flow_job_decoder_set_downscale_hints_by_placeholder_id, [:pointer, :pointer, :int32, :int64, :int64, :int64, :int64], :bool


    attach_function :flow_node_create_encoder, [:pointer, :pointer, :int32, :int32, :uint64], :int32
    attach_function :flow_node_create_primitive_copy_rect_to_canvas, [:pointer, :pointer, :int32, :uint32, :uint32, :uint32, :uint32, :uint32, :uint32], :int32
    attach_function :flow_node_create_primitive_crop, [:pointer, :pointer, :int32, :uint32, :uint32, :uint32, :uint32], :int32
    attach_function :flow_node_create_render_to_canvas_1d, [:pointer, :pointer, :int32, :bool, :uint32, :uint32, :int32, :int, :float, :int, :pointer, :pointer, :int], :pointer
    attach_function :flow_node_delete, [:pointer, :pointer, :int32], :bool
    attach_function :flow_edge_delete, [:pointer, :pointer, :int32], :bool
    attach_function :flow_edge_delete_all_connected_to_node, [:pointer, :pointer, :int32], :bool
    attach_function :flow_graph_get_inbound_edge_count_of_type, [:pointer, :pointer, :int32, :int], :int32
    attach_function :flow_graph_get_first_inbound_edge_of_type, [:pointer, :pointer, :int32, :int], :int32
    attach_function :flow_node_has_dimensions, [:pointer, :pointer, :int32], :bool
    attach_function :flow_node_inputs_have_dimensions, [:pointer, :pointer, :int32], :bool
    attach_function :flow_graph_duplicate_edges_to_another_node, [:pointer, :pointer, :int32, :int32, :bool, :bool], :bool
    attach_function :flow_edge_create, [:pointer, :pointer, :int32, :int32, :int], :int32
    callback(:flow_graph_visitor, [:pointer, :pointer, :pointer, :int32, :pointer, :pointer, :pointer], :bool)
    attach_function :flow_graph_walk, [:pointer, :pointer, :pointer, :flow_graph_visitor, :flow_graph_visitor, :pointer], :bool
    class FlowNodeinfoIndex < FFI::Struct
      layout(
          :index, :int32
      )
    end
    class FlowNodeinfoEncoderPlaceholder < FFI::Struct
      layout(
          :index, FlowNodeinfoIndex,
          :codec_id, :int
      )
    end
    class FlowNodeinfoCreatecanvas < FFI::Struct
      layout(
          :format, :int,
          :width, :uint,
          :height, :uint,
          :bgcolor, :uint32
      )
    end
    class FlowNodeinfoCrop < FFI::Struct
      layout(
          :x1, :uint32,
          :x2, :uint32,
          :y1, :uint32,
          :y2, :uint32
      )
    end
    class FlowNodeinfoCopyRectToCanvas < FFI::Struct
      layout(
          :x, :uint32,
          :y, :uint32,
          :from_x, :uint32,
          :from_y, :uint32,
          :width, :uint32,
          :height, :uint32
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
          :scale_to_width, :int32,
          :canvas_x, :uint32,
          :canvas_y, :uint32,
          :transpose_on_write, :bool,
          :scale_in_colorspace, :int,
          :sharpen_percent_goal, :float,
          :compositing_mode, :int,
          :matte_color, [:uint8, 4],
          :filter_list, :pointer
      )
    end
    attach_function :flow_node_execute_render_to_canvas_1d, [:pointer, :pointer, :pointer, :pointer, :pointer], :bool
    attach_function :flow_node_create_render1d, [:pointer, :pointer, :int32, :bool, :int32, :int, :float, :pointer, :int], :int32

    attach_function :flow_job_create, [:pointer], :pointer
    attach_function :flow_job_destroy, [:pointer, :pointer], :bool
    attach_function :flow_job_configure_recording, [:pointer, :pointer, :bool, :bool, :bool, :bool, :bool], :bool
    attach_function :flow_job_populate_dimensions_where_certain, [:pointer, :pointer, :pointer], :bool
    attach_function :flow_job_force_populate_dimensions, [:pointer, :pointer, :pointer], :bool
    attach_function :flow_job_execute_where_certain, [:pointer, :pointer, :pointer], :bool
    attach_function :flow_job_graph_fully_executed, [:pointer, :pointer, :pointer], :bool
    attach_function :flow_job_notify_graph_changed, [:pointer, :pointer, :pointer], :bool
    attach_function :flow_job_execute, [:pointer, :pointer, :pointer], :bool
    attach_function :flow_graph_post_optimize_flatten, [:pointer, :pointer, :pointer], :bool
    attach_function :flow_graph_optimize, [:pointer, :pointer, :pointer], :bool
    attach_function :flow_graph_pre_optimize_flatten, [:pointer, :pointer], :bool
    attach_function :flow_graph_get_edge_count, [:pointer, :pointer, :int32, :bool, :int, :bool, :bool], :int32
    attach_function :flow_graph_validate, [:pointer, :pointer], :bool
    attach_function :flow_node_create_generic, [:pointer, :pointer, :int32, :int], :int32
    attach_function :flow_graph_print_to_dot, [:pointer, :pointer, :pointer, :string], :bool
    #attach_function :flow_graph_print_to, [ :pointer, :pointer, :pointer ], :void

    attach_function :flow_bitmap_bgra_write_png, [:pointer, :pointer, :pointer, :pointer], :bool
    attach_function :flow_node_post_optimize_flatten, [:pointer, :pointer, :int32], :bool
    class FlowJobDecoderInfo < FFI::Struct
      layout(
          :codec_id, :int64,
          :preferred_mime_type, :strptr,
          :preferred_extension, :strptr,
          :frame_count, :size_t,
          :current_frame_index, :int64,
          :frame0_width, :int32,
          :frame0_height, :int32,
          :frame0_post_decode_format, :pixel_format,
      )
    end





    attach_function :flow_job_get_decoder_info, [:pointer, :pointer, :int32, :pointer], :pointer


  end
end
