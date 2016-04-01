module Imageflow
  class Job
    def initialize(context:)
      @c = context
      @ptr = @c.call_method(:job_create)
      @keepalive = []
      record_nothing #Don't record anything by default
    end


    def destroy!
      @c.call_method(:job_destroy, @ptr)
      @ptr = nil
    end

    def destroyed?
      @ptr.nil? || @ptr.null?
    end

    def add_input_buffer(placeholder_id:, bytes:)
      buffer = FFI::MemoryPointer.new(:char, bytes.bytesize) # Allocate memory sized to the data
      buffer.put_bytes(0, bytes)
      @keepalive << buffer

      io_in = @c.call_method(:io_create_from_memory, :mode_read_write_seekable, buffer, bytes.bytesize, @c.ptr, nil)

      @c.call_method(:job_add_io, @ptr, io_in, placeholder_id,  :flow_input)
    end

    def add_output_buffer(placeholder_id:)

      io_ptr = @c.call_method(:io_create_for_output_buffer, @c.ptr)

      @c.call_method(:job_add_io, @ptr, io_ptr, placeholder_id,  :flow_output)
    end

    def get_buffer(placeholder_id:)
      buffer_pointer = FFI::MemoryPointer.new(:pointer, 1) # Allocate memory sized to the data
      buffer_size = FFI::MemoryPointer.new(:uint64, 1) # Allocate memory sized to the data

      @c.call_method(:job_get_output_buffer, @ptr, placeholder_id, buffer_pointer, buffer_size)

      {buffer: buffer_pointer.get_pointer(0),
       buffer_size: buffer_size.get_uint64(0)}
    end

    def record_nothing
      configure_recording record_graph_versions: false,
                          record_frame_images: false,
                          render_graph_versions: false,
                          render_animated_graph: false,
                          render_last_graph: false


    end

    def debug_record_gif
      configure_recording record_graph_versions: true,
                          record_frame_images: true,
                          render_graph_versions: true,
                          render_animated_graph: true,
                          render_last_graph: true


    end

    def configure_recording(record_graph_versions:, record_frame_images:, render_last_graph:, render_graph_versions:, render_animated_graph:)
      @c.call_method(:job_configure_recording, @ptr, record_graph_versions, record_frame_images, render_last_graph, render_graph_versions, render_animated_graph)
    end

    def get_buffer_bytes(placeholder_id:)
      buffer = get_buffer(placeholder_id: placeholder_id)
      raise "Buffer pointer null" if buffer[:buffer] == FFI::Pointer.new(0)
      buffer[:buffer].get_bytes(0, buffer[:buffer_size])
    end

    def execute (graph:)
      @c.call_method(:job_execute, @ptr, graph.ptr_ptr_graph)
    end

    def get_decoder_info(placeholder_id:)
      info = Imageflow::Native::FlowJobDecoderInfo.new

      @c.call_method(:job_get_decoder_info, @ptr, placeholder_id.to_i, info)

      info
    end
    def set_decoder_downscale_hints(placeholder_id:, if_wider_than:,
                                     or_taller_than:,  downscaled_min_width:,
                                     downscaled_min_height:)
      @c.call_method(:job_decoder_set_downscale_hints_by_placeholder_id, @ptr, placeholder_id, if_wider_than, or_taller_than, downscaled_min_width, downscaled_min_height);
    end



  end
end