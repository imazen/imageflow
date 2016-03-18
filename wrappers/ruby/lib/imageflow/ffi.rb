module Imageflow
	module Native
		extend FFI::Library
		ffi_lib File.expand_path('../../../../../build/libimageflow.so', __FILE__)

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
		attach_function :flow_context_get_profiler_log, [ :pointer ], :pointer
		attach_function :flow_context_create, [  ], :pointer
		attach_function :flow_context_destroy, [ :pointer ], :void
		attach_function :flow_context_free_all_allocations, [ :pointer ], :void
		attach_function :flow_context_print_memory_info, [ :pointer ], :void
		attach_function :flow_context_error_message, [ :pointer, :string, :uint ], :string
		attach_function :flow_context_stacktrace, [ :pointer, :string, :uint ], :string
		attach_function :flow_context_has_error, [ :pointer ], :bool
		attach_function :flow_context_error_reason, [ :pointer ], :int
		attach_function :flow_context_free_static_caches, [  ], :void
		attach_function :flow_context_print_and_exit_if_err, [ :pointer ], :bool
		attach_function :flow_context_clear_error, [ :pointer ], :void
		attach_function :flow_context_print_error_to, [ :pointer, :pointer ], :void
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
		attach_function :flow_context_byte_to_floatspace, [ :pointer, :uint8 ], :float
		attach_function :flow_context_floatspace_to_byte, [ :pointer, :float ], :uint8
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
		attach_function :flow_bitmap_bgra_create, [ :pointer, :int, :int, :bool, :int ], :pointer
		attach_function :flow_bitmap_bgra_create_header, [ :pointer, :int, :int ], :pointer
		attach_function :flow_bitmap_bgra_destroy, [ :pointer, :pointer ], :void
		attach_function :flow_bitmap_bgra_flip_horizontal, [ :pointer, :pointer ], :bool
		attach_function :flow_bitmap_bgra_compare, [ :pointer, :pointer, :pointer, :pointer ], :bool
		attach_function :flow_RenderDetails_create, [ :pointer ], :pointer
		attach_function :flow_RenderDetails_create_with, [ :pointer, :int ], :pointer
		attach_function :flow_RenderDetails_render, [ :pointer, :pointer, :pointer, :pointer ], :bool
		attach_function :flow_RenderDetails_render_in_place, [ :pointer, :pointer, :pointer ], :bool
		attach_function :flow_RenderDetails_destroy, [ :pointer, :pointer ], :void
		attach_function :flow_interpolation_filter_exists, [ :int ], :bool
		attach_function :flow_interpolation_details_create, [ :pointer ], :pointer
		attach_function :flow_interpolation_details_create_bicubic_custom, [ :pointer, :double, :double, :double, :double ], :pointer
		attach_function :flow_interpolation_details_create_custom, [ :pointer, :double, :double, :flow_detailed_interpolation_method ], :pointer
		attach_function :flow_interpolation_details_create_from, [ :pointer, :int ], :pointer
		attach_function :flow_interpolation_details_percent_negative_weight, [ :pointer ], :double
		attach_function :flow_interpolation_details_destroy, [ :pointer, :pointer ], :void
		attach_function :flow_pixel_format_bytes_per_pixel, [ :int ], :uint32
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
		attach_function :flow_interpolation_line_contributions_create, [ :pointer, :uint32, :uint32, :pointer ], :pointer
		attach_function :flow_interpolation_line_contributions_destroy, [ :pointer, :pointer ], :void
		attach_function :flow_convolution_kernel_create, [ :pointer, :uint32 ], :pointer
		attach_function :flow_convolution_kernel_destroy, [ :pointer, :pointer ], :void
		attach_function :flow_convolution_kernel_create_guassian, [ :pointer, :double, :uint32 ], :pointer
		attach_function :flow_convolution_kernel_sum, [ :pointer ], :double
		attach_function :flow_convolution_kernel_normalize, [ :pointer, :float ], :void
		attach_function :flow_convolution_kernel_create_gaussian_normalized, [ :pointer, :double, :uint32 ], :pointer
		attach_function :flow_convolution_kernel_create_guassian_sharpen, [ :pointer, :double, :uint32 ], :pointer
		attach_function :flow_bitmap_bgra_populate_histogram, [ :pointer, :pointer, :pointer, :uint32, :uint32, :pointer ], :bool
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
		attach_function :flow_graph_create, [ :pointer, :uint32, :uint32, :uint32, :float ], :pointer
		attach_function :flow_graph_destroy, [ :pointer, :pointer ], :void
		attach_function :flow_graph_replace_if_too_small, [ :pointer, :pointer, :uint32, :uint32, :uint32 ], :bool
		attach_function :flow_graph_copy_and_resize, [ :pointer, :pointer, :uint32, :uint32, :uint32 ], :pointer
		attach_function :flow_graph_copy_info_bytes_to, [ :pointer, :pointer, :pointer, :int32, :int32 ], :int32
		attach_function :flow_edge_duplicate, [ :pointer, :pointer, :int32 ], :int32
		attach_function :flow_node_create_canvas, [ :pointer, :pointer, :int32, :int, :uint, :uint, :uint32 ], :int32
		attach_function :flow_node_create_scale, [ :pointer, :pointer, :int32, :uint, :uint ], :int32
		attach_function :flow_node_create_primitive_flip_vertical, [ :pointer, :pointer, :int32 ], :int32
		attach_function :flow_node_create_primitive_flip_horizontal, [ :pointer, :pointer, :int32 ], :int32
		attach_function :flow_node_create_clone, [ :pointer, :pointer, :int32 ], :int32
		attach_function :flow_node_create_transpose, [ :pointer, :pointer, :int32 ], :int32
		attach_function :flow_node_create_rotate_90, [ :pointer, :pointer, :int32 ], :int32
		attach_function :flow_node_create_rotate_180, [ :pointer, :pointer, :int32 ], :int32
		attach_function :flow_node_create_rotate_270, [ :pointer, :pointer, :int32 ], :int32
		attach_function :flow_node_create_resource_placeholder, [ :pointer, :pointer, :int32, :int32 ], :int32
		attach_function :flow_node_create_encoder_placeholder, [ :pointer, :pointer, :int32, :int32, :int ], :int32
		attach_function :flow_node_create_resource_bitmap_bgra, [ :pointer, :pointer, :int32, :pointer ], :int32
		attach_function :flow_node_create_primitive_copy_rect_to_canvas, [ :pointer, :pointer, :int32, :uint32, :uint32, :uint32, :uint32, :uint32, :uint32 ], :int32
		attach_function :flow_node_create_primitive_crop, [ :pointer, :pointer, :int32, :uint32, :uint32, :uint32, :uint32 ], :int32
		attach_function :flow_node_create_render_to_canvas_1d, [ :pointer, :pointer, :int32, :bool, :uint32, :uint32, :int32, :int, :float, :int, :pointer, :pointer, :int ], :pointer
		attach_function :flow_node_delete, [ :pointer, :pointer, :int32 ], :bool
		attach_function :flow_edge_delete, [ :pointer, :pointer, :int32 ], :bool
		attach_function :flow_edge_delete_all_connected_to_node, [ :pointer, :pointer, :int32 ], :bool
		attach_function :flow_graph_get_inbound_edge_count_of_type, [ :pointer, :pointer, :int32, :int ], :int32
		attach_function :flow_graph_get_first_inbound_edge_of_type, [ :pointer, :pointer, :int32, :int ], :int32
		attach_function :flow_edge_has_dimensions, [ :pointer, :pointer, :int32 ], :bool
		attach_function :flow_node_input_edges_have_dimensions, [ :pointer, :pointer, :int32 ], :bool
		attach_function :flow_graph_duplicate_edges_to_another_node, [ :pointer, :pointer, :int32, :int32, :bool, :bool ], :bool
		attach_function :flow_edge_create, [ :pointer, :pointer, :int32, :int32, :int ], :int32
		callback(:flow_graph_visitor, [ :pointer, :pointer, :pointer, :int32, :pointer, :pointer, :pointer ], :bool)
		attach_function :flow_graph_walk, [ :pointer, :pointer, :pointer, :flow_graph_visitor, :flow_graph_visitor, :pointer ], :bool
		class FlowNodeinfoIndex < FFI::Struct
		layout(
		       :index, :int32
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
		attach_function :flow_node_execute_render_to_canvas_1d, [ :pointer, :pointer, :pointer, :pointer, :pointer ], :bool
		attach_function :flow_node_create_render1d, [ :pointer, :pointer, :int32, :bool, :int32, :int, :float, :pointer, :int ], :int32
		FLOW_INPUT = 4
		FLOW_OUTPUT = 8

		attach_function :flow_job_create, [ :pointer ], :pointer
		attach_function :flow_job_destroy, [ :pointer, :pointer ], :void
		attach_function :flow_job_configure_recording, [ :pointer, :pointer, :bool, :bool, :bool, :bool, :bool ], :bool
		attach_function :flow_job_insert_resources_into_graph, [ :pointer, :pointer, :pointer ], :bool
		attach_function :flow_job_populate_dimensions_where_certain, [ :pointer, :pointer, :pointer ], :bool
		attach_function :flow_job_force_populate_dimensions, [ :pointer, :pointer, :pointer ], :bool
		attach_function :flow_job_execute_where_certain, [ :pointer, :pointer, :pointer ], :bool
		attach_function :flow_job_graph_fully_executed, [ :pointer, :pointer, :pointer ], :bool
		attach_function :flow_job_notify_graph_changed, [ :pointer, :pointer, :pointer ], :bool
		attach_function :flow_job_execute, [ :pointer, :pointer, :pointer ], :bool
		attach_function :flow_graph_post_optimize_flatten, [ :pointer, :pointer, :pointer ], :bool
		attach_function :flow_graph_optimize, [ :pointer, :pointer, :pointer ], :bool
		attach_function :flow_graph_pre_optimize_flatten, [ :pointer, :pointer ], :bool
		attach_function :flow_graph_get_edge_count, [ :pointer, :pointer, :int32, :bool, :int, :bool, :bool ], :int32
		attach_function :flow_graph_validate, [ :pointer, :pointer ], :bool
		attach_function :flow_job_add_bitmap_bgra, [ :pointer, :pointer, :int, :int32, :pointer ], :int32
		attach_function :flow_job_add_buffer, [ :pointer, :pointer, :int, :int32, :pointer, :uint, :bool ], :int32
		attach_function :flow_node_create_generic, [ :pointer, :pointer, :int32, :int ], :int32
		attach_function :flow_graph_print_to_dot, [ :pointer, :pointer, :pointer, :string ], :bool
		attach_function :flow_job_get_bitmap_bgra, [ :pointer, :pointer, :int32 ], :pointer
		attach_function :flow_job_get_buffer, [ :pointer, :pointer, :int32 ], :pointer
		#attach_function :flow_graph_print_to, [ :pointer, :pointer, :pointer ], :void
		class FlowJobResourceBuffer < FFI::Struct
		layout(
		       :buffer, :pointer,
		       :buffer_size, :uint,
		       :owned_by_job, :bool,
		       :codec_state, :pointer
		)
		end
		attach_function :flow_bitmap_bgra_write_png, [ :pointer, :pointer, :pointer, :pointer ], :bool
		attach_function :flow_node_post_optimize_flatten, [ :pointer, :pointer, :int32 ], :bool
	end
end
