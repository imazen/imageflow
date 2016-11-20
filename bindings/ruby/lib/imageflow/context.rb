module Imageflow
  class Context
    def self.release_auto_pointer(pointer)
      Native.context_destroy(pointer)
    end

    def initialize
      ptr = Native.context_create
      raise "Out of memory" if ptr.nil? || ptr.null?

      @c = FFI::AutoPointer.new(ptr, Context.method(:release_auto_pointer))
    end

    def destroy!
      #Todo, we could use begin_terminate to capture tear-down issues
      Native.context_destroy(@c)
      @c.autorelease = false
      @c = nil
    end

    def ptr
      @c
    end

    def is_destroyed?
      @c == nil
    end

    def raise_if_destroyed
      raise "Context is_destroyed; cannot be used." if is_destroyed?
    end

    def has_error?
      raise_if_destroyed
      Native.context_has_error(@c)
    end

    def raise_pending_error
      raise error_message if has_error?
    end


    def error_message(full_file_paths: true)
      raise_if_destroyed
      buffer = FFI::MemoryPointer.new(:char, 4096, true)
      Native.context_error_and_stacktrace(@c, buffer, 4096, full_file_paths)
      "\n" + buffer.read_string
    end

    def create_job (**args)
      Job.new context: self, **args
    end

    def call_method(name, *args)
      raise_if_destroyed
      raise_pending_error
      result = Native.send(name, @c, *args)
      raise_pending_error
      result
    end

    class JsonResponse
      attr_accessor :success, :status_code, :message, :data
      def ok?
        !!success
      end
    end

    def send_json(method:, data: )
      message_internal(method: method, data: data).to_parsed
    end

    class UnparsedResponse
      def self.from_pointer(buffer_ptr, size, status_code)
        r = UnparsedResponse::new()
        r.json_str = buffer_ptr.read_string(size)
        r.status_code = status_code
        r
      end
      attr_accessor :status_code, :json_str

      def to_parsed
        hash = JSON.parse self.json_str
        if hash["code"] != self.status_code
          raise "status_code #{response.status_code} is inconsistent with json code #{hash['code']}"
        end
        r = JsonResponse.new
        r.success = !!hash["success"]
        r.status_code = self.status_code
        r.message = hash["message"]
        r.data = hash["data"]
        r
      end
    end


    def message_internal(optional_job:  nil, method: , data:)
      json_str = JSON.generate data
      json_buffer = FFI::MemoryPointer.from_string(json_str)
      json_buffer_size = json_buffer.size - 1 #Drop null char
      response = if optional_job.nil?
                   call_method(:context_send_json, method, json_buffer, json_buffer_size)
      else
        call_method(:job_send_json, optional_job, method, json_buffer, json_buffer_size)
      end

      if response.nil?
        raise "Why didn't call_method catch this error? Was no error raised on the context?"
      end

      out_status_code = FFI::MemoryPointer.new(:int64, 1) # Allocate memory sized to the data
      out_buffer_ptr = buffer_pointer = FFI::MemoryPointer.new(:pointer, 1) # Allocate memory sized to the data
      out_buffer_size = FFI::MemoryPointer.new(:uint64, 1) # Allocate memory sized to the data


      if call_method(:json_response_read, response,out_status_code, out_buffer_ptr, out_buffer_size )
        UnparsedResponse.from_pointer(out_buffer_ptr.read_pointer, out_buffer_size.read_uint64 , out_status_code.read_int64)
      else
        raise "imageflow_json_response_read failed"
      end
    end



  end
end