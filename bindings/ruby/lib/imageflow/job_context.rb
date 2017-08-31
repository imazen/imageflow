module Imageflow
  class JobContext
    def self.release_auto_pointer(pointer)
      Native.context_destroy(pointer)
    end

    def initialize
      ptr = Native.context_create(3,0)
      if ptr.nil? || ptr.null?
        if Native.abi_compatible(3,0)
          raise "Out of memory"
        else
          raise "ABI incompatible"
        end
      end
      @keepalive = []
      @c = FFI::AutoPointer.new(ptr, JobContext.method(:release_auto_pointer))
    end

    def destroy!
      raise_if_destroyed
      raise_pending_error unless Native.context_begin_terminate(@c)
      Native.context_destroy(@c)
      @c.autorelease = false
      @c = nil
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
      raise fetch_error_message if has_error?
    end

    def error_as_http_code
      raise_if_destroyed
      Native.context_error_as_http_code(@c)
    end

    def error_as_exit_code
      raise_if_destroyed
      Native.context_error_as_exit_code(@c)
    end

    def fetch_error_message
      raise_if_destroyed
      return nil unless has_error?
      buffer = FFI::MemoryPointer.new(:char, 4096, true)
      bytes_written = FFI::MemoryPointer.new(:size_t, 1)

      Native.context_error_write_to_buffer(@c, buffer, 4096, bytes_written)
      # The return value (bool) tells us whether the write was completely successful.
      # We don't care if it's over 4kb
      buffer.read_string
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
      def self.from_context_error(c)
        raise "No error present!" unless c.has_error?
        r = JsonResponse.new
        r.success = false
        r.status_code = c.error_as_http_code
        r.message = c.fetch_error_message
        r.data = nil
        r
      end
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

    def message_internal(method: , data:, raise_error:)
      raise_if_destroyed
      raise_pending_error
      json_str = JSON.generate data
      json_buffer = FFI::MemoryPointer.from_string(json_str)
      json_buffer_size = json_buffer.size - 1 #Drop null char
      response =  Native.context_send_json(@c, method, json_buffer, json_buffer_size)

      raise_pending_error if raise_error

      if response.nil?
        raise "Why didn't call_method catch this error? Was no error raised on the context?" if raise_error
        nil
      else
        out_status_code = FFI::MemoryPointer.new(:int64, 1)
        out_buffer_ptr = FFI::MemoryPointer.new(:pointer, 1)
        out_buffer_size = FFI::MemoryPointer.new(:size_t, 1)

        if call_method(:json_response_read, response,out_status_code, out_buffer_ptr, out_buffer_size )
          UnparsedResponse.from_pointer(out_buffer_ptr.read_pointer, out_buffer_size.read_pointer.address , out_status_code.read_int64)
        else
          raise "imageflow_json_response_read failed"
        end
      end

    end

    def send_json(method:, data:, raise_error: )
      result = message_internal(method: method, data: data, raise_error: raise_error)
      result.nil? ? JsonResponse.from_context_error(self) : result.to_parsed
    end


    # def add_input_file(io_id:, filename:)
    #   call_method(:context_add_file, io_id, :flow_input, :mode_read_seekable, filename)
    # end

    # def add_output_file(io_id:, filename:)
    #   call_method(:context_add_file, io_id, :flow_output, :mode_write_seekable, filename)
    # end

    def add_input_buffer_from_file(io_id:,  filename: )
      add_input_buffer(io_id: io_id, bytes: File.read(filename))
    end
    def write_output_buffer_to_file(io_id:,  filename: )
      bytes = get_buffer_bytes(io_id: io_id)
      File.open(filename, 'wb' ) do |output|
        output.write bytes
      end
    end
    def add_input_buffer(io_id:, bytes:)
      buffer = FFI::MemoryPointer.new(:char, bytes.bytesize) # Allocate memory sized to the data
      buffer.put_bytes(0, bytes)
      @keepalive << buffer

      call_method(:context_add_input_buffer,io_id,buffer, bytes.bytesize, :outlives_context)
    end

    def add_output_buffer(io_id:)
       call_method(:context_add_output_buffer, io_id)
    end

    def get_buffer(io_id:)
      #Just allocate 128 bytes or so to store out pointers
      buffer_pointer = FFI::MemoryPointer.new(:pointer, 1)
      buffer_size = FFI::MemoryPointer.new(:size_t, 1)

      call_method(:context_get_output_buffer_by_id,  io_id, buffer_pointer, buffer_size)

      {buffer: buffer_pointer.read_pointer,
       buffer_size: buffer_size.read_pointer.address}
    end

    def get_buffer_bytes(io_id:)
      buffer = get_buffer(io_id: io_id)
      raise "Buffer pointer null" if buffer[:buffer] == FFI::Pointer.new(0)
      buffer[:buffer].get_bytes(0, buffer[:buffer_size])
    end

    def build (build: )
      self.send_json(method:"v0.1/build", data: build, raise_error: true)
    end

    def execute (framewise: )
      self.send_json(method: "v0.1/execute", data: {"framewise": framewise}, raise_error: true)
    end

    def get_image_info(io_id:)
      result = self.send_json(method: "v0.1/get_image_info", data:{"io_id": io_id}, raise_error: true)
      raise result.message unless result.ok?

      info = result.data["image_info"]
      info = info.nil? ? info : info.inject({}){|memo,(k,v)| memo[k.to_sym] = v; memo}
      info
    end

    def self.get_image_info_by_filename(filename)
      c = JobContext.new
      c.add_input_buffer_from_file(io_id: 0, filename: filename)
      #c.add_input_file io_id: 0, filename: filename
      info = c.get_image_info io_id: 0
      c.destroy!

      {width: info[:image_width],  height: info[:image_height], filename: filename}
    end

    def set_decoder_downscale_hints(io_id:, downscaled_min_width:,
                                    downscaled_min_height:,
                                    scale_luma_spatially: true,
                                    gamma_correct_for_srgb_during_spatial_luma_scaling: true)

      hints = {width: downscaled_min_width, height: downscaled_min_height, scale_luma_spatially: scale_luma_spatially, "gamma_correct_for_srgb_during_spatial_luma_scaling": gamma_correct_for_srgb_during_spatial_luma_scaling}

      result = send_json("v0.1/tell_decoder", {"io_id": io_id, "command": {"jpeg_downscale_hints": hints}}, raise_error: true)
      raise result.message unless result.ok?
    end


    private :raise_if_destroyed
    private :raise_pending_error
    private :call_method
    private :message_internal
  end
end