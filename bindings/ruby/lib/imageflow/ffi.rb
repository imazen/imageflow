module Imageflow
  module Native
    extend FFI::Library

    def self.dylib_build_dir
      File.expand_path("../../../../../imageflow_abi", __FILE__)
    end
    def self.dylib_path
      extension = FFI::Platform.is_os("darwin") ? "dylib" : "so"
      File.expand_path("../../../../../target/debug/libimageflow.#{extension}", __FILE__)
    end

    def self.ensure_compiled
      #How old before we skip
      seconds = 15
      unless File.exist?(self.dylib_path) && (Time.now - File.stat(self.dylib_path).mtime) < seconds
        %x[cd #{self.dylib_build_dir}/../ && cargo build --package imageflow_abi]
      end
      self.dylib_path
    end

    ENV["RUST_BACKTRACE"] ="1"

    ffi_lib self.ensure_compiled

    #We don't require users to prefix everything with 'imageflow'
    def self.attach_function (prefixed_name, *vars)
      super prefixed_name.to_s.gsub(/^imageflow_/, "").to_sym, prefixed_name, *vars
    end

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
    enum :flow_direction, [
        :flow_output, 8,
        :flow_input, 4
    ]

    enum :flow_pointer_lifetime, [
        :outlives_function_call, 0,
        :outlives_context, 1
    ]
    enum :flow_cleanup_with, [
        :cleanup_with_context, 0,
        :cleanup_with_first_job, 1
    ]


    attach_function :imageflow_context_create, [], :pointer
    attach_function :imageflow_context_begin_terminate, [:pointer], :bool
    attach_function :imageflow_context_destroy, [:pointer], :void

    attach_function :imageflow_context_has_error, [:pointer], :bool
    attach_function :imageflow_context_error_recoverable, [:pointer], :bool

    attach_function :imageflow_context_error_try_clear, [:pointer], :bool

    attach_function :imageflow_context_error_code, [:pointer], :int32
    attach_function :imageflow_context_error_as_exit_code, [:pointer], :int32
    attach_function :imageflow_context_error_as_http_code, [:pointer], :int32

    attach_function :imageflow_context_error_write_to_buffer, [:pointer, :pointer, :size_t, :pointer], :bool

    attach_function :imageflow_context_error_and_stacktrace, [:pointer, :pointer, :size_t, :bool], :int64



    attach_function :imageflow_job_send_json, [:pointer, :pointer, :string, :pointer, :size_t], :pointer, blocking: true
    attach_function :imageflow_context_send_json, [:pointer, :string, :pointer, :size_t], :pointer, blocking: true
    attach_function :imageflow_json_response_read, [:pointer, :pointer, :pointer, :pointer, :pointer], :bool
    attach_function :imageflow_json_response_destroy, [:pointer, :pointer], :bool


    attach_function :imageflow_job_create, [:pointer], :pointer
    attach_function :imageflow_job_destroy, [:pointer,  :pointer], :bool

    attach_function :imageflow_io_create_for_file, [:pointer, :flow_io_mode, :string], :pointer, blocking: true
    attach_function :imageflow_io_create_from_buffer, [:pointer, :pointer, :size_t, :flow_pointer_lifetime], :pointer, blocking: true
    attach_function :imageflow_io_create_for_output_buffer, [:pointer ], :pointer, blocking: true
    attach_function :imageflow_job_get_output_buffer_by_id, [:pointer, :pointer, :int32, :pointer, :pointer], :bool, blocking: true
    attach_function :imageflow_job_get_io, [:pointer, :pointer, :int32], :pointer, blocking: true
    attach_function :imageflow_job_add_io, [:pointer, :pointer, :pointer, :int32, :flow_direction], :bool, blocking: true

  end
end
