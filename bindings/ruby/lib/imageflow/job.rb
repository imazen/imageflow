module Imageflow
  class Job
    def initialize(context:)
      @c = context
      @ptr = @c.call_method(:job_create)
      @keepalive = []
    end

    def self.get_image_info_by_filename(filename, context: nil)
      c = context || Imageflow::Context.new
      job = Job.new context:c
      job.add_input_file placeholder_id: 0, filename: filename
      info = job.get_image_info placeholder_id: 0
      job.destroy!
      c.destroy! unless context == c

      {width: info[:frame0Width],  height: info[:frame0Height], filename: filename}
    end

    def destroy!
      @c.call_method(:job_destroy, @ptr)
      @ptr = nil
    end

    def destroyed?
      @ptr.nil? || @ptr.null?
    end


    def add_input_file(placeholder_id:, filename:)
      io_in = @c.call_method(:io_create_for_file, :mode_read_seekable, filename,  :cleanup_with_context)
      @c.call_method(:job_add_io, @ptr, io_in, placeholder_id,  :flow_input)
    end

    def add_output_file(placeholder_id:, filename:)
      io_out = @c.call_method(:io_create_for_file, :mode_write_seekable, filename,  :cleanup_with_context)
      @c.call_method(:job_add_io, @ptr, io_out, placeholder_id,  :flow_output)
    end

    def add_input_buffer(placeholder_id:, bytes:)
      buffer = FFI::MemoryPointer.new(:char, bytes.bytesize) # Allocate memory sized to the data
      buffer.put_bytes(0, bytes)
      @keepalive << buffer

      io_in = @c.call_method(:io_create_from_buffer,buffer, bytes.bytesize, :outlives_context, :cleanup_with_context)

      @c.call_method(:job_add_io, @ptr, io_in, placeholder_id,  :flow_input)
    end

    def add_output_buffer(placeholder_id:)
      io_ptr = @c.call_method(:io_create_for_output_buffer)
      @c.call_method(:job_add_io, @ptr, io_ptr, placeholder_id,  :flow_output)
    end

    def get_buffer(placeholder_id:)
      #Just allocate 128 bytes or so to store out pointers
      buffer_pointer = FFI::MemoryPointer.new(:pointer, 1)
      buffer_size = FFI::MemoryPointer.new(:uint64, 1)

      @c.call_method(:job_get_output_buffer_by_id, @ptr, placeholder_id, buffer_pointer, buffer_size)

      {buffer: buffer_pointer.get_pointer(0),
       buffer_size: buffer_size.get_uint64(0)}
    end

    def get_buffer_bytes(placeholder_id:)
      buffer = get_buffer(placeholder_id: placeholder_id)
      raise "Buffer pointer null" if buffer[:buffer] == FFI::Pointer.new(0)
      buffer[:buffer].get_bytes(0, buffer[:buffer_size])
    end

    def send_json(method, data )
      @c.message_internal(optional_job: @ptr, method: method, data: data).to_parsed
    end

    def execute (framewise:, graph_recording: nil)
      result = self.send_json("v0.1/execute", {"framewise": framewise})
      raise result.message unless result.ok?
    end

    def get_image_info(placeholder_id:)
      result = self.send_json("v0.1/get_image_info", {"ioId": placeholder_id})
      raise result.message unless result.ok?

      info = result.data["ImageInfo"]
      info = info.nil? ? info : info.inject({}){|memo,(k,v)| memo[k.to_sym] = v; memo}
      info[:frame0_width] = info[:frame0Width]
      info[:frame0_height] = info[:frame0Height]
      info[:frame0_post_decode_format] = info[:frame0PostDecodeFormat]
      info[:preferred_mime_type] = info[:preferredMimeType]
      info
    end

    def set_decoder_downscale_hints(placeholder_id:, downscaled_min_width:,
                                     downscaled_min_height:,
                                    scale_luma_spatially: true,
                                    gamma_correct_for_srgb_during_spatial_luma_scaling: true)

      hints = {width: downscaled_min_width, height: downscaled_min_height, scaleLumaSpatially: scale_luma_spatially, "gammaCorrectForSrgbDuringSpatialLumaScaling": gamma_correct_for_srgb_during_spatial_luma_scaling}

      result = send_json("v0.1/tell_decoder", {"ioId": placeholder_id, "command": {"JpegDownscaleHints": hints}})
      raise result.message unless result.ok?
    end
  end
end
