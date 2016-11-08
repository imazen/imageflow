module Imageflow
  module Native
    extend FFI::Library
    extension = FFI::Platform.is_os("darwin") ? "dylib" : "so"
    ffi_lib File.expand_path("../../../../../target/debug/libimageflowrs.#{extension}", __FILE__)

    def self.attach_function (prefixed_name, *vars)
      super prefixed_name.to_s.gsub(/^imageflow_/, "").to_sym, prefixed_name, *vars
    end



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


    attach_function :imageflow_context_create, [], :pointer
    attach_function :imageflow_context_begin_terminate, [:pointer], :bool
    attach_function :imageflow_context_destroy, [:pointer], :void

    attach_function :imageflow_context_has_error, [:pointer], :bool
    attach_function :imageflow_context_clear_error, [:pointer], :void

    attach_function :imageflow_context_error_and_stacktrace, [:pointer, :pointer, :size_t, :bool], :int64

    attach_function :imageflow_context_error_code, [:pointer], :int32

  # Skipped raise and add to stacktrace

    attach_function :imageflow_json_response_read, [:pointer, :pointer, :pointer, :pointer, :pointer], :bool
    attach_function :imageflow_json_response_destroy, [:pointer, :pointer], :bool

    attach_function :imageflow_context_send_json, [:pointer, :string, :pointer, :size_t], :pointer
    attach_function :imageflow_job_send_json, [:pointer,  :pointer, :string, :pointer, :size_t], :pointer

    attach_function :imageflow_job_create, [:pointer], :pointer

    attach_function :imageflow_job_destroy, [:pointer,  :pointer], :bool



    #PUB struct flow_io * flow_io_create_for_file(flow_c * c, flow_io_mode mode, const char * filename, void * owner);

    attach_function :imageflow_io_create_for_file, [:pointer, :flow_io_mode, :string, :pointer], :pointer, blocking: true

    # PUB struct flow_io* flow_io_create_from_memory(flow_context* c, flow_io_mode mode, uint8_t* memory, size_t length,
    #    void* owner, flow_destructor_function memory_free);
    attach_function :imageflow_io_create_from_memory, [:pointer, :flow_io_mode, :pointer, :size_t, :pointer, :pointer], :pointer, blocking: true
    #PUB struct flow_io* flow_io_create_for_output_buffer(flow_context* c, void* owner);
    attach_function :imageflow_io_create_for_output_buffer, [:pointer, :pointer], :pointer, blocking: true


    # // Returns false if the flow_io struct is disposed or not an output buffer type (or for any other error)
    # PUB bool flow_io_get_output_buffer(flow_context* c, struct flow_io* io, uint8_t** out_pointer_to_buffer,
    #                                                            size_t* out_length);
    #attach_function :imageflow_io_get_output_buffer, [:pointer, :pointer, :pointer, :pointer], :bool, blocking: true

    attach_function :imageflow_job_get_output_buffer_by_id, [:pointer, :pointer, :int32, :pointer, :pointer], :bool, blocking: true

    attach_function :imageflow_job_send_json, [:pointer, :pointer, :string, :pointer, :size_t], :pointer, blocking: true
    attach_function :imageflow_context_send_json, [:pointer, :string, :pointer, :size_t], :pointer, blocking: true

    attach_function :imageflow_json_response_read, [:pointer, :pointer, :pointer, :pointer, :pointer], :bool, blocking: true


    # pub unsafe extern fn imageflow_json_response_read(context: *mut Context,
    #     response_in: *const ImageflowJsonResponse,
    #     status_code_out: *mut i64,
    #     buffer_utf8_no_nulls_out: *mut *const libc::uint8_t,
    #     buffer_size_out: *mut libc::size_t) -> bool {
    #
    #
    #
    #
    # pub unsafe extern "C" fn imageflow_job_send_json(context: *mut Context,
    #     job: *mut Job,
    #     method: *const i8,
    #     json_buffer: *const libc::uint8_t,
    #     json_buffer_size: libc::size_t)
    # -> *const ImageflowJsonResponse {
    #   imageflow_send_json(context, job, ptr::null_mut(), method, json_buffer, json_buffer_size)
    # }
    #

    # PUB bool flow_io_write_output_buffer_to_file(flow_context* c, struct flow_io* io, const char* file_path);
   # attach_function :flow_io_write_output_buffer_to_file, [:pointer, :pointer, :string], :bool, blocking: true

    #PUB struct flow_io * flow_job_get_io(flow_context* c, struct flow_job * job, int32_t placeholder_id);
    attach_function :imageflow_job_get_io, [:pointer, :pointer, :int32], :pointer, blocking: true

    #PUB bool flow_job_get_output_buffer_by_placeholder(flow_context* c, struct flow_job * job, int32_t placeholder_id, uint8_t** out_pointer_to_buffer,
    #                                                                                                   size_t* out_length)
    #attach_function :flow_job_get_output_buffer, [:pointer, :pointer, :int32,  :pointer, :pointer], :bool
    # PUB bool flow_job_add_io(flow_context* c, struct flow_job* job, struct flow_io* io, int32_t placeholder_id,
    #                                                                                             FLOW_DIRECTION direction);
    attach_function :imageflow_job_add_io, [:pointer, :pointer, :pointer, :int32, :flow_direction], :bool, blocking: true



    #attach_function :flow_bitmap_bgra_write_png, [:pointer, :pointer, :pointer, :pointer], :bool
  end
end
