# frozen_string_literal: true

# Require the low-level FFI module
require_relative 'imageflow_ffi'
require 'json'

# Require all the generated data models
Dir[File.join(__dir__, 'imageflow', 'models', '*.rb')].each { |file| require file }

# Imageflow is the top-level module for the public Ruby API.
# It provides a clean, idiomatic interface to the Imageflow library.
module Imageflow
  # The Context class manages the lifecycle of a native Imageflow context.
  # It ensures that the native context is properly created and destroyed.
  class Context
    def initialize
      major = ImageflowFFI.imageflow_abi_version_major
      minor = ImageflowFFI.imageflow_abi_version_minor
      @context_ptr = ImageflowFFI.imageflow_context_create(major, minor)

      # Automatically free the context when the object is garbage collected.
      ObjectSpace.define_finalizer(self, self.class.finalize(@context_ptr))
    end

    def self.finalize(context_ptr)
      proc { ImageflowFFI.imageflow_context_destroy(context_ptr) }
    end

    # Sends a JSON command to the Imageflow context and returns the response.
    #
    # @param command [String] The command name (e.g., 'v1/info').
    # @param payload [Hash] The command payload, which will be serialized to JSON.
    # @return [Hash] The JSON response, parsed into a Ruby Hash.
    # @raise [RuntimeError] if the Imageflow library reports an error.
    def send_json(command, payload)
      json_string = JSON.generate(payload)
      json_buffer = FFI::MemoryPointer.from_string(json_string)

      # This response pointer must be freed with imageflow_json_response_destroy
      response_ptr = ImageflowFFI.imageflow_context_send_json(@context_ptr, command, json_buffer, json_string.bytesize)

      # Check for errors after every call that can produce one.
      if ImageflowFFI.imageflow_context_has_error(@context_ptr)
        error_buffer = FFI::MemoryPointer.new(:char, 1024) # 1KB for error message
        bytes_written_ptr = FFI::MemoryPointer.new(:size_t)
        ImageflowFFI.imageflow_context_error_write_to_buffer(@context_ptr, error_buffer, error_buffer.size, bytes_written_ptr)
        bytes_written = bytes_written_ptr.read(:size_t)
        error_message = error_buffer.read_string(bytes_written)
        # Always free the response pointer, even if we have an error.
        ImageflowFFI.imageflow_json_response_destroy(@context_ptr, response_ptr) if response_ptr && !response_ptr.null?
        raise "Imageflow error: #{error_message}"
      end

      return {} if response_ptr.nil? || response_ptr.null?

      begin
        status_code_ptr = FFI::MemoryPointer.new(:int)
        buffer_ptr_ptr = FFI::MemoryPointer.new(:pointer)
        buffer_size_ptr = FFI::MemoryPointer.new(:size_t)

        success = ImageflowFFI.imageflow_json_response_read(@context_ptr, response_ptr, status_code_ptr, buffer_ptr_ptr, buffer_size_ptr)

        if success
          buffer_ptr = buffer_ptr_ptr.read_pointer
          buffer_size = buffer_size_ptr.read(:size_t)
          json_response_string = buffer_ptr.read_string(buffer_size)
          return JSON.parse(json_response_string)
        else
          return { error: 'Failed to read JSON response from Imageflow' }
        end
      ensure
        # Ensure the response is always destroyed.
        ImageflowFFI.imageflow_json_response_destroy(@context_ptr, response_ptr)
      end
    end
  end

  class << self
    # Returns the version of the Imageflow ABI (Application Binary Interface)
    # as a 'major.minor' string.
    #
    # @return [String] The ABI version.
    def version
      "#{ImageflowFFI.imageflow_abi_version_major}.#{ImageflowFFI.imageflow_abi_version_minor}"
    end
  end
end
